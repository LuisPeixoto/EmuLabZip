//! Helps people connect an AI assistant: finds the `emulabzip` command the assistant will start.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInfo {
    /// The full path of the command, when it was found.
    pub path: Option<String>,
}

fn candidate(dir: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "emulabzip.exe" } else { "emulabzip" };
    let file = dir.join(name);
    file.is_file().then_some(file)
}

/// Looks for the command on `PATH`, then where `cargo install` puts it.
pub fn find_cli(path: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    let on_path = path.iter().flat_map(std::env::split_paths).find_map(|dir| candidate(&dir));
    on_path.or_else(|| home.and_then(|h| candidate(&h.join(".cargo").join("bin"))))
}

#[tauri::command]
pub fn ai_cli_info() -> CliInfo {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    CliInfo { path: find_cli(std::env::var_os("PATH"), home).map(|p| p.display().to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_command_on_the_path_before_cargo_bin() {
        let root = std::env::temp_dir().join(format!("ez-ai-{}", std::process::id()));
        let bin = root.join("bin");
        let cargo = root.join("home/.cargo/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&cargo).unwrap();
        let name = if cfg!(windows) { "emulabzip.exe" } else { "emulabzip" };
        assert_eq!(find_cli(Some(bin.clone().into_os_string()), Some(root.join("home"))), None);
        std::fs::write(cargo.join(name), b"x").unwrap();
        assert_eq!(find_cli(Some(bin.clone().into_os_string()), Some(root.join("home"))), Some(cargo.join(name)));
        std::fs::write(bin.join(name), b"x").unwrap();
        assert_eq!(find_cli(Some(bin.clone().into_os_string()), Some(root.join("home"))), Some(bin.join(name)));
        let _ = std::fs::remove_dir_all(&root);
    }
}
