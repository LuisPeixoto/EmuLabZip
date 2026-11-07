//! `emulabzip`: inspect Android emulators and iOS simulators from the terminal.

mod cmd;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use emulabzip_android::AndroidBackend;
use emulabzip_apple::AppleBackend;
use emulabzip_core::{BootOptions, Control, CrashKind, Lab, LogLevel, PerfettoPreset, Platform, SnapshotAction};

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
enum MappingsAction {
    /// Keep a mapping.txt, a source map or a .dSYM for an app and version
    Add {
        /// The app (package name or bundle id)
        app: String,
        /// The version it belongs to, as the crash shows it, such as "1.2.0 (42)"
        #[arg(id = "app_version", value_name = "VERSION")]
        version: String,
        /// The file (or the .dSYM folder)
        file: PathBuf,
        /// What it is, when it cannot be told from the file: mapping, sourcemap or dsym
        #[arg(short, long, value_parser = cmd::symbols::parse_kind)]
        kind: Option<emulabzip_symbols::store::Kind>,
    },
    /// List what is kept
    List {
        app: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Forget what is kept for an app and version
    Remove {
        app: String,
        #[arg(id = "app_version", value_name = "VERSION")]
        version: String,
        #[arg(short, long, value_parser = cmd::symbols::parse_kind)]
        kind: Option<emulabzip_symbols::store::Kind>,
    },
}

#[derive(Subcommand)]
enum Command {
    /// List running emulators, simulators and attached devices
    #[command(visible_alias = "ls")]
    Devices {
        /// Include emulators and simulators that are not running
        #[arg(short, long)]
        all: bool,
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
        /// Look for leaks in the dump right away
        #[arg(long)]
        analyze: bool,
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
        /// Slow the network with a preset: fast-3g, slow-3g, slow-wifi, high-latency, packet-loss or offline
        #[arg(long, value_name = "PRESET")]
        conditions: Option<String>,
        /// Download speed in kilobits per second (changes the preset, or stands alone)
        #[arg(long, value_name = "KBPS")]
        down: Option<u32>,
        /// Upload speed in kilobits per second
        #[arg(long, value_name = "KBPS")]
        up: Option<u32>,
        /// Time added to every answer, in milliseconds
        #[arg(long, value_name = "MS")]
        latency: Option<u32>,
        /// How much of the traffic stalls like a lost packet, in percent
        #[arg(long, value_name = "PERCENT")]
        loss: Option<f32>,
        /// Answer from a HAR file or a recorded session, so the app never reaches the internet
        #[arg(long, value_name = "FILE")]
        replay: Option<PathBuf>,
        /// With --replay: let requests that are not in the recording go to the real server (they fail by default)
        #[arg(long, requires = "replay")]
        passthrough: bool,
        /// With --replay: make each answer wait as long as the server did
        #[arg(long, requires = "replay")]
        timing: bool,
    },
    /// Time how long an app takes to open: cold, warm or hot, several times, with the median
    Startup {
        /// Package name or bundle id
        app: String,
        /// cold (the process is stopped first), warm (back button) or hot (home button; on iOS, another app in front)
        #[arg(short, long, default_value = "cold", value_parser = cmd::startup::parse_kind)]
        kind: emulabzip_core::StartKind,
        /// How many times to open it (1 to 50)
        #[arg(short, long, default_value_t = 5)]
        runs: u32,
        #[arg(long)]
        json: bool,
    },
    /// Measure the energy an app uses over some seconds, while you use it
    Energy {
        /// Package name or bundle id
        app: String,
        /// How long to measure, in seconds (Ctrl+C reads what there is)
        #[arg(short, long, default_value_t = 30)]
        seconds: u64,
        #[arg(long)]
        json: bool,
    },
    /// Show which UI frameworks an app is built with: Flutter, React Native, Compose or SwiftUI
    Frameworks {
        /// Package name or bundle id
        app: String,
        #[arg(long)]
        json: bool,
    },
    /// Look inside a Flutter debug or profile app through its Dart VM service
    Flutter {
        #[command(subcommand)]
        action: FlutterAction,
    },
    /// Look inside a React Native app through Metro and Hermes
    #[command(name = "rn", alias = "react-native")]
    Rn {
        #[command(subcommand)]
        action: RnAction,
    },
    /// Look inside heap dumps for leaks
    Heap {
        #[command(subcommand)]
        action: HeapAction,
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
        /// The app on screen. On iOS its frameworks (SwiftUI, Flutter, React Native) decide how the views are marked
        #[arg(long)]
        app: Option<String>,
    },
    /// Check the screen for missing labels, small touch targets and low contrast
    A11y {
        /// Show the screen as someone with colour blindness sees it, and list the text that stops being readable:
        /// protanopia, deuteranopia, tritanopia or achromatopsia
        #[arg(long, value_name = "KIND", value_parser = parse_vision)]
        simulate: Option<emulabzip_core::Vision>,
        /// With --simulate: write the changed screenshot to this file
        #[arg(short, long, requires = "simulate")]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Exit with an error when something at least this serious is found: note, warning or error
        #[arg(long, value_name = "LEVEL", value_parser = parse_severity)]
        fail_on: Option<emulabzip_core::Severity>,
    },
    /// Compare screenshots with a baseline, to see when the screen changes
    Visual {
        #[command(subcommand)]
        action: VisualAction,
    },
    /// Find text that does not fit: cut off, off screen or not mirrored; on this screen or across languages and text sizes
    I18n {
        #[command(subcommand)]
        action: I18nAction,
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
        /// Limits to hold the run to (default: ./emulabzip.toml, when there is one)
        #[arg(long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// An earlier session to compare with
        #[arg(long, value_name = "FILE")]
        baseline: Option<PathBuf>,
        /// Fail when a number got worse than the baseline by more than the tolerance
        #[arg(long)]
        fail_on_regression: bool,
        /// Record the screen, in pieces of this many seconds (default 30)
        #[arg(long, value_name = "SECONDS", num_args = 0..=1, default_missing_value = "30", value_parser = cmd::session::parse_video_secs)]
        video: Option<u32>,
        /// Record touches (Android)
        #[arg(long)]
        taps: bool,
        /// Answer the app's requests from a HAR file or a session, so the run does not depend on any server (needs --network)
        #[arg(long, value_name = "FILE")]
        replay: Option<PathBuf>,
        /// With --replay: let requests that are not in the recording go to the real server
        #[arg(long, requires = "replay")]
        passthrough: bool,
        /// With --replay: make each answer wait as long as the server did
        #[arg(long, requires = "replay")]
        timing: bool,
        /// With --replay: fail the run when the app asked for something the recording does not hold
        #[arg(long, requires = "replay")]
        fail_on_miss: bool,
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
        /// Read it with this mapping.txt, source map or .dSYM, besides what is stored for the app (repeat for more)
        #[arg(short, long, value_name = "FILE")]
        mapping: Vec<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Keep the mapping.txt, source maps and .dSYMs that make crashes readable
    Mappings {
        #[command(subcommand)]
        action: MappingsAction,
    },
    /// Read a stack pasted from a log or a bug report with a mapping.txt or a source map
    Symbolicate {
        /// The stack, in a file (or - for standard input)
        stack: PathBuf,
        /// A mapping.txt or source map (repeat for more)
        #[arg(short, long, value_name = "FILE")]
        mapping: Vec<PathBuf>,
    },
    /// Write what a bug report needs into one HTML file, and a link that opens the issue
    Report {
        /// Make it from a recorded session instead of the device as it is now
        #[arg(long, value_name = "FILE")]
        session: Option<PathBuf>,
        /// The app the report is about
        #[arg(short, long)]
        app: Option<String>,
        /// Where to write the HTML
        #[arg(short, long, value_name = "FILE", default_value = "emulabzip-report.html")]
        output: PathBuf,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Where the issue goes: github:owner/repo, gitlab:[host/]group/project or jira:site/PROJECTID/ISSUETYPEID
        #[arg(long, value_name = "TRACKER")]
        issue: Option<String>,
        /// Leave a part out: screenshot, logs, requests, crashes or video (repeat for more)
        #[arg(long, value_name = "PART", value_parser = cmd::report::parse_without)]
        without: Vec<String>,
        /// Open the report (and the issue link) in the browser
        #[arg(long)]
        open: bool,
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
    /// Save an app's data to one file, put it back, look inside a file, or compare two
    State {
        #[command(subcommand)]
        action: StateAction,
    },
    /// Android emulators (AVDs): list, create, start, stop, wipe and delete
    Avd {
        #[command(subcommand)]
        action: LifeAction,
    },
    /// iOS simulators: list, create, start, stop, erase and delete
    Sim {
        #[command(subcommand)]
        action: LifeAction,
    },
    /// An emulator's own snapshots of its whole state (the running emulator, `-d` to pick it)
    Snapshot {
        #[command(subcommand)]
        action: SnapshotCmd,
    },
    /// Change an app's data: a row, a preference, a property list value. The app is stopped and the old file kept
    Edit {
        #[command(subcommand)]
        action: EditAction,
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

#[derive(Subcommand)]
enum VisualAction {
    /// Save the screen as a new baseline
    Capture {
        /// A name: letters, digits, . _ and -
        name: String,
        /// Where baselines are kept (default: `visual`, or `[visual] baseline_dir` of emulabzip.toml)
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Show a clean status bar (9:41, full battery and signal) for the shot, so the clock does not differ
        #[arg(long)]
        status_bar: bool,
    },
    /// Replace a baseline with the screen as it is now
    Update {
        name: String,
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        status_bar: bool,
        /// Yes, replace it
        #[arg(long)]
        yes: bool,
    },
    /// Compare the screen with a baseline. Exits with an error over the limit, and draws the difference
    Compare {
        name: String,
        #[arg(long)]
        dir: Option<PathBuf>,
        /// How much of the screen may change, in percent (default 0.1, or `[visual] threshold_percent`)
        #[arg(long)]
        threshold: Option<f64>,
        /// An emulabzip.toml to read `[visual]` from
        #[arg(long)]
        config: Option<PathBuf>,
        /// Where to draw the difference (default: `<dir>/diffs/<name>.png`)
        #[arg(long)]
        diff: Option<PathBuf>,
        #[arg(long)]
        status_bar: bool,
        #[arg(long)]
        json: bool,
    },
    /// List the baselines
    List {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Compare two screenshot files
    Diff {
        a: PathBuf,
        b: PathBuf,
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Compare the screenshots of two recorded sessions (`session record --screens`), in order
    Session {
        a: PathBuf,
        b: PathBuf,
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long)]
        config: Option<PathBuf>,
        /// A folder to draw the differences in
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum I18nAction {
    /// Check the screen as it is now
    Check {
        /// The screen is shown in a right-to-left language: also check that the layout was mirrored
        #[arg(long)]
        rtl: bool,
        #[arg(long)]
        json: bool,
        /// Exit with an error when something at least this serious is found: note, warning or error
        #[arg(long, value_name = "LEVEL", value_parser = parse_severity)]
        fail_on: Option<emulabzip_core::Severity>,
    },
    /// Try the screen in other languages and text sizes, and say what gets worse. The device is put back at the end
    Sweep {
        /// Languages to try, such as en-XA (long text) and ar-XB (right to left)
        #[arg(long, value_delimiter = ',', default_value = "en-XA,ar-XB")]
        locales: Vec<String>,
        /// Text sizes to try, in percent of normal
        #[arg(long, value_delimiter = ',', default_value = "100,130,200")]
        scales: Vec<u32>,
        /// The app whose language is changed (Android changes one app at a time)
        #[arg(short, long)]
        app: Option<String>,
        /// How long to wait after each change, in milliseconds
        #[arg(long, default_value_t = 1500)]
        settle_ms: u64,
        #[arg(long)]
        json: bool,
        /// Exit with an error when any setting is worse than the device as it is now
        #[arg(long)]
        fail_on_worse: bool,
    },
}

#[derive(Subcommand)]
enum StateAction {
    /// Save an app's data (databases, preferences, files; not caches) to a .ezstate file
    Save {
        app: String,
        #[arg(short, long)]
        output: PathBuf,
        /// Also keep the app's folder on shared storage (Android)
        #[arg(long)]
        external: bool,
    },
    /// Replace an app's data with a saved state. The data it replaces is saved first
    Restore {
        file: PathBuf,
        /// Yes, replace the data
        #[arg(long)]
        yes: bool,
    },
    /// What a state file holds
    Show {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// What changed in the structure of the data: tables, columns, indexes and keys. Without a second file, against the device now
    Diff {
        a: PathBuf,
        b: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LifeAction {
    /// The devices, running or not
    List {
        #[arg(long)]
        json: bool,
    },
    /// What a device can be made from: images and device types (Android), runtimes and types (iOS)
    Options {
        #[arg(long)]
        json: bool,
    },
    /// Make a device
    Create {
        name: String,
        /// Android: a system image like `system-images;android-35;google_apis;x86_64`. iOS: a runtime identifier
        #[arg(long)]
        image: String,
        /// Android: a device definition like `pixel_6`. iOS: a device type identifier
        #[arg(long)]
        model: String,
    },
    /// Start a device that is not running
    Boot {
        /// Name or id
        device: String,
        /// Start from factory state, and throw the data away (Android)
        #[arg(long)]
        wipe: bool,
        /// Do not resume the quick-boot snapshot (Android)
        #[arg(long)]
        cold: bool,
        /// No window (Android)
        #[arg(long)]
        headless: bool,
        /// Yes, with --wipe
        #[arg(long)]
        yes: bool,
    },
    /// Stop a running device
    Shutdown { device: String },
    /// Throw a device's data away (an iOS simulator is erased)
    #[command(alias = "erase")]
    Wipe {
        device: String,
        #[arg(long)]
        yes: bool,
    },
    /// Remove a device for good
    Delete {
        device: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum SnapshotCmd {
    List {
        #[arg(long)]
        json: bool,
    },
    Save {
        name: String,
    },
    /// Go back to a snapshot; the emulator's current state is lost
    Load {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    Delete {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(clap::Args)]
struct EditTarget {
    app: String,
    /// The file, relative to the storage root
    path: String,
    #[arg(short, long)]
    root: Option<String>,
    /// Yes, change it
    #[arg(long)]
    yes: bool,
}

#[derive(clap::Args)]
struct RowArgs {
    #[arg(short, long)]
    table: String,
    /// The row's rowid
    #[arg(long)]
    rowid: Option<i64>,
    /// A primary key column of the row, as column=value (for tables without a rowid)
    #[arg(long)]
    pk: Vec<String>,
}

#[derive(clap::Args)]
struct Assignments {
    /// column=value; a whole number or a decimal is stored as one, anything else as text
    #[arg(long)]
    set: Vec<String>,
    /// column=value, stored as text
    #[arg(long)]
    text: Vec<String>,
    /// A column to set to NULL
    #[arg(long)]
    null: Vec<String>,
}

#[derive(Subcommand)]
enum EditAction {
    /// Change columns of one row of a SQLite table
    Cell {
        #[command(flatten)]
        target: EditTarget,
        #[command(flatten)]
        row: RowArgs,
        #[command(flatten)]
        values: Assignments,
    },
    /// Add a row
    Insert {
        #[command(flatten)]
        target: EditTarget,
        #[arg(short, long)]
        table: String,
        #[command(flatten)]
        values: Assignments,
    },
    /// Delete one row
    Delete {
        #[command(flatten)]
        target: EditTarget,
        #[command(flatten)]
        row: RowArgs,
    },
    /// Set a SharedPreferences value
    PrefSet {
        #[command(flatten)]
        target: EditTarget,
        key: String,
        /// string, int, long, float, boolean or set (items separated by commas)
        #[arg(long = "as", default_value = "string")]
        kind: String,
        value: String,
    },
    PrefRemove {
        #[command(flatten)]
        target: EditTarget,
        key: String,
    },
    /// Set a value in a property list; a nested key is written as `a/b`
    PlistSet {
        #[command(flatten)]
        target: EditTarget,
        key: String,
        /// string, integer, real, boolean, date or data
        #[arg(long = "as", default_value = "string")]
        kind: String,
        value: String,
    },
    PlistRemove {
        #[command(flatten)]
        target: EditTarget,
        key: String,
    },
    /// Put back the files an edit replaced (the folder an edit printed)
    Undo {
        folder: PathBuf,
        #[arg(long)]
        yes: bool,
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
enum FlutterAction {
    /// The widget tree, as the Flutter inspector shows it
    Tree {
        /// Package name or bundle id
        app: String,
        /// Where the VM service listens, when it cannot be found in the log: http://127.0.0.1:41233/token=/
        #[arg(long)]
        uri: Option<String>,
        /// How many levels to print
        #[arg(long, default_value_t = 12)]
        depth: usize,
        #[arg(long)]
        json: bool,
    },
    /// Build and raster times of the frames the app draws while you use it
    Frames {
        app: String,
        #[arg(long)]
        uri: Option<String>,
        /// How long to record, in seconds
        #[arg(short, long, default_value_t = 10)]
        seconds: u64,
        #[arg(long)]
        json: bool,
    },
    /// The Dart heap and the classes that use the most of it
    Memory {
        app: String,
        #[arg(long)]
        uri: Option<String>,
        /// How many classes to list
        #[arg(long, default_value_t = 15)]
        top: usize,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum RnAction {
    /// The React component tree
    Tree {
        /// The app's package name or bundle id, to pick its runtime among Metro's
        app: String,
        /// Metro's port on this computer
        #[arg(long, default_value_t = cmd::frameworks::DEFAULT_METRO)]
        metro: u16,
        /// How many levels to print
        #[arg(long, default_value_t = 14)]
        depth: usize,
        #[arg(long)]
        json: bool,
    },
    /// The app's state: Redux, React Query, React contexts and named stores. `--watch` follows Redux actions
    State {
        app: String,
        #[arg(long, default_value_t = cmd::frameworks::DEFAULT_METRO)]
        metro: u16,
        /// Keep watching: print each Redux action and what changed (wraps `dispatch` until you stop)
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        json: bool,
    },
    /// Hermes' heap usage
    Heap {
        app: String,
        #[arg(long, default_value_t = cmd::frameworks::DEFAULT_METRO)]
        metro: u16,
        #[arg(long)]
        json: bool,
    },
    /// What the app writes to the JavaScript console
    Console {
        app: String,
        #[arg(long, default_value_t = cmd::frameworks::DEFAULT_METRO)]
        metro: u16,
        /// How long to listen, in seconds
        #[arg(short, long, default_value_t = 10)]
        seconds: u64,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum HeapAction {
    /// Find leaked Activities and Fragments in a .hprof (Android), or leaked objects in a .memgraph (iOS, on a Mac)
    Analyze {
        file: PathBuf,
        #[arg(long)]
        json: bool,
        /// How many of the biggest objects and classes to list
        #[arg(long, default_value_t = 10)]
        top: usize,
        /// Look in every heap, not only the app's
        #[arg(long)]
        all_heaps: bool,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// Show what got better or worse between two sessions
    Compare {
        /// The earlier session
        baseline: PathBuf,
        /// The session to judge
        run: PathBuf,
        /// How much worse a number can get before it counts, in percent
        #[arg(long, default_value_t = 10.0)]
        tolerance: f64,
        #[arg(long)]
        json: bool,
    },
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
        /// Record the screen, in pieces of this many seconds (default 30)
        #[arg(long, value_name = "SECONDS", num_args = 0..=1, default_missing_value = "30", value_parser = cmd::session::parse_video_secs)]
        video: Option<u32>,
        /// Record touches (Android: taps, long presses and swipes)
        #[arg(long)]
        taps: bool,
        /// Answer the app's requests from a HAR file or a session while recording (needs --network)
        #[arg(long, value_name = "FILE")]
        replay: Option<PathBuf>,
        /// With --replay: let requests that are not in the recording go to the real server
        #[arg(long, requires = "replay")]
        passthrough: bool,
        /// With --replay: make each answer wait as long as the server did
        #[arg(long, requires = "replay")]
        timing: bool,
    },
    /// Everything a session recorded on one time axis: touches, log problems, requests, slow frames, GC and crashes
    Timeline {
        file: PathBuf,
        #[arg(long)]
        json: bool,
        /// Only these lanes: taps, logs, requests, frames, gc, crashes, screens or video (repeat for more)
        #[arg(long, value_name = "LANE", value_parser = parse_lane)]
        lane: Vec<emulabzip_session::Lane>,
    },
    /// Write the pieces of a session's screen recording into a folder
    Video {
        file: PathBuf,
        /// The folder to write them to
        #[arg(short, long, value_name = "DIR", default_value = ".")]
        output: PathBuf,
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

fn parse_lane(text: &str) -> Result<emulabzip_session::Lane, String> {
    emulabzip_session::Lane::parse(text)
        .ok_or_else(|| "use taps, logs, requests, frames, gc, crashes, screens or video".to_string())
}

fn parse_vision(text: &str) -> Result<emulabzip_core::Vision, String> {
    emulabzip_core::Vision::parse(text).ok_or_else(|| {
        format!("`{text}` is not a kind of colour blindness: use protanopia, deuteranopia, tritanopia or achromatopsia")
    })
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
    /// Show an app in another language: a tag such as de-DE, or the pseudo-locales en-XA and ar-XB
    Locale {
        locale: String,
        /// The app (Android changes one app at a time; on iOS the whole simulator changes)
        #[arg(short, long)]
        app: Option<String>,
    },
    /// Put the language back
    ResetLocale {
        #[arg(short, long)]
        app: Option<String>,
    },
    /// Make text bigger or smaller: 100 is normal, 130 and 200 are common accessibility settings
    FontScale { percent: u32 },
    /// Change the screen size and density (Android)
    Display {
        /// Width in pixels; give the height too
        #[arg(long, requires = "height")]
        width: Option<u32>,
        #[arg(long, requires = "width")]
        height: Option<u32>,
        /// Density in dpi
        #[arg(long)]
        density: Option<u32>,
    },
    /// Put the screen size and density back (Android)
    ResetDisplay,
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
        ControlAction::Locale { locale, app } => {
            let said = format!("Language set to {locale}");
            apply(lab, device, Control::Locale { locale, app }, &said).await
        }
        ControlAction::ResetLocale { app } => {
            apply(lab, device, Control::ResetLocale { app }, "Language back to normal").await
        }
        ControlAction::FontScale { percent } => {
            apply(lab, device, Control::FontScale { percent }, &format!("Text size set to {percent}%")).await
        }
        ControlAction::Display { width, height, density } => {
            apply(lab, device, Control::Display { width, height, density }, "Display changed").await
        }
        ControlAction::ResetDisplay => apply(lab, device, Control::ResetDisplay, "Display back to normal").await,
    }
}

/// Windows gives the main thread 1 MB, which the argument parser for this many commands can use up
/// before anything runs. The command runs on a thread with room to spare, on every platform.
const STACK_BYTES: usize = 16 * 1024 * 1024;

async fn run_life(lab: &Lab, platform: Platform, action: LifeAction) -> anyhow::Result<()> {
    use cmd::lifecycle as life;
    match action {
        LifeAction::List { json } => life::list(lab, platform, json).await,
        LifeAction::Options { json } => life::options(lab, platform, json).await,
        LifeAction::Create { name, image, model } => life::create(lab, platform, &name, &image, &model).await,
        LifeAction::Boot { device, wipe, cold, headless, yes } => {
            life::boot(lab, platform, &device, BootOptions { wipe, cold, headless }, yes).await
        }
        LifeAction::Shutdown { device } => life::shutdown(lab, platform, &device).await,
        LifeAction::Wipe { device, yes } => life::wipe(lab, platform, &device, yes).await,
        LifeAction::Delete { device, yes } => life::delete(lab, platform, &device, yes).await,
    }
}

async fn run_edit(lab: &Lab, device: Option<&str>, action: EditAction) -> anyhow::Result<()> {
    use cmd::edit;
    fn target(t: &EditTarget) -> edit::Target<'_> {
        edit::Target { app: &t.app, path: &t.path, root: t.root.as_deref() }
    }
    let assign = |v: Assignments| edit::Assign { set: v.set, text: v.text, null: v.null };
    match action {
        EditAction::Cell { target: t, row, values } => {
            let r = edit::Row { table: &row.table, rowid: row.rowid, pk: &row.pk };
            edit::cell(lab, device, target(&t), r, assign(values), t.yes).await
        }
        EditAction::Insert { target: t, table, values } => {
            edit::insert(lab, device, target(&t), &table, assign(values), t.yes).await
        }
        EditAction::Delete { target: t, row } => {
            edit::delete(lab, device, target(&t), &row.table, row.rowid, &row.pk, t.yes).await
        }
        EditAction::PrefSet { target: t, key, kind, value } => {
            edit::pref_set(lab, device, target(&t), &key, &kind, &value, t.yes).await
        }
        EditAction::PrefRemove { target: t, key } => edit::pref_remove(lab, device, target(&t), &key, t.yes).await,
        EditAction::PlistSet { target: t, key, kind, value } => {
            edit::plist_set(lab, device, target(&t), &key, &kind, &value, t.yes).await
        }
        EditAction::PlistRemove { target: t, key } => edit::plist_remove(lab, device, target(&t), &key, t.yes).await,
        EditAction::Undo { folder, yes } => edit::undo_edit(lab, device, &folder, yes).await,
    }
}

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
        Command::Devices { all, json } => cmd::devices::run(&lab, all, json).await,
        Command::Info { json } => cmd::info::run(&lab, device, json).await,
        Command::Apps { all, running, json } => cmd::apps::run(&lab, device, all, running, json).await,
        Command::Logs { app, level, tag, grep, regex, tail, json } => {
            let args = cmd::logs::Args { device: cli.device.clone(), app, level, tag, grep, regex, tail, json };
            cmd::logs::run(&lab, args).await
        }
        Command::Screenshot { output } => cmd::screenshot::run(&lab, device, output).await,
        Command::Perf { app, interval, json } => cmd::perf::run(&lab, device, app, interval, json).await,
        Command::Mem { app, json } => cmd::mem::run(&lab, device, &app, json).await,
        Command::Heapdump { app, output, analyze } => cmd::mem::heap_dump(&lab, device, &app, output, analyze).await,
        Command::Files { app, path, root, json } => {
            cmd::files::list(&lab, device, &app, root.as_deref(), &path, json).await
        }
        Command::Pull { app, path, root, output } => {
            cmd::files::pull(&lab, device, &app, root.as_deref(), &path, output).await
        }
        Command::Net { action: Some(NetAction::Import { file, har, json, dns }), .. } => {
            cmd::net::import(&file, har, json, dns).await
        }
        Command::Net {
            action: None,
            port,
            no_connect,
            har,
            json,
            rules,
            http3,
            show_h3,
            conditions,
            down,
            up,
            latency,
            loss,
            replay,
            passthrough,
            timing,
        } => {
            let conditions = cmd::net::ConditionArgs {
                preset: conditions,
                down_kbps: down,
                up_kbps: up,
                latency_ms: latency,
                loss_percent: loss,
            };
            let args = cmd::net::NetArgs {
                device: cli.device.clone(),
                port,
                no_connect,
                har,
                json,
                rules,
                http3,
                show_h3,
                conditions,
                replay: replay.map(|file| cmd::net::ReplayArgs { file, passthrough, timing }),
            };
            cmd::net::run(&lab, args).await
        }
        Command::Frameworks { app, json } => cmd::frameworks::detect(&lab, device, &app, json).await,
        Command::Flutter { action: FlutterAction::Tree { app, uri, depth, json } } => {
            cmd::frameworks::flutter_tree(&lab, device, &app, uri.as_deref(), depth, json).await
        }
        Command::Flutter { action: FlutterAction::Frames { app, uri, seconds, json } } => {
            cmd::frameworks::flutter_frames(&lab, device, &app, uri.as_deref(), seconds, json).await
        }
        Command::Flutter { action: FlutterAction::Memory { app, uri, top, json } } => {
            cmd::frameworks::flutter_memory(&lab, device, &app, uri.as_deref(), top, json).await
        }
        Command::Rn { action: RnAction::Tree { app, metro, depth, json } } => {
            cmd::frameworks::rn_tree(&app, metro, depth, json).await
        }
        Command::Rn { action: RnAction::State { app, metro, watch, json } } => {
            cmd::frameworks::rn_state(&app, metro, watch, json).await
        }
        Command::Rn { action: RnAction::Heap { app, metro, json } } => {
            cmd::frameworks::rn_heap(&app, metro, json).await
        }
        Command::Rn { action: RnAction::Console { app, metro, seconds, json } } => {
            cmd::frameworks::rn_console(&app, metro, seconds, json).await
        }
        Command::Startup { app, kind, runs, json } => cmd::startup::startup(&lab, device, &app, kind, runs, json).await,
        Command::Energy { app, seconds, json } => cmd::startup::energy(&lab, device, &app, seconds, json).await,
        Command::Heap { action: HeapAction::Analyze { file, json, top, all_heaps } } => {
            cmd::heap::analyze(&file, json, top, all_heaps).await
        }
        Command::Layout { json, png, depth, app } => {
            cmd::layout::layout(&lab, device, json, png, depth, app.as_deref()).await
        }
        Command::Visual { action } => match action {
            VisualAction::Capture { name, dir, status_bar } => {
                cmd::visual::capture(&lab, device, &name, dir, status_bar, false).await
            }
            VisualAction::Update { name, dir, status_bar, yes } => {
                cmd::visual::update(&lab, device, &name, dir, status_bar, yes).await
            }
            VisualAction::Compare { name, dir, threshold, config, diff, status_bar, json } => {
                cmd::visual::compare(&lab, device, &name, dir, threshold, config, diff, status_bar, json).await
            }
            VisualAction::List { dir, json } => cmd::visual::list(dir, json),
            VisualAction::Diff { a, b, threshold, config, output, json } => {
                cmd::visual::files(&a, &b, threshold, config, output, json)
            }
            VisualAction::Session { a, b, threshold, config, output, json } => {
                cmd::visual::sessions(&a, &b, threshold, config, output, json)
            }
        },
        Command::I18n { action: I18nAction::Check { rtl, json, fail_on } } => {
            cmd::i18n::check(&lab, device, rtl, json, fail_on).await
        }
        Command::I18n { action: I18nAction::Sweep { locales, scales, app, settle_ms, json, fail_on_worse } } => {
            let args = cmd::i18n::SweepArgs { locales, scales, app, settle_ms, json, fail_on_worse };
            cmd::i18n::sweep(&lab, device, args).await
        }
        Command::A11y { simulate: Some(vision), output, json, fail_on } => {
            cmd::layout::simulate(&lab, device, vision, output, json, fail_on).await
        }
        Command::A11y { simulate: None, json, fail_on, .. } => cmd::layout::a11y(&lab, device, json, fail_on).await,
        Command::Session {
            action:
                SessionAction::Record {
                    output,
                    app,
                    network,
                    port,
                    screens,
                    duration,
                    video,
                    taps,
                    replay,
                    passthrough,
                    timing,
                },
        } => {
            let replay = replay.map(|file| cmd::net::ReplayArgs { file, passthrough, timing });
            let args =
                cmd::session::RecordArgs { out: output, app, network, port, screens, duration, video, taps, replay };
            cmd::session::record(&lab, device, args).await
        }
        Command::Session { action: SessionAction::Timeline { file, json, lane } } => {
            cmd::session::timeline(&file, json, &lane)
        }
        Command::Session { action: SessionAction::Video { file, output } } => cmd::session::video(&file, &output),
        Command::Session { action: SessionAction::Show { file, json, markdown } } => {
            cmd::session::show(&file, json, markdown)
        }
        Command::Session { action: SessionAction::Compare { baseline, run, tolerance, json } } => {
            cmd::session::compare(&baseline, &run, tolerance, json)
        }
        Command::Ci {
            output,
            app,
            network,
            port,
            screens,
            fail_on_crash,
            summary,
            config,
            baseline,
            fail_on_regression,
            video,
            taps,
            replay,
            passthrough,
            timing,
            fail_on_miss,
            command,
        } => {
            let replay = replay.map(|file| cmd::net::ReplayArgs { file, passthrough, timing });
            let args = cmd::ci::Args {
                out: output,
                app,
                network,
                port,
                screens,
                fail_on_crash,
                summary,
                config,
                baseline,
                fail_on_regression,
                video,
                taps,
                replay,
                fail_on_miss,
                command,
            };
            cmd::ci::run(&lab, device, args).await
        }
        Command::Compare { a, b, all, json } => cmd::compare::run(&lab, &a, &b, all, json).await,
        Command::Crashes { app, kind, json } => cmd::crashes::list(&lab, device, app.as_deref(), kind, json).await,
        Command::Crash { id, raw, all_threads, mapping, json } => {
            cmd::crashes::show(&lab, device, &id, raw, json, all_threads, &mapping).await
        }
        Command::Mappings { action } => match action {
            MappingsAction::Add { app, version, file, kind } => cmd::symbols::add(&app, &version, &file, kind),
            MappingsAction::List { app, json } => cmd::symbols::list(app.as_deref(), json),
            MappingsAction::Remove { app, version, kind } => cmd::symbols::remove(&app, &version, kind),
        },
        Command::Symbolicate { stack, mapping } => cmd::symbols::symbolicate(&stack, &mapping),
        Command::Report { session, app, output, title, description, issue, without, open } => {
            let args = cmd::report::Args { session, app, output, title, description, issue, without, open };
            cmd::report::run(&lab, device, args).await
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
        Command::State { action } => match action {
            StateAction::Save { app, output, external } => {
                cmd::state::save(&lab, device, &app, &output, external).await
            }
            StateAction::Restore { file, yes } => cmd::state::restore(&lab, device, &file, yes).await,
            StateAction::Show { file, json } => cmd::state::show(&file, json),
            StateAction::Diff { a, b, json } => cmd::state::diff(&lab, device, &a, b.as_deref(), json).await,
        },
        Command::Avd { action } => run_life(&lab, Platform::Android, action).await,
        Command::Sim { action } => run_life(&lab, Platform::Ios, action).await,
        Command::Snapshot { action } => match action {
            SnapshotCmd::List { json } => cmd::lifecycle::snapshots(&lab, device, json).await,
            SnapshotCmd::Save { name } => {
                cmd::lifecycle::snapshot(&lab, device, SnapshotAction::Save, &name, true).await
            }
            SnapshotCmd::Load { name, yes } => {
                cmd::lifecycle::snapshot(&lab, device, SnapshotAction::Load, &name, yes).await
            }
            SnapshotCmd::Delete { name, yes } => {
                cmd::lifecycle::snapshot(&lab, device, SnapshotAction::Delete, &name, yes).await
            }
        },
        Command::Edit { action } => run_edit(&lab, device, action).await,
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
