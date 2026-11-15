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
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, InitializeResult, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::Serialize;
use tokio::sync::watch;

use crate::args;

pub(crate) mod tools;

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
    hub: std::sync::Arc<crate::hub::Hub>,
    pub(crate) policy: crate::policy::Policy,
    tool_router: ToolRouter<EmuLabServer>,
}

impl EmuLabServer {
    pub fn new(lab: Lab) -> Self {
        Self::with_policy(lab, crate::policy::Policy::default())
    }

    /// The server, offering the tools that change or destroy data only when the policy allows them.
    pub fn with_policy(lab: Lab, policy: crate::policy::Policy) -> Self {
        let mut tool_router = Self::devices_router()
            + Self::profiling_router()
            + Self::frameworks_router()
            + Self::network_router()
            + Self::watches_router()
            + Self::storage_router()
            + Self::storage_data_router()
            + Self::recording_router()
            + Self::more_router()
            + Self::quality_router()
            + Self::sessions_router()
            + Self::crashes_router()
            + Self::controls_router()
            + Self::automation_router();
        if policy.allow_writes {
            tool_router += Self::writes_router();
        }
        // The list of gated tools is what is offered with writes on, and nothing else; nothing human-only is offered.
        debug_assert!(crate::policy::GATED.iter().all(|n| tool_router.has_route(n) == policy.allow_writes));
        debug_assert!(crate::policy::NEVER.iter().all(|n| !tool_router.has_route(n)));
        EmuLabServer { lab, hub: crate::hub::Hub::new(), policy, tool_router }
    }

    /// What the server keeps between calls.
    pub fn hub(&self) -> std::sync::Arc<crate::hub::Hub> {
        self.hub.clone()
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

/// A build to read: a file with the extension of an APK, an AAB or an IPA. Anything else is refused, so a tool made for
/// builds cannot be asked to open other files.
fn package_path(file: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(file);
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("apk" | "aab" | "ipa")) {
        return Err(format!("`{file}` is not an .apk, .aab or .ipa file"));
    }
    if !path.is_file() {
        return Err(format!("there is no file at {file}"));
    }
    Ok(path)
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

#[tool_handler(router = self.tool_router)]
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
        for part in a.without.iter().flatten() {
            match part.as_str() {
                "screenshot" => options.include_screenshot = false,
                "logs" => options.include_logs = false,
                "requests" => options.include_requests = false,
                "crashes" => options.include_crashes = false,
                "video" => options.include_video = false,
                other => {
                    return Err(format!(
                        "`{other}` is not a part of a report: use screenshot, logs, requests, crashes or video"
                    ));
                }
            }
        }
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
