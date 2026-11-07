//! What a bug report holds, and how it is gathered.

use emulabzip_core::{CrashReport, Device, InfoSection, Lab, LogEntry, LogLevel, LogOptions, Target};
use emulabzip_net::Flow;
use emulabzip_session::timeline::{Lane, Severity, Timeline};
use emulabzip_session::{Session, summarize};
use emulabzip_symbols::store::Store;
use emulabzip_symbols::symbolicate::{Source, symbolicate};
use serde::{Deserialize, Serialize};

/// Most lines of each list, so a report stays one file a person can read.
pub const MAX_PROBLEMS: usize = 60;
pub const MAX_TAIL: usize = 80;
pub const MAX_REQUESTS: usize = 40;
pub const MAX_FRAMES: usize = 40;
const MAX_MESSAGE: usize = 600;
/// A screenshot bigger than this is left out, with a note.
pub const MAX_SCREENSHOT: usize = 6 * 1024 * 1024;
/// A clip up to this size is part of the HTML file. A bigger one is written next to it.
pub const MAX_INLINE_VIDEO: usize = 20 * 1024 * 1024;
/// A clip bigger than this is left out, with a note.
pub const MAX_VIDEO: usize = 100 * 1024 * 1024;
/// The name of the clip written next to the report when it is too big to be part of it.
pub const VIDEO_FILE: &str = "emulabzip-report.mp4";
/// How much of the run the timeline strip shows, ending at the crash or at the end of the run.
pub const STRIP_WINDOW_MS: u64 = 60_000;
/// How long before the crash a clip opens, in milliseconds.
const CLIP_LEAD_MS: u64 = 2000;
/// Most marks on the strip.
pub const MAX_STRIP_MARKS: usize = 300;

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub title: String,
    pub description: String,
    pub include_screenshot: bool,
    pub include_logs: bool,
    pub include_requests: bool,
    pub include_crashes: bool,
    /// The piece of the screen recording around the crash, when the session has one.
    pub include_video: bool,
}

impl Options {
    pub fn everything() -> Self {
        Options {
            include_screenshot: true,
            include_logs: true,
            include_requests: true,
            include_crashes: true,
            include_video: true,
            ..Default::default()
        }
    }
}

/// A piece of screen recording.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoClip {
    /// An mp4 file.
    pub bytes: Vec<u8>,
    /// What it shows, for the caption.
    pub label: String,
    /// Where to start playing, in milliseconds into the clip: a little before the crash, so the page opens on what led
    /// to it.
    pub at_ms: u64,
}

/// One thing that happened, drawn as a dot on the strip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StripMark {
    /// Milliseconds from the start of the strip.
    pub t_ms: u64,
    /// `taps`, `requests`, `logs`, `frames`, `gc` or `crashes`.
    pub lane: String,
    /// `info`, `warning` or `error`.
    pub severity: String,
    pub label: String,
}

/// The last minute before the crash (or the end of the run), on one axis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Strip {
    pub window_ms: u64,
    pub ends_at_crash: bool,
    pub marks: Vec<StripMark>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub time: String,
    pub level: String,
    pub tag: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSection {
    /// Lines the report was made from.
    pub total: usize,
    /// Warnings, errors and failures, the first of them.
    pub problems: Vec<LogLine>,
    pub problems_total: usize,
    /// The last lines, whatever they say.
    pub tail: Vec<LogLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedRequest {
    pub method: String,
    pub url: String,
    /// 0 when no answer came.
    pub status: u16,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
    /// The start of the answer, when it was text.
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashSection {
    pub kind: String,
    pub title: String,
    pub process: String,
    pub when: Option<String>,
    /// A thread name, and its frames as one line each.
    pub threads: Vec<(String, Vec<String>)>,
    /// Frames were made readable with a mapping, source map or dSYM.
    pub readable: bool,
    pub applied: Vec<String>,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BugReport {
    pub title: String,
    pub description: String,
    pub created_ms: u64,
    pub tool: String,
    /// The device, as facts.
    pub device: Vec<(String, String)>,
    pub app: Option<String>,
    pub app_version: Option<String>,
    /// PNG.
    #[serde(skip)]
    pub screenshot: Option<Vec<u8>>,
    /// The screen recording around the crash.
    #[serde(skip)]
    pub video: Option<VideoClip>,
    /// What happened in the last minute, when the report was made from a session.
    pub strip: Option<Strip>,
    pub logs: LogSection,
    pub requests: Vec<FailedRequest>,
    pub requests_total: usize,
    pub crashes: Vec<CrashSection>,
    /// Things the person should know about what is in or out of the report.
    pub notes: Vec<String>,
    /// The recording the report was made from, when it was.
    pub session_note: Option<String>,
}

fn cut(text: &str, max: usize) -> String {
    let one: String = text.lines().next().unwrap_or("").chars().collect();
    if one.chars().count() <= max {
        return one;
    }
    let mut s: String = one.chars().take(max).collect();
    s.push('…');
    s
}

fn line_of(e: &LogEntry) -> LogLine {
    LogLine {
        time: e.time.clone(),
        level: e.level.letter().to_string(),
        tag: e.tag.clone(),
        message: cut(&e.message, MAX_MESSAGE),
    }
}

/// Builds the log section from entries, oldest first.
pub fn log_section(entries: &[LogEntry]) -> LogSection {
    let problem = |e: &&LogEntry| matches!(e.level, LogLevel::Warn | LogLevel::Error | LogLevel::Fatal);
    let problems_total = entries.iter().filter(problem).count();
    // The errors matter more than the warnings: when there are too many, keep the errors first.
    let mut chosen: Vec<&LogEntry> =
        entries.iter().filter(|e| matches!(e.level, LogLevel::Error | LogLevel::Fatal)).take(MAX_PROBLEMS).collect();
    if chosen.len() < MAX_PROBLEMS {
        let room = MAX_PROBLEMS - chosen.len();
        chosen.extend(entries.iter().filter(|e| e.level == LogLevel::Warn).take(room));
    }
    chosen.sort_by_key(|e| e.seq);
    LogSection {
        total: entries.len(),
        problems: chosen.into_iter().map(line_of).collect(),
        problems_total,
        tail: entries.iter().rev().take(MAX_TAIL).rev().map(line_of).collect(),
    }
}

/// The requests of a HAR document that failed: no answer, or a status of 400 and up.
pub fn failed_from_har(har: &serde_json::Value) -> (Vec<FailedRequest>, usize) {
    let entries = har.pointer("/log/entries").and_then(|e| e.as_array()).cloned().unwrap_or_default();
    let total = entries.len();
    let failed = entries
        .iter()
        .filter_map(|e| {
            let status = e.pointer("/response/status").and_then(|s| s.as_u64()).unwrap_or(0) as u16;
            let error = e.get("comment").and_then(|c| c.as_str()).filter(|c| !c.is_empty()).map(|c| cut(c, 300));
            if status != 0 && status < 400 && error.is_none() {
                return None;
            }
            let text = e
                .pointer("/response/content/text")
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty() && t.len() < 2_000_000);
            Some(FailedRequest {
                method: e.pointer("/request/method").and_then(|m| m.as_str()).unwrap_or("").to_string(),
                url: e.pointer("/request/url").and_then(|m| m.as_str()).unwrap_or("").to_string(),
                status,
                error,
                duration_ms: e.get("time").and_then(|t| t.as_f64()).filter(|t| *t >= 0.0).map(|t| t as u64),
                snippet: text.map(|t| cut(t, 300)),
            })
        })
        .take(MAX_REQUESTS)
        .collect();
    (failed, total)
}

/// The requests of a proxy's flows that failed.
pub fn failed_from_flows(flows: &[Flow]) -> (Vec<FailedRequest>, usize) {
    let http: Vec<&Flow> = flows.iter().filter(|f| f.kind == emulabzip_net::FlowKind::Http).collect();
    let failed = http
        .iter()
        .filter(|f| f.error.is_some() || f.status.is_none_or(|s| s >= 400))
        .take(MAX_REQUESTS)
        .map(|f| FailedRequest {
            method: f.method.clone(),
            url: f.url.clone(),
            status: f.status.unwrap_or(0),
            error: f.error.as_deref().map(|e| cut(e, 300)),
            duration_ms: f.timing.total_ms,
            snippet: std::str::from_utf8(&f.response_body[..f.response_body.len().min(300)])
                .ok()
                .filter(|t| !t.trim().is_empty())
                .map(|t| cut(t, 300)),
        })
        .collect();
    (failed, http.len())
}

fn frame_line(f: &emulabzip_core::Frame) -> String {
    let symbol = if f.symbol.is_empty() { "(no symbol)" } else { f.symbol.as_str() };
    let place = match (f.image.as_deref(), f.location.as_deref()) {
        (Some(i), Some(l)) => format!("{i}:{l}"),
        (Some(i), None) => i.to_string(),
        (None, Some(l)) => l.to_string(),
        (None, None) => String::new(),
    };
    if place.is_empty() { format!("{:>2}  {symbol}", f.index) } else { format!("{:>2}  {symbol}  ({place})", f.index) }
}

/// A crash, made readable with whatever the store has for its app and version.
pub fn crash_section(report: &CrashReport, store: &Store) -> CrashSection {
    let sources: Vec<Source> = match (&report.summary.app, &report.app_version) {
        (Some(app), version) => store.find(app, version.as_deref()).iter().map(Source::from).collect(),
        _ => Vec::new(),
    };
    let s = symbolicate(report, &sources);
    // What crashed first: a Java crash lists the outer exceptions before the root cause, and the cause is what to read.
    let mut ordered: Vec<&emulabzip_core::ThreadTrace> =
        s.report.threads.iter().filter(|t| t.crashed || !t.frames.is_empty()).collect();
    ordered.sort_by_key(|t| !t.crashed);
    let threads = ordered
        .into_iter()
        .take(3)
        .map(|t| {
            let mut lines: Vec<String> = t.frames.iter().take(MAX_FRAMES).map(frame_line).collect();
            if t.frames.len() > MAX_FRAMES {
                lines.push(format!("… {} more", t.frames.len() - MAX_FRAMES));
            }
            (if t.crashed { format!("{} (crashed)", t.name) } else { t.name.clone() }, lines)
        })
        .collect();
    CrashSection {
        kind: s.report.summary.kind.label().to_string(),
        title: s.report.summary.title.clone(),
        process: s.report.summary.process.clone(),
        when: s.report.summary.time_ms.map(|ms| format!("{ms}")),
        threads,
        readable: !s.applied.is_empty(),
        applied: s.applied.iter().map(|a| a.describe()).collect(),
        missing: s.missing,
    }
}

/// The same crash, again and again (a loop, or a retry), is one section that says how often it happened.
fn collapse_repeats(sections: Vec<CrashSection>, notes: &mut Vec<String>) -> Vec<CrashSection> {
    let mut out: Vec<(CrashSection, usize)> = Vec::new();
    for c in sections {
        match out.iter_mut().find(|(o, _)| o.kind == c.kind && o.title == c.title && o.threads == c.threads) {
            Some((_, n)) => *n += 1,
            None => out.push((c, 1)),
        }
    }
    out.into_iter()
        .map(|(c, n)| {
            if n > 1 {
                notes.push(format!("The crash \"{}\" happened {n} times; it is shown once.", c.title));
            }
            c
        })
        .collect()
}

fn facts_of(device: &Device) -> Vec<(String, String)> {
    let mut facts =
        vec![("Device".to_string(), device.name.clone()), ("Platform".to_string(), format!("{:?}", device.platform))];
    if let Some(v) = &device.os_version {
        facts.push(("OS version".into(), v.clone()));
    }
    if let Some(a) = device.api_level {
        facts.push(("API level".into(), a.to_string()));
    }
    if let Some(m) = &device.model {
        facts.push(("Model".into(), m.clone()));
    }
    if let Some(a) = &device.abi {
        facts.push(("ABI".into(), a.clone()));
    }
    facts
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// The last minute before the crash, or before the end of the run, as marks on one axis. A part the person left out of
/// the report is left out of the strip too: its marks carry the text of the log and the addresses of the requests.
fn strip_of(timeline: &Timeline, crash_at: Option<u64>, options: &Options) -> Option<Strip> {
    let crash_at = crash_at.filter(|_| options.include_crashes);
    let end = crash_at.unwrap_or(timeline.duration_ms);
    let start = end.saturating_sub(STRIP_WINDOW_MS);
    let lane = |l: Lane| match l {
        Lane::Taps | Lane::Frames => true,
        Lane::Requests => options.include_requests,
        Lane::Logs | Lane::Gc => options.include_logs,
        Lane::Crashes => options.include_crashes,
        Lane::Screens | Lane::Video => false,
    };
    let mut marks: Vec<StripMark> = timeline
        .events
        .iter()
        .filter(|e| lane(e.lane) && e.t_ms >= start && e.t_ms <= end + 1000)
        .map(|e| StripMark {
            t_ms: e.t_ms - start,
            lane: e.lane.name().to_string(),
            severity: match e.severity {
                Severity::Info => "info",
                Severity::Warning => "warning",
                Severity::Error => "error",
            }
            .to_string(),
            label: e.label.clone(),
        })
        .collect();
    if marks.is_empty() {
        return None;
    }
    if marks.len() > MAX_STRIP_MARKS {
        // Keep the worst, then the rest in time order.
        marks.sort_by_key(|m| (m.severity != "error", m.severity != "warning", m.t_ms));
        marks.truncate(MAX_STRIP_MARKS);
        marks.sort_by_key(|m| m.t_ms);
    }
    Some(Strip { window_ms: end - start, ends_at_crash: crash_at.is_some(), marks })
}

/// The piece of recording that holds the crash, or the last piece when there was no crash.
fn video_clip(session: &Session, crash_at: Option<u64>) -> Result<VideoClip, String> {
    let at = crash_at.map(|t| session.meta.started_ms + t);
    let pick = at
        .and_then(|t| {
            session
                .video
                .iter()
                .enumerate()
                .find(|(_, v)| v.start_ms <= t && t <= v.end_ms + 2000)
                .or_else(|| session.video.iter().enumerate().rev().find(|(_, v)| v.start_ms <= t))
        })
        .or_else(|| session.video.iter().enumerate().next_back());
    let Some((i, piece)) = pick else { return Err("The recording has no video.".into()) };
    let bytes = session.video_file(&piece.file).map_err(|e| format!("The video could not be read: {e}"))?;
    if bytes.len() > MAX_VIDEO {
        return Err("The video was too big to include.".into());
    }
    let secs = piece.end_ms.saturating_sub(piece.start_ms) / 1000;
    let what = if crash_at.is_some() { "around the crash" } else { "the last piece of the run" };
    // Open two seconds before the crash.
    let at_ms = at.map_or(0, |t| t.saturating_sub(piece.start_ms).saturating_sub(CLIP_LEAD_MS));
    Ok(VideoClip {
        bytes,
        label: format!("Screen recording, {secs} s, {what} (piece {} of {}).", i + 1, session.video.len()),
        at_ms,
    })
}

fn tool() -> String {
    format!("EmuLabZip {}", env!("CARGO_PKG_VERSION"))
}

impl BugReport {
    /// The video, when it is too big to be part of the HTML file and has to be written next to it as [`VIDEO_FILE`].
    pub fn video_sidecar(&self) -> Option<&[u8]> {
        self.video.as_ref().filter(|v| v.bytes.len() > MAX_INLINE_VIDEO).map(|v| v.bytes.as_slice())
    }

    /// A report from a recorded session: everything it holds.
    pub fn from_session(session: &Session, options: &Options, store: &Store) -> BugReport {
        let summary = summarize(session);
        let mut report = BugReport {
            title: options.title.clone(),
            description: options.description.clone(),
            created_ms: now_ms(),
            tool: tool(),
            device: vec![
                ("Device".into(), session.meta.device.name.clone()),
                ("Platform".into(), format!("{:?}", session.meta.device.platform)),
                ("OS version".into(), session.meta.device.os_version.clone().unwrap_or_default()),
                ("Model".into(), session.meta.device.model.clone().unwrap_or_default()),
            ]
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .collect(),
            app: session.meta.app.clone(),
            ..Default::default()
        };
        if options.include_logs {
            report.logs = log_section(&session.logs);
        }
        if options.include_requests
            && let Some(har) = &session.har
        {
            (report.requests, report.requests_total) = failed_from_har(har);
        }
        if options.include_crashes {
            let sections = session.crashes.iter().map(|c| crash_section(c, store)).collect();
            report.crashes = collapse_repeats(sections, &mut report.notes);
            report.app_version = session.crashes.iter().find_map(|c| c.app_version.clone());
        }
        if options.include_screenshot {
            // The last screenshot of the run is the closest to what went wrong.
            match session.screens.last().and_then(|name| session.screen(name).ok()) {
                Some(png) if png.len() <= MAX_SCREENSHOT => report.screenshot = Some(png),
                Some(_) => report.notes.push("The screenshot was too big to include.".into()),
                None => {}
            }
        }
        let timeline = emulabzip_session::timeline::build(session);
        let crash_at = timeline.events.iter().filter(|e| e.lane == Lane::Crashes).map(|e| e.t_ms).max();
        report.strip = strip_of(&timeline, crash_at, options);
        if options.include_video && !session.video.is_empty() {
            match video_clip(session, crash_at) {
                Ok(clip) => report.video = Some(clip),
                Err(note) => report.notes.push(note),
            }
        }
        if session.partial {
            report.notes.push("The recording was cut short, so this is what it held up to then.".into());
        }
        let secs = session.meta.ended_ms.map(|e| e.saturating_sub(session.meta.started_ms) / 1000);
        report.session_note = Some(match secs {
            Some(s) => format!(
                "Made from a recording of {s} s: {} log lines, {} requests, {} crashes.",
                summary.logs.total,
                summary.network.as_ref().map_or(0, |n| n.requests),
                summary.crashes.len()
            ),
            None => format!("Made from a recording: {} log lines.", summary.logs.total),
        });
        report
    }

    /// A report of the device as it is now: its facts, a screenshot, the log of the app, and its crashes of the last
    /// half hour. `flows` are the requests the proxy saw, when it runs.
    pub async fn collect(
        lab: &Lab,
        target: &Target,
        app: Option<&str>,
        flows: &[Flow],
        options: &Options,
        store: &Store,
    ) -> BugReport {
        let mut report = BugReport {
            title: options.title.clone(),
            description: options.description.clone(),
            created_ms: now_ms(),
            tool: tool(),
            app: app.map(str::to_string),
            ..Default::default()
        };
        match lab.resolve(Some(&target.id)).await {
            Ok(device) => report.device = facts_of(&device),
            Err(e) => report.notes.push(format!("The device facts could not be read: {e}")),
        }
        if let Ok(sections) = lab.info(target).await {
            report.device.extend(extra_facts(&sections));
        }
        if options.include_screenshot {
            match lab.screenshot(target).await {
                Ok(png) if png.len() <= MAX_SCREENSHOT => report.screenshot = Some(png),
                Ok(_) => report.notes.push("The screenshot was too big to include.".into()),
                Err(e) => report.notes.push(format!("No screenshot: {e}")),
            }
        }
        if options.include_logs {
            match read_logs(lab, target, app).await {
                Ok(entries) => report.logs = log_section(&entries),
                Err(e) => report.notes.push(format!("No log: {e}")),
            }
        }
        if options.include_requests {
            (report.requests, report.requests_total) = failed_from_flows(flows);
            if flows.is_empty() {
                report
                    .notes
                    .push("No requests were recorded: start the proxy and connect the device to include them.".into());
            }
        }
        if options.include_crashes {
            match lab.crashes(target, app).await {
                Ok(list) => {
                    let cutoff = now_ms().saturating_sub(30 * 60 * 1000);
                    let mut sections = Vec::new();
                    for summary in list.iter().filter(|c| c.time_ms.is_none_or(|t| t >= cutoff)).take(10) {
                        if let Ok(full) = lab.crash(target, &summary.id).await {
                            if report.app_version.is_none() {
                                report.app_version = full.app_version.clone();
                            }
                            sections.push(crash_section(&full, store));
                        }
                    }
                    report.crashes = collapse_repeats(sections, &mut report.notes);
                    report.crashes.truncate(3);
                }
                Err(e) => report.notes.push(format!("No crashes could be read: {e}")),
            }
        }
        report
    }
}

/// A few more facts, from the device's info: the build and the screen.
fn extra_facts(sections: &[InfoSection]) -> Vec<(String, String)> {
    let wanted = ["Build", "Resolution", "Density", "Memory", "Locale", "Language", "Time zone"];
    sections
        .iter()
        .flat_map(|s| s.items.iter())
        .filter(|(k, _)| wanted.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Reads the log for a moment: what is replayed, and then a short wait for anything more.
async fn read_logs(lab: &Lab, target: &Target, app: Option<&str>) -> emulabzip_core::Result<Vec<LogEntry>> {
    let mut stream = lab.logs(target, LogOptions { tail: 600, verbose: false }).await?;
    let mut entries = Vec::new();
    let _ = tokio::time::timeout(std::time::Duration::from_millis(1500), async {
        while let Some(e) = stream.next().await {
            entries.push(e);
            if entries.len() >= 5000 {
                break;
            }
        }
    })
    .await;
    if let Some(app) = app {
        // Without the process list, the tag or a mention of the app is the way to tell its lines.
        let mine: Vec<LogEntry> =
            entries.iter().filter(|e| e.message.contains(app) || e.tag.contains(app)).cloned().collect();
        if mine.len() >= 10 {
            return Ok(mine);
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(seq: u64, level: LogLevel, message: &str) -> LogEntry {
        LogEntry {
            seq,
            time: "10-23 14:02:11.123".into(),
            pid: 1,
            tid: 1,
            level,
            tag: "App".into(),
            message: message.into(),
            subsystem: None,
        }
    }

    #[test]
    fn keeps_the_problems_and_the_end_of_the_log() {
        let mut entries: Vec<LogEntry> = (0..300).map(|i| entry(i, LogLevel::Info, &format!("line {i}"))).collect();
        entries.push(entry(300, LogLevel::Error, "boom\nsecond line"));
        entries.push(entry(301, LogLevel::Warn, "careful"));
        let s = log_section(&entries);
        assert_eq!((s.total, s.problems_total), (302, 2));
        assert_eq!(
            s.problems.iter().map(|p| p.message.as_str()).collect::<Vec<_>>(),
            ["boom", "careful"],
            "one line each"
        );
        assert_eq!(s.tail.len(), MAX_TAIL);
        assert_eq!(s.tail.last().unwrap().message, "careful");
        assert_eq!(s.problems[0].level, "E");
    }

    #[test]
    fn errors_win_over_warnings_when_there_are_too_many() {
        let mut entries: Vec<LogEntry> = (0..100).map(|i| entry(i, LogLevel::Warn, "w")).collect();
        entries.extend((100..130).map(|i| entry(i, LogLevel::Error, "e")));
        let s = log_section(&entries);
        assert_eq!(s.problems.len(), MAX_PROBLEMS);
        assert_eq!(s.problems.iter().filter(|p| p.level == "E").count(), 30);
        assert_eq!(s.problems_total, 130);
        // Still in the order they happened.
        assert!(s.problems.windows(2).all(|w| w[0].time <= w[1].time));
    }

    #[test]
    fn a_long_message_is_cut() {
        let s = log_section(&[entry(1, LogLevel::Error, &"x".repeat(5000))]);
        assert_eq!(s.problems[0].message.chars().count(), MAX_MESSAGE + 1);
        assert!(s.problems[0].message.ends_with('…'));
    }

    #[test]
    fn a_crash_that_repeats_is_shown_once() {
        let section = |title: &str, when: &str| CrashSection {
            kind: "Crash".into(),
            title: title.into(),
            process: "p".into(),
            when: Some(when.into()),
            threads: vec![("main (crashed)".into(), vec!["0  a".into()])],
            readable: false,
            applied: vec![],
            missing: vec![],
        };
        let mut notes = Vec::new();
        let out = collapse_repeats(
            vec![section("boom", "1"), section("other", "2"), section("boom", "3"), section("boom", "4")],
            &mut notes,
        );
        assert_eq!(out.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["boom", "other"]);
        assert_eq!(notes, ["The crash \"boom\" happened 3 times; it is shown once."]);
    }

    #[test]
    fn the_thread_that_crashed_comes_first() {
        use emulabzip_core::{CrashKind, CrashReport, CrashSummary, Frame, FrameOrigin, ThreadTrace};
        let frame = |i: u32, name: &str| Frame::new(i, name, None, None, FrameOrigin::App);
        let thread =
            |name: &str, crashed: bool| ThreadTrace { name: name.into(), crashed, frames: vec![frame(0, name)] };
        let report = CrashReport {
            summary: CrashSummary {
                id: "x".into(),
                kind: CrashKind::Crash,
                app: None,
                process: "p".into(),
                pid: None,
                time_ms: None,
                title: "t".into(),
            },
            details: vec![],
            threads: vec![thread("outer exception", false), thread("root cause", true)],
            hints: vec![],
            app_version: None,
            raw: String::new(),
        };
        let s = crash_section(&report, &Store::at(std::env::temp_dir().join("emulabzip-report-none")));
        assert_eq!(s.threads[0].0, "root cause (crashed)");
        assert_eq!(s.threads[1].0, "outer exception");
    }

    #[test]
    fn finds_the_failed_requests_of_a_har() {
        let har = serde_json::json!({ "log": { "entries": [
            { "time": 12.0, "request": { "method": "GET", "url": "https://a/ok" }, "response": { "status": 200, "content": { "text": "fine" } }, "comment": "" },
            { "time": 80.0, "request": { "method": "POST", "url": "https://a/bad" }, "response": { "status": 500, "content": { "text": "{\"error\":\"boom\"}" } }, "comment": "" },
            { "time": 5.0, "request": { "method": "GET", "url": "https://a/down" }, "response": { "status": 0, "content": {} }, "comment": "connection refused" }
        ]}});
        let (failed, total) = failed_from_har(&har);
        assert_eq!(total, 3);
        assert_eq!(failed.len(), 2);
        assert_eq!((failed[0].status, failed[0].snippet.as_deref()), (500, Some("{\"error\":\"boom\"}")));
        assert_eq!((failed[1].status, failed[1].error.as_deref()), (0, Some("connection refused")));
        assert_eq!(failed_from_har(&serde_json::json!({})).1, 0);
    }
}
