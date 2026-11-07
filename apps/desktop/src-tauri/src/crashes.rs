//! Commands for the Crashes tool.

use emulabzip_core::{CrashReport, CrashSummary, Target};
use emulabzip_symbols::store::{Kind, Store, Stored};
use emulabzip_symbols::symbolicate::{Source, symbolicate};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::{CmdResult, ask_folder, ask_open_path, ask_save_path, err};
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

/// A crash, and what was used to make it readable.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadableCrash {
    pub report: CrashReport,
    /// Files that made frames readable, such as `mapping.txt (mapping.txt, 12 frames)`.
    pub applied: Vec<String>,
    /// Files that would make more of it readable, and are not there.
    pub missing: Vec<String>,
}

/// One crash. Unless `original`, its frames are read with the mappings, source maps and dSYMs stored for its app and
/// version.
#[tauri::command]
pub async fn crash_get(
    state: State<'_, AppState>,
    target: Target,
    id: String,
    original: bool,
) -> CmdResult<ReadableCrash> {
    let report = state.lab.crash(&target, &id).await.map_err(err)?;
    if original {
        return Ok(ReadableCrash { report, applied: Vec::new(), missing: Vec::new() });
    }
    // Reading a big mapping takes a moment and must not hold up the interface.
    tokio::task::spawn_blocking(move || {
        let sources: Vec<Source> = match &report.summary.app {
            Some(app) => Store::default().find(app, report.app_version.as_deref()).iter().map(Source::from).collect(),
            None => Vec::new(),
        };
        let s = symbolicate(&report, &sources);
        ReadableCrash {
            applied: s
                .applied
                .iter()
                .map(|a| format!("{} ({}, {} frames)", a.source, a.kind.label(), a.frames))
                .collect(),
            missing: s.missing,
            report: s.report,
        }
    })
    .await
    .map_err(err)
}

/// Asks for a `mapping.txt`, a source map or a `.dSYM` folder, and keeps it for an app and version. `None` when the
/// user cancels.
#[tauri::command]
pub async fn mapping_add(app: AppHandle, package: String, version: String, folder: bool) -> CmdResult<Option<Stored>> {
    let picked = if folder {
        ask_folder(&app).await
    } else {
        ask_open_path(&app, ("Mapping or source map", &["txt", "map", "json"])).await
    };
    let Some(path) = picked else { return Ok(None) };
    let kind = if folder { Some(Kind::Dsym) } else { None };
    tokio::task::spawn_blocking(move || Store::default().add(&package, &version, &path, kind))
        .await
        .map_err(err)?
        .map(Some)
        .map_err(err)
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
