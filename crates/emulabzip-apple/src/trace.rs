//! Recording Instruments traces with `xcrun xctrace`.
//!
//! `xctrace` is the command-line face of Instruments. It records a template (Time Profiler,
//! Allocations, SwiftUI, …) from a simulator into a `.trace` document, which opens in Instruments.
//! Time Profiler recordings are also exported to XML, so the app can draw a flame graph.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use emulabzip_core::{Error, Result, TraceFile, TraceFormat, TraceRequest};
use tokio::process::Command;
use tokio::sync::watch;

use crate::perf::app_pid;

/// How long to wait past the requested duration for `xctrace` to finish writing the document.
const GRACE: Duration = Duration::from_secs(120);

/// The tables that hold CPU samples, and the templates that record them.
const CPU_TABLES: &[&str] = &["time-profile", "cpu-profile"];

pub fn xctrace<I, S>(args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = Command::new("xcrun");
    cmd.arg("xctrace").args(args).stdin(Stdio::null()).kill_on_drop(true);
    cmd
}

/// The template names in the output of `xctrace list templates`, standard ones first.
pub fn parse_templates(output: &str) -> Vec<String> {
    let mut templates = Vec::new();
    let mut in_templates = false;
    for line in output.lines().map(str::trim) {
        if let Some(title) = line.strip_prefix("==").and_then(|l| l.strip_suffix("==")) {
            in_templates = title.trim().ends_with("Templates");
        } else if in_templates && !line.is_empty() {
            templates.push(line.to_string());
        }
    }
    templates
}

pub async fn templates() -> Result<Vec<String>> {
    if !cfg!(target_os = "macos") {
        return Ok(Vec::new());
    }
    let out = xctrace(["list", "templates"]).output().await.map_err(missing)?;
    Ok(parse_templates(&String::from_utf8_lossy(&out.stdout)))
}

fn missing(e: std::io::Error) -> Error {
    match e.kind() {
        std::io::ErrorKind::NotFound => {
            Error::ToolMissing("xcrun", "Install Xcode and run `xcode-select --install` to record with Instruments.")
        }
        _ => e.into(),
    }
}

/// True when `DevToolsSecurity` says Developer Mode is off. Unknown counts as on.
async fn developer_mode_disabled() -> bool {
    Command::new("DevToolsSecurity")
        .arg("-status")
        .stdin(Stdio::null())
        .output()
        .await
        .is_ok_and(|o| is_disabled(&String::from_utf8_lossy(&o.stdout)))
}

fn is_disabled(status: &str) -> bool {
    status.to_lowercase().contains("developer mode is currently disabled")
}

/// What to tell the user when `xctrace` stopped with an error.
pub fn explain(stderr: &str) -> String {
    let message = stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("xctrace failed");
    let lower = stderr.to_lowercase();
    if lower.contains("developer mode") {
        "Instruments needs Developer Mode on this Mac. Run `sudo DevToolsSecurity -enable` in a terminal (it asks for your password), then record again."
            .to_string()
    } else if lower.contains("template") && (lower.contains("not found") || lower.contains("invalid")) {
        format!("Instruments does not know that template: {message}")
    } else {
        format!("xctrace could not record: {message}")
    }
}

pub async fn record(
    udid: &str,
    request: &TraceRequest,
    out_dir: &Path,
    mut stop: watch::Receiver<bool>,
) -> Result<TraceFile> {
    let TraceRequest::Instruments { template, app, duration_s } = request else {
        return Err(Error::Unsupported("Perfetto and simpleperf recordings on iOS"));
    };
    if !cfg!(target_os = "macos") {
        return Err(Error::Unsupported("Instruments recordings on this computer"));
    }
    if template.trim().is_empty() || template.starts_with('-') {
        return Err(Error::invalid("template", template.clone()));
    }
    // Without Developer Mode `xctrace` does not fail: it waits for an approval nobody can give.
    if developer_mode_disabled().await {
        return Err(Error::failed(explain("Developer mode is currently disabled.")));
    }
    tokio::fs::create_dir_all(out_dir).await?;
    let output = out_dir.join("recording.trace");
    let limit = format!("{duration_s}s");
    let mut args: Vec<String> =
        ["record", "--device", udid, "--template", template, "--time-limit", &limit, "--no-prompt", "--output"]
            .map(String::from)
            .into();
    args.push(output.display().to_string());
    match app {
        Some(bundle) => {
            let pid = app_pid(udid, bundle)
                .await
                .ok_or_else(|| Error::failed(format!("{bundle} is not running. Start it, then record again.")))?;
            args.extend(["--attach".into(), pid.to_string()]);
        }
        None => args.push("--all-processes".into()),
    }

    let mut child = xctrace(&args).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(missing)?;
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let reader = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut text = String::new();
        stderr.read_to_string(&mut text).await.ok();
        text
    });

    let waiting = tokio::time::timeout(Duration::from_secs(u64::from(*duration_s)) + GRACE, async {
        let mut stopped = false;
        loop {
            tokio::select! {
                status = child.wait() => break status,
                changed = stop.changed(), if !stopped => {
                    if changed.is_ok() && *stop.borrow() {
                        stopped = true;
                        // Ctrl-C makes xctrace finish the document with what it has.
                        if let Some(pid) = child.id() {
                            let _ = Command::new("kill").args(["-INT", &pid.to_string()]).status().await;
                        }
                    }
                }
            }
        }
    })
    .await;
    let status = match waiting {
        Ok(status) => status?,
        Err(_) => {
            let _ = child.kill().await;
            return Err(Error::failed(
                "xctrace did not finish in time. If Instruments never started recording, check that Developer Mode is on (`DevToolsSecurity -status`).",
            ));
        }
    };
    let said = reader.await.unwrap_or_default();
    if !status.success() && !output.exists() {
        return Err(Error::failed(explain(&said)));
    }

    let mut extras = Vec::new();
    for table in CPU_TABLES {
        let xml = out_dir.join(format!("{table}.xml"));
        if export(&output, table, &xml).await {
            extras.push(xml);
            break;
        }
    }
    Ok(TraceFile { path: output, format: TraceFormat::InstrumentsBundle, extras })
}

/// Exports one table of a recording. False when the recording has no such table.
async fn export(trace: &Path, table: &str, to: &Path) -> bool {
    let xpath = format!("/trace-toc/run[@number=\"1\"]/data/table[@schema=\"{table}\"]");
    let ok = xctrace(["export", "--input"])
        .arg(trace)
        .args(["--xpath", &xpath, "--output"])
        .arg(to)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success());
    // An absent table still gives a valid, empty document; only a table with rows is useful.
    if ok && tokio::fs::read_to_string(to).await.is_ok_and(|x| x.contains("<row")) {
        return true;
    }
    let _ = tokio::fs::remove_file(to).await;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "== Standard Templates ==\nActivity Monitor\nAllocations\nTime Profiler\n\n== Custom Templates ==\nMy Template\n\n== Devices ==\niPhone 17 (F8CE84C1-04D1-4E33-B52B-02B59E8A09C5) (Booted)\n";

    #[test]
    fn lists_only_templates() {
        assert_eq!(parse_templates(LISTING), ["Activity Monitor", "Allocations", "Time Profiler", "My Template"]);
        assert!(parse_templates("").is_empty());
    }

    #[test]
    fn reads_the_developer_mode_status() {
        assert!(is_disabled("Developer mode is currently disabled.\n"));
        assert!(!is_disabled("Developer mode is currently enabled.\n"));
        assert!(!is_disabled(""));
    }

    #[test]
    fn explains_developer_mode_in_plain_words() {
        let e = explain("Developer mode is currently disabled. Run `DevToolsSecurity -enable`.\n");
        assert!(e.contains("sudo DevToolsSecurity -enable") && e.contains("Developer Mode"));
        assert!(explain("Template 'Nope' not found\n").contains("does not know that template"));
        assert_eq!(explain(""), "xctrace could not record: xctrace failed");
    }

    #[tokio::test]
    async fn refuses_requests_that_are_not_for_instruments() {
        let (_tx, rx) = watch::channel(false);
        let request = TraceRequest::CpuProfile { app: None, duration_s: 5, system_wide: true };
        assert!(matches!(record("x", &request, Path::new("/tmp/none"), rx).await, Err(Error::Unsupported(_))));
    }

    #[tokio::test]
    async fn refuses_a_template_that_looks_like_an_option() {
        let (_tx, rx) = watch::channel(false);
        let request = TraceRequest::Instruments { template: "--output".into(), app: None, duration_s: 5 };
        if cfg!(target_os = "macos") {
            assert!(matches!(record("x", &request, Path::new("/tmp/none"), rx).await, Err(Error::Invalid { .. })));
        }
    }
}
