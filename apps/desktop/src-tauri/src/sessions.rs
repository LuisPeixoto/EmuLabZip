//! Commands for the Sessions view: recording a device into a session file, and opening one.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use emulabzip_core::{CrashSummary, LogEntry, PerfSample, Target, data_dir, diff};
use emulabzip_session::{Meta, Options, Recorder, Session, Stats, Summary, summarize};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::{CmdResult, ask_open_path, err};
use crate::state::AppState;

/// The most log lines sent to the view; the rest stay in the file.
const MAX_LOGS: usize = 20_000;

pub struct Active {
    recorder: Recorder,
    since: std::time::Instant,
    device: String,
    network: bool,
}

fn dir() -> PathBuf {
    data_dir().join("sessions")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub recording: bool,
    pub device: Option<String>,
    pub seconds: u64,
    pub logs: u64,
    pub samples: u64,
    pub crashes: u64,
    pub screens: u64,
    pub network: bool,
}

fn status_of(active: Option<&Active>) -> RecordingStatus {
    match active {
        None => RecordingStatus {
            recording: false,
            device: None,
            seconds: 0,
            logs: 0,
            samples: 0,
            crashes: 0,
            screens: 0,
            network: false,
        },
        Some(a) => {
            let Stats { logs, samples, crashes, screens } = a.recorder.stats();
            RecordingStatus {
                recording: true,
                device: Some(a.device.clone()),
                seconds: a.since.elapsed().as_secs(),
                logs,
                samples,
                crashes,
                screens,
                network: a.network,
            }
        }
    }
}

#[tauri::command]
pub async fn session_status(state: State<'_, AppState>) -> CmdResult<RecordingStatus> {
    Ok(status_of(state.recording.lock().await.as_ref()))
}

fn stamp() -> String {
    // 20261024-143355, from the clock, without a date library.
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[tauri::command]
pub async fn session_start(
    state: State<'_, AppState>,
    target: Target,
    app: Option<String>,
    network: bool,
    screens_every: Option<u64>,
) -> CmdResult<RecordingStatus> {
    let mut slot = state.recording.lock().await;
    if slot.is_some() {
        return Err("A recording is already running.".into());
    }
    let device = state.lab.resolve(Some(&target.id)).await.map_err(err)?;
    if network {
        let rt = state.net.lock().await;
        if rt.proxy.is_none() || !rt.connected.contains(&target) {
            return Err("To record the network, connect this device in the Network view first.".into());
        }
        state.flows.clear();
    }
    tokio::fs::create_dir_all(dir()).await.map_err(err)?;
    let name: String = device.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let out = dir().join(format!("{name}-{}.ezsession", stamp()));
    let options = Options {
        app,
        screens_every: screens_every.filter(|s| *s > 0).map(Duration::from_secs),
        flows: network.then(|| state.flows.clone()),
        ..Default::default()
    };
    let recorder = Recorder::start(state.lab.clone(), &device, out, options).await.map_err(err)?;
    *slot = Some(Active { recorder, since: std::time::Instant::now(), device: device.name, network });
    Ok(status_of(slot.as_ref()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub path: String,
}

#[tauri::command]
pub async fn session_stop(state: State<'_, AppState>) -> CmdResult<Saved> {
    let active = state.recording.lock().await.take().ok_or("There is no recording to stop.")?;
    let path = active.recorder.finish().await.map_err(err)?;
    Ok(Saved { path: path.display().to_string() })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_ms: u64,
}

/// The sessions recorded from the app, newest first.
#[tauri::command]
pub async fn session_list() -> CmdResult<Vec<SessionFile>> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir()).await else { return Ok(out) };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == emulabzip_session::EXTENSION)
            && let Ok(meta) = entry.metadata().await
        {
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64);
            out.push(SessionFile {
                name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                path: path.display().to_string(),
                size: meta.len(),
                modified_ms,
            });
        }
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.modified_ms));
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRow {
    pub index: usize,
    pub method: String,
    pub url: String,
    pub status: u64,
    pub ms: f64,
    pub size: i64,
    pub started: String,
    pub content_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub path: String,
    pub meta: Meta,
    pub summary: Summary,
    pub logs: Vec<LogEntry>,
    pub logs_total: usize,
    pub perf: Vec<PerfSample>,
    pub requests: Vec<RequestRow>,
    pub crashes: Vec<CrashSummary>,
    pub screens: Vec<String>,
}

fn requests(har: &serde_json::Value) -> Vec<RequestRow> {
    let Some(entries) = har.pointer("/log/entries").and_then(|v| v.as_array()) else { return Vec::new() };
    entries
        .iter()
        .enumerate()
        .map(|(index, e)| RequestRow {
            index,
            method: e.pointer("/request/method").and_then(|v| v.as_str()).unwrap_or("").into(),
            url: e.pointer("/request/url").and_then(|v| v.as_str()).unwrap_or("").into(),
            status: e.pointer("/response/status").and_then(|v| v.as_u64()).unwrap_or(0),
            ms: e.get("time").and_then(|v| v.as_f64()).unwrap_or(0.0),
            size: e.pointer("/response/content/size").and_then(|v| v.as_i64()).unwrap_or(0),
            started: e.get("startedDateTime").and_then(|v| v.as_str()).unwrap_or("").into(),
            content_type: e.pointer("/response/content/mimeType").and_then(|v| v.as_str()).unwrap_or("").into(),
        })
        .collect()
}

/// Opens a session: the given path, or one the user picks. `None` when cancelled.
#[tauri::command]
pub async fn session_open(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> CmdResult<Option<SessionView>> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => match ask_open_path(&app, ("EmuLabZip session", &[emulabzip_session::EXTENSION])).await {
            Some(p) => p,
            None => return Ok(None),
        },
    };
    let opened = path.clone();
    let session = tokio::task::spawn_blocking(move || Session::open(&opened)).await.map_err(err)?.map_err(err)?;
    let summary = summarize(&session);
    let skip = session.logs.len().saturating_sub(MAX_LOGS);
    let view = SessionView {
        path: path.display().to_string(),
        meta: session.meta.clone(),
        summary,
        logs: session.logs[skip..].to_vec(),
        logs_total: session.logs.len(),
        perf: session.perf.clone(),
        requests: session.har.as_ref().map(requests).unwrap_or_default(),
        crashes: session.crashes.iter().map(|c| c.summary.clone()).collect(),
        screens: session.screens.clone(),
    };
    *state.opened.lock() = Some(Arc::new(session));
    Ok(Some(view))
}

fn opened(state: &AppState) -> CmdResult<Arc<Session>> {
    state.opened.lock().clone().ok_or_else(|| "No session is open.".to_string())
}

/// One request of the open session, as HAR describes it: headers, body and timings.
#[tauri::command]
pub fn session_request(state: State<'_, AppState>, index: usize) -> CmdResult<serde_json::Value> {
    let session = opened(&state)?;
    session
        .har
        .as_ref()
        .and_then(|h| h.pointer("/log/entries"))
        .and_then(|e| e.get(index))
        .cloned()
        .ok_or_else(|| "That request is not in the session.".to_string())
}

#[tauri::command]
pub fn session_screen(state: State<'_, AppState>, name: String) -> CmdResult<String> {
    let png = opened(&state)?.screen(&name).map_err(err)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

#[tauri::command]
pub async fn session_delete(path: String) -> CmdResult<()> {
    let path = PathBuf::from(path);
    // Only files of the sessions folder are removed from here.
    if path.parent() != Some(dir().as_path()) || path.extension().is_none_or(|e| e != emulabzip_session::EXTENSION) {
        return Err("Only sessions recorded by EmuLabZip can be deleted here.".into());
    }
    tokio::fs::remove_file(&path).await.map_err(err)
}

#[tauri::command]
pub fn session_reveal(app: AppHandle, path: String) -> CmdResult<()> {
    let path = Path::new(&path);
    app.opener().reveal_item_in_dir(path).map_err(err)
}

// ---------------------------------------------------------------------------------------------
// Compare
// ---------------------------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub rows: Vec<emulabzip_core::DiffRow>,
}

#[tauri::command]
pub async fn compare_info(state: State<'_, AppState>, a: Target, b: Target) -> CmdResult<Comparison> {
    let (ia, ib) = tokio::join!(state.lab.info(&a), state.lab.info(&b));
    Ok(Comparison { rows: diff(&ia.map_err(err)?, &ib.map_err(err)?) })
}
