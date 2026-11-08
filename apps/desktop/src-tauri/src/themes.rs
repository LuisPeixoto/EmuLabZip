//! Importing and exporting themes as `.json` files. The front end checks what a theme holds; these commands only move bytes,
//! and refuse anything that could not be a theme: a file that is too big, or that is not a JSON object.

use std::path::Path;

use tauri::AppHandle;

use crate::commands::{CmdResult, ask_open_path, ask_save_path, err};

/// A theme is a few hundred bytes of colours. Anything bigger than this is not one.
const MAX_THEME_BYTES: u64 = 64 * 1024;

/// The text of a theme file, when it is small enough and a JSON object.
fn read_theme(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    if !meta.is_file() || meta.len() > MAX_THEME_BYTES {
        return Err("That file is too big to be a theme.".into());
    }
    let text = std::fs::read_to_string(path).map_err(|_| "That file is not text.".to_string())?;
    check_json(&text)?;
    Ok(text)
}

fn check_json(text: &str) -> Result<(), String> {
    if text.len() as u64 > MAX_THEME_BYTES {
        return Err("That is too big to be a theme.".into());
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) if v.is_object() => Ok(()),
        _ => Err("A theme is a JSON object.".into()),
    }
}

/// Asks which file to open, and gives back its text for the interface to check. `None` when the person cancels.
#[tauri::command]
pub async fn theme_import(app: AppHandle) -> CmdResult<Option<String>> {
    let Some(path) = ask_open_path(&app, ("Theme", &["json"])).await else { return Ok(None) };
    tokio::task::spawn_blocking(move || read_theme(&path)).await.map_err(err)?.map(Some)
}

/// Asks where to save a theme, and writes it there. `None` when the person cancels.
#[tauri::command]
pub async fn theme_export(app: AppHandle, name: String, json: String) -> CmdResult<Option<String>> {
    check_json(&json)?;
    let file: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).take(48).collect();
    let file = if file.is_empty() { "theme".to_string() } else { file };
    let Some(path) = ask_save_path(&app, &format!("{file}.json"), ("Theme", &["json"])).await else { return Ok(None) };
    tokio::fs::write(&path, json).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-themes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_small_json_object_is_read() {
        let dir = scratch("ok");
        let file = dir.join("t.json");
        std::fs::write(&file, r#"{"name":"x","base":"dark"}"#).unwrap();
        assert!(read_theme(&file).unwrap().contains("dark"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_big_file_or_one_that_is_not_an_object_is_refused() {
        let dir = scratch("bad");
        let big = dir.join("big.json");
        std::fs::write(&big, format!(r#"{{"x":"{}"}}"#, "a".repeat(70_000))).unwrap();
        assert!(read_theme(&big).unwrap_err().contains("too big"));
        let list = dir.join("list.json");
        std::fs::write(&list, "[1,2]").unwrap();
        assert!(read_theme(&list).unwrap_err().contains("JSON object"));
        let junk = dir.join("junk.json");
        std::fs::write(&junk, "not json").unwrap();
        assert!(read_theme(&junk).is_err());
        assert!(read_theme(&dir).is_err(), "a folder is not a theme");
        assert!(check_json(&format!(r#"{{"x":"{}"}}"#, "a".repeat(70_000))).is_err());
        std::fs::remove_dir_all(dir).ok();
    }
}
