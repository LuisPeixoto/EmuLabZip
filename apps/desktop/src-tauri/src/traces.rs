//! Commands for the Traces tool: recording, the library of recordings, and looking inside them.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use emulabzip_core::{Target, TraceFormat, TraceRequest};
use emulabzip_trace::flame::{Hotspot, Node, folded, hotspots};
use emulabzip_trace::library::{self, Recording};
use emulabzip_trace::perfetto::processor::{self, Processor, Rows};
use emulabzip_trace::perfetto::summary::{Summary, summarize};
use emulabzip_trace::perfetto::ui;
use emulabzip_trace::{simpleperf, xctrace};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::watch;

use crate::commands::{CmdResult, ask_save_path, err};
use crate::state::AppState;

/// How often a recording reports how long it has been running.
const TICK: Duration = Duration::from_millis(250);

/// How long the flame graph's hot list is.
const HOTSPOTS: usize = 15;

/// What a recording reports while it runs.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TraceEvent {
    #[serde(rename_all = "camelCase")]
    Recording {
        elapsed_ms: u64,
        stopping: bool,
    },
    Done {
        recording: Box<Recording>,
    },
    Failed {
        message: String,
    },
}

/// Starts a recording. Progress arrives on `on_event`; the last event is `done` or `failed`.
/// Returns the session to pass to [`trace_stop`].
#[tauri::command]
pub async fn trace_start(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Target,
    device: String,
    request: TraceRequest,
    on_event: Channel<TraceEvent>,
) -> CmdResult<u64> {
    let id = library::new_id();
    let dir = library::folder(&id).map_err(err)?;
    let (stop, stopped) = watch::channel(false);
    let session = state.next_id();
    state.trace_stops.lock().insert(session, stop);

    let task = tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let started = Instant::now();
        let mut flag = stopped.clone();
        let recording = state.lab.record_trace(&target, &request, &dir, stopped);
        tokio::pin!(recording);
        let mut tick = tokio::time::interval(TICK);
        let result = loop {
            tokio::select! {
                result = &mut recording => break result,
                _ = tick.tick() => {
                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    let _ = on_event.send(TraceEvent::Recording { elapsed_ms, stopping: *flag.borrow_and_update() });
                }
            }
        };
        let event = match result.and_then(|file| Recording::save(&id, target.platform, &device, &request, &file)) {
            Ok(recording) => TraceEvent::Done { recording: Box::new(recording) },
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                TraceEvent::Failed { message: e.to_string() }
            }
        };
        state.trace_stops.lock().remove(&session);
        let _ = on_event.send(event);
    });
    state.register_session(session, task);
    Ok(session)
}

/// Ends a recording early. What was recorded so far is kept.
#[tauri::command]
pub fn trace_stop(state: State<'_, AppState>, session: u64) {
    if let Some(stop) = state.trace_stops.lock().get(&session) {
        let _ = stop.send(true);
    }
}

#[tauri::command]
pub async fn trace_templates(state: State<'_, AppState>, target: Target) -> CmdResult<Vec<String>> {
    state.lab.trace_templates(&target).await.map_err(err)
}

#[tauri::command]
pub fn trace_list() -> Vec<Recording> {
    library::list()
}

#[tauri::command]
pub async fn trace_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.forget_trace(&id).await;
    library::delete(&id).map_err(err)
}

/// What the Perfetto trace processor is and whether it is on this computer yet.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorStatus {
    pub supported: bool,
    pub installed: bool,
    pub version: &'static str,
    pub size_bytes: Option<u64>,
    pub url: Option<String>,
}

#[tauri::command]
pub fn trace_processor_status() -> ProcessorStatus {
    let asset = processor::asset().ok();
    ProcessorStatus {
        supported: asset.is_some(),
        installed: processor::installed().is_some(),
        version: processor::VERSION,
        size_bytes: asset.map(|a| a.size),
        url: asset.map(|a| processor::download_url(&a)),
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub done: u64,
    pub total: Option<u64>,
}

/// Downloads and verifies the trace processor, reporting progress.
#[tauri::command]
pub async fn trace_install_processor(on_progress: Channel<DownloadProgress>) -> CmdResult<()> {
    let progress: processor::Progress = Arc::new(move |done, total| {
        let _ = on_progress.send(DownloadProgress { done, total });
    });
    processor::ensure_installed(progress).await.map(|_| ()).map_err(err)
}

/// A recording, ready to look at.
#[derive(Serialize)]
#[serde(tag = "view", rename_all = "camelCase")]
pub enum Opened {
    /// A Perfetto trace: the summary, and SQL on demand.
    Perfetto { summary: Box<Summary> },
    /// CPU samples as a flame graph.
    #[serde(rename_all = "camelCase")]
    Cpu(Box<CpuView>),
    /// An Instruments document that could not be read here: open it in Instruments.
    #[serde(rename_all = "camelCase")]
    External { reason: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuView {
    /// `simpleperf` or `instruments`.
    pub source: &'static str,
    /// `ns` when weights are CPU time, `events` for event counts.
    pub unit: &'static str,
    pub processes: Vec<ProcessRow>,
    pub flame: Node,
    pub hotspots: Vec<Hotspot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRow {
    /// What to pass back to [`trace_flame`]: a pid for simpleperf, the name for Instruments.
    pub key: String,
    pub name: String,
    pub samples: u64,
    pub weight: u64,
    pub threads: Vec<ThreadRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRow {
    pub key: String,
    pub name: String,
    pub weight: u64,
}

/// A parsed CPU profile, kept for the recording that is open so picking a process is instant.
pub enum Loaded {
    Simpleperf(Box<simpleperf::Profile>),
    Instruments(xctrace::Profile),
}

async fn loaded(state: &AppState, rec: &Recording) -> CmdResult<Option<Arc<Loaded>>> {
    if let Some((id, profile)) = &*state.profile.lock()
        && *id == rec.id
    {
        return Ok(Some(profile.clone()));
    }
    let profile = match rec.format {
        TraceFormat::SimpleperfReport => {
            let path = rec.path();
            let bytes = tokio::fs::read(&path).await.map_err(err)?;
            let parsed = tokio::task::spawn_blocking(move || simpleperf::parse(&bytes)).await.map_err(err)?;
            Loaded::Simpleperf(Box::new(parsed.map_err(err)?))
        }
        TraceFormat::InstrumentsBundle => {
            let Some(xml) = rec.extra_paths().into_iter().find(|p| p.extension().is_some_and(|e| e == "xml")) else {
                return Ok(None);
            };
            let text = tokio::fs::read_to_string(&xml).await.map_err(err)?;
            let parsed = tokio::task::spawn_blocking(move || xctrace::parse(&text)).await.map_err(err)?;
            Loaded::Instruments(parsed.map_err(err)?)
        }
        TraceFormat::Perfetto => return Ok(None),
    };
    let profile = Arc::new(profile);
    *state.profile.lock() = Some((rec.id.clone(), profile.clone()));
    Ok(Some(profile))
}

fn cpu_view(profile: &Loaded, process: Option<&str>, thread: Option<&str>) -> CmdResult<CpuView> {
    match profile {
        Loaded::Simpleperf(p) => {
            let processes = p.processes(0, false);
            // Start with the busiest process, which is what people want to see first.
            let pid = match process {
                Some(k) => Some(k.parse::<u32>().map_err(err)?),
                None => processes.first().map(|p| p.pid),
            };
            let tid = thread.map(|t| t.parse::<u32>().map_err(err)).transpose()?;
            let flame = p.flame(&simpleperf::Filter { pid, tid, event: 0, include_idle: false });
            Ok(CpuView {
                source: "simpleperf",
                unit: if p.unit(0) == simpleperf::Unit::Nanoseconds { "ns" } else { "events" },
                hotspots: hotspots(&flame, HOTSPOTS),
                flame,
                processes: processes
                    .into_iter()
                    .map(|p| ProcessRow {
                        key: p.pid.to_string(),
                        name: p.name,
                        samples: p.samples,
                        weight: p.weight,
                        threads: p
                            .threads
                            .into_iter()
                            .map(|t| ThreadRow { key: t.tid.to_string(), name: t.name, weight: t.weight })
                            .collect(),
                    })
                    .collect(),
            })
        }
        Loaded::Instruments(p) => {
            let processes = p.processes();
            let name = process.map(str::to_string).or_else(|| processes.first().map(|p| p.name.clone()));
            let flame = p.flame(name.as_deref());
            Ok(CpuView {
                source: "instruments",
                unit: "ns",
                hotspots: hotspots(&flame, HOTSPOTS),
                flame,
                processes: processes
                    .into_iter()
                    .map(|p| ProcessRow {
                        key: p.name.clone(),
                        name: p.name,
                        samples: 0,
                        weight: p.weight,
                        threads: vec![],
                    })
                    .collect(),
            })
        }
    }
}

/// Opens a recording. A Perfetto trace needs the trace processor to be installed first.
#[tauri::command]
pub async fn trace_open(state: State<'_, AppState>, id: String) -> CmdResult<Opened> {
    let rec = library::get(&id).map_err(err)?;
    match rec.format {
        TraceFormat::Perfetto => {
            let processor = processor_for(&state, &rec).await?;
            let summary = summarize(&*processor).await.map_err(err)?;
            Ok(Opened::Perfetto { summary: Box::new(summary) })
        }
        _ => match loaded(&state, &rec).await? {
            Some(profile) => Ok(Opened::Cpu(Box::new(cpu_view(&profile, None, None)?))),
            None => Ok(Opened::External {
                reason:
                    "This recording has no CPU samples EmuLabZip can draw. Open it in Instruments to see all of it."
                        .into(),
            }),
        },
    }
}

/// The flame graph of another process or thread of the open recording.
#[tauri::command]
pub async fn trace_flame(
    state: State<'_, AppState>,
    id: String,
    process: Option<String>,
    thread: Option<String>,
) -> CmdResult<CpuView> {
    let rec = library::get(&id).map_err(err)?;
    let profile = loaded(&state, &rec).await?.ok_or("This recording has no CPU samples.")?;
    cpu_view(&profile, process.as_deref(), thread.as_deref())
}

/// The call tree as folded stacks, for `flamegraph.pl` and speedscope.
#[tauri::command]
pub async fn trace_folded(
    state: State<'_, AppState>,
    id: String,
    process: Option<String>,
    thread: Option<String>,
) -> CmdResult<String> {
    Ok(folded(&trace_flame(state, id, process, thread).await?.flame))
}

/// The trace processor that has `rec` loaded, starting it if another trace was open.
async fn processor_for(state: &AppState, rec: &Recording) -> CmdResult<Arc<Processor>> {
    let mut current = state.processor.lock().await;
    if let Some((id, p)) = &*current
        && *id == rec.id
    {
        return Ok(p.clone());
    }
    *current = None;
    let binary = processor::installed().ok_or("The Perfetto trace processor is not installed yet.")?;
    let started = Processor::start(&binary, &rec.path()).await.map_err(err)?;
    let started = Arc::new(started);
    *current = Some((rec.id.clone(), started.clone()));
    Ok(started)
}

/// Runs SQL on a Perfetto recording.
#[tauri::command]
pub async fn trace_query(state: State<'_, AppState>, id: String, sql: String) -> CmdResult<Rows> {
    let rec = library::get(&id).map_err(err)?;
    if rec.format != TraceFormat::Perfetto {
        return Err("SQL works on Perfetto recordings.".into());
    }
    processor_for(&state, &rec).await?.query(&sql).await.map_err(err)
}

/// Opens a Perfetto recording in the Perfetto UI in the browser, through a one-shot local server.
#[tauri::command]
pub async fn trace_open_perfetto_ui(app: AppHandle, id: String) -> CmdResult<()> {
    let rec = library::get(&id).map_err(err)?;
    if rec.format != TraceFormat::Perfetto {
        return Err("Only Perfetto recordings open in the Perfetto UI.".into());
    }
    let handoff = ui::serve(&rec.path()).await.map_err(err)?;
    app.opener().open_url(&handoff.url, None::<&str>).map_err(err)?;
    // Keep serving until the page has fetched the file, or the wait runs out.
    tauri::async_runtime::spawn(handoff.finished());
    Ok(())
}

/// Opens a recording in the app that understands it: Instruments for a `.trace`; for the rest,
/// shows the file in the file manager.
#[tauri::command]
pub fn trace_open_external(app: AppHandle, id: String) -> CmdResult<()> {
    let rec = library::get(&id).map_err(err)?;
    let path: PathBuf = rec.path();
    match rec.format {
        TraceFormat::InstrumentsBundle => app.opener().open_path(path.display().to_string(), None::<&str>).map_err(err),
        _ => app.opener().reveal_item_in_dir(path).map_err(err),
    }
}

/// Copies a recording out of the library. Returns the path, or `None` if the user cancelled.
#[tauri::command]
pub async fn trace_save(app: AppHandle, id: String) -> CmdResult<Option<String>> {
    let rec = library::get(&id).map_err(err)?;
    let from = rec.path();
    if from.is_dir() {
        return Err("This recording is a folder. Use “Show in folder” to copy it.".into());
    }
    let ext = from.extension().and_then(|e| e.to_str()).unwrap_or("bin").to_string();
    let label = match rec.format {
        TraceFormat::Perfetto => "Perfetto trace",
        TraceFormat::SimpleperfReport => "simpleperf report",
        TraceFormat::InstrumentsBundle => "Instruments trace",
    };
    let name = format!("{}.{ext}", rec.id);
    let Some(to) = ask_save_path(&app, &name, (label, &[&ext])).await else { return Ok(None) };
    tokio::fs::copy(&from, &to).await.map_err(err)?;
    Ok(Some(to.display().to_string()))
}
