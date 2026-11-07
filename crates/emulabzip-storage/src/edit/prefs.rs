//! Changing a SharedPreferences file: setting a value of a given type, adding a key, removing one.
//!
//! The file is read into its entries and written out again in the layout Android uses, so a value keeps its type, a set
//! keeps its items, and a file that round-trips through here reads the same to the app.

use emulabzip_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// A preference with its type. JSON carries it as `{ "type": "int", "value": 3 }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum PrefValue {
    String(String),
    Int(i32),
    Long(i64),
    Float(f32),
    Boolean(bool),
    Set(Vec<String>),
}

impl PrefValue {
    /// A value from a type name and text, as a person types them: a set is its items separated by commas.
    pub fn parse(kind: &str, text: &str) -> Result<PrefValue> {
        let bad = || Error::invalid("preference value", format!("{text} (for {kind})"));
        Ok(match kind {
            "string" => PrefValue::String(text.to_string()),
            "int" => PrefValue::Int(text.trim().parse().map_err(|_| bad())?),
            "long" => PrefValue::Long(text.trim().parse().map_err(|_| bad())?),
            "float" => {
                let f: f32 = text.trim().parse().map_err(|_| bad())?;
                if !f.is_finite() {
                    return Err(bad());
                }
                PrefValue::Float(f)
            }
            "boolean" => PrefValue::Boolean(match text.trim() {
                "true" => true,
                "false" => false,
                _ => return Err(bad()),
            }),
            "set" => PrefValue::Set(if text.is_empty() {
                Vec::new()
            } else {
                text.split(',').map(|s| s.trim().to_string()).collect()
            }),
            other => return Err(Error::invalid("preference type", other)),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    #[serde(flatten)]
    pub value: PrefValue,
}

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("preferences", detail)
}

/// Reads the entries, in file order.
pub fn read(xml: &str) -> Result<Vec<Entry>> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| bad(e.to_string()))?;
    let map = doc.root_element();
    if map.tag_name().name() != "map" {
        return Err(bad("the root element is not <map>"));
    }
    let mut out = Vec::new();
    for node in map.children().filter(roxmltree::Node::is_element) {
        let name = node.attribute("name").ok_or_else(|| bad("an entry has no name"))?.to_string();
        let attr = || node.attribute("value").ok_or_else(|| bad(format!("`{name}` has no value")));
        let value = match node.tag_name().name() {
            "string" => PrefValue::String(node.text().unwrap_or_default().to_string()),
            "int" => PrefValue::Int(attr()?.parse().map_err(|_| bad(format!("`{name}` is not an int")))?),
            "long" => PrefValue::Long(attr()?.parse().map_err(|_| bad(format!("`{name}` is not a long")))?),
            "float" => PrefValue::Float(attr()?.parse().map_err(|_| bad(format!("`{name}` is not a float")))?),
            "boolean" => PrefValue::Boolean(attr()? == "true"),
            "set" => PrefValue::Set(
                node.children()
                    .filter(roxmltree::Node::is_element)
                    .map(|c| c.text().unwrap_or_default().to_string())
                    .collect(),
            ),
            other => return Err(bad(format!("`{name}` is a <{other}>, which preferences do not have"))),
        };
        out.push(Entry { name, value });
    }
    Ok(out)
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters other than tab and newline cannot be in XML 1.0.
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Writes entries in the layout Android writes.
pub fn render(entries: &[Entry]) -> String {
    let mut out = String::from("<?xml version='1.0' encoding='utf-8' standalone='yes' ?>\n<map>\n");
    for e in entries {
        let name = escape(&e.name);
        match &e.value {
            PrefValue::String(s) => out.push_str(&format!("    <string name=\"{name}\">{}</string>\n", escape(s))),
            PrefValue::Int(v) => out.push_str(&format!("    <int name=\"{name}\" value=\"{v}\" />\n")),
            PrefValue::Long(v) => out.push_str(&format!("    <long name=\"{name}\" value=\"{v}\" />\n")),
            // `{:?}` keeps the `.0` of a whole number, as Java prints it.
            PrefValue::Float(v) => out.push_str(&format!("    <float name=\"{name}\" value=\"{v:?}\" />\n")),
            PrefValue::Boolean(v) => out.push_str(&format!("    <boolean name=\"{name}\" value=\"{v}\" />\n")),
            PrefValue::Set(items) if items.is_empty() => out.push_str(&format!("    <set name=\"{name}\" />\n")),
            PrefValue::Set(items) => {
                out.push_str(&format!("    <set name=\"{name}\">\n"));
                for item in items {
                    out.push_str(&format!("        <string>{}</string>\n", escape(item)));
                }
                out.push_str("    </set>\n");
            }
        }
    }
    out.push_str("</map>\n");
    out
}

/// Sets `key` to `value`: replaces it where it is, or adds it at the end. An empty key is refused.
pub fn set(xml: &str, key: &str, value: PrefValue) -> Result<String> {
    if key.is_empty() {
        return Err(Error::invalid("preference key", key));
    }
    let mut entries = read(xml)?;
    match entries.iter_mut().find(|e| e.name == key) {
        Some(e) => e.value = value,
        None => entries.push(Entry { name: key.to_string(), value }),
    }
    Ok(render(&entries))
}

/// Removes `key`. A key that is not there is an error, so a typo is not taken for success.
pub fn remove(xml: &str, key: &str) -> Result<String> {
    let mut entries = read(xml)?;
    let before = entries.len();
    entries.retain(|e| e.name != key);
    if entries.len() == before {
        return Err(Error::NotFound(format!("there is no preference called `{key}`")));
    }
    Ok(render(&entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version='1.0' encoding='utf-8' standalone='yes' ?>
<map>
    <string name="token">abc &amp; &lt;def&gt;</string>
    <int name="launches" value="3" />
    <boolean name="onboarded" value="true" />
    <long name="last_sync" value="1792773580000" />
    <float name="ratio" value="0.5" />
    <float name="whole" value="2.0" />
    <set name="tags">
        <string>a</string>
        <string>b, with comma</string>
    </set>
    <set name="none" />
    <string name="empty"></string>
</map>
"#;

    #[test]
    fn a_file_reads_and_renders_to_the_same_entries() {
        let entries = read(XML).unwrap();
        assert_eq!(entries.len(), 9);
        let again = read(&render(&entries)).unwrap();
        assert_eq!(entries, again, "nothing is lost on the way out and back");
        assert_eq!(entries[0].value, PrefValue::String("abc & <def>".into()));
        assert_eq!(entries[6].value, PrefValue::Set(vec!["a".into(), "b, with comma".into()]));
        assert_eq!(entries[7].value, PrefValue::Set(vec![]));
        // A whole float keeps its `.0`, like Java prints it.
        assert!(render(&entries).contains("name=\"whole\" value=\"2.0\""));
    }

    #[test]
    fn a_value_is_replaced_added_or_removed() {
        let out = set(XML, "launches", PrefValue::Int(4)).unwrap();
        assert!(out.contains("<int name=\"launches\" value=\"4\" />") && !out.contains("value=\"3\""));
        let out = set(&out, "new_key", PrefValue::String("v <&>".into())).unwrap();
        let entries = read(&out).unwrap();
        assert_eq!(entries.last().unwrap().name, "new_key", "added at the end");
        assert_eq!(entries.last().unwrap().value, PrefValue::String("v <&>".into()));
        // The type can change with the value.
        let out = set(&out, "launches", PrefValue::String("many".into())).unwrap();
        assert!(
            read(&out).unwrap().iter().any(|e| e.name == "launches" && e.value == PrefValue::String("many".into()))
        );
        let out = remove(&out, "ratio").unwrap();
        assert!(!read(&out).unwrap().iter().any(|e| e.name == "ratio"));
        assert!(remove(&out, "ratio").is_err(), "a key that is not there is an error");
        assert!(set(&out, "", PrefValue::Int(1)).is_err());
    }

    #[test]
    fn values_come_from_text_with_their_type() {
        assert_eq!(PrefValue::parse("int", " 7 ").unwrap(), PrefValue::Int(7));
        assert_eq!(PrefValue::parse("boolean", "true").unwrap(), PrefValue::Boolean(true));
        assert_eq!(PrefValue::parse("set", "a, b").unwrap(), PrefValue::Set(vec!["a".into(), "b".into()]));
        assert_eq!(PrefValue::parse("set", "").unwrap(), PrefValue::Set(vec![]));
        assert!(PrefValue::parse("int", "3.5").is_err());
        assert!(PrefValue::parse("int", "99999999999").is_err());
        assert!(PrefValue::parse("boolean", "yes").is_err());
        assert!(PrefValue::parse("float", "NaN").is_err());
        assert!(PrefValue::parse("map", "x").is_err());
    }

    #[test]
    fn what_is_not_preferences_is_refused() {
        assert!(read("<plist/>").is_err());
        assert!(read("not xml").is_err());
        assert!(read("<map><thing name=\"a\"/></map>").is_err());
        assert!(read("<map><int name=\"a\" value=\"x\"/></map>").is_err());
        assert!(read("<map><int value=\"1\"/></map>").is_err());
    }

    #[test]
    fn json_carries_the_type() {
        let e = Entry { name: "n".into(), value: PrefValue::Int(3) };
        assert_eq!(serde_json::to_string(&e).unwrap(), r#"{"name":"n","type":"int","value":3}"#);
        let back: Entry = serde_json::from_str(r#"{"name":"s","type":"set","value":["a"]}"#).unwrap();
        assert_eq!(back.value, PrefValue::Set(vec!["a".into()]));
    }
}
