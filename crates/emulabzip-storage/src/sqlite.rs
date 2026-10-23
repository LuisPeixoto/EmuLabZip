//! A read-only SQLite inspector.

use std::path::Path;

use emulabzip_core::{Error, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;

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
