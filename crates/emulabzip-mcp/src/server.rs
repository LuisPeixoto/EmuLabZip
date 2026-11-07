//! The tools. Each one is a thin wrapper over [`Lab`], so a device behaves the same here as in
//! the app and on the command line.

use std::time::Duration;

use base64::Engine;
use emulabzip_core::{
    Appearance, Control, Device, Grant, Lab, LogFilter, LogLevel, LogOptions, PerfOptions, PerfettoPreset, StartKind,
    StatusBar, Target, TraceFormat, TraceRequest,
};
use emulabzip_plugin as plugin;
use emulabzip_trace::flame::hotspots;
use emulabzip_trace::library::{self, Recording};
use emulabzip_trace::perfetto::processor::{self, Processor};
use emulabzip_trace::perfetto::summary::summarize;
use emulabzip_trace::{simpleperf, xctrace};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, InitializeResult, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;
use tokio::sync::watch;

use crate::args;

/// The longest text a tool returns. Assistants read what we send, so keep it focused.
const MAX_TEXT: usize = 60_000;
/// The largest file `read_file` returns.
const MAX_FILE: usize = 256 * 1024;
/// How long a log snapshot waits for the next line before it stops.
const LOG_IDLE: Duration = Duration::from_millis(700);
const LOG_LIMIT: Duration = Duration::from_secs(4);
/// The longest recording a tool starts, in seconds.
const MAX_RECORD_S: u32 = 120;

#[derive(Clone)]
pub struct EmuLabServer {
    lab: Lab,
}

impl EmuLabServer {
    pub fn new(lab: Lab) -> Self {
        EmuLabServer { lab }
    }

    async fn device(&self, query: &Option<String>) -> Result<(Device, Target), String> {
        let device = self.lab.resolve(query.as_deref()).await.map_err(|e| e.to_string())?;
        let target = device.target();
        Ok((device, target))
    }

    /// A device by name or id, running or not.
    async fn find_device(&self, query: &str, platform: Option<&str>) -> Result<Device, String> {
        let platform = match platform {
            None => None,
            Some("android") => Some(emulabzip_core::Platform::Android),
            Some("ios") => Some(emulabzip_core::Platform::Ios),
            Some(other) => return Err(format!("`{other}` is not a platform; use android or ios")),
        };
        let (devices, _) = self.lab.all_devices().await;
        let mut found: Vec<Device> = devices
            .into_iter()
            .filter(|d| platform.is_none_or(|p| d.platform == p))
            .filter(|d| d.id == query || d.name == query || d.id == format!("avd:{query}"))
            .collect();
        match found.len() {
            0 => Err(format!("no device is called {query}; list_all_devices shows them")),
            1 => Ok(found.remove(0)),
            _ => Err(format!("several devices are called {query}; give the platform or the id")),
        }
    }

    async fn target(&self, query: &Option<String>) -> Result<Target, String> {
        self.device(query).await.map(|(_, t)| t)
    }

    async fn flutter(&self, a: &args::Flutter) -> Result<emulabzip_frameworks::flutter::Flutter, String> {
        let uri = match &a.uri {
            None => None,
            Some(text) => Some(
                emulabzip_frameworks::flutter::VmUri::parse(text)
                    .ok_or_else(|| format!("`{text}` is not an address like http://127.0.0.1:41233/token=/"))?,
            ),
        };
        let target = self.target(&a.device).await?;
        emulabzip_frameworks::flutter::connect_app(&self.lab, &target, uri).await.map_err(|e| e.to_string())
    }

    async fn control(&self, query: &Option<String>, control: Control, done: &str) -> Result<String, String> {
        let (device, target) = self.device(query).await?;
        self.lab.control(&target, &control).await.map_err(|e| e.to_string())?;
        Ok(format!("{done} ({})", device.name))
    }
}

/// Cuts text that is too long, and says so.
fn cap(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n… (cut: ask for less, or narrow it down)");
    }
    text
}

fn json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map(|t| cap(t, MAX_TEXT)).map_err(|e| e.to_string())
}

fn reply(result: Result<String, String>) -> CallToolResult {
    match result {
        Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
    }
}

fn recording(id: &str) -> Result<Recording, String> {
    library::get(id).map_err(|e| e.to_string())
}

/// Where sessions recorded from the app and the command line are kept.
fn sessions_dir() -> std::path::PathBuf {
    emulabzip_core::data_dir().join("sessions")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionEntry {
    id: String,
    size: u64,
    modified_ms: u64,
    /// The recording did not finish: it is a folder, not a file.
    partial: bool,
}

fn sessions_list() -> Result<String, String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(sessions_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let (id, partial) = match name.strip_suffix(".ezsession.partial") {
                Some(id) => (id.to_string(), true),
                None => match name.strip_suffix(".ezsession") {
                    Some(id) => (id.to_string(), false),
                    None => continue,
                },
            };
            let meta = entry.metadata().ok();
            let modified_ms = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64);
            out.push(SessionEntry { id, size: meta.map_or(0, |m| m.len()), modified_ms, partial });
        }
    }
    out.sort_by_key(|e| std::cmp::Reverse(e.modified_ms));
    json(&out)
}

/// Where states saved from the app, the command line and here are kept.
fn states_dir() -> std::path::PathBuf {
    emulabzip_core::data_dir().join("states")
}

/// A saved state by name, never by path.
fn state_file(name: &str) -> Result<std::path::PathBuf, String> {
    if !valid_session_id(name) {
        return Err(format!("`{name}` is not the name of a saved state; list_states shows them"));
    }
    let path = states_dir().join(format!("{name}.{}", emulabzip_storage::snapshot::EXTENSION));
    if path.is_file() { Ok(path) } else { Err(format!("no saved state is called {name}; list_states shows them")) }
}

fn states_list() -> Result<String, String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(states_dir()) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            let Some(name) = file.strip_suffix(&format!(".{}", emulabzip_storage::snapshot::EXTENSION)) else {
                continue;
            };
            let modified_ms = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64);
            let meta = emulabzip_storage::snapshot::open(&entry.path()).ok().map(|s| s.meta);
            out.push((modified_ms, serde_json::json!({ "name": name, "modifiedMs": modified_ms, "state": meta })));
        }
    }
    out.sort_by_key(|(ms, _)| std::cmp::Reverse(*ms));
    json(&out.into_iter().map(|(_, v)| v).collect::<Vec<_>>())
}

/// A session name is a file name: nothing that could point outside the sessions folder.
fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !id.contains("..")
}

fn open_session(id: &str) -> Result<emulabzip_session::Session, String> {
    if !valid_session_id(id) {
        return Err(format!("`{id}` is not a session name: use one from list_sessions"));
    }
    let dir = sessions_dir();
    let path = [format!("{id}.ezsession"), format!("{id}.ezsession.partial")]
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
        .ok_or_else(|| format!("there is no session called `{id}`; list_sessions shows what exists"))?;
    emulabzip_session::Session::open(&path).map_err(|e| e.to_string())
}

fn session_summary(id: &str) -> Result<String, String> {
    json(&emulabzip_session::summarize(&open_session(id)?))
}

/// The most events `session_timeline` returns.
const MAX_TIMELINE_EVENTS: usize = 2000;

fn session_timeline(a: &args::SessionTimeline) -> Result<String, String> {
    // Everything that can be checked without opening the session is checked first.
    let lanes = a
        .lanes
        .iter()
        .flatten()
        .map(|l| {
            emulabzip_session::Lane::parse(l).ok_or_else(|| {
                format!("`{l}` is not a lane: use taps, logs, requests, frames, gc, crashes, screens or video")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let limit = a.limit.unwrap_or(200).clamp(1, MAX_TIMELINE_EVENTS);
    let session = open_session(&a.id)?;
    let timeline = emulabzip_session::timeline::build(&session);
    let events = timeline.only(&lanes);
    let total = events.len();
    json(&serde_json::json!({
        "startMs": timeline.start_ms,
        "durationMs": timeline.duration_ms,
        "clock": timeline.clock,
        "lanes": timeline.lanes,
        "events": events.into_iter().take(limit).collect::<Vec<_>>(),
        "eventsTotal": total,
        "truncated": total > limit,
    }))
}

/// What went into making a crash readable, next to the crash.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Readable {
    #[serde(flatten)]
    report: emulabzip_core::CrashReport,
    symbolication: Symbolication,
}

#[derive(Serialize)]
struct Symbolication {
    applied: Vec<String>,
    missing: Vec<String>,
}

fn readable(report: emulabzip_core::CrashReport) -> Readable {
    use emulabzip_symbols::store::Store;
    use emulabzip_symbols::symbolicate::{Source, symbolicate};
    let sources: Vec<Source> = match &report.summary.app {
        Some(app) => Store::default().find(app, report.app_version.as_deref()).iter().map(Source::from).collect(),
        None => Vec::new(),
    };
    let s = symbolicate(&report, &sources);
    Readable {
        symbolication: Symbolication { applied: s.applied.iter().map(|a| a.describe()).collect(), missing: s.missing },
        report: s.report,
    }
}

fn slug(text: &str) -> String {
    let s: String =
        text.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    s.chars().take(40).collect::<String>().trim_end_matches('-').to_string()
}

fn grant_of(action: &str) -> Result<Grant, String> {
    match action {
        "grant" => Ok(Grant::Grant),
        "revoke" => Ok(Grant::Revoke),
        "reset" => Ok(Grant::Reset),
        other => Err(format!("`{other}` is not an action: use grant, revoke or reset")),
    }
}

#[tool_router]
impl EmuLabServer {
    #[tool(
        description = "List the running Android emulators, iOS simulators and attached devices, with their ids, state and OS version."
    )]
    async fn list_devices(&self) -> CallToolResult {
        let (devices, errors) = self.lab.devices().await;
        let mut value = serde_json::json!({ "devices": devices });
        if !errors.is_empty() {
            value["problems"] = errors.iter().map(|(p, e)| format!("{p:?}: {e}")).collect::<Vec<_>>().into();
        }
        reply(json(&value))
    }

    #[tool(description = "Facts about a device: build, hardware, display, battery, memory and locale.")]
    async fn device_info(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(async { json(&self.lab.info(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?) }.await)
    }

    #[tool(
        description = "List the apps on a device, with their package names or bundle ids, versions and process ids when running."
    )]
    async fn list_apps(&self, Parameters(a): Parameters<args::Apps>) -> CallToolResult {
        reply(
            async {
                let apps = self.lab.apps(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
                let apps: Vec<_> = apps
                    .into_iter()
                    .filter(|app| a.all.unwrap_or(false) || app.user || app.pid.is_some())
                    .filter(|app| !a.running.unwrap_or(false) || app.pid.is_some())
                    .collect();
                json(&apps)
            }
            .await,
        )
    }

    #[tool(
        description = "Read recent log lines (logcat on Android, the unified log on iOS), filtered by app, level, tag or text. Returns a snapshot, newest last."
    )]
    async fn read_logs(&self, Parameters(a): Parameters<args::Logs>) -> CallToolResult {
        reply(self.read_logs_inner(a).await)
    }

    #[tool(description = "Take a screenshot of a device. Returns the image.")]
    async fn screenshot(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        match async { self.lab.screenshot(&self.target(&a.device).await?).await.map_err(|e| e.to_string()) }.await {
            Ok(png) => CallToolResult::success(vec![ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(png),
                "image/png",
            )]),
            Err(message) => reply(Err(message)),
        }
    }

    #[tool(
        description = "Measure CPU, memory, frame rate and battery for a few seconds: samples one second apart, device-wide or for one app."
    )]
    async fn perf_snapshot(&self, Parameters(a): Parameters<args::Perf>) -> CallToolResult {
        reply(self.perf_inner(a).await)
    }

    #[tool(description = "Where an app's memory goes: PSS by category on Android, footprint by VM region on iOS.")]
    async fn memory_report(&self, Parameters(a): Parameters<args::App>) -> CallToolResult {
        reply(
            async { json(&self.lab.memory(&self.target(&a.device).await?, &a.app).await.map_err(|e| e.to_string())?) }
                .await,
        )
    }

    #[tool(
        description = "Time how long an app takes to open (cold, warm or hot), several times. Returns the median and every run. It stops and opens the app on the device."
    )]
    async fn measure_startup(&self, Parameters(a): Parameters<args::Startup>) -> CallToolResult {
        reply(
            async {
                let kind = match a.kind.as_deref() {
                    None => StartKind::Cold,
                    Some(k) => StartKind::parse(k).ok_or("kind is cold, warm or hot")?,
                };
                let runs = a.runs.unwrap_or(3).clamp(1, 20);
                let target = self.target(&a.device).await?;
                json(&self.lab.startup(&target, &a.app, kind, runs).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Measure the energy an app uses over a few seconds (at most 120). Android reports the system's estimate, wake locks, jobs and network; iOS reports CPU time. Use the app meanwhile."
    )]
    async fn measure_energy(&self, Parameters(a): Parameters<args::Energy>) -> CallToolResult {
        reply(
            async {
                let seconds = a.seconds.unwrap_or(20).clamp(1, 120);
                let target = self.target(&a.device).await?;
                self.lab.energy_begin(&target, &a.app).await.map_err(|e| e.to_string())?;
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                json(&self.lab.energy_end(&target, &a.app, seconds).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Take a heap dump of a debuggable Android app and look for leaks: destroyed Activities and removed Fragments still alive, each with the path from a GC root that keeps it, plus the objects that keep the most memory alive."
    )]
    async fn analyze_heap(&self, Parameters(a): Parameters<args::Heap>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                let dump = self.lab.heap_dump(&target, &a.app).await.map_err(|e| e.to_string())?;
                if !dump.file_name.ends_with(".hprof") {
                    return Err("only Android .hprof dumps can be analyzed here; open a .memgraph in Xcode".to_string());
                }
                let options = emulabzip_heap::Options { top: 10, all_heaps: a.all_heaps.unwrap_or(false) };
                let bytes = dump.bytes;
                let analysis = tokio::task::spawn_blocking(move || emulabzip_heap::analyze(&bytes, &options))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                json(&analysis)
            }
            .await,
        )
    }

    #[tool(
        description = "Which UI frameworks an app is built with, found from its files: Flutter, React Native, Hermes, Jetpack Compose or SwiftUI."
    )]
    async fn detect_frameworks(&self, Parameters(a): Parameters<args::App>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                json(&self.lab.frameworks(&target, &a.app).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "The widget tree of a running Flutter debug or profile app, with the file and line each widget of the app's own code was made at."
    )]
    async fn flutter_widget_tree(&self, Parameters(a): Parameters<args::Flutter>) -> CallToolResult {
        reply(
            async {
                let flutter = self.flutter(&a).await?;
                let root = flutter.widget_tree().await.map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "creationTracked": flutter.creation_tracked(), "tree": root }))
            }
            .await,
        )
    }

    #[tool(
        description = "Build and raster times of the frames a Flutter app draws over a few seconds (at most 30), with the janky ones counted. Nothing is drawn while the app is idle, so use it meanwhile."
    )]
    async fn flutter_frames(&self, Parameters(a): Parameters<args::Flutter>) -> CallToolResult {
        reply(
            async {
                let seconds = a.seconds.unwrap_or(8).clamp(1, 30);
                let flutter = self.flutter(&a).await?;
                let mut stats = flutter.frames(seconds).await.map_err(|e| e.to_string())?;
                // The list of every frame is for a chart; an assistant needs the summary.
                stats.timeline.clear();
                json(&stats)
            }
            .await,
        )
    }

    #[tool(description = "The Dart heap of a running Flutter app and the classes that use the most memory.")]
    async fn flutter_memory(&self, Parameters(a): Parameters<args::Flutter>) -> CallToolResult {
        reply(async { json(&self.flutter(&a).await?.memory(15).await.map_err(|e| e.to_string())?) }.await)
    }

    #[tool(
        description = "List every emulator and simulator, running or not (a stopped one has state \"shutdown\"), with ids to start them with boot_device."
    )]
    async fn list_all_devices(&self) -> CallToolResult {
        let (devices, errors) = self.lab.all_devices().await;
        let mut value = serde_json::json!({ "devices": devices });
        if !errors.is_empty() {
            value["problems"] = errors.iter().map(|(p, e)| format!("{p:?}: {e}")).collect::<Vec<_>>().into();
        }
        reply(json(&value))
    }

    #[tool(
        description = "Start an emulator or simulator that is not running, and wait until it is up. It never wipes the device."
    )]
    async fn boot_device(&self, Parameters(a): Parameters<args::BootDevice>) -> CallToolResult {
        reply(
            async {
                let device = self.find_device(&a.device, a.platform.as_deref()).await?;
                let options =
                    emulabzip_core::BootOptions { wipe: false, cold: false, headless: a.headless.unwrap_or(false) };
                json(&self.lab.boot_device(&device.target(), &options).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(description = "Stop a running emulator or simulator. What is on it is kept.")]
    async fn shutdown_device(&self, Parameters(a): Parameters<args::ShutdownDevice>) -> CallToolResult {
        reply(
            async {
                let device = self.find_device(&a.device, a.platform.as_deref()).await?;
                self.lab.shutdown_device(&device.target()).await.map_err(|e| e.to_string())?;
                Ok(format!("{} is shut down.", device.name))
            }
            .await,
        )
    }

    #[tool(
        description = "Save an app's data (databases, preferences, files; not caches) to a state file kept by EmuLabZip, to compare later with diff_state. Stops the app. Putting a state back is not offered here."
    )]
    async fn save_state(&self, Parameters(a): Parameters<args::SaveState>) -> CallToolResult {
        reply(
            async {
                let name = match &a.name {
                    Some(n) if valid_session_id(n) => n.clone(),
                    Some(n) => return Err(format!("`{n}` is not a plain name")),
                    None => {
                        let ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis());
                        format!("{}-{ms}", slug(&a.app))
                    }
                };
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let dir = states_dir();
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let out = dir.join(format!("{name}.{}", emulabzip_storage::snapshot::EXTENSION));
                let target = self.target(&a.device).await?;
                let meta = emulabzip_storage::snapshot::save(&self.lab, &target, &a.app, &out, false)
                    .await
                    .map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "name": name, "file": out.display().to_string(), "state": meta }))
            }
            .await,
        )
    }

    #[tool(description = "The state files EmuLabZip has saved, newest first, by the names diff_state takes.")]
    async fn list_states(&self) -> CallToolResult {
        reply(states_list())
    }

    #[tool(
        description = "What changed in the structure of an app's data between two saved states, or between a saved state and the app now: tables, columns, indexes and preference keys added, removed or changed."
    )]
    async fn diff_state(&self, Parameters(a): Parameters<args::DiffState>) -> CallToolResult {
        reply(
            async {
                let before = state_file(&a.before)?;
                let changes = match &a.after {
                    Some(after) => emulabzip_storage::snapshot::compare(&before, &state_file(after)?),
                    None => {
                        let app = emulabzip_storage::snapshot::open(&before).map_err(|e| e.to_string())?.meta.app;
                        let target = self.target(&a.device).await?;
                        let now = std::env::temp_dir().join(format!(
                            "emulabzip-mcp-now-{}.{}",
                            std::process::id(),
                            emulabzip_storage::snapshot::EXTENSION
                        ));
                        let _ = std::fs::remove_file(&now);
                        let saved = emulabzip_storage::snapshot::save(&self.lab, &target, &app, &now, false).await;
                        let result = saved.and_then(|_| emulabzip_storage::snapshot::compare(&before, &now));
                        let _ = std::fs::remove_file(&now);
                        result
                    }
                }
                .map_err(|e| e.to_string())?;
                json(&serde_json::json!({
                    "changes": changes.iter().map(|c| c.line()).collect::<Vec<_>>(),
                    "detail": changes,
                }))
            }
            .await,
        )
    }

    #[tool(
        description = "The snapshots of a running Android emulator (its whole state, saved by the emulator itself)."
    )]
    async fn list_emulator_snapshots(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                json(&self.lab.emulator_snapshots(&target).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Save the running Android emulator's whole state as a snapshot with this name. Loading or deleting snapshots is not offered here."
    )]
    async fn save_emulator_snapshot(&self, Parameters(a): Parameters<args::EmulatorSnapshot>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                self.lab
                    .emulator_snapshot(&target, emulabzip_core::SnapshotAction::Save, &a.name, false)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("Saved snapshot {}.", a.name))
            }
            .await,
        )
    }

    #[tool(
        description = "The state of a React Native development app: Redux stores, React Query caches, React contexts and stores registered in globalThis.__EMULABZIP_STORES__, read through Metro (must be running). Release builds have none of this."
    )]
    async fn react_state(&self, Parameters(a): Parameters<args::ReactNative>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&a.app, port).await.map_err(|e| e.to_string())?;
                Ok(cap(json(&rn.state().await.map_err(|e| e.to_string())?)?, 200_000))
            }
            .await,
        )
    }

    #[tool(
        description = "The React component tree of a React Native development app, with props, read through Metro (must be running). Metro allows one debugger at a time, so this can close React Native DevTools."
    )]
    async fn react_tree(&self, Parameters(a): Parameters<args::ReactNative>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&a.app, port).await.map_err(|e| e.to_string())?;
                json(&rn.tree().await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "List an app's storage roots, or a folder in one. Android needs a debuggable app for the private `data` root."
    )]
    async fn list_files(&self, Parameters(a): Parameters<args::Files>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                match a.root.as_deref() {
                    None if a.path.is_none() => {
                        json(&self.lab.storage_roots(&target, &a.app).await.map_err(|e| e.to_string())?)
                    }
                    root => {
                        let entries = self
                            .lab
                            .list_files(&target, &a.app, root.unwrap_or("data"), a.path.as_deref().unwrap_or(""))
                            .await;
                        json(&entries.map_err(|e| e.to_string())?)
                    }
                }
            }
            .await,
        )
    }

    #[tool(description = "Read one file of an app: text as it is, binary files as base64. Files over 256 KB are cut.")]
    async fn read_file(&self, Parameters(a): Parameters<args::Files>) -> CallToolResult {
        reply(self.read_file_inner(a).await)
    }

    #[tool(
        description = "The views on a device's screen as a tree: class, text, label, id, size and position, and whether each can be tapped. Node numbers can be quoted back to the user. iOS needs Simulator.app open and the Accessibility permission."
    )]
    async fn get_layout(&self, Parameters(a): Parameters<args::Layout>) -> CallToolResult {
        reply(self.layout_inner(a).await)
    }

    #[tool(
        description = "Check the screen in front for accessibility problems: tappable views without a name, touch targets under 48 dp (Android) or 44 pt (iOS), text with too little contrast, and neighbours that sound the same. Each finding names the view, the WCAG criterion and a fix."
    )]
    async fn audit_accessibility(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let snapshot = self.lab.layout(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
                json(&emulabzip_core::a11y::audit(&snapshot))
            }
            .await,
        )
    }

    #[tool(
        description = "List the sessions recorded with EmuLabZip (`emulabzip session record`, `emulabzip ci`, or the app), newest first."
    )]
    async fn list_sessions(&self) -> CallToolResult {
        reply(sessions_list())
    }

    #[tool(
        description = "Summarize a recorded session: duration, log errors and the most repeated ones, crashes, requests and the slowest, CPU and memory peaks."
    )]
    async fn session_summary(&self, Parameters(a): Parameters<args::Session>) -> CallToolResult {
        reply(session_summary(&a.id))
    }

    #[tool(
        description = "Everything a recorded session captured on one time axis, oldest first: touches, log warnings and errors, requests, slow frames (a stutter is one event), garbage collections, crashes, screenshots and pieces of screen video. Times are milliseconds from the start of the recording. Pick lanes to keep it short."
    )]
    async fn session_timeline(&self, Parameters(a): Parameters<args::SessionTimeline>) -> CallToolResult {
        reply(session_timeline(&a))
    }

    #[tool(description = "List the crashes, native crashes and ANRs a device has recorded, newest first.")]
    async fn list_crashes(&self, Parameters(a): Parameters<args::Crashes>) -> CallToolResult {
        reply(
            async {
                let kind = match a.kind.as_deref() {
                    None => None,
                    Some("crash") => Some(emulabzip_core::CrashKind::Crash),
                    Some("native") => Some(emulabzip_core::CrashKind::NativeCrash),
                    Some("anr") => Some(emulabzip_core::CrashKind::Anr),
                    Some(other) => return Err(format!("`{other}` is not a kind: use crash, native or anr")),
                };
                let list = self
                    .lab
                    .crashes(&self.target(&a.device).await?, a.app.as_deref())
                    .await
                    .map_err(|e| e.to_string())?;
                json(&list.into_iter().filter(|c| kind.is_none_or(|k| c.kind == k)).collect::<Vec<_>>())
            }
            .await,
        )
    }

    #[tool(
        description = "One crash with its exception or signal, the stack of the thread that crashed (and others), and hints. Names scrambled by R8 or ProGuard, JavaScript positions and native iOS addresses are made readable with the mapping.txt, source maps and dSYMs stored for the app and version (`symbolication` says which were used, and what is missing)."
    )]
    async fn get_crash(&self, Parameters(a): Parameters<args::Crash>) -> CallToolResult {
        reply(
            async {
                let mut report =
                    self.lab.crash(&self.target(&a.device).await?, &a.id).await.map_err(|e| e.to_string())?;
                if !a.raw.unwrap_or(false) {
                    report.raw.clear();
                }
                if a.original.unwrap_or(false) { json(&report) } else { json(&readable(report)) }
            }
            .await,
        )
    }

    #[tool(
        description = "Write a bug report as one self-contained HTML file in EmuLabZip's reports folder: a screenshot, the device, the warnings and errors of the log, the failed requests and the readable crashes, from the device as it is now or from a recorded session. Returns the file's path, a Markdown summary for an issue, and optionally a link that opens a pre-filled issue. Nothing is sent anywhere."
    )]
    async fn create_bug_report(&self, Parameters(a): Parameters<args::BugReport>) -> CallToolResult {
        reply(self.bug_report(a).await)
    }

    #[tool(
        description = "List the traces recorded with EmuLabZip (Perfetto system traces, simpleperf CPU profiles, Instruments recordings)."
    )]
    async fn list_traces(&self) -> CallToolResult {
        reply(json(&library::list()))
    }

    #[tool(
        description = "Summarize a recording: for a Perfetto trace, CPU by process and thread, frame times and jank, memory and the busiest slices; for a CPU profile, the hottest functions."
    )]
    async fn trace_summary(&self, Parameters(a): Parameters<args::Recording>) -> CallToolResult {
        reply(self.trace_summary_inner(&a.id).await)
    }

    #[tool(
        description = "Run PerfettoSQL on a Perfetto recording (tables such as slice, sched, thread, process, counter). Returns up to 5000 rows."
    )]
    async fn trace_query(&self, Parameters(a): Parameters<args::Query>) -> CallToolResult {
        reply(
            async {
                let rec = recording(&a.id)?;
                if rec.format != TraceFormat::Perfetto {
                    return Err("SQL works on Perfetto recordings.".into());
                }
                let processor = open_processor(&rec).await?;
                json(&processor.query(&a.sql).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Record a trace on a device and keep it in the library: a Perfetto system trace (Android), a simpleperf CPU profile (Android) or an Instruments template (iOS Simulator). Takes as long as the duration."
    )]
    async fn record_trace(&self, Parameters(a): Parameters<args::RecordTrace>) -> CallToolResult {
        reply(self.record_inner(a).await)
    }

    #[tool(
        description = "List the plugins installed on this computer: programs that collect their own data from a device. Only enabled plugins can be run."
    )]
    async fn list_plugins(&self) -> CallToolResult {
        let (plugins, problems) = plugin::discover();
        let approvals = plugin::Approvals::load();
        let listed: Vec<_> = plugins
            .iter()
            .map(|p| serde_json::json!({ "name": p.manifest.name, "description": p.manifest.description, "mode": p.manifest.mode, "platforms": p.manifest.platforms, "enabled": approvals.is_approved(p) }))
            .collect();
        let problems: Vec<String> = problems.iter().map(|p| format!("{}: {}", p.dir.display(), p.message)).collect();
        reply(json(&serde_json::json!({ "plugins": listed, "problems": problems })))
    }

    #[tool(
        description = "Run an enabled plugin on a device and return what it reported: metrics, tables, text and log lines. Plugins that the user has not enabled are refused."
    )]
    async fn run_plugin(&self, Parameters(a): Parameters<args::RunPlugin>) -> CallToolResult {
        reply(self.run_plugin_inner(a).await)
    }

    #[tool(
        description = "What a device supports changing, its appearance, and the permissions of an app when `app` is given."
    )]
    async fn control_info(&self, Parameters(a): Parameters<args::App>) -> CallToolResult {
        reply(
            async {
                json(
                    &self
                        .lab
                        .controls(&self.target(&a.device).await?, Some(&a.app))
                        .await
                        .map_err(|e| e.to_string())?,
                )
            }
            .await,
        )
    }

    #[tool(
        description = "Change the device's location (emulators and simulators). Without lat and lon, go back to the default location."
    )]
    async fn control_location(&self, Parameters(a): Parameters<args::Location>) -> CallToolResult {
        reply(
            async {
                match (a.lat, a.lon) {
                    (Some(lat), Some(lon)) => {
                        self.control(&a.device, Control::Location { lat, lon }, "Location set").await
                    }
                    (None, None) => self.control(&a.device, Control::ClearLocation, "Location reset").await,
                    _ => Err("Give both lat and lon, or neither".into()),
                }
            }
            .await,
        )
    }

    #[tool(description = "Switch the device between light and dark mode.")]
    async fn control_appearance(&self, Parameters(a): Parameters<args::Appearance>) -> CallToolResult {
        reply(
            async {
                let appearance = match a.mode.as_str() {
                    "light" => Appearance::Light,
                    "dark" => Appearance::Dark,
                    other => return Err(format!("`{other}` is not a mode: use light or dark")),
                };
                self.control(&a.device, Control::Appearance { appearance }, &format!("Appearance set to {}", a.mode))
                    .await
            }
            .await,
        )
    }

    #[tool(
        description = "Show an app in another language or a pseudo-locale (en-XA for long text, ar-XB for right to left), to find text that does not fit; or put the language back. Android changes one app (Android 13+); iOS changes the whole simulator and apps pick it up when they start."
    )]
    async fn control_locale(&self, Parameters(a): Parameters<args::Locale>) -> CallToolResult {
        reply(
            async {
                match a.locale {
                    Some(locale) => {
                        let said = format!("Language set to {locale}");
                        self.control(&a.device, Control::Locale { locale, app: a.app }, &said).await
                    }
                    None => {
                        self.control(&a.device, Control::ResetLocale { app: a.app }, "Language back to normal").await
                    }
                }
            }
            .await,
        )
    }

    #[tool(
        description = "Make the device's text bigger or smaller, as a percentage of normal (100 is normal, 130 and 200 are common accessibility settings), to find text that does not fit. Set 100 to put it back."
    )]
    async fn control_font_scale(&self, Parameters(a): Parameters<args::FontScale>) -> CallToolResult {
        reply(
            self.control(
                &a.device,
                Control::FontScale { percent: a.percent },
                &format!("Text size set to {}%", a.percent),
            )
            .await,
        )
    }

    #[tool(
        description = "Change the screen size (pixels) and density (dpi) of an Android device, or put them back with reset."
    )]
    async fn control_display_size(&self, Parameters(a): Parameters<args::DisplaySize>) -> CallToolResult {
        reply(
            async {
                if a.reset.unwrap_or(false) {
                    return self.control(&a.device, Control::ResetDisplay, "Display back to normal").await;
                }
                self.control(
                    &a.device,
                    Control::Display { width: a.width, height: a.height, density: a.density },
                    "Display changed",
                )
                .await
            }
            .await,
        )
    }

    #[tool(
        description = "Show a clean status bar (fixed time, battery and signal), for screenshots; or clear the override."
    )]
    async fn control_status_bar(&self, Parameters(a): Parameters<args::StatusBar>) -> CallToolResult {
        reply(
            async {
                if a.clear.unwrap_or(false) {
                    return self.control(&a.device, Control::ClearStatusBar, "Status bar back to normal").await;
                }
                let status_bar = StatusBar { time: a.time, battery: a.battery, wifi: a.wifi, cellular: a.cellular };
                self.control(&a.device, Control::StatusBar { status_bar }, "Status bar overridden").await
            }
            .await,
        )
    }

    #[tool(
        description = "Grant, revoke or reset one permission of an app (an Android permission name, or an iOS service such as photos)."
    )]
    async fn control_permission(&self, Parameters(a): Parameters<args::Permission>) -> CallToolResult {
        reply(
            async {
                let grant = grant_of(&a.action)?;
                let done = format!("{}: {}", a.permission, a.action);
                self.control(&a.device, Control::Permission { app: a.app, permission: a.permission, grant }, &done)
                    .await
            }
            .await,
        )
    }

    #[tool(description = "Post a notification (Android) or send a simulated push to an app (iOS).")]
    async fn control_notification(&self, Parameters(a): Parameters<args::Notification>) -> CallToolResult {
        reply(
            self.control(
                &a.device,
                Control::Notification {
                    app: a.app.unwrap_or_else(|| "com.android.shell".into()),
                    title: a.title,
                    body: a.body,
                    payload: a.payload,
                },
                "Notification sent",
            )
            .await,
        )
    }

    #[tool(
        description = "Open a web address or an app's own link on the device. Links that read local files or run script are refused."
    )]
    async fn open_url(&self, Parameters(a): Parameters<args::OpenUrl>) -> CallToolResult {
        reply(self.control(&a.device, Control::OpenUrl { url: a.url }, "Link opened").await)
    }

    #[tool(
        description = "Pretend the battery is at a level, charging or not (Android); or go back to the real battery."
    )]
    async fn control_battery(&self, Parameters(a): Parameters<args::Battery>) -> CallToolResult {
        reply(
            async {
                if a.reset.unwrap_or(false) {
                    return self.control(&a.device, Control::ResetBattery, "Battery back to normal").await;
                }
                let level = a.level.ok_or("Give a level, or set reset to true")?;
                self.control(
                    &a.device,
                    Control::Battery { level, charging: a.charging.unwrap_or(false) },
                    "Battery overridden",
                )
                .await
            }
            .await,
        )
    }
}

impl EmuLabServer {
    async fn layout_inner(&self, a: args::Layout) -> Result<String, String> {
        let snapshot = self.lab.layout(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
        let depth = a.depth.unwrap_or(10).clamp(1, 40);
        let unit = if snapshot.platform == emulabzip_core::Platform::Ios { "pt" } else { "px" };
        let mut out = format!(
            "{} views on a {}x{} {unit} screen (density {}). Format: #node class “text/label” @id size at x,y [flags]\n",
            snapshot.root.count(),
            snapshot.screen.width,
            snapshot.screen.height,
            snapshot.screen.density
        );
        fn walk(n: &emulabzip_core::UiNode, level: usize, max: usize, out: &mut String) {
            let name = n.label.as_deref().or(n.text.as_deref()).filter(|t| !t.trim().is_empty());
            let flags: Vec<&str> = [
                n.clickable.then_some("tap"),
                n.editable.then_some("edit"),
                n.scrollable.then_some("scroll"),
                (!n.enabled).then_some("disabled"),
            ]
            .into_iter()
            .flatten()
            .collect();
            out.push_str(&format!(
                "{}#{} {}{}{} {}x{} at {},{}{}\n",
                "  ".repeat(level),
                n.id,
                n.class.rsplit('.').next().unwrap_or(&n.class),
                name.map(|t| format!(" “{}”", t.chars().take(60).collect::<String>())).unwrap_or_default(),
                n.resource_id
                    .as_deref()
                    .map(|i| format!(" @{}", i.rsplit(['/', ':']).next().unwrap_or(i)))
                    .unwrap_or_default(),
                n.bounds.width,
                n.bounds.height,
                n.bounds.x,
                n.bounds.y,
                if flags.is_empty() { String::new() } else { format!(" [{}]", flags.join(",")) }
            ));
            if level + 1 < max {
                for c in &n.children {
                    walk(c, level + 1, max, out);
                }
            } else if !n.children.is_empty() {
                out.push_str(&format!(
                    "{}… {} more inside\n",
                    "  ".repeat(level + 1),
                    n.children.iter().map(|c| c.count()).sum::<usize>()
                ));
            }
        }
        walk(&snapshot.root, 0, depth, &mut out);
        Ok(cap(out, MAX_TEXT))
    }

    async fn read_logs_inner(&self, a: args::Logs) -> Result<String, String> {
        let target = self.target(&a.device).await?;
        let level = a
            .level
            .as_deref()
            .map(str::parse::<LogLevel>)
            .transpose()
            .map_err(|_| "level must be verbose, debug, info, warn, error or fatal".to_string())?;
        let mut filter = LogFilter { min_level: level, pids: Vec::new(), tag: a.tag, text: a.grep, regex: false };
        if let Some(app) = &a.app {
            let apps = self.lab.apps(&target).await.map_err(|e| e.to_string())?;
            filter.pids = apps.iter().filter(|x| &x.id == app).filter_map(|x| x.pid).collect();
            if filter.pids.is_empty() {
                return Err(format!("{app} is not running, so it has no log lines to read"));
            }
        }
        let matcher = filter.compile().map_err(|e| e.to_string())?;
        let want = a.lines.unwrap_or(200).clamp(1, 1000);
        let mut stream =
            self.lab.logs(&target, LogOptions { tail: 1000, verbose: true }).await.map_err(|e| e.to_string())?;
        let mut lines: Vec<String> = Vec::new();
        let deadline = tokio::time::Instant::now() + LOG_LIMIT;
        // The device replays its recent lines at once; stop when they dry up.
        while let Ok(Some(entry)) =
            tokio::time::timeout_at(deadline.min(tokio::time::Instant::now() + LOG_IDLE), stream.next()).await
        {
            if matcher.matches(&entry) {
                lines.push(entry.to_line());
                if lines.len() > want {
                    lines.remove(0);
                }
            }
        }
        if lines.is_empty() {
            return Ok("No log lines matched.".into());
        }
        Ok(cap(lines.join("\n"), MAX_TEXT))
    }

    async fn perf_inner(&self, a: args::Perf) -> Result<String, String> {
        let target = self.target(&a.device).await?;
        let want = a.samples.unwrap_or(3).clamp(1, 10);
        let mut stream =
            self.lab.perf(&target, PerfOptions { app: a.app, interval_ms: 1000 }).await.map_err(|e| e.to_string())?;
        let mut samples = Vec::new();
        while samples.len() < want as usize {
            match tokio::time::timeout(Duration::from_secs(8), stream.next()).await {
                Ok(Some(s)) => samples.push(s),
                _ => break,
            }
        }
        json(&samples)
    }

    async fn read_file_inner(&self, a: args::Files) -> Result<String, String> {
        let target = self.target(&a.device).await?;
        let path = a.path.as_deref().filter(|p| !p.is_empty()).ok_or("Give the path of a file")?;
        let bytes = self
            .lab
            .read_file(&target, &a.app, a.root.as_deref().unwrap_or("data"), path)
            .await
            .map_err(|e| e.to_string())?;
        let total = bytes.len();
        let shown = &bytes[..total.min(MAX_FILE)];
        let value = match std::str::from_utf8(shown) {
            Ok(text) => {
                serde_json::json!({ "size": total, "encoding": "utf8", "content": text, "truncated": total > MAX_FILE })
            }
            Err(_) => serde_json::json!({
                "size": total,
                "encoding": "base64",
                "content": base64::engine::general_purpose::STANDARD.encode(shown),
                "truncated": total > MAX_FILE,
            }),
        };
        json(&value)
    }

    async fn run_plugin_inner(&self, a: args::RunPlugin) -> Result<String, String> {
        let (plugins, _) = plugin::discover();
        let found = plugins
            .into_iter()
            .find(|p| p.manifest.name == a.name)
            .ok_or_else(|| format!("There is no plugin called {}", a.name))?;
        if !plugin::Approvals::load().is_approved(&found) {
            return Err(format!(
                "{} is not enabled. Ask the user to enable it (`emulabzip plugin enable {}`, or the Plugins view in the app): it runs a program on their computer.",
                a.name, a.name
            ));
        }
        let (device, target) = self.device(&a.device).await?;
        if !found.manifest.supports(target.platform) {
            return Err(format!("{} does not support {}", a.name, target.platform.label()));
        }
        let ctx = plugin::Context { device: target.id.clone(), platform: target.platform, app: a.app };
        let (stop, stopped) = watch::channel(false);
        let listen = match found.manifest.mode {
            plugin::Mode::Stream => Some(Duration::from_secs(a.seconds.unwrap_or(5).clamp(1, 60))),
            plugin::Mode::Once => None,
        };
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let running = plugin::run(&found, &ctx, stopped, move |e| {
            if let Ok(mut list) = sink.lock() {
                list.push(e);
            }
        });
        tokio::pin!(running);
        let stopper = async {
            match listen {
                Some(d) => tokio::time::sleep(d).await,
                None => std::future::pending().await,
            }
        };
        tokio::pin!(stopper);
        let mut sent_stop = false;
        let outcome = loop {
            tokio::select! {
                outcome = &mut running => break outcome.map_err(|e| e.to_string())?,
                _ = &mut stopper, if !sent_stop => {
                    sent_stop = true;
                    let _ = stop.send(true);
                }
            }
        };
        let events = events.lock().map(|l| l.clone()).unwrap_or_default();
        json(
            &serde_json::json!({ "plugin": a.name, "device": device.name, "outcome": format!("{outcome:?}"), "events": events }),
        )
    }

    async fn trace_summary_inner(&self, id: &str) -> Result<String, String> {
        let rec = recording(id)?;
        match rec.format {
            TraceFormat::Perfetto => {
                let processor = open_processor(&rec).await?;
                json(&summarize(&processor).await.map_err(|e| e.to_string())?)
            }
            TraceFormat::SimpleperfReport => {
                let profile = simpleperf::parse(&std::fs::read(rec.path()).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
                let processes = profile.processes(0, false);
                let busiest = processes
                    .iter()
                    .find(|p| p.name != "simpleperf")
                    .or(processes.first())
                    .ok_or("the profile has no samples")?;
                let flame = profile.flame(&simpleperf::Filter { pid: Some(busiest.pid), ..Default::default() });
                json(
                    &serde_json::json!({ "process": busiest.name, "unit": profile.unit(0), "hotFunctions": hotspots(&flame, 25) }),
                )
            }
            TraceFormat::InstrumentsBundle => {
                let xml = rec
                    .extra_paths()
                    .into_iter()
                    .find(|p| p.extension().is_some_and(|e| e == "xml"))
                    .ok_or("This recording has no CPU samples to summarize. Open it in Instruments.")?;
                let profile = xctrace::parse(&std::fs::read_to_string(xml).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
                let busiest = profile.processes().into_iter().next().ok_or("the recording has no samples")?;
                json(
                    &serde_json::json!({ "process": busiest.name, "hotFunctions": hotspots(&profile.flame(Some(&busiest.name)), 25) }),
                )
            }
        }
    }

    async fn record_inner(&self, a: args::RecordTrace) -> Result<String, String> {
        let (device, target) = self.device(&a.device).await?;
        let duration_s = a.duration_s.unwrap_or(10).clamp(1, MAX_RECORD_S);
        let request = match a.kind.as_str() {
            "perfetto" => {
                let preset = match a.preset.as_deref().unwrap_or("cpu") {
                    "cpu" => PerfettoPreset::Cpu,
                    "frames" => PerfettoPreset::Frames,
                    "memory" => PerfettoPreset::Memory,
                    other => return Err(format!("`{other}` is not a preset: use cpu, frames or memory")),
                };
                TraceRequest::Perfetto { preset, config: None, duration_s, app: a.app }
            }
            "cpu" => TraceRequest::CpuProfile { app: a.app, duration_s, system_wide: a.system_wide.unwrap_or(false) },
            "instruments" => TraceRequest::Instruments {
                template: a.template.unwrap_or_else(|| "Time Profiler".into()),
                app: a.app,
                duration_s,
            },
            other => return Err(format!("`{other}` is not a kind: use perfetto, cpu or instruments")),
        };
        let id = library::new_id();
        let dir = library::folder(&id).map_err(|e| e.to_string())?;
        let (_keep, stop) = watch::channel(false);
        let saved = match self.lab.record_trace(&target, &request, &dir, stop).await {
            Ok(file) => Recording::save(&id, target.platform, &device.name, &request, &file).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        if saved.is_err() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        json(&saved?)
    }
}

/// Starts the trace processor on a recording. It is not downloaded here: that is the user's call.
async fn open_processor(rec: &Recording) -> Result<Processor, String> {
    let binary = processor::installed().ok_or("The Perfetto trace processor is not installed. Run `emulabzip trace summary <file>` once, or open a Perfetto trace in the app, to download it.")?;
    Processor::start(&binary, &rec.path()).await.map_err(|e| e.to_string())
}

#[tool_handler]
impl ServerHandler for EmuLabServer {
    fn get_info(&self) -> ServerConfig {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "EmuLabZip inspects and controls Android emulators and iOS simulators. Start with list_devices. \
             Every tool takes an optional `device`; it can be left out when only one device is running. \
             Tools that change the device (control_*, open_url, record_trace, boot_device, shutdown_device, save_state) act on it for real. \
             Restoring state, wiping, deleting and editing data are not offered to assistants.",
        )
    }
}

impl EmuLabServer {
    async fn bug_report(&self, a: args::BugReport) -> Result<String, String> {
        use emulabzip_report::issue::{Tracker, issue_url};
        use emulabzip_report::{BugReport, Options, html, markdown};
        use emulabzip_symbols::store::Store;

        // What can be checked without a device is checked first.
        let tracker = a.issue.as_deref().map(Tracker::parse).transpose().map_err(|e| e.to_string())?;
        if a.session.is_some() && a.app.is_some() {
            return Err("`app` is for a report of the device as it is now; a session already knows its app".into());
        }
        let mut options = Options::everything();
        options.title = a.title.clone().unwrap_or_default();
        options.description = a.description.clone().unwrap_or_default();
        let store = Store::default();
        let mut report = match &a.session {
            Some(id) => {
                if !valid_session_id(id) {
                    return Err(format!("`{id}` is not a session name: use one from list_sessions"));
                }
                BugReport::from_session(&open_session(id)?, &options, &store)
            }
            None => {
                let target = self.target(&a.device).await?;
                BugReport::collect(&self.lab, &target, a.app.as_deref(), &[], &options, &store).await
            }
        };
        if report.title.trim().is_empty() {
            report.title = match (report.crashes.first(), &report.app) {
                (Some(c), _) => c.title.clone(),
                (None, Some(app)) => format!("Bug in {app}"),
                _ => "Bug report".into(),
            };
        }
        let dir = emulabzip_core::data_dir().join("reports");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stem = format!("{}-{}", report.created_ms, slug(&report.title));
        // A clip too big to be part of the file goes next to it, so the two get a folder of their own.
        let (path, video) = match report.video_sidecar() {
            Some(clip) => {
                let folder = dir.join(&stem);
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let video = folder.join(emulabzip_report::VIDEO_FILE);
                std::fs::write(&video, clip).map_err(|e| e.to_string())?;
                (folder.join("emulabzip-report.html"), Some(video))
            }
            None => (dir.join(format!("{stem}.html")), None),
        };
        std::fs::write(&path, html::html(&report)).map_err(|e| e.to_string())?;
        let summary = markdown::markdown(&report);
        json(&serde_json::json!({
            "path": path,
            "video": video,
            "hasVideo": report.video.is_some(),
            "title": report.title,
            "crashes": report.crashes.len(),
            "failedRequests": report.requests.len(),
            "notes": report.notes,
            "issueUrl": tracker.map(|t| issue_url(&t, &report.title, &summary)),
            "markdown": summary,
        }))
    }
}
