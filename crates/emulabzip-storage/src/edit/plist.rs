//! Changing one value of a property list, and writing the file back in the format it came in.

use base64::Engine;
use emulabzip_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// A plist value with its type. JSON carries it as `{ "type": "integer", "value": 3 }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum PlistValue {
    String(String),
    Integer(i64),
    Real(f64),
    Boolean(bool),
    /// ISO 8601, `2026-10-23T13:00:00Z`.
    Date(String),
    /// Bytes, as base64.
    Data(String),
}

impl PlistValue {
    fn to_plist(&self) -> Result<plist::Value> {
        Ok(match self {
            PlistValue::String(s) => plist::Value::String(s.clone()),
            PlistValue::Integer(i) => plist::Value::Integer((*i).into()),
            PlistValue::Real(f) if f.is_finite() => plist::Value::Real(*f),
            PlistValue::Real(_) => return Err(Error::invalid("number", "not finite")),
            PlistValue::Boolean(b) => plist::Value::Boolean(*b),
            PlistValue::Date(d) => {
                plist::Value::Date(plist::Date::from_xml_format(d).map_err(|_| Error::invalid("date", d.clone()))?)
            }
            PlistValue::Data(d) => plist::Value::Data(
                base64::engine::general_purpose::STANDARD
                    .decode(d)
                    .map_err(|_| Error::invalid("data", "not base64"))?,
            ),
        })
    }

    /// A value from a type name and text, as a person types them.
    pub fn parse(kind: &str, text: &str) -> Result<PlistValue> {
        let bad = || Error::invalid("property list value", format!("{text} (for {kind})"));
        Ok(match kind {
            "string" => PlistValue::String(text.to_string()),
            "integer" => PlistValue::Integer(text.trim().parse().map_err(|_| bad())?),
            "real" => PlistValue::Real(text.trim().parse().map_err(|_| bad())?),
            "boolean" => PlistValue::Boolean(match text.trim() {
                "true" => true,
                "false" => false,
                _ => return Err(bad()),
            }),
            "date" => PlistValue::Date(text.trim().to_string()),
            "data" => PlistValue::Data(text.trim().to_string()),
            other => return Err(Error::invalid("property list type", other)),
        })
    }
}

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("property list", detail)
}

fn read(bytes: &[u8]) -> Result<(plist::Value, bool)> {
    let binary = bytes.starts_with(b"bplist00");
    let value = plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(|e| bad(e.to_string()))?;
    Ok((value, binary))
}

fn write(value: &plist::Value, binary: bool) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    if binary {
        value.to_writer_binary(&mut out).map_err(|e| bad(e.to_string()))?;
    } else {
        value.to_writer_xml(&mut out).map_err(|e| bad(e.to_string()))?;
    }
    Ok(out)
}

/// Follows `path` (dictionary keys, array positions) down to the parent of the last step.
fn parent<'a>(root: &'a mut plist::Value, path: &[String]) -> Result<&'a mut plist::Value> {
    let mut at = root;
    for step in &path[..path.len() - 1] {
        at = match at {
            plist::Value::Dictionary(d) => {
                d.get_mut(step).ok_or_else(|| Error::NotFound(format!("there is no `{step}`")))?
            }
            plist::Value::Array(a) => {
                let i: usize = step.parse().map_err(|_| Error::invalid("array position", step.clone()))?;
                a.get_mut(i).ok_or_else(|| Error::NotFound(format!("there is no item {i}")))?
            }
            _ => return Err(Error::invalid("path", step.clone())),
        };
    }
    Ok(at)
}

/// Sets the value at `path`. A key of a dictionary that is not there is added; an array position may be one past the end,
/// which adds an item.
pub fn set(bytes: &[u8], path: &[String], value: &PlistValue) -> Result<Vec<u8>> {
    if path.is_empty() || path.iter().any(String::is_empty) {
        return Err(Error::invalid("path", path.join("/")));
    }
    let (mut root, binary) = read(bytes)?;
    let new = value.to_plist()?;
    let last = path.last().expect("not empty");
    match parent(&mut root, path)? {
        plist::Value::Dictionary(d) => {
            d.insert(last.clone(), new);
        }
        plist::Value::Array(a) => {
            let i: usize = last.parse().map_err(|_| Error::invalid("array position", last.clone()))?;
            match i.cmp(&a.len()) {
                std::cmp::Ordering::Less => a[i] = new,
                std::cmp::Ordering::Equal => a.push(new),
                std::cmp::Ordering::Greater => return Err(Error::NotFound(format!("there is no item {i}"))),
            }
        }
        _ => return Err(Error::invalid("path", path.join("/"))),
    }
    write(&root, binary)
}

/// Removes the value at `path`. It has to be there.
pub fn remove(bytes: &[u8], path: &[String]) -> Result<Vec<u8>> {
    if path.is_empty() {
        return Err(Error::invalid("path", ""));
    }
    let (mut root, binary) = read(bytes)?;
    let last = path.last().expect("not empty");
    match parent(&mut root, path)? {
        plist::Value::Dictionary(d) => {
            d.remove(last).ok_or_else(|| Error::NotFound(format!("there is no `{last}`")))?;
        }
        plist::Value::Array(a) => {
            let i: usize = last.parse().map_err(|_| Error::invalid("array position", last.clone()))?;
            if i >= a.len() {
                return Err(Error::NotFound(format!("there is no item {i}")));
            }
            a.remove(i);
        }
        _ => return Err(Error::invalid("path", path.join("/"))),
    }
    write(&root, binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Name</key><string>Example</string>
  <key>Launches</key><integer>3</integer>
  <key>Nested</key><dict><key>Flag</key><true/><key>List</key><array><string>a</string><string>b</string></array></dict>
</dict></plist>"#;

    fn p(path: &str) -> Vec<String> {
        path.split('/').map(str::to_string).collect()
    }

    fn json(bytes: &[u8]) -> serde_json::Value {
        crate::plist::to_json(bytes).unwrap()
    }

    #[test]
    fn a_value_is_changed_where_it_is_and_keeps_its_neighbours() {
        let out = set(XML, &p("Launches"), &PlistValue::Integer(4)).unwrap();
        assert_eq!(json(&out)["Launches"], 4);
        assert_eq!(json(&out)["Name"], "Example");
        let out = set(&out, &p("Nested/Flag"), &PlistValue::Boolean(false)).unwrap();
        assert_eq!(json(&out)["Nested"]["Flag"], false);
        let out = set(&out, &p("Nested/List/1"), &PlistValue::String("B".into())).unwrap();
        assert_eq!(json(&out)["Nested"]["List"], serde_json::json!(["a", "B"]));
        // A new key, a new item at the end, and a changed type.
        let out = set(&out, &p("Nested/New"), &PlistValue::Real(1.5)).unwrap();
        assert_eq!(json(&out)["Nested"]["New"], 1.5);
        let out = set(&out, &p("Nested/List/2"), &PlistValue::String("c".into())).unwrap();
        assert_eq!(json(&out)["Nested"]["List"], serde_json::json!(["a", "B", "c"]));
        let out = set(&out, &p("Name"), &PlistValue::Integer(1)).unwrap();
        assert_eq!(json(&out)["Name"], 1);
    }

    #[test]
    fn a_binary_plist_stays_binary_and_an_xml_one_stays_xml() {
        let out = set(XML, &p("Launches"), &PlistValue::Integer(9)).unwrap();
        assert!(out.starts_with(b"<?xml"), "{:?}", String::from_utf8_lossy(&out[..20]));
        let (value, _) = read(XML).unwrap();
        let mut bin = Vec::new();
        value.to_writer_binary(&mut bin).unwrap();
        assert!(bin.starts_with(b"bplist00"));
        let out = set(&bin, &p("Launches"), &PlistValue::Integer(9)).unwrap();
        assert!(out.starts_with(b"bplist00"));
        assert_eq!(json(&out)["Launches"], 9);
    }

    #[test]
    fn dates_and_data_are_typed() {
        let out = set(XML, &p("Since"), &PlistValue::Date("2026-10-23T13:00:00Z".into())).unwrap();
        assert_eq!(json(&out)["Since"], "2026-10-23T13:00:00Z");
        let out = set(&out, &p("Token"), &PlistValue::Data("AQID".into())).unwrap();
        assert_eq!(json(&out)["Token"], "<data AQID>");
        assert!(set(XML, &p("Since"), &PlistValue::Date("yesterday".into())).is_err());
        assert!(set(XML, &p("Token"), &PlistValue::Data("not base64!".into())).is_err());
        assert!(set(XML, &p("N"), &PlistValue::Real(f64::INFINITY)).is_err());
    }

    #[test]
    fn a_value_is_removed_and_a_wrong_path_is_an_error() {
        let out = remove(XML, &p("Nested/List/0")).unwrap();
        assert_eq!(json(&out)["Nested"]["List"], serde_json::json!(["b"]));
        let out = remove(&out, &p("Name")).unwrap();
        assert!(json(&out).get("Name").is_none());
        assert!(remove(XML, &p("Nope")).is_err());
        assert!(remove(XML, &p("Nested/List/9")).is_err());
        assert!(remove(XML, &p("Name/Deeper")).is_err());
        assert!(set(XML, &p("Nope/Deeper"), &PlistValue::Integer(1)).is_err());
        assert!(set(XML, &p("Nested/List/9"), &PlistValue::Integer(1)).is_err());
        assert!(set(XML, &[], &PlistValue::Integer(1)).is_err());
        assert!(set(b"not a plist", &p("a"), &PlistValue::Integer(1)).is_err());
    }

    #[test]
    fn values_come_from_text_with_their_type() {
        assert_eq!(PlistValue::parse("integer", "5").unwrap(), PlistValue::Integer(5));
        assert_eq!(PlistValue::parse("boolean", "false").unwrap(), PlistValue::Boolean(false));
        assert!(PlistValue::parse("integer", "x").is_err());
        assert!(PlistValue::parse("uid", "1").is_err());
    }
}
