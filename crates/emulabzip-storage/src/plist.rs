//! Property lists (binary or XML), as JSON for display.

use base64::Engine;
use emulabzip_core::{Error, Result};
use serde_json::{Map, Value};

/// Reads a binary or XML property list. Dates become ISO 8601 strings; data becomes a short
/// description, or base64 when it is small.
pub fn to_json(bytes: &[u8]) -> Result<Value> {
    let value = plist::Value::from_reader(std::io::Cursor::new(bytes))
        .map_err(|e| Error::parse("property list", e.to_string()))?;
    Ok(convert(value))
}

fn convert(v: plist::Value) -> Value {
    match v {
        plist::Value::Array(a) => Value::Array(a.into_iter().map(convert).collect()),
        plist::Value::Dictionary(d) => {
            Value::Object(d.into_iter().map(|(k, v)| (k, convert(v))).collect::<Map<_, _>>())
        }
        plist::Value::Boolean(b) => Value::Bool(b),
        plist::Value::Data(d) if d.len() <= 64 => {
            Value::String(format!("<data {}>", base64::engine::general_purpose::STANDARD.encode(d)))
        }
        plist::Value::Data(d) => Value::String(format!("<{} bytes>", d.len())),
        plist::Value::Date(d) => Value::String(d.to_xml_format()),
        plist::Value::Real(f) => serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number),
        plist::Value::Integer(i) => {
            i.as_signed().map(Value::from).or_else(|| i.as_unsigned().map(Value::from)).unwrap_or(Value::Null)
        }
        plist::Value::String(s) => Value::String(s),
        plist::Value::Uid(u) => Value::String(format!("<uid {}>", u.get())),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_xml_plists() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Example</string>
  <key>Launches</key><integer>3</integer>
  <key>Onboarded</key><true/>
  <key>Ratio</key><real>0.5</real>
  <key>Token</key><data>AQID</data>
  <key>Since</key><date>2026-10-23T13:00:00Z</date>
  <key>Tags</key><array><string>a</string></array>
</dict></plist>"#;
        let v = to_json(xml).unwrap();
        assert_eq!(v["CFBundleName"], "Example");
        assert_eq!(v["Launches"], 3);
        assert_eq!(v["Onboarded"], true);
        assert_eq!(v["Ratio"], 0.5);
        assert_eq!(v["Token"], "<data AQID>");
        assert_eq!(v["Since"], "2026-10-23T13:00:00Z");
        assert_eq!(v["Tags"][0], "a");
    }

    #[test]
    fn reads_binary_plists() {
        let mut dict = plist::Dictionary::new();
        dict.insert("k".into(), plist::Value::String("v".into()));
        let mut bytes = Vec::new();
        plist::Value::Dictionary(dict).to_writer_binary(&mut bytes).unwrap();
        assert!(bytes.starts_with(b"bplist00"));
        assert_eq!(to_json(&bytes).unwrap()["k"], "v");
    }

    #[test]
    fn rejects_garbage() {
        assert!(to_json(b"nope").is_err());
    }
}
