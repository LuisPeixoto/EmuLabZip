//! Commands for the Layout view: the views on screen, and the accessibility audit of them.

use base64::Engine;
use emulabzip_core::a11y::{Audit, audit};
use emulabzip_core::{LayoutSnapshot, Target};
use serde::Serialize;
use tauri::State;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutResult {
    pub layout: LayoutSnapshot,
    /// The screenshot taken with the layout, as base64 PNG.
    pub screenshot: String,
    pub audit: Audit,
}

#[tauri::command]
pub async fn layout_read(state: State<'_, AppState>, target: Target, app: Option<String>) -> CmdResult<LayoutResult> {
    let mut layout = state.lab.layout(&target).await.map_err(err)?;
    // iOS views have no host class that gives their framework away, so the app's files say it for the whole screen.
    if let Some(app) = app
        && layout.platform == emulabzip_core::Platform::Ios
        && let Ok(found) = state.lab.frameworks(&target, &app).await
        && let Some(framework) = emulabzip_core::layout::screen_framework(&found)
    {
        layout.root.mark_all(framework);
    }
    // The audit reads the screenshot's pixels, which takes a moment for a large screen.
    let for_audit = layout.clone();
    let audit = tokio::task::spawn_blocking(move || audit(&for_audit)).await.map_err(err)?;
    let screenshot = base64::engine::general_purpose::STANDARD.encode(&layout.screenshot);
    Ok(LayoutResult { layout, screenshot, audit })
}
