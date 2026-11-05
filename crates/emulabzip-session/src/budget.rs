//! Checking a run against the limits of `emulabzip.toml`.

use serde::Serialize;

use crate::config::Budget;
use crate::summary::Summary;

/// The numbers of a session that limits and baselines are about, by the name a limit has in the config.
pub fn metrics(s: &Summary) -> Vec<(&'static str, Option<f64>, &'static str)> {
    vec![
        ("start_time_ms", s.startup.as_ref().map(|st| st.median_ms as f64), "ms"),
        ("memory_peak_mb", s.perf.as_ref().and_then(|p| p.memory_peak_kb).map(|kb| kb as f64 / 1024.0), "MB"),
        ("cpu_avg_percent", s.perf.as_ref().and_then(|p| p.cpu_avg).map(f64::from), "%"),
        ("janky_frames_percent", s.perf.as_ref().and_then(|p| p.jank_peak).map(f64::from), "%"),
        ("requests", s.network.as_ref().map(|n| n.requests as f64), ""),
        ("failed_requests", s.network.as_ref().map(|n| n.failed as f64), ""),
        ("log_errors", Some(s.logs.errors as f64), ""),
        ("crashes", Some(s.crashes.len() as f64), ""),
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    pub metric: String,
    pub limit: f64,
    pub actual: f64,
    pub unit: String,
}

/// The limits that were broken. A limit about something that was not measured (a start time, when no
/// start time was taken) is a violation too: a budget nobody checks is not a budget.
pub fn check(summary: &Summary, budget: &Budget) -> Vec<Violation> {
    let limits: [(&str, Option<f64>); 8] = [
        ("start_time_ms", budget.start_time_ms.map(|v| v as f64)),
        ("memory_peak_mb", budget.memory_peak_mb),
        ("cpu_avg_percent", budget.cpu_avg_percent),
        ("janky_frames_percent", budget.janky_frames_percent),
        ("requests", budget.requests.map(|v| v as f64)),
        ("failed_requests", budget.failed_requests.map(|v| v as f64)),
        ("log_errors", budget.log_errors.map(|v| v as f64)),
        ("crashes", budget.crashes.map(|v| v as f64)),
    ];
    let measured = metrics(summary);
    let mut out = Vec::new();
    for (name, limit) in limits {
        let Some(limit) = limit else { continue };
        let (_, actual, unit) = measured.iter().find(|m| m.0 == name).copied().unwrap_or((name, None, ""));
        match actual {
            Some(actual) if actual > limit => {
                out.push(Violation { metric: name.into(), limit, actual, unit: unit.into() })
            }
            Some(_) => {}
            // `actual` is NaN for "not measured": it is shown as such, and always counts.
            None => out.push(Violation { metric: name.into(), limit, actual: f64::NAN, unit: unit.into() }),
        }
    }
    out
}

/// The name of a metric as a person reads it.
pub fn label(metric: &str) -> &'static str {
    match metric {
        "start_time_ms" => "Start time",
        "memory_peak_mb" => "Memory peak",
        "cpu_avg_percent" => "CPU, average",
        "janky_frames_percent" => "Janky frames, peak",
        "requests" => "Requests",
        "failed_requests" => "Failed requests",
        "log_errors" => "Log errors",
        "crashes" => "Crashes",
        _ => "Other",
    }
}

pub fn number(v: f64, unit: &str) -> String {
    if v.is_nan() {
        return "not measured".into();
    }
    let text = if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") };
    if unit.is_empty() {
        text
    } else if unit == "%" {
        format!("{text}%")
    } else {
        format!("{text} {unit}")
    }
}

/// A Markdown table of what was broken, or a line saying everything held.
pub fn markdown(violations: &[Violation], limits_set: usize) -> String {
    if limits_set == 0 {
        return String::new();
    }
    if violations.is_empty() {
        return format!("\n### Budgets\n\nAll {limits_set} limits held.\n");
    }
    let mut out = String::from("\n### Budgets\n\n| Limit | Allowed | Actual | |\n|---|---|---|---|\n");
    for v in violations {
        out.push_str(&format!(
            "| {} | {} | {} | ❌ |\n",
            label(&v.metric),
            number(v.limit, &v.unit),
            number(v.actual, &v.unit)
        ));
    }
    out
}

/// How many limits the budget sets.
pub fn count(budget: &Budget) -> usize {
    [
        budget.start_time_ms.is_some(),
        budget.memory_peak_mb.is_some(),
        budget.cpu_avg_percent.is_some(),
        budget.janky_frames_percent.is_some(),
        budget.requests.is_some(),
        budget.failed_requests.is_some(),
        budget.log_errors.is_some(),
        budget.crashes.is_some(),
    ]
    .into_iter()
    .filter(|b| *b)
    .count()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::format::DeviceMeta;
    use crate::summary::{CrashLine, LogStats, NetworkStats, PerfStats};
    use emulabzip_core::{Platform, StartKind, StartupReport, StartupRun};

    pub fn summary() -> Summary {
        Summary {
            device: DeviceMeta {
                id: "a".into(),
                name: "Pixel".into(),
                platform: Platform::Android,
                os_version: None,
                model: None,
            },
            app: Some("com.example".into()),
            started_ms: 0,
            duration_ms: Some(10_000),
            partial: false,
            logs: LogStats { total: 100, warnings: 5, errors: 4, dropped: 0 },
            top_errors: vec![],
            crashes: vec![],
            network: Some(NetworkStats { requests: 50, failed: 2, slowest: vec![] }),
            perf: Some(PerfStats {
                samples: 10,
                cpu_avg: Some(20.0),
                cpu_peak: Some(50.0),
                memory_peak_kb: Some(256 * 1024),
                fps_avg: None,
                jank_peak: Some(3.0),
            }),
            startup: StartupReport::new(
                "com.example",
                StartKind::Cold,
                vec![StartupRun { total_ms: 700, wait_ms: None, launch_state: None }],
            ),
            screens: 0,
        }
    }

    #[test]
    fn what_stays_under_its_limit_is_fine() {
        let budget = Budget {
            start_time_ms: Some(800),
            memory_peak_mb: Some(300.0),
            requests: Some(50),
            crashes: Some(0),
            ..Default::default()
        };
        assert!(check(&summary(), &budget).is_empty());
    }

    #[test]
    fn a_broken_limit_says_by_how_much() {
        let budget =
            Budget { start_time_ms: Some(500), log_errors: Some(3), cpu_avg_percent: Some(25.0), ..Default::default() };
        let v = check(&summary(), &budget);
        assert_eq!(
            v.iter().map(|v| (v.metric.as_str(), v.limit, v.actual)).collect::<Vec<_>>(),
            [("start_time_ms", 500.0, 700.0), ("log_errors", 3.0, 4.0)]
        );
    }

    #[test]
    fn a_crash_breaks_a_limit_of_zero() {
        let mut s = summary();
        s.crashes.push(CrashLine { id: "c".into(), kind: "Crash".into(), title: "boom".into(), app: None });
        let v = check(&s, &Budget { crashes: Some(0), ..Default::default() });
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn a_limit_on_something_not_measured_counts_as_broken() {
        let mut s = summary();
        s.startup = None;
        s.network = None;
        let v = check(&s, &Budget { start_time_ms: Some(800), requests: Some(10), ..Default::default() });
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|v| v.actual.is_nan()));
        assert!(markdown(&v, 2).contains("not measured"));
    }

    #[test]
    fn the_report_is_a_table_or_a_line() {
        let budget = Budget { log_errors: Some(3), crashes: Some(0), ..Default::default() };
        let md = markdown(&check(&summary(), &budget), count(&budget));
        assert!(md.contains("| Log errors | 3 | 4 | ❌ |"), "{md}");
        assert!(markdown(&[], 4).contains("All 4 limits held"));
        assert_eq!(markdown(&[], 0), "");
    }
}
