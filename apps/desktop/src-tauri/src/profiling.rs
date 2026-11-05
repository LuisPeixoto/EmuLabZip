//! Commands for the Performance and Memory tools.

use emulabzip_core::memory::diff;
use emulabzip_core::{
    CategoryDelta, EnergyReport, MemoryReport, PerfOptions, PerfSample, StartKind, StartupReport, Target,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::commands::{CmdResult, ask_open_path, ask_save_path, err};
use crate::state::AppState;

/// Starts sampling. Each sample is sent on `on_sample` as soon as it is taken.
#[tauri::command]
pub async fn start_perf(
    state: State<'_, AppState>,
    target: Target,
    options: PerfOptions,
    on_sample: Channel<PerfSample>,
) -> CmdResult<u64> {
    let mut stream = state.lab.perf(&target, options).await.map_err(err)?;
    let task = tauri::async_runtime::spawn(async move {
        while let Some(sample) = stream.next().await {
            if on_sample.send(sample).is_err() {
                break;
            }
        }
    });
    Ok(state.add_session(task))
}

#[tauri::command]
pub async fn memory_report(state: State<'_, AppState>, target: Target, app: String) -> CmdResult<MemoryReport> {
    state.lab.memory(&target, &app).await.map_err(err)
}

#[tauri::command]
pub fn memory_diff(before: MemoryReport, after: MemoryReport) -> Vec<CategoryDelta> {
    diff(&before, &after)
}

/// Captures a heap dump and asks where to save it. Returns the path, or `None` if cancelled.
#[tauri::command]
pub async fn save_heap_dump(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Target,
    package: String,
) -> CmdResult<Option<String>> {
    let dump = state.lab.heap_dump(&target, &package).await.map_err(err)?;
    let (label, ext) =
        if dump.file_name.ends_with(".memgraph") { ("Memory graph", "memgraph") } else { ("Heap dump", "hprof") };
    let Some(path) = ask_save_path(&app, &dump.file_name, (label, &[ext])).await else { return Ok(None) };
    tokio::fs::write(&path, dump.bytes).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}

/// Times how long the app takes to open, several times.
#[tauri::command]
pub async fn startup_measure(
    state: State<'_, AppState>,
    target: Target,
    app: String,
    kind: StartKind,
    runs: u32,
) -> CmdResult<StartupReport> {
    state.lab.startup(&target, &app, kind, runs).await.map_err(err)
}

/// Starts counting the energy the app uses.
#[tauri::command]
pub async fn energy_begin(state: State<'_, AppState>, target: Target, app: String) -> CmdResult<()> {
    state.lab.energy_begin(&target, &app).await.map_err(err)
}

/// Reads what the app used since `energy_begin`, and puts the device's battery back.
#[tauri::command]
pub async fn energy_end(
    state: State<'_, AppState>,
    target: Target,
    app: String,
    seconds: u64,
) -> CmdResult<EnergyReport> {
    state.lab.energy_end(&target, &app, seconds).await.map_err(err)
}

/// Asks for a heap dump on disk. Returns its path, or `None` if cancelled.
#[tauri::command]
pub async fn pick_heap_dump(app: AppHandle) -> CmdResult<Option<String>> {
    Ok(ask_open_path(&app, ("Heap dump", &["hprof"])).await.map(|p| p.display().to_string()))
}

/// Looks for leaks in an `.hprof` file. Big dumps take seconds, so it runs off the UI thread.
#[tauri::command]
pub async fn heap_analyze(path: String, all_heaps: bool) -> CmdResult<emulabzip_heap::Analysis> {
    let data = tokio::fs::read(&path).await.map_err(err)?;
    let options = emulabzip_heap::Options { top: 25, all_heaps };
    tokio::task::spawn_blocking(move || emulabzip_heap::analyze(&data, &options)).await.map_err(err)?.map_err(err)
}
