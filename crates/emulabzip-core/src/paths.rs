//! Where EmuLabZip keeps its files.

use std::path::PathBuf;

/// The folder for the certificate authority and other private files:
/// `~/Library/Application Support/EmuLabZip` on macOS, `%APPDATA%\EmuLabZip` on Windows and
/// `~/.local/share/EmuLabZip` on Linux. Set `EMULABZIP_HOME` to use another folder, for example
/// a throwaway one while trying things out.
pub fn data_dir() -> PathBuf {
    match std::env::var_os("EMULABZIP_HOME").filter(|v| !v.is_empty()) {
        Some(home) => PathBuf::from(home),
        None => dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("EmuLabZip"),
    }
}
