//! Commands for rules that use files: picking them, and approving WebAssembly scripts.

use emulabzip_net::script::{Approvals, Script};
use serde::Serialize;
use tauri::AppHandle;

use crate::commands::{CmdResult, ask_folder, ask_open_path, err};

/// Asks which file to use for a rule. `kind` is `openapi` or `script`; `None` when the person cancels.
#[tauri::command]
pub async fn rule_pick_file(app: AppHandle, kind: String) -> CmdResult<Option<String>> {
    let path = match kind.as_str() {
        "openapi" => ask_open_path(&app, ("OpenAPI description", &["yaml", "yml", "json"])).await,
        "script" => ask_open_path(&app, ("WebAssembly script", &["wasm"])).await,
        "file" => ask_open_path(&app, ("File", &["*"])).await,
        other => return Err(format!("`{other}` is not a kind of file a rule uses.")),
    };
    Ok(path.map(|p| p.display().to_string()))
}

/// Asks which folder to serve files from.
#[tauri::command]
pub async fn rule_pick_folder(app: AppHandle) -> CmdResult<Option<String>> {
    Ok(ask_folder(&app).await.map(|p| p.display().to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInfo {
    pub name: String,
    pub sha256: String,
    pub on_request: bool,
    pub on_response: bool,
    pub approved: bool,
}

fn describe(script: &Script) -> ScriptInfo {
    ScriptInfo {
        name: script.name.clone(),
        sha256: script.sha256.clone(),
        on_request: script.on_request,
        on_response: script.on_response,
        approved: Approvals::load().is_approved(&script.sha256),
    }
}

/// What a script file is, and whether it may run. A file that is not a valid script says why.
#[tauri::command]
pub async fn script_info(path: String) -> CmdResult<ScriptInfo> {
    let script = tokio::task::spawn_blocking(move || Script::load(std::path::Path::new(&path)))
        .await
        .map_err(err)?
        .map_err(err)?;
    Ok(describe(&script))
}

/// Lets a script run. The approval is for the exact bytes of the file as it is now.
#[tauri::command]
pub async fn script_approve(path: String) -> CmdResult<ScriptInfo> {
    let script = tokio::task::spawn_blocking(move || Script::load(std::path::Path::new(&path)))
        .await
        .map_err(err)?
        .map_err(err)?;
    Approvals::load().approve(&script).map_err(err)?;
    Ok(describe(&script))
}
