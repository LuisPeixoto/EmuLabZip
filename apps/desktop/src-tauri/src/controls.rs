//! Commands for the Controls tool.

use emulabzip_core::{Control, ControlInfo, Target};
use tauri::State;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

/// What the device can be told to do, its appearance, and the permissions of `app`.
#[tauri::command]
pub async fn controls_info(state: State<'_, AppState>, target: Target, app: Option<String>) -> CmdResult<ControlInfo> {
    state.lab.controls(&target, app.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn control_apply(state: State<'_, AppState>, target: Target, control: Control) -> CmdResult<()> {
    state.lab.control(&target, &control).await.map_err(err)
}
