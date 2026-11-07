//! Editing rows of a SQLite database, one at a time, by `rowid` or by primary key.

use std::path::Path;

use base64::Engine;
use emulabzip_core::{Error, Result};
use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OpenFlags, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::sqlite::quote_ident;

/// A value of a cell, with its type. JSON carries it as `{ "type": "text", "value": "…" }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    /// Bytes, as base64.
    Blob(String),
}

impl Cell {
    fn to_sql(&self) -> Result<Sql> {
        Ok(match self {
            Cell::Null => Sql::Null,
            Cell::Integer(i) => Sql::Integer(*i),
            Cell::Real(f) if f.is_finite() => Sql::Real(*f),
            Cell::Real(_) => return Err(Error::invalid("number", "not finite")),
            Cell::Text(t) => Sql::Text(t.clone()),
            Cell::Blob(b) => Sql::Blob(
                base64::engine::general_purpose::STANDARD
                    .decode(b)
                    .map_err(|_| Error::invalid("blob", "not base64"))?,
            ),
        })
    }
}

/// Which row. Most tables have a `rowid`; a table made `WITHOUT ROWID` is told by its primary key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum RowKey {
    Rowid(i64),
    Pk(Vec<(String, Cell)>),
}

fn err(e: rusqlite::Error) -> Error {
    Error::failed(format!("SQLite: {e}"))
}

struct TableInfo {
    columns: Vec<String>,
}

/// A database opened for changing. Use a copy of the file, with its `-wal` and `-shm` next to it.
pub struct Editor {
    conn: Connection,
}

impl Editor {
    pub fn open(path: &Path) -> Result<Editor> {
        let conn =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
                .map_err(err)?;
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).map_err(err)?;
        // The app's own rules about rows that point at each other hold for an edit too.
        conn.pragma_update(None, "foreign_keys", true).map_err(err)?;
        Ok(Editor { conn })
    }

    /// The columns of a real table. Views and SQLite's own tables are refused.
    fn table(&self, name: &str) -> Result<TableInfo> {
        let kind: Option<String> =
            self.conn.query_row("SELECT type FROM sqlite_master WHERE name = ?1", [name], |r| r.get(0)).ok();
        if kind.as_deref() != Some("table") || name.starts_with("sqlite_") {
            return Err(Error::invalid("table", name));
        }
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({})", quote_ident(name))).map_err(err)?;
        let columns = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(err)?
            .collect::<std::result::Result<_, _>>()
            .map_err(err)?;
        Ok(TableInfo { columns })
    }

    fn check_column(info: &TableInfo, column: &str) -> Result<()> {
        if info.columns.iter().any(|c| c == column) { Ok(()) } else { Err(Error::invalid("column", column)) }
    }

    /// `WHERE` for a key, and its parameters.
    fn filter(info: &TableInfo, key: &RowKey) -> Result<(String, Vec<Sql>)> {
        match key {
            RowKey::Rowid(id) => Ok(("rowid = ?".to_string(), vec![Sql::Integer(*id)])),
            RowKey::Pk(parts) => {
                if parts.is_empty() {
                    return Err(Error::invalid("row key", "empty"));
                }
                let mut sql = Vec::new();
                let mut params = Vec::new();
                for (column, value) in parts {
                    Self::check_column(info, column)?;
                    sql.push(format!("{} IS ?", quote_ident(column)));
                    params.push(value.to_sql()?);
                }
                Ok((sql.join(" AND "), params))
            }
        }
    }

    /// Runs one statement that has to change exactly one row; otherwise nothing is changed.
    fn one(&mut self, sql: &str, params: Vec<Sql>) -> Result<()> {
        let tx = self.conn.transaction().map_err(err)?;
        let changed = tx.execute(sql, params_from_iter(params)).map_err(err)?;
        if changed != 1 {
            return Err(Error::failed(format!(
                "The edit would have changed {changed} rows instead of one, so nothing was changed."
            )));
        }
        tx.commit().map_err(err)
    }

    pub fn update(&mut self, table: &str, key: &RowKey, column: &str, value: &Cell) -> Result<()> {
        let info = self.table(table)?;
        Self::check_column(&info, column)?;
        let (filter, mut params) = Self::filter(&info, key)?;
        let mut all = vec![value.to_sql()?];
        all.append(&mut params);
        self.one(&format!("UPDATE {} SET {} = ? WHERE {filter}", quote_ident(table), quote_ident(column)), all)
    }

    pub fn delete(&mut self, table: &str, key: &RowKey) -> Result<()> {
        let info = self.table(table)?;
        let (filter, params) = Self::filter(&info, key)?;
        self.one(&format!("DELETE FROM {} WHERE {filter}", quote_ident(table)), params)
    }

    /// Adds a row. Columns that are left out get their defaults. Returns its `rowid`.
    pub fn insert(&mut self, table: &str, values: &[(String, Cell)]) -> Result<i64> {
        let info = self.table(table)?;
        let sql = if values.is_empty() {
            format!("INSERT INTO {} DEFAULT VALUES", quote_ident(table))
        } else {
            for (column, _) in values {
                Self::check_column(&info, column)?;
            }
            let names: Vec<String> = values.iter().map(|(c, _)| quote_ident(c)).collect();
            let marks = vec!["?"; values.len()].join(", ");
            format!("INSERT INTO {} ({}) VALUES ({marks})", quote_ident(table), names.join(", "))
        };
        let params = values.iter().map(|(_, v)| v.to_sql()).collect::<Result<Vec<_>>>()?;
        self.one(&sql, params)?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Folds the write-ahead log into the file, so the file alone holds everything and can be put back on a device.
    pub fn finish(self) -> Result<()> {
        self.conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).map_err(err)?;
        // Leave the rollback journal as it was found: nothing in the file asks for the log any more.
        drop(self.conn);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(name: &str, wal: bool) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-edit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.db");
        let conn = Connection::open(&path).unwrap();
        if wal {
            conn.pragma_update(None, "journal_mode", "wal").unwrap();
        }
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL, avatar BLOB);
             INSERT INTO users (name, score) VALUES ('ana', 9.5), ('bo', NULL), ('cy', 7);
             CREATE TABLE kv (k TEXT, sub TEXT, v TEXT, PRIMARY KEY (k, sub)) WITHOUT ROWID;
             INSERT INTO kv VALUES ('a', 'x', '1'), ('a', 'y', '2');
             CREATE TABLE child (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users(id));
             INSERT INTO child (user_id) VALUES (1);
             CREATE VIEW top AS SELECT name FROM users;",
        )
        .unwrap();
        path
    }

    fn name_of(path: &Path, id: i64) -> Option<String> {
        Connection::open(path).unwrap().query_row("SELECT name FROM users WHERE id = ?", [id], |r| r.get(0)).ok()
    }

    #[test]
    fn a_cell_is_changed_by_rowid_with_its_type() {
        let path = make("cell", false);
        let mut e = Editor::open(&path).unwrap();
        e.update("users", &RowKey::Rowid(2), "name", &Cell::Text("bob".into())).unwrap();
        e.update("users", &RowKey::Rowid(2), "score", &Cell::Real(1.5)).unwrap();
        e.update("users", &RowKey::Rowid(1), "avatar", &Cell::Blob("AQID".into())).unwrap();
        e.update("users", &RowKey::Rowid(3), "score", &Cell::Null).unwrap();
        e.finish().unwrap();
        let conn = Connection::open(&path).unwrap();
        let row: (String, f64) =
            conn.query_row("SELECT name, score FROM users WHERE id = 2", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(row, ("bob".to_string(), 1.5));
        let blob: Vec<u8> = conn.query_row("SELECT avatar FROM users WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(blob, vec![1, 2, 3]);
        let score: Option<f64> = conn.query_row("SELECT score FROM users WHERE id = 3", [], |r| r.get(0)).unwrap();
        assert_eq!(score, None);
    }

    #[test]
    fn a_without_rowid_table_is_edited_by_its_primary_key() {
        let path = make("pk", false);
        let mut e = Editor::open(&path).unwrap();
        let key = RowKey::Pk(vec![("k".into(), Cell::Text("a".into())), ("sub".into(), Cell::Text("y".into()))]);
        e.update("kv", &key, "v", &Cell::Text("changed".into())).unwrap();
        e.finish().unwrap();
        let conn = Connection::open(&path).unwrap();
        let v: String = conn.query_row("SELECT v FROM kv WHERE k='a' AND sub='y'", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "changed");
        let other: String = conn.query_row("SELECT v FROM kv WHERE k='a' AND sub='x'", [], |r| r.get(0)).unwrap();
        assert_eq!(other, "1", "only the row of that key");
    }

    #[test]
    fn rows_are_added_and_deleted() {
        let path = make("rows", false);
        let mut e = Editor::open(&path).unwrap();
        let id = e
            .insert("users", &[("name".into(), Cell::Text("dee".into())), ("score".into(), Cell::Integer(3))])
            .unwrap();
        assert_eq!(id, 4);
        e.delete("users", &RowKey::Rowid(2)).unwrap();
        e.finish().unwrap();
        assert_eq!(name_of(&path, 4).as_deref(), Some("dee"));
        assert_eq!(name_of(&path, 2), None);
    }

    #[test]
    fn what_is_not_exact_changes_nothing() {
        let path = make("exact", false);
        let mut e = Editor::open(&path).unwrap();
        // A row that is not there: zero rows changed.
        assert!(e.update("users", &RowKey::Rowid(99), "name", &Cell::Text("x".into())).is_err());
        // A constraint: NOT NULL.
        assert!(e.update("users", &RowKey::Rowid(1), "name", &Cell::Null).is_err());
        // A foreign key: the child points at user 1.
        assert!(e.delete("users", &RowKey::Rowid(1)).is_err());
        // Names that are not columns or tables, views, and SQLite's own tables.
        assert!(e.update("users", &RowKey::Rowid(1), "nope", &Cell::Null).is_err());
        assert!(e.update("users; DROP TABLE users", &RowKey::Rowid(1), "name", &Cell::Null).is_err());
        assert!(e.update("top", &RowKey::Rowid(1), "name", &Cell::Null).is_err());
        assert!(e.delete("sqlite_master", &RowKey::Rowid(1)).is_err());
        assert!(e.update("users", &RowKey::Rowid(1), "name; --", &Cell::Null).is_err());
        assert!(e.update("users", &RowKey::Pk(vec![]), "name", &Cell::Null).is_err());
        assert!(e.update("users", &RowKey::Rowid(1), "avatar", &Cell::Blob("not base64!".into())).is_err());
        assert!(e.update("users", &RowKey::Rowid(1), "score", &Cell::Real(f64::NAN)).is_err());
        e.finish().unwrap();
        assert_eq!(name_of(&path, 1).as_deref(), Some("ana"));
        let conn = Connection::open(&path).unwrap();
        let n: i64 = conn.query_row("SELECT count(*) FROM users", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 3);
    }

    #[test]
    fn a_wal_database_ends_up_in_one_file() {
        let path = make("wal", true);
        let mut e = Editor::open(&path).unwrap();
        e.update("users", &RowKey::Rowid(1), "name", &Cell::Text("wal edit".into())).unwrap();
        e.finish().unwrap();
        let wal = path.with_extension("db-wal");
        assert!(!wal.exists() || std::fs::metadata(&wal).unwrap().len() == 0, "the log is folded in");
        // The main file alone holds the edit.
        let alone = path.with_file_name("alone.db");
        std::fs::copy(&path, &alone).unwrap();
        assert_eq!(name_of(&alone, 1).as_deref(), Some("wal edit"));
    }

    #[test]
    fn cells_round_trip_through_json() {
        let c: Cell = serde_json::from_str(r#"{"type":"text","value":"hi"}"#).unwrap();
        assert_eq!(c, Cell::Text("hi".into()));
        assert_eq!(serde_json::to_string(&Cell::Null).unwrap(), r#"{"type":"null"}"#);
        let k: RowKey = serde_json::from_str(r#"{"kind":"rowid","value":7}"#).unwrap();
        assert_eq!(k, RowKey::Rowid(7));
    }
}
