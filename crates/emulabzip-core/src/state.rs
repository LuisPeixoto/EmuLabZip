//! Saving and restoring an app's data, and writing into its storage.
//!
//! A backend writes the sandbox as plain `tar` files into a folder it is given and reads them back; packing them into one
//! `.ezstate` file, and the schemas kept beside them, is the job of `emulabzip-storage`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The name of the archive of an app's private data.
pub const DATA_TAR: &str = "data.tar";
/// The name of the archive of what the app keeps on shared storage, when asked for.
pub const EXTERNAL_TAR: &str = "external.tar";

/// The most a single write into an app's storage may carry.
pub const MAX_WRITE_BYTES: usize = 256 * 1024 * 1024;

/// What `save_state` wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateFiles {
    /// The app's private data.
    pub data: PathBuf,
    /// The app's files on shared storage, when they were asked for and there were any.
    pub external: Option<PathBuf>,
    /// The version of the app the data belongs to, `1.2.0 (42)` or `1.2.0`.
    pub app_version: Option<String>,
}

/// Whether an archive entry may be unpacked into a sandbox: relative, with no `..`, no empty or NUL-bearing parts.
pub fn is_safe_entry(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\0')
        && !path.contains('\\')
        && !path.split('/').any(|p| p == "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_entries_stay_inside_the_sandbox() {
        assert!(is_safe_entry("./databases/app.db"));
        assert!(is_safe_entry("shared_prefs/a.xml"));
        assert!(!is_safe_entry("/etc/passwd"));
        assert!(!is_safe_entry("../x"));
        assert!(!is_safe_entry("a/../../x"));
        assert!(!is_safe_entry(""));
        assert!(!is_safe_entry("a\\b"));
        assert!(!is_safe_entry("a\0b"));
    }
}
