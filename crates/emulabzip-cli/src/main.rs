//! `emulabzip`: inspect Android emulators and iOS simulators from the terminal.

mod cmd;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use emulabzip_android::AndroidBackend;
use emulabzip_apple::AppleBackend;
use emulabzip_core::{Control, CrashKind, Lab, LogLevel, PerfettoPreset};

#[derive(Parser)]
#[command(
    name = "emulabzip",
    version,
    about = "Inspect, monitor and debug apps in Android emulators and iOS simulators"
)]
#[command(propagate_version = true)]
struct Cli {
    /// Device id (adb serial or simulator UDID) or name. Optional when only one device is running.
    #[arg(short, long, global = true, env = "EMULABZIP_DEVICE")]
    device: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List running emulators, simulators and attached devices
    #[command(visible_alias = "ls")]
    Devices {
        /// Print JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Show build, hardware, display, battery and locale facts
    Info {
        #[arg(long)]
        json: bool,
    },
    /// List installed apps
    Apps {
        /// Include system apps
        #[arg(short, long)]
        all: bool,
        /// Only apps that are running
        #[arg(short, long)]
        running: bool,
        #[arg(long)]
        json: bool,
    },
    /// Follow the device log (logcat on Android, the unified log on iOS)
    Logs {
        /// Only lines from this app (package name or bundle id)
        #[arg(short, long)]
        app: Option<String>,
        /// Minimum level: verbose, debug, info, warn, error or fatal (or V, D, I, W, E, F)
        #[arg(short, long)]
        level: Option<LogLevel>,
        /// Only lines whose tag contains this text
        #[arg(short, long)]
        tag: Option<String>,
        /// Only lines whose tag or message contains this text
        #[arg(short, long)]
        grep: Option<String>,
        /// Treat --grep as a regular expression
        #[arg(short = 'E', long)]
        regex: bool,
        /// Recent lines to show before following (Android only)
        #[arg(long, default_value_t = 100)]
        tail: u32,
        /// Print one JSON object per line
        #[arg(long)]
        json: bool,
    },
    /// Save a PNG screenshot of the device screen
    Screenshot {
        /// Output file. Defaults to `<device>-<timestamp>.png` in the current directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Sample CPU, memory, frames and battery, one line per sample
    Perf {
        /// App to sample (package name or bundle id). Without it, only device-wide values
        #[arg(short, long)]
        app: Option<String>,
        /// Time between samples, in milliseconds
        #[arg(short, long, default_value_t = 1000)]
        interval: u32,
        #[arg(long)]
        json: bool,
    },
    /// Show where an app's memory goes
    Mem {
        /// Package name or bundle id
        app: String,
        #[arg(long)]
        json: bool,
    },
    /// Save a heap dump (.hprof on Android) or memory graph (.memgraph on iOS)
    Heapdump {
        app: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List an app's files
    Files {
        app: String,
        /// Folder, relative to the storage root
        #[arg(default_value = "")]
        path: String,
        /// Storage root: `data` or `external` on Android; `data`, `bundle` or `group:<id>` on iOS
        #[arg(short, long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Copy one of an app's files to this computer
    Pull {
        app: String,
        /// File, relative to the storage root
        path: String,
        #[arg(short, long)]
        root: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Capture a device's HTTP(S) traffic: run the proxy, point the device at it, print each request
    #[command(args_conflicts_with_subcommands = true)]
    Net {
        #[command(subcommand)]
        action: Option<NetAction>,
        /// Port for the proxy, on this computer's loopback interface
        #[arg(short, long, default_value_t = 9090, env = "EMULABZIP_PORT")]
        port: u16,
        /// Only run the proxy; do not change the device's settings
        #[arg(long)]
        no_connect: bool,
        /// Write the captured requests to a HAR file when stopping
        #[arg(long, value_name = "FILE")]
        har: Option<PathBuf>,
        /// Print one JSON object per request
        #[arg(long)]
        json: bool,
        /// Mock, delay or block requests with the rules in this JSON file
        #[arg(long, value_name = "FILE")]
        rules: Option<PathBuf>,
        /// Talk HTTP/3 to servers that offer it
        #[arg(long)]
        http3: bool,
        /// Show apps the servers' HTTP/3 offers instead of hiding them
        #[arg(long, requires = "http3")]
        show_h3: bool,
    },
    /// Show the views on the screen, with their names and sizes
    Layout {
        /// Print the whole tree as JSON
        #[arg(long)]
        json: bool,
        /// Also save the screenshot taken with the layout
        #[arg(long, value_name = "FILE")]
        png: Option<PathBuf>,
        /// How many levels of the tree to print
        #[arg(long, default_value_t = 12)]
        depth: usize,
    },
    /// Check the screen for missing labels, small touch targets and low contrast
    A11y {
        #[arg(long)]
        json: bool,
        /// Exit with an error when something at least this serious is found: note, warning or error
        #[arg(long, value_name = "LEVEL", value_parser = parse_severity)]
        fail_on: Option<emulabzip_core::Severity>,
    },
    /// Record logs, samples, requests and crashes into a session file, and read it back
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Record a session around a command, such as a test run, and exit with the command's code
    Ci {
        /// Where to write the session
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        /// The app the run is about: samples are taken for it, and crashes are limited to it
        #[arg(short, long)]
        app: Option<String>,
        /// Record the HTTP(S) traffic too: the device is pointed at the proxy for the run
        #[arg(long)]
        network: bool,
        #[arg(short, long, default_value_t = 9090, env = "EMULABZIP_PORT")]
        port: u16,
        /// Take a screenshot every this many seconds
        #[arg(long, value_name = "SECONDS")]
        screens: Option<u64>,
        /// Fail the run when the app crashed while it ran
        #[arg(long)]
        fail_on_crash: bool,
        /// Also write the report, in Markdown, to this file
        #[arg(long, value_name = "FILE")]
        summary: Option<PathBuf>,
        /// The command to run
        #[arg(last = true, required = true, value_name = "COMMAND")]
        command: Vec<String>,
    },
    /// Show what differs between two devices
    Compare {
        a: String,
        b: String,
        /// List the facts that match too
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// List the crashes, native crashes and ANRs a device has recorded
    Crashes {
        /// Only this app (package name or bundle id)
        #[arg(short, long)]
        app: Option<String>,
        /// Only this kind: crash, native or anr
        #[arg(short, long, value_parser = cmd::crashes::parse_kind)]
        kind: Option<CrashKind>,
        #[arg(long)]
        json: bool,
    },
    /// Show one crash with its stack and what to try next
    Crash {
        /// An id from `emulabzip crashes`
        id: String,
        /// Print the report as the device wrote it
        #[arg(long)]
        raw: bool,
        /// Print every thread, not only the one that crashed
        #[arg(long)]
        all_threads: bool,
        #[arg(long)]
        json: bool,
    },
    /// Change what the device reports or does: location, appearance, status bar, permissions, …
    Control {
        #[command(subcommand)]
        action: ControlAction,
    },
    /// Run your own collectors: programs that print JSON lines for a device
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Serve the devices to an AI assistant over MCP (standard input and output)
    Mcp,
    /// Record and analyze traces: Perfetto, simpleperf flame graphs and Instruments
    Trace {
        #[command(subcommand)]
        action: TraceAction,
    },
    /// Manage the certificate that lets EmuLabZip decrypt HTTPS
    Ca {
        #[command(subcommand)]
        action: CaAction,
    },
    /// Inspect one of an app's SQLite databases: tables, rows or a read-only query
    Db {
        app: String,
        /// Database file, relative to the storage root, for example `databases/app.db`
        path: String,
        #[arg(short, long)]
        root: Option<String>,
        /// Print the rows of this table
        #[arg(short, long)]
        table: Option<String>,
        /// Run a read-only SQL statement
        #[arg(short, long)]
        query: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u64,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Preset {
    /// Scheduling, CPU frequency and idle states
    Cpu,
    /// Frame timeline and jank
    Frames,
    /// Memory counters, plus native allocations of --app
    Memory,
    /// Your own Perfetto text configuration, from --config
    Custom,
}

impl From<Preset> for PerfettoPreset {
    fn from(p: Preset) -> Self {
        match p {
            Preset::Cpu => PerfettoPreset::Cpu,
            Preset::Frames => PerfettoPreset::Frames,
            Preset::Memory => PerfettoPreset::Memory,
            Preset::Custom => PerfettoPreset::Custom,
        }
    }
}

#[derive(Subcommand)]
enum SessionAction {
    /// Follow a device until Ctrl+C, and write what happened to a .ezsession file
    Record {
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        #[arg(short, long)]
        app: Option<String>,
        /// Record the HTTP(S) traffic too: the device is pointed at the proxy while recording
        #[arg(long)]
        network: bool,
        #[arg(short, long, default_value_t = 9090, env = "EMULABZIP_PORT")]
        port: u16,
        /// Take a screenshot every this many seconds
        #[arg(long, value_name = "SECONDS")]
        screens: Option<u64>,
        /// Stop by itself after this many seconds
        #[arg(long, value_name = "SECONDS")]
        duration: Option<u64>,
    },
    /// Summarize a session: errors, crashes, slow requests, CPU and memory
    Show {
        /// A .ezsession file, or the .partial folder of a recording that did not finish
        file: PathBuf,
        #[arg(long)]
        json: bool,
        /// Print the report as Markdown
        #[arg(long, conflicts_with = "json")]
        markdown: bool,
    },
}

fn parse_severity(text: &str) -> Result<emulabzip_core::Severity, String> {
    emulabzip_core::Severity::parse(text).ok_or_else(|| "use note, warning or error".to_string())
}

#[derive(Subcommand)]
enum NetAction {
    /// Read a packet capture (.pcap or .pcapng, such as `emulator -tcpdump` writes) as requests
    Import {
        file: PathBuf,
        /// Write the requests to a HAR file
        #[arg(long, value_name = "FILE")]
        har: Option<PathBuf>,
        /// Print one JSON object per request
        #[arg(long)]
        json: bool,
        /// Also list the DNS lookups
        #[arg(long)]
        dns: bool,
    },
}

#[derive(Subcommand)]
enum TraceAction {
    /// Record a Perfetto system trace (Android 9 and newer)
    Record {
        #[arg(short, long, value_enum, default_value = "cpu")]
        preset: Preset,
        /// Perfetto text configuration, for --preset custom
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Seconds to record. Ctrl+C stops earlier and keeps what was recorded
        #[arg(short = 'D', long, default_value_t = 10)]
        duration: u32,
        /// App to look at more closely
        #[arg(short, long)]
        app: Option<String>,
        /// Output file. Defaults to `trace-<timestamp>.pftrace`
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Open the trace in the Perfetto UI when it is saved
        #[arg(long)]
        open: bool,
    },
    /// Sample an app's CPU (or the whole system) and print its hot functions
    Cpu {
        /// App to profile: it must be running, and debuggable or profileable
        #[arg(short, long)]
        app: Option<String>,
        /// Profile every process (needs root: emulator images without Google Play)
        #[arg(long)]
        system: bool,
        #[arg(short = 'D', long, default_value_t = 10)]
        duration: u32,
        /// Save the simpleperf report (protobuf)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Save the call tree as folded stacks, for flamegraph.pl and speedscope
        #[arg(long, value_name = "FILE")]
        folded: Option<PathBuf>,
        /// How many hot functions to print
        #[arg(long, default_value_t = 15)]
        top: usize,
    },
    /// Record an Instruments template from an iOS Simulator (needs Developer Mode on this Mac)
    Instruments {
        #[arg(short, long, default_value = "Time Profiler")]
        template: String,
        #[arg(short, long)]
        app: Option<String>,
        #[arg(short = 'D', long, default_value_t = 10)]
        duration: u32,
        /// Output `.trace` document. Defaults to `recording-<timestamp>.trace`
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = 15)]
        top: usize,
    },
    /// Summarize a Perfetto trace: CPU, frames, memory and the busiest slices
    Summary {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Run SQL on a Perfetto trace
    Query {
        file: PathBuf,
        sql: String,
        #[arg(long)]
        json: bool,
    },
    /// Open a Perfetto trace in the Perfetto UI
    Open { file: PathBuf },
    /// List the recordings made in the desktop app
    List {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LocationAction {
    /// Put the device at these coordinates, in degrees
    Set { lat: String, lon: String },
    /// Go back to no simulated location (the emulator's default position on Android)
    Clear,
}

#[derive(Subcommand)]
enum StatusBarAction {
    /// Show a clean status bar
    Set {
        /// For example 9:41
        #[arg(long)]
        time: Option<String>,
        /// Battery level, 0 to 100
        #[arg(long, value_parser = clap::value_parser!(u8).range(..=100))]
        battery: Option<u8>,
        /// Show full Wi-Fi signal (or hide it with --wifi=false)
        #[arg(long)]
        wifi: Option<bool>,
        /// Show full cellular signal (or hide it with --cellular=false)
        #[arg(long)]
        cellular: Option<bool>,
    },
    /// Go back to the real status bar
    Clear,
}

#[derive(Subcommand)]
enum BatteryAction {
    /// Pretend the battery is at this level (Android)
    Set {
        /// 0 to 100
        #[arg(value_parser = clap::value_parser!(u8).range(..=100))]
        level: u8,
        #[arg(long)]
        charging: bool,
    },
    /// Go back to the real battery
    Reset,
}

#[derive(Subcommand)]
enum ControlAction {
    /// Show what the device supports, its appearance and the permissions of --app
    Info {
        #[arg(short, long)]
        app: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Location {
        #[command(subcommand)]
        action: LocationAction,
    },
    /// Switch between light and dark mode
    Appearance {
        /// light or dark
        #[arg(value_parser = ["light", "dark"])]
        mode: String,
    },
    Statusbar {
        #[command(subcommand)]
        action: StatusBarAction,
    },
    Battery {
        #[command(subcommand)]
        action: BatteryAction,
    },
    /// Grant, revoke or reset a permission (an Android permission name, or an iOS service such as photos)
    Permission {
        app: String,
        permission: String,
        /// grant, revoke or reset
        #[arg(value_parser = ["grant", "revoke", "reset"])]
        action: String,
    },
    /// Post a notification (Android) or send a simulated push to an app (iOS)
    Notify {
        /// The app that receives the push on iOS; ignored on Android
        #[arg(default_value = "com.android.shell")]
        app: String,
        #[arg(long, default_value = "EmuLabZip")]
        title: String,
        #[arg(long, default_value = "A notification from your computer")]
        body: String,
        /// A JSON file with the whole push payload (iOS)
        #[arg(long, value_name = "FILE")]
        payload: Option<PathBuf>,
    },
    /// Open a web address, or a link of an app, on the device
    Open { url: String },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List the installed plugins and whether each is enabled
    List {
        #[arg(long)]
        json: bool,
    },
    /// Start a new plugin from a template
    New { name: String },
    /// Copy a plugin folder into EmuLabZip's plugins folder (it starts disabled)
    Install {
        folder: PathBuf,
        /// Replace a plugin of the same name
        #[arg(long)]
        force: bool,
    },
    /// Allow a plugin to run from the app and from assistants
    Enable { name: String },
    /// Stop allowing a plugin to run
    Disable { name: String },
    /// Run a plugin on a device and print what it reports
    Run {
        name: String,
        /// The app to hand to the plugin
        #[arg(short, long)]
        app: Option<String>,
        /// Stop a streaming plugin after this many seconds
        #[arg(long)]
        seconds: Option<u64>,
        /// Print each event as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CaAction {
    /// Make the device trust the certificate (system store when the device allows it)
    Install,
    /// Print where the certificate file is
    Path,
}

async fn run_control(lab: &Lab, device: Option<&str>, action: ControlAction) -> anyhow::Result<()> {
    use cmd::control::{appearance, apply, grant, read_payload, status_bar};
    match action {
        ControlAction::Info { app, json } => cmd::control::info(lab, device, app.as_deref(), json).await,
        ControlAction::Location { action: LocationAction::Set { lat, lon } } => {
            apply(lab, device, Control::Location { lat, lon }, "Location set").await
        }
        ControlAction::Location { action: LocationAction::Clear } => {
            apply(lab, device, Control::ClearLocation, "Location cleared").await
        }
        ControlAction::Appearance { mode } => {
            let dark = mode == "dark";
            apply(
                lab,
                device,
                Control::Appearance { appearance: appearance(dark) },
                &format!("Appearance set to {mode}"),
            )
            .await
        }
        ControlAction::Statusbar { action: StatusBarAction::Set { time, battery, wifi, cellular } } => {
            apply(
                lab,
                device,
                Control::StatusBar { status_bar: status_bar(time, battery, wifi, cellular) },
                "Status bar overridden",
            )
            .await
        }
        ControlAction::Statusbar { action: StatusBarAction::Clear } => {
            apply(lab, device, Control::ClearStatusBar, "Status bar back to normal").await
        }
        ControlAction::Battery { action: BatteryAction::Set { level, charging } } => {
            apply(lab, device, Control::Battery { level, charging }, "Battery overridden").await
        }
        ControlAction::Battery { action: BatteryAction::Reset } => {
            apply(lab, device, Control::ResetBattery, "Battery back to normal").await
        }
        ControlAction::Permission { app, permission, action } => {
            let grant = grant(&action).map_err(anyhow::Error::msg)?;
            apply(
                lab,
                device,
                Control::Permission { app, permission: permission.clone(), grant },
                &format!("{permission}: {action}"),
            )
            .await
        }
        ControlAction::Notify { app, title, body, payload } => {
            let payload = read_payload(payload)?;
            apply(lab, device, Control::Notification { app, title, body, payload }, "Notification sent").await
        }
        ControlAction::Open { url } => apply(lab, device, Control::OpenUrl { url }, "Link opened").await,
    }
}

/// Windows gives the main thread 1 MB, which the argument parser for this many commands can use up
/// before anything runs. The command runs on a thread with room to spare, on every platform.
const STACK_BYTES: usize = 16 * 1024 * 1024;

fn main() -> ExitCode {
    let runner = std::thread::Builder::new().name("emulabzip".into()).stack_size(STACK_BYTES).spawn(|| {
        match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
            Ok(runtime) => runtime.block_on(run()),
            Err(e) => {
                eprintln!("error: could not start the async runtime: {e}");
                ExitCode::FAILURE
            }
        }
    });
    match runner.map(std::thread::JoinHandle::join) {
        Ok(Ok(code)) => code,
        _ => ExitCode::FAILURE,
    }
}

async fn run() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_env("EMULABZIP_LOG").unwrap_or_else(|_| "warn".into()))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let lab = Lab::new().with(AndroidBackend::new()).with(AppleBackend::new());
    let device = cli.device.as_deref();

    let result = match cli.command {
        Command::Devices { json } => cmd::devices::run(&lab, json).await,
        Command::Info { json } => cmd::info::run(&lab, device, json).await,
        Command::Apps { all, running, json } => cmd::apps::run(&lab, device, all, running, json).await,
        Command::Logs { app, level, tag, grep, regex, tail, json } => {
            let args = cmd::logs::Args { device: cli.device.clone(), app, level, tag, grep, regex, tail, json };
            cmd::logs::run(&lab, args).await
        }
        Command::Screenshot { output } => cmd::screenshot::run(&lab, device, output).await,
        Command::Perf { app, interval, json } => cmd::perf::run(&lab, device, app, interval, json).await,
        Command::Mem { app, json } => cmd::mem::run(&lab, device, &app, json).await,
        Command::Heapdump { app, output } => cmd::mem::heap_dump(&lab, device, &app, output).await,
        Command::Files { app, path, root, json } => {
            cmd::files::list(&lab, device, &app, root.as_deref(), &path, json).await
        }
        Command::Pull { app, path, root, output } => {
            cmd::files::pull(&lab, device, &app, root.as_deref(), &path, output).await
        }
        Command::Net { action: Some(NetAction::Import { file, har, json, dns }), .. } => {
            cmd::net::import(&file, har, json, dns).await
        }
        Command::Net { action: None, port, no_connect, har, json, rules, http3, show_h3 } => {
            let args =
                cmd::net::NetArgs { device: cli.device.clone(), port, no_connect, har, json, rules, http3, show_h3 };
            cmd::net::run(&lab, args).await
        }
        Command::Layout { json, png, depth } => cmd::layout::layout(&lab, device, json, png, depth).await,
        Command::A11y { json, fail_on } => cmd::layout::a11y(&lab, device, json, fail_on).await,
        Command::Session { action: SessionAction::Record { output, app, network, port, screens, duration } } => {
            let args = cmd::session::RecordArgs { out: output, app, network, port, screens, duration };
            cmd::session::record(&lab, device, args).await
        }
        Command::Session { action: SessionAction::Show { file, json, markdown } } => {
            cmd::session::show(&file, json, markdown)
        }
        Command::Ci { output, app, network, port, screens, fail_on_crash, summary, command } => {
            let args = cmd::ci::Args { out: output, app, network, port, screens, fail_on_crash, summary, command };
            cmd::ci::run(&lab, device, args).await
        }
        Command::Compare { a, b, all, json } => cmd::compare::run(&lab, &a, &b, all, json).await,
        Command::Crashes { app, kind, json } => cmd::crashes::list(&lab, device, app.as_deref(), kind, json).await,
        Command::Crash { id, raw, all_threads, json } => {
            cmd::crashes::show(&lab, device, &id, raw, json, all_threads).await
        }
        Command::Control { action } => run_control(&lab, device, action).await,
        Command::Plugin { action } => match action {
            PluginAction::List { json } => cmd::plugin::list(json),
            PluginAction::New { name } => cmd::plugin::new(&name),
            PluginAction::Install { folder, force } => cmd::plugin::install(&folder, force),
            PluginAction::Enable { name } => cmd::plugin::enable(&name, true),
            PluginAction::Disable { name } => cmd::plugin::enable(&name, false),
            PluginAction::Run { name, app, seconds, json } => {
                cmd::plugin::run(&lab, device, &name, app, seconds, json).await
            }
        },
        Command::Mcp => emulabzip_mcp::serve(lab.clone()).await.map_err(|e| anyhow::anyhow!("{e}")),
        Command::Trace { action } => match action {
            TraceAction::Record { preset, config, duration, app, output, open } => {
                let args = cmd::trace::RecordArgs {
                    device: cli.device.clone(),
                    preset: preset.into(),
                    config,
                    duration,
                    app,
                    output,
                    open,
                };
                cmd::trace::record_perfetto(&lab, args).await
            }
            TraceAction::Cpu { app, system, duration, output, folded, top } => {
                let args =
                    cmd::trace::CpuArgs { device: cli.device.clone(), app, system, duration, output, folded, top };
                cmd::trace::record_cpu(&lab, args).await
            }
            TraceAction::Instruments { template, app, duration, output, top } => {
                let args =
                    cmd::trace::InstrumentsArgs { device: cli.device.clone(), template, app, duration, output, top };
                cmd::trace::record_instruments(&lab, args).await
            }
            TraceAction::Summary { file, json } => cmd::trace::summary(&file, json).await,
            TraceAction::Query { file, sql, json } => cmd::trace::query(&file, &sql, json).await,
            TraceAction::Open { file } => cmd::trace::perfetto_ui(&file).await,
            TraceAction::List { json } => cmd::trace::list(json),
        },
        Command::Ca { action: CaAction::Install } => cmd::net::install_ca(&lab, device).await,
        Command::Ca { action: CaAction::Path } => cmd::net::path(),
        Command::Db { app, path, root, table, query, limit, offset, json } => {
            let args = cmd::files::DbArgs { app, path, root, table, query, limit, offset, json };
            cmd::files::db(&lab, device, args).await
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        // An exit code the command chose, after it said what it had to.
        Err(e) if e.downcast_ref::<cmd::ci::Exit>().is_some() => {
            ExitCode::from(e.downcast_ref::<cmd::ci::Exit>().map_or(1, |x| x.0))
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
