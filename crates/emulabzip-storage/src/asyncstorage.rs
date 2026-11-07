//! React Native's AsyncStorage: on Android a SQLite database (`databases/RKStorage`, table `catalystLocalStorage`), on iOS
//! a `manifest.json` next to files named for the MD5 of the key, for values too big for the manifest.

use emulabzip_core::{Error, Result};
use serde::Serialize;
use serde_json::Value;

use crate::sqlite::Database;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    /// The value as stored: text, usually JSON.
    pub value: Option<String>,
    /// The value parsed, when it is JSON.
    pub json: Option<Value>,
    /// On iOS, a value left out of the manifest is in a file of its own, named for the key.
    pub in_file: bool,
}

fn entry(key: String, value: Option<String>) -> KeyValue {
    let json =
        value.as_deref().and_then(|v| serde_json::from_str::<Value>(v).ok()).filter(|v| v.is_object() || v.is_array());
    KeyValue { in_file: value.is_none(), key, value, json }
}

/// Whether a database is the Android store.
pub fn is_android_store(db: &Database) -> bool {
    db.tables().is_ok_and(|t| t.iter().any(|t| t.name == "catalystLocalStorage"))
}

/// The pairs of the Android store, by key.
pub fn from_database(db: &Database) -> Result<Vec<KeyValue>> {
    let page = db.query("SELECT key, value FROM catalystLocalStorage ORDER BY key", 100_000)?;
    Ok(page
        .rows
        .into_iter()
        .filter_map(|row| Some(entry(row.first()?.as_str()?.to_string(), Some(row.get(1)?.as_str()?.to_string()))))
        .collect())
}

/// The pairs of an iOS manifest, by key.
pub fn from_manifest(text: &str) -> Result<Vec<KeyValue>> {
    let value: Value = serde_json::from_str(text).map_err(|e| Error::parse("AsyncStorage manifest", e.to_string()))?;
    let Value::Object(map) = value else { return Err(Error::parse("AsyncStorage manifest", "it is not an object")) };
    let mut out: Vec<KeyValue> = map
        .into_iter()
        .map(|(k, v)| match v {
            Value::String(s) => entry(k, Some(s)),
            Value::Null => entry(k, None),
            other => entry(k, Some(other.to_string())),
        })
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_android_store() {
        let dir = std::env::temp_dir().join(format!("emulabzip-async-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("RKStorage");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE catalystLocalStorage (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO catalystLocalStorage VALUES ('cart', '{\"items\":[1,2]}'), ('theme', 'dark'), ('@user:id', '42');",
        )
        .unwrap();
        drop(conn);
        let db = Database::open(&path).unwrap();
        assert!(is_android_store(&db));
        let pairs = from_database(&db).unwrap();
        assert_eq!(pairs.iter().map(|p| p.key.as_str()).collect::<Vec<_>>(), ["@user:id", "cart", "theme"]);
        let cart = &pairs[1];
        assert_eq!(cart.json.as_ref().unwrap()["items"][1], 2);
        assert!(pairs[2].json.is_none() && pairs[2].value.as_deref() == Some("dark"));
        assert!(pairs[0].json.is_none(), "a bare number stays text");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn reads_the_ios_manifest_and_marks_values_kept_in_files() {
        let pairs = from_manifest(r#"{"theme":"dark","big":null,"cart":"{\"a\":1}"}"#).unwrap();
        assert_eq!(
            pairs.iter().map(|p| (p.key.as_str(), p.in_file)).collect::<Vec<_>>(),
            [("big", true), ("cart", false), ("theme", false)]
        );
        assert_eq!(pairs[1].json.as_ref().unwrap()["a"], 1);
        assert!(from_manifest("[1]").is_err());
        assert!(from_manifest("nope").is_err());
    }
}
