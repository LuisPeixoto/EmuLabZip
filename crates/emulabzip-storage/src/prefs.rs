//! Android SharedPreferences files (`shared_prefs/*.xml`).

use emulabzip_core::{Error, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pref {
    pub key: String,
    /// `string`, `int`, `long`, `float`, `boolean` or `set`.
    pub kind: String,
    pub value: String,
}

/// Parses a SharedPreferences file:
///
/// ```xml
/// <map>
///     <string name="token">abc</string>
///     <int name="launches" value="3" />
///     <set name="tags"><string>a</string><string>b</string></set>
/// </map>
/// ```
pub fn parse(xml: &str) -> Result<Vec<Pref>> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| Error::parse("preferences", e.to_string()))?;
    let map = doc.root_element();
    if map.tag_name().name() != "map" {
        return Err(Error::parse("preferences", "the root element is not <map>"));
    }
    let mut prefs: Vec<Pref> = map
        .children()
        .filter(roxmltree::Node::is_element)
        .filter_map(|node| {
            let key = node.attribute("name")?.to_string();
            let kind = node.tag_name().name().to_string();
            let value = match kind.as_str() {
                "string" => node.text().unwrap_or_default().to_string(),
                "set" => node
                    .children()
                    .filter(roxmltree::Node::is_element)
                    .map(|c| c.text().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => node.attribute("value").unwrap_or_default().to_string(),
            };
            Some(Pref { key, kind, value })
        })
        .collect();
    prefs.sort_by_key(|p| p.key.to_lowercase());
    Ok(prefs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_type() {
        let xml = r#"<?xml version='1.0' encoding='utf-8' standalone='yes' ?>
<map>
    <string name="token">abc &amp; def</string>
    <int name="launches" value="3" />
    <boolean name="onboarded" value="true" />
    <long name="last_sync" value="1792773580000" />
    <float name="ratio" value="0.5" />
    <set name="Tags"><string>a</string><string>b</string></set>
    <string name="empty"></string>
</map>"#;
        let prefs = parse(xml).unwrap();
        assert_eq!(prefs.len(), 7);
        let get = |k: &str| prefs.iter().find(|p| p.key == k).unwrap();
        assert_eq!(get("token").value, "abc & def");
        assert_eq!((get("launches").kind.as_str(), get("launches").value.as_str()), ("int", "3"));
        assert_eq!(get("Tags").value, "a, b");
        assert_eq!(get("empty").value, "");
        assert_eq!(prefs[0].key, "empty");
    }

    #[test]
    fn rejects_other_xml() {
        assert!(parse("<resources/>").is_err());
        assert!(parse("not xml").is_err());
    }
}
