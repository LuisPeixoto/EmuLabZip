//! Browsing an app's files.

use serde::{Deserialize, Serialize};

/// A place an app keeps files, for example its private data directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageRoot {
    /// Stable identifier passed back to the backend (`data`, `external`, `bundle`…).
    pub id: String,
    pub label: String,
    /// Where it lives, for display.
    pub location: String,
    /// Why the root cannot be read, for example a release build on Android.
    pub unavailable: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    /// Path relative to the root, with `/` separators.
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    /// Seconds since the Unix epoch.
    pub modified: Option<u64>,
}

/// Files larger than this are not read into memory for previews.
pub const MAX_READ_BYTES: u64 = 256 * 1024 * 1024;

/// Joins a directory and a name relative to a root, without leading or trailing slashes.
pub fn join(dir: &str, name: &str) -> String {
    let dir = dir.trim_matches('/');
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

/// Rejects relative paths that try to leave the root (`..`) or are absolute.
pub fn is_safe_relative(path: &str) -> bool {
    !path.starts_with('/') && !path.contains('\0') && !path.split('/').any(|p| p == "..")
}

/// Sorts directories first, then by name, case-insensitively.
pub fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_paths() {
        assert_eq!(join("", "files"), "files");
        assert_eq!(join("/databases/", "app.db"), "databases/app.db");
    }

    #[test]
    fn rejects_escaping_paths() {
        assert!(is_safe_relative("databases/app.db"));
        assert!(is_safe_relative(""));
        assert!(!is_safe_relative("../other"));
        assert!(!is_safe_relative("files/../../x"));
        assert!(!is_safe_relative("/etc/passwd"));
    }

    #[test]
    fn sorts_directories_first() {
        let e =
            |name: &str, is_dir| FileEntry { name: name.into(), path: name.into(), is_dir, size: 0, modified: None };
        let mut v = vec![e("b.txt", false), e("Zed", true), e("a.txt", false), e("cache", true)];
        sort_entries(&mut v);
        assert_eq!(v.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["cache", "Zed", "a.txt", "b.txt"]);
    }
}
