//! Comparing two devices: what differs between their facts.

use serde::{Deserialize, Serialize};

use crate::InfoSection;

/// One fact, as two devices report it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRow {
    pub section: String,
    pub key: String,
    pub a: Option<String>,
    pub b: Option<String>,
    pub same: bool,
}

/// Lines up the facts of two devices by section and name. Facts only one device has show an empty
/// side; sections and facts keep the order of the first device, then those only the second has.
pub fn diff(a: &[InfoSection], b: &[InfoSection]) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut titles: Vec<&str> = a.iter().map(|s| s.title.as_str()).collect();
    for s in b {
        if !titles.contains(&s.title.as_str()) {
            titles.push(&s.title);
        }
    }
    for title in titles {
        let left = a.iter().find(|s| s.title == title);
        let right = b.iter().find(|s| s.title == title);
        let value = |s: Option<&InfoSection>, key: &str| {
            s.and_then(|s| s.items.iter().find(|(k, _)| k == key)).map(|(_, v)| v.clone())
        };
        let mut keys: Vec<&str> = left.map(|s| s.items.iter().map(|(k, _)| k.as_str()).collect()).unwrap_or_default();
        for (k, _) in right.map(|s| s.items.as_slice()).unwrap_or_default() {
            if !keys.contains(&k.as_str()) {
                keys.push(k);
            }
        }
        for key in keys {
            let (va, vb) = (value(left, key), value(right, key));
            rows.push(DiffRow { section: title.to_string(), key: key.to_string(), same: va == vb, a: va, b: vb });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(title: &str, items: &[(&str, &str)]) -> InfoSection {
        InfoSection { title: title.into(), items: items.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }

    #[test]
    fn matching_facts_are_the_same_and_others_differ() {
        let a = [section("Software", &[("OS", "14"), ("API", "34")]), section("Display", &[("Size", "1080x2400")])];
        let b = [section("Software", &[("OS", "13"), ("API", "34")]), section("Display", &[("Size", "1080x2400")])];
        let rows = diff(&a, &b);
        let differing: Vec<_> =
            rows.iter().filter(|r| !r.same).map(|r| (r.key.as_str(), r.a.as_deref(), r.b.as_deref())).collect();
        assert_eq!(differing, [("OS", Some("14"), Some("13"))]);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn facts_only_one_device_has_show_an_empty_side() {
        let a = [section("Software", &[("OS", "14")]), section("Battery", &[("Level", "80")])];
        let b = [section("Software", &[("OS", "14"), ("Xcode", "26")]), section("Locale", &[("Region", "BR")])];
        let rows = diff(&a, &b);
        let find = |k: &str| rows.iter().find(|r| r.key == k).unwrap();
        assert_eq!(
            (find("Level").a.as_deref(), find("Level").b.as_deref(), find("Level").same),
            (Some("80"), None, false)
        );
        assert_eq!((find("Xcode").a.as_deref(), find("Xcode").b.as_deref()), (None, Some("26")));
        assert_eq!(find("Region").section, "Locale");
        // The first device's order, then what only the second has.
        let order: Vec<_> = rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(order, ["OS", "Xcode", "Level", "Region"]);
    }
}
