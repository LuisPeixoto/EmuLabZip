//! `emulabzip`: inspect Android emulators and iOS simulators from the terminal.

mod cmd;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use emulabzip_android::AndroidBackend;
use emulabzip_apple::AppleBackend;
use emulabzip_core::{Lab, LogLevel};

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
}

#[tokio::main]
async fn main() -> ExitCode {
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
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
