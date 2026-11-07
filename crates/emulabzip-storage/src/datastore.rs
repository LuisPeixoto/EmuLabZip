//! Jetpack DataStore files: Preferences DataStore (`*.preferences_pb`) as typed entries, and Proto DataStore as a message
//! without its schema.

use emulabzip_core::{Error, Result};
use serde_json::Value;

use crate::prefs::Pref;
use crate::proto::{self, Field};

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("DataStore", detail)
}

fn text(bytes: &[u8]) -> Result<String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| bad("a string is not UTF-8"))
}

/// Reads a Preferences DataStore. Its message is `PreferenceMap { map<string, Value> preferences = 1 }`, and a `Value` holds
/// one of: boolean (1), float (2), integer (3), long (4), string (5), string set (6), double (7) or bytes (8).
pub fn parse_preferences(bytes: &[u8]) -> Result<Vec<Pref>> {
    let mut out = Vec::new();
    for (number, field) in proto::fields(bytes).map_err(|e| bad(e.to_string()))? {
        let (1, Field::Bytes(entry)) = (number, field) else { return Err(bad("this is not a Preferences DataStore")) };
        let mut key = None;
        let mut value = None;
        for (n, f) in proto::fields(entry).map_err(|e| bad(e.to_string()))? {
            match (n, f) {
                (1, Field::Bytes(k)) => key = Some(text(k)?),
                (2, Field::Bytes(v)) => value = Some(v),
                _ => {}
            }
        }
        let (Some(key), Some(value)) = (key, value) else { return Err(bad("an entry has no key or no value")) };
        out.push(pref(key, value)?);
    }
    out.sort_by_key(|p| p.key.to_lowercase());
    Ok(out)
}

fn pref(key: String, value: &[u8]) -> Result<Pref> {
    let fields = proto::fields(value).map_err(|e| bad(e.to_string()))?;
    let (number, field) = fields.into_iter().next().ok_or_else(|| bad(format!("`{key}` has no value")))?;
    let (kind, text_value, items) = match (number, field) {
        (1, Field::Varint(v)) => ("boolean", (v != 0).to_string(), vec![]),
        (2, Field::Fixed32(v)) => ("float", format!("{:?}", f32::from_bits(v)), vec![]),
        (3, Field::Varint(v)) => ("int", (v as i32).to_string(), vec![]),
        (4, Field::Varint(v)) => ("long", (v as i64).to_string(), vec![]),
        (5, Field::Bytes(b)) => ("string", text(b)?, vec![]),
        (6, Field::Bytes(b)) => {
            let mut items = Vec::new();
            for (n, f) in proto::fields(b).map_err(|e| bad(e.to_string()))? {
                if let (1, Field::Bytes(s)) = (n, f) {
                    items.push(text(s)?);
                }
            }
            ("set", items.join(", "), items)
        }
        (7, Field::Fixed64(v)) => ("double", format!("{:?}", f64::from_bits(v)), vec![]),
        (8, Field::Bytes(b)) => ("bytes", b.iter().map(|x| format!("{x:02x}")).collect(), vec![]),
        (n, _) => {
            return Err(bad(format!("`{key}` holds a value of kind {n}, which a Preferences DataStore does not have")));
        }
    };
    Ok(Pref { key, kind: kind.to_string(), value: text_value, items })
}

/// Reads a Proto DataStore (or any protobuf file) without its schema, as JSON keyed by field number.
pub fn parse_proto(bytes: &[u8]) -> Result<Value> {
    proto::to_json(bytes).map_err(|e| bad(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &[u8] = include_bytes!("../tests/fixtures/demo.preferences_pb");

    #[test]
    fn reads_a_real_preferences_datastore() {
        let prefs = parse_preferences(REAL).unwrap();
        let get = |k: &str| prefs.iter().find(|p| p.key == k).unwrap_or_else(|| panic!("{k} is missing: {prefs:?}"));
        assert_eq!(prefs.len(), 6);
        assert_eq!((get("language").kind.as_str(), get("language").value.as_str()), ("string", "pt-BR"));
        assert_eq!((get("font_scale").kind.as_str(), get("font_scale").value.as_str()), ("int", "110"));
        assert_eq!((get("installed_at").kind.as_str(), get("installed_at").value.as_str()), ("long", "1792700000000"));
        assert_eq!((get("volume").kind.as_str(), get("volume").value.as_str()), ("double", "0.8"));
        assert_eq!((get("notifications").kind.as_str(), get("notifications").value.as_str()), ("boolean", "true"));
        assert_eq!(get("favourites").items, ["apples", "pears"]);
        assert_eq!(get("favourites").kind, "set");
    }

    #[test]
    fn an_empty_datastore_is_empty_and_other_files_are_refused() {
        assert!(parse_preferences(&[]).unwrap().is_empty());
        assert!(parse_preferences(b"SQLite format 3\0").is_err());
        assert!(parse_preferences(&[0x08, 0x01]).is_err(), "field 1 as a number is not an entry");
        assert!(parse_preferences(&[0x0a, 0x02, 0x0a, 0x00]).is_err(), "an entry without a value");
    }

    #[test]
    fn a_proto_datastore_reads_without_its_schema() {
        let v = parse_proto(REAL).unwrap();
        assert_eq!(v["1"].as_array().unwrap().len(), 6, "six entries under field 1");
        assert_eq!(v["1"][0]["1"], "language");
        assert!(parse_proto(&[0x0a, 0x05]).is_err());
    }
}
