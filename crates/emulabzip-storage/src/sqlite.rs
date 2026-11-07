//! A read-only SQLite inspector.

use std::path::Path;

use emulabzip_core::{Error, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;

use crate::edit::sqlite::{Cell, RowKey};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub name: String,
    pub decl_type: String,
    pub primary_key: bool,
    pub not_null: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub name: String,
    /// `table` or `view`.
    pub kind: String,
    pub rows: u64,
    pub columns: Vec<Column>,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    /// More rows exist past the limit.
    pub truncated: bool,
}

/// A page of a table with what is needed to change its rows: a key for each, when the table can be edited.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyedRows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    /// One key per row, when `editable`.
    pub keys: Vec<RowKey>,
    pub truncated: bool,
    pub editable: bool,
    /// Why the rows cannot be edited.
    pub reason: Option<String>,
}

fn err(e: rusqlite::Error) -> Error {
    Error::parse("SQLite database", e.to_string())
}

/// Quotes an identifier for use in SQL.
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A database opened for inspection. Writes are refused with `PRAGMA query_only`.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens a database. Pass a copy: SQLite may need to replay a write-ahead log next to it.
    pub fn open(path: &Path) -> Result<Self> {
        let conn =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
                .or_else(|_| Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY))
                .map_err(err)?;
        conn.pragma_update(None, "query_only", true).map_err(err)?;
        // Fails early with a clear message when the file is not a database.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).map_err(err)?;
        Ok(Database { conn })
    }

    pub fn tables(&self) -> Result<Vec<Table>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, type, coalesce(sql, '') FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY type, name")
            .map_err(err)?;
        let list: Vec<(String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
        let mut tables = Vec::new();
        for (name, kind, sql) in list {
            let rows = self
                .conn
                .query_row(&format!("SELECT count(*) FROM {}", quote_ident(&name)), [], |r| r.get::<_, i64>(0))
                .unwrap_or(0) as u64;
            let mut info = self.conn.prepare(&format!("PRAGMA table_info({})", quote_ident(&name))).map_err(err)?;
            let columns = info
                .query_map([], |r| {
                    Ok(Column {
                        name: r.get(1)?,
                        decl_type: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                        not_null: r.get::<_, i64>(3)? != 0,
                        primary_key: r.get::<_, i64>(5)? != 0,
                    })
                })
                .map_err(err)?
                .collect::<Result<_, _>>()
                .map_err(err)?;
            tables.push(Table { name, kind, rows, columns, sql });
        }
        Ok(tables)
    }

    /// A page of a table, in rowid order when there is one.
    pub fn rows(&self, table: &str, offset: u64, limit: u64) -> Result<Rows> {
        let limit = limit.min(10_000);
        // One extra row tells whether there is a next page.
        let sql = format!("SELECT * FROM {} LIMIT {} OFFSET {}", quote_ident(table), limit + 1, offset);
        self.query(&sql, limit)
    }

    /// A page of a table, with a key for each row so it can be edited. A view, or a table with no way to tell its rows
    /// apart, comes back without keys and says why.
    pub fn rows_keyed(&self, table: &str, offset: u64, limit: u64) -> Result<KeyedRows> {
        let limit = limit.min(10_000);
        let (kind, sql): (String, Option<String>) = self
            .conn
            .query_row("SELECT type, sql FROM sqlite_master WHERE name = ?1", [table], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| Error::invalid("table", table))?;
        let plain = |reason: &str| -> Result<KeyedRows> {
            let page = self.rows(table, offset, limit)?;
            Ok(KeyedRows {
                columns: page.columns,
                rows: page.rows,
                keys: Vec::new(),
                truncated: page.truncated,
                editable: false,
                reason: Some(reason.to_string()),
            })
        };
        if kind != "table" {
            return plain("A view has no rows of its own to edit.");
        }
        if table.starts_with("sqlite_") {
            return plain("SQLite's own tables are not edited.");
        }
        let without_rowid = sql.as_deref().is_some_and(|s| s.to_uppercase().contains("WITHOUT ROWID"));
        if !without_rowid {
            let query = format!("SELECT rowid, * FROM {} LIMIT {} OFFSET {}", quote_ident(table), limit + 1, offset);
            let page = self.query(&query, limit)?;
            let mut keys = Vec::new();
            let mut rows = Vec::new();
            for mut row in page.rows {
                let id = row.remove(0).as_i64().ok_or_else(|| Error::failed("a row has no rowid"))?;
                keys.push(RowKey::Rowid(id));
                rows.push(row);
            }
            let columns = page.columns.into_iter().skip(1).collect();
            return Ok(KeyedRows { columns, rows, keys, truncated: page.truncated, editable: true, reason: None });
        }
        // A table made WITHOUT ROWID is told by its primary key.
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({})", quote_ident(table))).map_err(err)?;
        let mut pk: Vec<(i64, usize, String)> = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(5)?, r.get::<_, i64>(0)? as usize, r.get::<_, String>(1)?)))
            .map_err(err)?
            .filter_map(|r| r.ok())
            .filter(|(order, _, _)| *order > 0)
            .collect();
        pk.sort();
        if pk.is_empty() {
            return plain("This table has no primary key to tell its rows apart.");
        }
        let query = format!("SELECT * FROM {} LIMIT {} OFFSET {}", quote_ident(table), limit + 1, offset);
        let mut stmt = self.conn.prepare(&query).map_err(err)?;
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let width = columns.len();
        let mut rows = Vec::new();
        let mut keys = Vec::new();
        let mut truncated = false;
        let mut cursor = stmt.query([]).map_err(err)?;
        while let Some(row) = cursor.next().map_err(err)? {
            if rows.len() as u64 >= limit {
                truncated = true;
                break;
            }
            let mut parts = Vec::new();
            for (_, index, name) in &pk {
                parts.push((name.clone(), cell_of(row.get_ref(*index).map_err(err)?)));
            }
            keys.push(RowKey::Pk(parts));
            rows.push((0..width).map(|i| row.get_ref(i).map(to_json).map_err(err)).collect::<Result<Vec<_>>>()?);
        }
        Ok(KeyedRows { columns, rows, keys, truncated, editable: true, reason: None })
    }

    /// Runs a read-only statement and returns up to `limit` rows.
    pub fn query(&self, sql: &str, limit: u64) -> Result<Rows> {
        let mut stmt = self.conn.prepare(sql).map_err(err)?;
        if !stmt.readonly() {
            return Err(Error::Unsupported("Changing the database"));
        }
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let width = columns.len();
        let mut rows = Vec::new();
        let mut truncated = false;
        let mut cursor = stmt.query([]).map_err(err)?;
        while let Some(row) = cursor.next().map_err(err)? {
            if rows.len() as u64 >= limit {
                truncated = true;
                break;
            }
            let mut values = Vec::with_capacity(width);
            for i in 0..width {
                values.push(to_json(row.get_ref(i).map_err(err)?));
            }
            rows.push(values);
        }
        Ok(Rows { columns, rows, truncated })
    }
}

fn cell_of(v: ValueRef<'_>) -> Cell {
    use base64::Engine;
    match v {
        ValueRef::Null => Cell::Null,
        ValueRef::Integer(i) => Cell::Integer(i),
        ValueRef::Real(f) => Cell::Real(f),
        ValueRef::Text(t) => Cell::Text(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Cell::Blob(base64::engine::general_purpose::STANDARD.encode(b)),
    }
}

fn to_json(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::String(format!("<{} bytes>", b.len())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (tempdir::Dir, Database) {
        let dir = tempdir::Dir::new("sqlite");
        let path = dir.0.join("app.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL, avatar BLOB);
             INSERT INTO users (name, score, avatar) VALUES ('ana', 9.5, x'0102'), ('bo', NULL, NULL), ('cy', 7, NULL);
             CREATE VIEW top AS SELECT name FROM users WHERE score > 8;
             CREATE TABLE \"odd \"\"name\" (x);",
        )
        .unwrap();
        drop(conn);
        let db = Database::open(&path).unwrap();
        (dir, db)
    }

    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new(name: &str) -> Self {
                let p = std::env::temp_dir().join(format!(
                    "emulabzip-{name}-{}-{:?}",
                    std::process::id(),
                    std::thread::current().id()
                ));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn pages_come_with_keys_when_the_table_can_be_edited() {
        let (d, db) = sample();
        let page = db.rows_keyed("users", 0, 10).unwrap();
        assert!(page.editable && page.reason.is_none());
        assert_eq!(page.columns, ["id", "name", "score", "avatar"]);
        assert_eq!(page.keys, [RowKey::Rowid(1), RowKey::Rowid(2), RowKey::Rowid(3)]);
        assert_eq!(page.rows[0][1], "ana");
        // A view is shown without keys, and says why.
        let view = db.rows_keyed("top", 0, 10).unwrap();
        assert!(!view.editable && view.keys.is_empty() && view.reason.as_deref().unwrap().contains("view"));
        assert!(!view.rows.is_empty());
        assert!(db.rows_keyed("nope", 0, 10).is_err());
        // A table made WITHOUT ROWID is keyed by its primary key.
        let path = d.0.join("pk.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE kv (k TEXT, sub INTEGER, v TEXT, PRIMARY KEY (k, sub)) WITHOUT ROWID; INSERT INTO kv VALUES ('a', 1, 'x'), ('b', 2, 'y'); CREATE TABLE nokey (a, b) WITHOUT ROWID;").ok();
        conn.execute_batch("CREATE TABLE loose (a, b);").unwrap();
        drop(conn);
        let db = Database::open(&path).unwrap();
        let kv = db.rows_keyed("kv", 0, 10).unwrap();
        assert!(kv.editable);
        assert_eq!(
            kv.keys[1],
            RowKey::Pk(vec![("k".into(), Cell::Text("b".into())), ("sub".into(), Cell::Integer(2))])
        );
        assert_eq!(kv.rows[1], [Value::from("b"), Value::from(2), Value::from("y")]);
        assert!(db.rows_keyed("loose", 0, 10).unwrap().editable, "an ordinary table has a rowid");
    }

    #[test]
    fn lists_tables_and_views() {
        let (_d, db) = sample();
        let tables = db.tables().unwrap();
        let names: Vec<_> = tables.iter().map(|t| (t.name.as_str(), t.kind.as_str(), t.rows)).collect();
        assert_eq!(names, [("odd \"name", "table", 0), ("users", "table", 3), ("top", "view", 1)]);
        let users = &tables[1];
        assert!(users.columns[0].primary_key);
        assert!(users.columns[1].not_null);
        assert_eq!(users.columns[2].decl_type, "REAL");
    }

    #[test]
    fn pages_rows_and_converts_values() {
        let (_d, db) = sample();
        let page = db.rows("users", 0, 2).unwrap();
        assert_eq!(page.columns, ["id", "name", "score", "avatar"]);
        assert_eq!(page.rows.len(), 2);
        assert!(page.truncated);
        assert_eq!(page.rows[0][1], "ana");
        assert_eq!(page.rows[0][3], "<2 bytes>");
        assert!(page.rows[1][2].is_null());
        assert_eq!(db.rows("users", 2, 10).unwrap().rows[0][1], "cy");
    }

    #[test]
    fn runs_read_only_queries() {
        let (_d, db) = sample();
        let r = db.query("SELECT name FROM users ORDER BY score DESC", 10).unwrap();
        assert_eq!(r.rows[0][0], "ana");
        assert!(matches!(db.query("DELETE FROM users", 10).unwrap_err(), Error::Unsupported(_)));
        assert!(db.query("SELEC nonsense", 10).is_err());
    }

    #[test]
    fn rejects_files_that_are_not_databases() {
        let dir = tempdir::Dir::new("notdb");
        let path = dir.0.join("x.db");
        std::fs::write(&path, b"definitely not sqlite, just some text that is long enough to be a header").unwrap();
        assert!(Database::open(&path).is_err());
    }
}
