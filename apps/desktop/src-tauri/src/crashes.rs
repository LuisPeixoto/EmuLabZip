//! Commands for the Crashes tool.

use emulabzip_core::{CrashReport, CrashSummary, Target};
use tauri::{AppHandle, State};

use crate::commands::{CmdResult, ask_save_path, err};
use crate::state::AppState;

/// The crashes of a device, newest first. `app` limits them to one package or bundle id.
#[tauri::command]
pub async fn crash_list(
    state: State<'_, AppState>,
    target: Target,
    app: Option<String>,
) -> CmdResult<Vec<CrashSummary>> {
    state.lab.crashes(&target, app.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn crash_get(state: State<'_, AppState>, target: Target, id: String) -> CmdResult<CrashReport> {
    state.lab.crash(&target, &id).await.map_err(err)
}

/// Saves a report as the device wrote it. Returns the path, or `None` if the user cancelled.
#[tauri::command]
pub async fn crash_save(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Target,
    id: String,
) -> CmdResult<Option<String>> {
    let report = state.lab.crash(&target, &id).await.map_err(err)?;
    let name = format!("{}.txt", id.replace(':', "-"));
    let Some(path) = ask_save_path(&app, &name, ("Crash report", &["txt"])).await else { return Ok(None) };
    tokio::fs::write(&path, report.raw).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}
