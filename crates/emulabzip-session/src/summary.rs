//! Numbers and a readable report from a session.

use std::collections::HashMap;

use emulabzip_core::{LogLevel, Platform};
use serde::Serialize;

use crate::format::DeviceMeta;
use crate::reader::Session;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Repeated {
    pub message: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStats {
    pub total: u64,
    pub warnings: u64,
    pub errors: u64,
    pub dropped: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashLine {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub app: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slow {
    pub method: String,
    pub url: String,
    pub status: u64,
    pub ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStats {
    pub requests: u64,
    /// Requests with no response, or a 4xx or 5xx one.
    pub failed: u64,
    pub slowest: Vec<Slow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfStats {
    pub samples: u64,
    pub cpu_avg: Option<f32>,
    pub cpu_peak: Option<f32>,
    pub memory_peak_kb: Option<u64>,
    pub fps_avg: Option<f32>,
    pub jank_peak: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub device: DeviceMeta,
    pub app: Option<String>,
    pub started_ms: u64,
    pub duration_ms: Option<u64>,
    /// The recording was cut short.
    pub partial: bool,
    pub logs: LogStats,
    pub top_errors: Vec<Repeated>,
    pub crashes: Vec<CrashLine>,
    pub network: Option<NetworkStats>,
    pub perf: Option<PerfStats>,
    pub screens: usize,
}

/// A message with its numbers replaced, so `retry 3 of 5` and `retry 4 of 5` count as one.
fn shape(message: &str) -> String {
    let mut out = String::with_capacity(message.len().min(140));
    let mut in_number = false;
    for c in message.chars().take(140) {
        if c.is_ascii_digit() {
            if !in_number {
                out.push('#');
            }
            in_number = true;
        } else {
            in_number = false;
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0f64, 0u32), |(s, n), v| (s + f64::from(v), n + 1));
    (n > 0).then(|| (sum / f64::from(n)) as f32)
}

fn max_f(values: impl Iterator<Item = f32>) -> Option<f32> {
    values.fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))))
}

fn network(har: &serde_json::Value) -> Option<NetworkStats> {
    let entries = har.pointer("/log/entries")?.as_array()?;
    let mut slow: Vec<Slow> = Vec::new();
    let mut failed = 0;
    for e in entries {
        let status = e.pointer("/response/status").and_then(|v| v.as_u64()).unwrap_or(0);
        if status == 0 || status >= 400 {
            failed += 1;
        }
        slow.push(Slow {
            method: e.pointer("/request/method").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            url: e.pointer("/request/url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            status,
            ms: e.get("time").and_then(|v| v.as_f64()).unwrap_or(0.0),
        });
    }
    slow.sort_by(|a, b| b.ms.total_cmp(&a.ms));
    slow.truncate(5);
    Some(NetworkStats { requests: entries.len() as u64, failed, slowest: slow })
}

pub fn summarize(session: &Session) -> Summary {
    let count = |level: LogLevel| session.logs.iter().filter(|l| l.level == level).count() as u64;
    let errors = count(LogLevel::Error) + count(LogLevel::Fatal);
    let mut repeated: HashMap<String, u64> = HashMap::new();
    for l in session.logs.iter().filter(|l| l.level >= LogLevel::Error) {
        *repeated.entry(format!("{}: {}", l.tag, shape(&l.message))).or_default() += 1;
    }
    let mut top_errors: Vec<Repeated> =
        repeated.into_iter().map(|(message, count)| Repeated { message, count }).collect();
    top_errors.sort_by(|a, b| b.count.cmp(&a.count).then(a.message.cmp(&b.message)));
    top_errors.truncate(5);

    let perf = (!session.perf.is_empty()).then(|| PerfStats {
        samples: session.perf.len() as u64,
        // The app's numbers when an app was sampled, the device's otherwise.
        cpu_avg: mean(session.perf.iter().filter_map(|s| s.app_cpu.or(s.device_cpu))),
        cpu_peak: max_f(session.perf.iter().filter_map(|s| s.app_cpu.or(s.device_cpu))),
        memory_peak_kb: session.perf.iter().filter_map(|s| s.app_memory_kb.or(s.device_memory_used_kb)).max(),
        fps_avg: mean(session.perf.iter().filter_map(|s| s.fps)),
        jank_peak: max_f(session.perf.iter().filter_map(|s| s.jank_percent)),
    });

    Summary {
        device: session.meta.device.clone(),
        app: session.meta.app.clone(),
        started_ms: session.meta.started_ms,
        duration_ms: session.meta.duration_ms(),
        partial: session.partial,
        logs: LogStats {
            total: session.logs.len() as u64,
            warnings: count(LogLevel::Warn),
            errors,
            dropped: session.meta.logs_dropped,
        },
        top_errors,
        crashes: session
            .crashes
            .iter()
            .map(|c| CrashLine {
                id: c.summary.id.clone(),
                kind: c.summary.kind.label().to_string(),
                title: c.summary.title.clone(),
                app: c.summary.app.clone(),
            })
            .collect(),
        network: session.har.as_ref().and_then(network),
        perf,
        screens: session.screens.len(),
    }
}

fn duration(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}h {:02}m {:02}s", s / 3600, s % 3600 / 60, s % 60)
    } else if s >= 60 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

fn md_escape(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// The report as Markdown, for a pull request comment or `$GITHUB_STEP_SUMMARY`.
pub fn markdown(s: &Summary) -> String {
    let platform = match s.device.platform {
        Platform::Android => "Android",
        Platform::Ios => "iOS",
    };
    let mut out = String::from("## EmuLabZip session\n\n");
    out.push_str(&format!(
        "**{}** ({platform}{}){}{}\n\n",
        s.device.name,
        s.device.os_version.as_ref().map(|v| format!(" {v}")).unwrap_or_default(),
        s.app.as_ref().map(|a| format!(" · `{a}`")).unwrap_or_default(),
        s.duration_ms.map(|d| format!(" · {}", duration(d))).unwrap_or_default(),
    ));
    if s.partial {
        out.push_str("> The recording was cut short, so the numbers below cover only part of the run.\n\n");
    }
    out.push_str("| | |\n|---|---|\n");
    out.push_str(&format!(
        "| Crashes | {} |\n",
        if s.crashes.is_empty() { "none".to_string() } else { s.crashes.len().to_string() }
    ));
    out.push_str(&format!("| Log errors | {} |\n| Log warnings | {} |\n", s.logs.errors, s.logs.warnings));
    if let Some(n) = &s.network {
        out.push_str(&format!("| Requests | {} ({} failed) |\n", n.requests, n.failed));
    }
    if let Some(p) = &s.perf {
        if let (Some(avg), Some(peak)) = (p.cpu_avg, p.cpu_peak) {
            out.push_str(&format!("| CPU | {avg:.0}% average, {peak:.0}% peak |\n"));
        }
        if let Some(kb) = p.memory_peak_kb {
            out.push_str(&format!("| Memory peak | {:.0} MB |\n", kb as f64 / 1024.0));
        }
        if let Some(fps) = p.fps_avg {
            out.push_str(&format!("| Frame rate | {fps:.0} fps |\n"));
        }
    }
    if !s.crashes.is_empty() {
        out.push_str("\n### Crashes\n\n");
        for c in &s.crashes {
            out.push_str(&format!("- **{}**: {}\n", c.kind, md_escape(&c.title)));
        }
    }
    if !s.top_errors.is_empty() {
        out.push_str("\n### Most repeated errors\n\n");
        for e in &s.top_errors {
            out.push_str(&format!("- {}× `{}`\n", e.count, md_escape(&e.message).replace('`', "'")));
        }
    }
    if let Some(n) = s.network.as_ref().filter(|n| !n.slowest.is_empty()) {
        out.push_str("\n### Slowest requests\n\n| Request | Status | Time |\n|---|---|---|\n");
        for r in &n.slowest {
            let url: String = r.url.chars().take(90).collect();
            out.push_str(&format!(
                "| {} `{}` | {} | {:.0} ms |\n",
                r.method,
                md_escape(&url),
                if r.status == 0 { "failed".into() } else { r.status.to_string() },
                r.ms
            ));
        }
    }
    out
}
