//! The UI frameworks an app is built with, as far as its files tell.
//!
//! Flutter, React Native, Jetpack Compose and SwiftUI each draw the screen their own way, so the usual view
//! tree says little about them. Knowing which one an app uses decides which tool can look inside.

use serde::{Deserialize, Serialize};

/// A framework the app is built with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Framework {
    /// A stable id: `flutter`, `react-native`, `hermes`, `compose` or `swiftui`.
    pub id: String,
    /// The name to show.
    pub name: String,
    pub version: Option<String>,
    /// What gave it away, such as the file found in the app.
    pub evidence: String,
}

impl Framework {
    pub fn new(id: &str, evidence: impl Into<String>) -> Self {
        let name = match id {
            "flutter" => "Flutter",
            "react-native" => "React Native",
            "hermes" => "Hermes",
            "compose" => "Jetpack Compose",
            "swiftui" => "SwiftUI",
            other => other,
        };
        Framework { id: id.into(), name: name.into(), version: None, evidence: evidence.into() }
    }

    pub fn with_version(mut self, version: Option<String>) -> Self {
        self.version = version.filter(|v| !v.is_empty());
        self
    }
}

/// Adds `found` unless a framework with its id is already listed, then keeps the list in a steady order.
pub fn add(list: &mut Vec<Framework>, found: Framework) {
    if !list.iter().any(|f| f.id == found.id) {
        list.push(found);
    }
    list.sort_by_key(|f| ["flutter", "react-native", "hermes", "compose", "swiftui"].iter().position(|i| *i == f.id));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_framework_is_listed_once_in_a_steady_order() {
        let mut list = Vec::new();
        add(&mut list, Framework::new("compose", "x"));
        add(&mut list, Framework::new("flutter", "y"));
        add(&mut list, Framework::new("compose", "again"));
        assert_eq!(list.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["flutter", "compose"]);
        assert_eq!(list[0].name, "Flutter");
        assert_eq!(list[1].evidence, "x");
    }

    #[test]
    fn an_empty_version_is_no_version() {
        assert_eq!(Framework::new("flutter", "").with_version(Some(String::new())).version, None);
        assert_eq!(Framework::new("flutter", "").with_version(Some("3.3".into())).version.as_deref(), Some("3.3"));
    }
}
