//! The tools. Each one is a thin wrapper over [`Lab`], so a device behaves the same here as in
//! the app and on the command line.

use std::time::Duration;

use base64::Engine;
use emulabzip_core::{
    Appearance, Control, Device, Grant, Lab, LogFilter, LogLevel, LogOptions, PerfOptions, PerfettoPreset, StatusBar,
    Target, TraceFormat, TraceRequest,
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

    async fn target(&self, query: &Option<String>) -> Result<Target, String> {
        self.device(query).await.map(|(_, t)| t)
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
        description = "One crash with its exception or signal, the stack of the thread that crashed (and others), and hints for symbolicating it."
    )]
    async fn get_crash(&self, Parameters(a): Parameters<args::Crash>) -> CallToolResult {
        reply(
            async {
                let mut report =
                    self.lab.crash(&self.target(&a.device).await?, &a.id).await.map_err(|e| e.to_string())?;
                if !a.raw.unwrap_or(false) {
                    report.raw.clear();
                }
                json(&report)
            }
            .await,
        )
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
             Tools that change the device (control_*, open_url, record_trace) act on it for real.",
        )
    }
}
