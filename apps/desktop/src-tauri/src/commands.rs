//! Commands invoked by the interface.

use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use emulabzip_core::{App, Device, InfoSection, LogEntry, LogOptions, Platform, Target};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::AppState;

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    pub os: &'static str,
    pub arch: &'static str,
    pub version: &'static str,
}

#[tauri::command]
pub fn host_info() -> HostInfo {
    HostInfo { os: std::env::consts::OS, arch: std::env::consts::ARCH, version: env!("CARGO_PKG_VERSION") }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformStatus {
    pub platform: Platform,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicesReport {
    pub devices: Vec<Device>,
    pub platforms: Vec<PlatformStatus>,
}

#[tauri::command]
pub async fn list_devices(state: State<'_, AppState>) -> CmdResult<DevicesReport> {
    let mut devices = Vec::new();
    let mut platforms = Vec::new();
    for backend in state.lab.backends() {
        let available = backend.is_available().await;
        let mut error = None;
        if available {
            match backend.devices().await {
                Ok(mut list) => devices.append(&mut list),
                Err(e) => error = Some(e.to_string()),
            }
        }
        platforms.push(PlatformStatus { platform: backend.platform(), available, error });
    }
    Ok(DevicesReport { devices, platforms })
}

#[tauri::command]
pub async fn device_info(state: State<'_, AppState>, target: Target) -> CmdResult<Vec<InfoSection>> {
    state.lab.info(&target).await.map_err(err)
}

#[tauri::command]
pub async fn list_apps(state: State<'_, AppState>, target: Target) -> CmdResult<Vec<App>> {
    state.lab.apps(&target).await.map_err(err)
}

/// A screenshot as base64 PNG, for the preview.
#[tauri::command]
pub async fn screenshot(state: State<'_, AppState>, target: Target) -> CmdResult<String> {
    let png = state.lab.screenshot(&target).await.map_err(err)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

/// Asks where to save, on the native dialog. Files are only written to a path the user picked.
pub(crate) async fn ask_save_path(app: &AppHandle, name: &str, filter: (&str, &[&str])) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_file_name(name).add_filter(filter.0, filter.1).save_file(move |path| {
        let _ = tx.send(path.and_then(|p| p.into_path().ok()));
    });
    rx.await.ok().flatten()
}

/// Asks which file to open. `None` when the user cancels.
pub(crate) async fn ask_open_path(app: &AppHandle, filter: (&str, &[&str])) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().add_filter(filter.0, filter.1).pick_file(move |path| {
        let _ = tx.send(path.and_then(|p| p.into_path().ok()));
    });
    rx.await.ok().flatten()
}

/// Saves a PNG the UI already has (base64), returning the path, or `None` if the user cancelled.
#[tauri::command]
pub async fn save_png(app: AppHandle, name: String, data: String) -> CmdResult<Option<String>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).map_err(err)?;
    let Some(path) = ask_save_path(&app, &name, ("PNG image", &["png"])).await else { return Ok(None) };
    tokio::fs::write(&path, bytes).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}

/// Saves text (exported logs), returning the path, or `None` if the user cancelled.
#[tauri::command]
pub async fn save_text(app: AppHandle, name: String, contents: String) -> CmdResult<Option<String>> {
    let ext = if name.ends_with(".jsonl") { "jsonl" } else { "txt" };
    let Some(path) = ask_save_path(&app, &name, ("Text", &[ext])).await else { return Ok(None) };
    tokio::fs::write(&path, contents).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogBatch {
    pub entries: Vec<LogEntry>,
    /// Lines discarded because the interface could not keep up.
    pub dropped: u64,
    /// The stream ended (device disconnected, emulator closed).
    pub ended: bool,
}

/// The most lines sent in one batch. The iOS unified log can produce thousands per second.
const MAX_BATCH: usize = 4000;

/// Starts following the device log. Lines arrive on `on_batch` every 100 ms.
#[tauri::command]
pub async fn start_logs(
    state: State<'_, AppState>,
    target: Target,
    options: LogOptions,
    on_batch: Channel<LogBatch>,
) -> CmdResult<u64> {
    let mut stream = state.lab.logs(&target, options).await.map_err(err)?;
    let task = tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        let mut batch: Vec<LogEntry> = Vec::new();
        let mut dropped = 0u64;
        loop {
            tokio::select! {
                entry = stream.next() => match entry {
                    Some(e) => {
                        if batch.len() == MAX_BATCH {
                            batch.remove(0);
                            dropped += 1;
                        }
                        batch.push(e);
                    }
                    None => {
                        let _ = on_batch.send(LogBatch { entries: std::mem::take(&mut batch), dropped, ended: true });
                        break;
                    }
                },
                _ = tick.tick() => {
                    if !batch.is_empty() || dropped > 0 {
                        let out = LogBatch { entries: std::mem::take(&mut batch), dropped, ended: false };
                        dropped = 0;
                        if on_batch.send(out).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });
    Ok(state.add_session(task))
}

#[tauri::command]
pub fn stop_session(state: State<'_, AppState>, id: u64) {
    state.stop_session(id);
}
