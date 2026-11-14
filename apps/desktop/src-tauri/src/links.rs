//! Commands for the Deep links view: open a link, check it, and keep the history.

use emulabzip_core::links::{self, HistoryEntry};
use emulabzip_core::{Control, Target};
use emulabzip_net::links::{Report, check};
use tauri::State;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

#[tauri::command]
pub fn link_history() -> Vec<HistoryEntry> {
    links::load_history()
}

/// Opens a link on the device and remembers it.
#[tauri::command]
pub async fn link_open(state: State<'_, AppState>, target: Target, url: String, app: Option<String>) -> CmdResult<()> {
    state.lab.control(&target, &Control::OpenUrl { url: url.clone() }).await.map_err(err)?;
    let _ = links::note_link(&url, app.as_deref(), None);
    Ok(())
}

/// Checks a link against its domain's file and what the device says, and remembers the result.
#[tauri::command]
pub async fn link_check(
    state: State<'_, AppState>,
    target: Target,
    url: String,
    app: Option<String>,
) -> CmdResult<Report> {
    let report = check(&state.lab, &target, &url, app.as_deref()).await.map_err(err)?;
    let _ = links::note_link(&url, app.as_deref(), Some(report.worst));
    Ok(report)
}

/// Asks Android to verify an app's domains again.
#[tauri::command]
pub async fn link_verify(state: State<'_, AppState>, target: Target, app: String) -> CmdResult<()> {
    state.lab.control(&target, &Control::VerifyAppLinks { app }).await.map_err(err)
}

#[tauri::command]
pub fn link_pin(url: String, pinned: bool) -> CmdResult<Vec<HistoryEntry>> {
    links::pin_link(&url, pinned).map_err(err)?;
    Ok(links::load_history())
}

#[tauri::command]
pub fn link_forget(url: String) -> CmdResult<Vec<HistoryEntry>> {
    links::forget_link(&url).map_err(err)?;
    Ok(links::load_history())
}
