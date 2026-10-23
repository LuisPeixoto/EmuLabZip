//! Streaming the unified log of a simulator with `log stream --style ndjson`.

use emulabzip_core::{LogEntry, LogLevel, LogOptions, Result, Stream};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::simctl;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    #[serde(default)]
    event_type: String,
    #[serde(default)]
    message_type: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default, rename = "processID")]
    process_id: u64,
    #[serde(default, rename = "threadID")]
    thread_id: u64,
    #[serde(default)]
    process_image_path: String,
    #[serde(default)]
    subsystem: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    event_message: String,
}

/// The arguments for `simctl` that follow the unified log of `udid`.
pub fn args(udid: &str, options: LogOptions) -> Vec<String> {
    let level = if options.verbose { "debug" } else { "info" };
    ["spawn", udid, "log", "stream", "--style", "ndjson", "--level", level].map(String::from).to_vec()
}

/// Maps unified logging message types onto the shared levels. `Default` is the level of a plain
/// `os_log` call and of `print`-style logging, so it is shown as info.
pub fn level(message_type: &str) -> LogLevel {
    match message_type {
        "Debug" => LogLevel::Debug,
        "Info" | "Default" => LogLevel::Info,
        "Error" => LogLevel::Error,
        "Fault" => LogLevel::Fatal,
        _ => LogLevel::Verbose,
    }
}

/// Parses one ndjson line. Activity, signpost and status lines return `None`.
pub fn parse_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if !line.starts_with('{') {
        return None;
    }
    let event: Event = serde_json::from_str(line).ok()?;
    if event.event_type != "logEvent" {
        return None;
    }
    let process = event.process_image_path.rsplit('/').next().unwrap_or_default();
    let subsystem = match (event.subsystem.is_empty(), event.category.is_empty()) {
        (true, _) => None,
        (false, true) => Some(event.subsystem),
        (false, false) => Some(format!("{}:{}", event.subsystem, event.category)),
    };
    Some(LogEntry {
        seq: 0,
        time: short_time(&event.timestamp),
        pid: u32::try_from(event.process_id).unwrap_or(u32::MAX),
        tid: (event.thread_id & u64::from(u32::MAX)) as u32,
        level: level(&event.message_type),
        tag: if process.is_empty() { "kernel".to_string() } else { process.to_string() },
        message: event.event_message,
        subsystem,
    })
}

/// `2026-10-23 12:57:51.564104-0300` becomes `10-23 12:57:51.564`, the layout logcat uses, so
/// both platforms line up in the same table.
fn short_time(ts: &str) -> String {
    ts.get(5..23).unwrap_or(ts).to_string()
}

/// Starts `log stream` in the simulator and parses its output until the stream is dropped.
pub fn stream(udid: &str, options: LogOptions) -> Result<Stream<LogEntry>> {
    let mut child = simctl::spawn(args(udid, options))?;
    let stdout = child.stdout.take().expect("piped stdout");
    Ok(Stream::spawn(4096, |tx| async move {
        // Owning the child here means aborting the task kills `log stream`.
        let _child = child;
        let mut lines = BufReader::with_capacity(256 * 1024, stdout).lines();
        let mut seq = 0u64;
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(mut entry) = parse_line(&line) {
                seq += 1;
                entry.seq = seq;
                if tx.send(entry).await.is_err() {
                    break;
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/log-stream.ndjson");

    #[test]
    fn parses_a_fixture() {
        let entries: Vec<_> = FIXTURE.lines().filter_map(parse_line).collect();
        assert_eq!(entries.len(), 10);
        let first = &entries[0];
        assert_eq!(first.time.len(), "10-23 12:57:51.564".len());
        assert!(entries.iter().all(|e| !e.tag.is_empty()));
        assert!(entries.iter().any(|e| e.pid > 0));
        assert!(entries.iter().any(|e| e.level == LogLevel::Error));
        assert!(entries.iter().any(|e| e.level == LogLevel::Fatal));
        assert!(entries.iter().any(|e| e.subsystem.as_deref() == Some("com.apple.network:connection")));
    }

    #[test]
    fn skips_non_log_lines() {
        assert!(parse_line("getpwuid_r did not find a match for uid 501").is_none());
        assert!(parse_line(r#"{"eventType":"activityCreateEvent","eventMessage":"x"}"#).is_none());
        assert!(parse_line("{not json").is_none());
    }

    #[test]
    fn maps_levels() {
        assert_eq!(level("Default"), LogLevel::Info);
        assert_eq!(level("Fault"), LogLevel::Fatal);
        assert_eq!(level("Debug"), LogLevel::Debug);
    }

    #[test]
    fn shortens_timestamps() {
        assert_eq!(short_time("2026-10-23 12:57:51.564104-0300"), "10-23 12:57:51.564");
        assert_eq!(short_time("odd"), "odd");
    }

    #[test]
    fn builds_arguments() {
        let a = args("UDID", LogOptions { tail: 0, verbose: false });
        assert_eq!(a.join(" "), "spawn UDID log stream --style ndjson --level info");
    }
}
