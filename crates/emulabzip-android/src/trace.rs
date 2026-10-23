//! Recording traces: Perfetto system traces and simpleperf CPU profiles.
//!
//! Both tools ship with Android (`/system/bin/perfetto` from Android 9, `simpleperf` from Android
//! 8), so nothing is installed on the device. Each recording uses its own files on the device and
//! removes them when it ends, whether it worked, failed or was stopped.

use std::path::Path;
use std::time::Duration;

use emulabzip_adb::{AdbClient, command, is_package_name};
use emulabzip_core::{Error, PerfettoPreset, Result, TraceFile, TraceFormat, TraceRequest};
use emulabzip_trace::perfetto::config;
use tokio::sync::watch;

/// Perfetto is part of the system image from Android 9 (API 28).
pub const MIN_PERFETTO_API: u32 = 28;

/// Where Perfetto may write: the `traced` service and the shell user both reach it.
const PERFETTO_DIR: &str = "/data/misc/perfetto-traces";

/// How long to wait past the requested duration for the tool to finish and flush.
const GRACE: Duration = Duration::from_secs(90);

pub async fn record(
    adb: &AdbClient,
    serial: &str,
    request: &TraceRequest,
    out_dir: &Path,
    stop: watch::Receiver<bool>,
) -> Result<TraceFile> {
    if let Some(app) = request.app()
        && !is_package_name(app)
    {
        return Err(Error::invalid("package name", app));
    }
    tokio::fs::create_dir_all(out_dir).await?;
    let id = unique_id();
    match request {
        TraceRequest::Perfetto { preset, config, duration_s, app } => {
            perfetto(adb, serial, &id, *preset, config.as_deref(), *duration_s, app.as_deref(), out_dir, stop).await
        }
        TraceRequest::CpuProfile { app, duration_s, system_wide } => {
            simpleperf(adb, serial, &id, app.as_deref(), *duration_s, *system_wide, out_dir, stop).await
        }
        TraceRequest::Instruments { .. } => Err(Error::Unsupported("Instruments templates on Android")),
    }
}

/// A name that is safe in file names and shell words, and does not repeat between recordings.
fn unique_id() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("emulabzip-{}-{}", now.as_secs(), now.subsec_millis())
}

async fn api_level(adb: &AdbClient, serial: &str) -> Result<u32> {
    let out = adb.shell(serial, "getprop ro.build.version.sdk").await?;
    out.trim().parse().map_err(|_| Error::Parse { what: "Android version", detail: out.trim().to_string() })
}

#[allow(clippy::too_many_arguments)]
async fn perfetto(
    adb: &AdbClient,
    serial: &str,
    id: &str,
    preset: PerfettoPreset,
    custom: Option<&str>,
    duration_s: u32,
    app: Option<&str>,
    out_dir: &Path,
    mut stop: watch::Receiver<bool>,
) -> Result<TraceFile> {
    let api = api_level(adb, serial).await?;
    if api < MIN_PERFETTO_API {
        return Err(Error::Unsupported("System traces before Android 9 (Perfetto is not on those images)"));
    }
    if api < 30 {
        // Android 9 and 10 ship the daemons switched off.
        adb.shell(serial, "setprop persist.traced.enable 1").await?;
    }
    let text = config::config(preset, id, duration_s, app, custom)?;
    let cfg = format!("/data/local/tmp/{id}.cfg");
    let remote = format!("{PERFETTO_DIR}/{id}.pftrace");
    adb.push(serial, &cfg, 0o644, text.as_bytes()).await?;

    // The config goes in through stdin: SELinux does not let the perfetto client open files in
    // /data/local/tmp itself.
    let run = format!("{} | {}", command(["cat", &cfg]), command(["perfetto", "--txt", "-c", "-", "-o", &remote]));
    let recording = async { adb.shell(serial, &format!("{run} 2>&1")).await };
    tokio::pin!(recording);
    let budget = Duration::from_secs(u64::from(duration_s)) + GRACE;
    let mut stopped = false;
    let outcome = tokio::time::timeout(budget, async {
        loop {
            tokio::select! {
                out = &mut recording => break out,
                changed = stop.changed(), if !stopped => {
                    if changed.is_ok() && *stop.borrow() {
                        stopped = true;
                        // Ctrl-C ends the session cleanly, so what was recorded so far is flushed.
                        let _ = adb.shell(serial, &interrupt("perfetto", &remote)).await;
                    }
                }
            }
        }
    })
    .await;

    let result = match outcome {
        Ok(Ok(out)) => finish_perfetto(adb, serial, &remote, &out, out_dir).await,
        Ok(Err(e)) => Err(e),
        Err(_) => Err(Error::failed("Perfetto did not finish in time")),
    };
    let _ = adb.shell(serial, &command(["rm", "-f", &cfg, &remote])).await;
    result
}

async fn finish_perfetto(
    adb: &AdbClient,
    serial: &str,
    remote: &str,
    output: &str,
    out_dir: &Path,
) -> Result<TraceFile> {
    let path = out_dir.join("trace.pftrace");
    let mut file = tokio::fs::File::create(&path).await?;
    let pulled = adb.pull(serial, remote, &mut file).await;
    drop(file);
    match pulled {
        Ok(size) if size > 0 => Ok(TraceFile { path, format: TraceFormat::Perfetto, extras: vec![] }),
        _ => {
            let _ = tokio::fs::remove_file(&path).await;
            Err(Error::failed(perfetto_failure(output)))
        }
    }
}

/// Removes the color codes Perfetto writes even when it is not on a terminal.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// What to tell the user when no trace came out: Perfetto's own complaint, if it gave one.
fn perfetto_failure(output: &str) -> String {
    let plain = strip_ansi(output);
    let reason = plain
        .lines()
        .map(str::trim)
        .find(|l| l.contains("rror") || l.contains("denied") || l.contains("not found") || l.contains("Failed"))
        .unwrap_or("the device wrote no trace");
    format!("Perfetto did not record a trace: {reason}")
}

#[allow(clippy::too_many_arguments)]
async fn simpleperf(
    adb: &AdbClient,
    serial: &str,
    id: &str,
    app: Option<&str>,
    duration_s: u32,
    system_wide: bool,
    out_dir: &Path,
    mut stop: watch::Receiver<bool>,
) -> Result<TraceFile> {
    if !system_wide && app.is_none() {
        return Err(Error::invalid("target", "choose an app, or profile the whole system"));
    }
    if system_wide && adb.shell(serial, "id -u").await?.trim() != "0" {
        // Sampling every process needs root, which only emulator images without Google Play allow.
        let said = adb.root(serial).await?;
        if !said.contains("restarting") && !said.contains("already running as root") {
            return Err(Error::failed(
                "Profiling the whole system needs root. Use an emulator image without Google Play, or pick an app.",
            ));
        }
    }
    if let Some(app) = app.filter(|_| !system_wide)
        && adb.shell(serial, &command(["pidof", app])).await?.trim().is_empty()
    {
        // simpleperf would wait for the app to start, for as long as it takes.
        return Err(Error::failed(format!("{app} is not running. Start it, then record again.")));
    }
    let data = format!("/data/local/tmp/{id}.data");
    let report = format!("/data/local/tmp/{id}.pb");
    let secs = duration_s.to_string();
    let mut args = vec!["simpleperf", "record", "-g", "--duration", &secs, "-o", &data];
    match app {
        Some(app) if !system_wide => args.extend(["--app", app]),
        _ => args.push("-a"),
    }
    let run = command(&args);
    let recording = async { adb.shell(serial, &format!("{run} 2>&1")).await };
    tokio::pin!(recording);
    let mut stopped = false;
    let outcome = tokio::time::timeout(Duration::from_secs(u64::from(duration_s)) + GRACE, async {
        loop {
            tokio::select! {
                out = &mut recording => break out,
                changed = stop.changed(), if !stopped => {
                    if changed.is_ok() && *stop.borrow() {
                        stopped = true;
                        // Ctrl-C makes simpleperf stop sampling and write what it has.
                        let _ = adb.shell(serial, &interrupt("simpleperf", &data)).await;
                    }
                }
            }
        }
    })
    .await;

    let result = match outcome {
        Ok(Ok(out)) => finish_simpleperf(adb, serial, &data, &report, &out, out_dir).await,
        Ok(Err(e)) => Err(e),
        Err(_) => Err(Error::failed("simpleperf did not finish in time")),
    };
    let _ = adb.shell(serial, &command(["rm", "-f", &data, &report])).await;
    result
}

/// Sends SIGINT to the `tool` process that writes `file`. The first letter in brackets keeps the
/// pattern from matching this very command line.
fn interrupt(tool: &str, file: &str) -> String {
    let pattern = format!("{tool}.*[{}]{}", &file[..1], &file[1..]);
    format!("for p in $(pgrep -f {}); do kill -INT $p; done", emulabzip_adb::quote::quote(&pattern))
}

async fn finish_simpleperf(
    adb: &AdbClient,
    serial: &str,
    data: &str,
    report: &str,
    output: &str,
    out_dir: &Path,
) -> Result<TraceFile> {
    if let Some(reason) = simpleperf_failure(output) {
        return Err(Error::failed(reason));
    }
    let said = adb
        .shell(
            serial,
            &format!(
                "{} 2>&1",
                command(["simpleperf", "report-sample", "--protobuf", "--show-callchain", "-i", data, "-o", report])
            ),
        )
        .await?;
    let pb = out_dir.join("profile.pb");
    let mut file = tokio::fs::File::create(&pb).await?;
    let size = adb.pull(serial, report, &mut file).await;
    drop(file);
    if !size.is_ok_and(|s| s > 0) {
        let _ = tokio::fs::remove_file(&pb).await;
        return Err(Error::failed(format!("simpleperf wrote no report: {}", said.trim())));
    }
    // The raw recording opens in Android Studio and `simpleperf report`.
    let raw = out_dir.join("perf.data");
    let mut file = tokio::fs::File::create(&raw).await?;
    let extras = match adb.pull(serial, data, &mut file).await {
        Ok(_) => vec![raw],
        Err(_) => vec![],
    };
    Ok(TraceFile { path: pb, format: TraceFormat::SimpleperfReport, extras })
}

/// A readable reason when simpleperf could not profile, from what it printed.
fn simpleperf_failure(output: &str) -> Option<String> {
    let line = |needle: &str| output.lines().map(str::trim).find(|l| l.contains(needle));
    if let Some(l) = line("isn't profileable").or_else(|| line("is not profileable")).or_else(|| line("not debuggable"))
    {
        return Some(format!("{l}. Only debuggable or profileable apps can be sampled."));
    }
    if line("is not running").is_some() || line("Can't find process").is_some() {
        return Some("The app is not running. Start it, then record again.".into());
    }
    if let Some(l) = line("Permission denied").or_else(|| line("not permitted")) {
        return Some(format!("The device refused to sample: {l}"));
    }
    let recorded = line("Samples recorded:")
        .and_then(|l| l.split_once("Samples recorded:"))
        .map(|(_, rest)| rest.trim_start().chars().take_while(char::is_ascii_digit).collect::<String>());
    if recorded.as_deref() == Some("0") {
        return Some("No samples were recorded. Was the app busy while recording?".into());
    }
    line("simpleperf E").or_else(|| line("simpleperf F")).map(|l| format!("simpleperf failed: {l}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_safe_in_shell_words_and_file_names() {
        let id = unique_id();
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "{id}");
    }

    #[test]
    fn strips_color_codes() {
        assert_eq!(strip_ansi("\u{1b}[90m[1.0]\u{1b}[0m \u{1b}[31mbad\u{1b}[0m"), "[1.0] bad");
    }

    #[test]
    fn explains_why_perfetto_recorded_nothing() {
        assert_eq!(
            perfetto_failure("Connected to the Perfetto traced service\nFailed to open /x: Permission denied\n"),
            "Perfetto did not record a trace: Failed to open /x: Permission denied"
        );
        assert_eq!(perfetto_failure(""), "Perfetto did not record a trace: the device wrote no trace");
    }

    #[test]
    fn explains_why_simpleperf_could_not_profile() {
        let not_profileable = "simpleperf E cmd_record.cpp:100] Package 'com.example' isn't profileable.\n";
        assert!(simpleperf_failure(not_profileable).unwrap().contains("debuggable or profileable"));
        assert!(
            simpleperf_failure("simpleperf I x] Samples recorded: 0. Samples lost: 0.\n")
                .unwrap()
                .contains("No samples")
        );
        assert!(simpleperf_failure("simpleperf I Stopped.\nSamples recorded: 1234. Samples lost: 0.\n").is_none());
        assert!(simpleperf_failure("simpleperf E x: App is not running\n").unwrap().contains("not running"));
    }

    #[test]
    fn interrupts_only_its_own_simpleperf() {
        let cmd = interrupt("simpleperf", "/data/local/tmp/emulabzip-1-2.data");
        assert!(cmd.contains("pgrep -f") && cmd.contains("kill -INT"));
        assert!(cmd.contains("simpleperf.*[/]data/local/tmp/emulabzip-1-2.data"), "{cmd}");
    }
}
