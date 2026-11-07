//! What is inside an app's databases and preferences, for comparing two snapshots.

use std::path::Path;

use emulabzip_core::{Error, Result};

/// Describes the databases and preferences inside a state archive, as JSON. Not there yet.
pub fn scan_json(_archive: &Path) -> Result<Vec<u8>> {
    Err(Error::Unsupported("Scanning an archive for schemas"))
}
