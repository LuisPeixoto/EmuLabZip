//! Saving an app's data to a file, putting it back, and comparing two. Putting back needs `confirm`.

use std::path::PathBuf;

use emulabzip_core::Target;
use emulabzip_storage::schema::{Difference, FileSchema};
use emulabzip_storage::snapshot::{self, StateMeta};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::{CmdResult, ask_open_path, ask_save_path, err};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedState {
    pub path: String,
    pub meta: StateMeta,
}

#[tauri::command]
pub async fn state_save(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Target,
    package: String,
    external: bool,
) -> CmdResult<Option<SavedState>> {
    let Some(path) = ask_save_path(&app, &format!("{package}.ezstate"), ("App state", &["ezstate"])).await else {
        return Ok(None);
    };
    // The save dialog may have asked about replacing a file; the library never writes over one.
    if path.exists() {
        std::fs::remove_file(&path).map_err(err)?;
    }
    let meta = snapshot::save(&state.lab, &target, &package, &path, external).await.map_err(err)?;
    Ok(Some(SavedState { path: path.display().to_string(), meta }))
}

#[tauri::command]
pub async fn state_pick(app: AppHandle) -> CmdResult<Option<String>> {
    Ok(ask_open_path(&app, ("App state", &["ezstate"])).await.map(|p| p.display().to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateInfo {
    pub path: String,
    pub meta: StateMeta,
    pub files: Vec<FileSchema>,
}

#[tauri::command]
pub async fn state_info(path: String) -> CmdResult<StateInfo> {
    tokio::task::spawn_blocking(move || {
        let state = snapshot::open(std::path::Path::new(&path))?;
        let files = state.schemas()?.files;
        Ok::<_, emulabzip_core::Error>(StateInfo { path, meta: state.meta, files })
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    pub backup: String,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub async fn state_restore(
    state: State<'_, AppState>,
    target: Target,
    path: String,
    confirm: bool,
) -> CmdResult<Restored> {
    let r = snapshot::restore(&state.lab, &target, std::path::Path::new(&path), confirm).await.map_err(err)?;
    Ok(Restored { backup: r.backup.display().to_string(), warnings: r.warnings })
}

#[derive(Serialize)]
pub struct Differences {
    pub changes: Vec<Difference>,
    pub lines: Vec<String>,
}

fn differences(changes: Vec<Difference>) -> Differences {
    let lines = changes.iter().map(Difference::line).collect();
    Differences { changes, lines }
}

#[tauri::command]
pub async fn state_compare(a: String, b: String) -> CmdResult<Differences> {
    tokio::task::spawn_blocking(move || snapshot::compare(std::path::Path::new(&a), std::path::Path::new(&b)))
        .await
        .map_err(err)?
        .map(differences)
        .map_err(err)
}

/// A saved state against the app as it is on the device now.
#[tauri::command]
pub async fn state_compare_now(state: State<'_, AppState>, target: Target, path: String) -> CmdResult<Differences> {
    let before = snapshot::open(std::path::Path::new(&path)).map_err(err)?;
    let now: PathBuf = std::env::temp_dir().join(format!("emulabzip-now-{}.ezstate", std::process::id()));
    let _ = std::fs::remove_file(&now);
    snapshot::save(&state.lab, &target, &before.meta.app, &now, false).await.map_err(err)?;
    let result = snapshot::compare(std::path::Path::new(&path), &now);
    let _ = std::fs::remove_file(&now);
    result.map(differences).map_err(err)
}
