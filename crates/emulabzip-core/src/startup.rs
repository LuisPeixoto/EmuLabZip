//! How long an app takes to open.

use serde::{Deserialize, Serialize};

/// What state the app is in before it is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartKind {
    /// The process is gone: the system starts it from nothing.
    Cold,
    /// The process is alive but the screen was closed (the back button).
    Warm,
    /// The app is only in the background (the home button).
    Hot,
}

impl StartKind {
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "cold" => Some(StartKind::Cold),
            "warm" => Some(StartKind::Warm),
            "hot" => Some(StartKind::Hot),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StartKind::Cold => "cold",
            StartKind::Warm => "warm",
            StartKind::Hot => "hot",
        }
    }
}

/// One measured opening.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupRun {
    /// From the request to open the app until its first screen was drawn.
    pub total_ms: u64,
    /// Of which, waiting for the system to get ready (`WaitTime` on Android).
    pub wait_ms: Option<u64>,
    /// What the system called it, such as `COLD`, `WARM` or `HOT` on Android.
    pub launch_state: Option<String>,
}

/// Several openings of one app, summed up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupReport {
    pub app: String,
    pub kind: StartKind,
    pub runs: Vec<StartupRun>,
    pub median_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    /// The time that 9 of 10 openings beat (nearest rank).
    pub p90_ms: u64,
}

impl StartupReport {
    /// Sums up the runs. Returns `None` when there are none.
    pub fn new(app: impl Into<String>, kind: StartKind, runs: Vec<StartupRun>) -> Option<Self> {
        let mut times: Vec<u64> = runs.iter().map(|r| r.total_ms).collect();
        if times.is_empty() {
            return None;
        }
        times.sort_unstable();
        let n = times.len();
        let median_ms = if n % 2 == 1 { times[n / 2] } else { (times[n / 2 - 1] + times[n / 2]) / 2 };
        let p90_ms = times[((n as f64 * 0.9).ceil() as usize).clamp(1, n) - 1];
        Some(StartupReport { app: app.into(), kind, median_ms, min_ms: times[0], max_ms: times[n - 1], p90_ms, runs })
    }
}

/// The most openings one measurement may ask for.
pub const MAX_RUNS: u32 = 50;

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(times: &[u64]) -> Vec<StartupRun> {
        times.iter().map(|&t| StartupRun { total_ms: t, wait_ms: None, launch_state: None }).collect()
    }

    #[test]
    fn the_median_of_an_odd_and_an_even_number_of_runs() {
        assert_eq!(StartupReport::new("a", StartKind::Cold, runs(&[900, 300, 600])).unwrap().median_ms, 600);
        assert_eq!(StartupReport::new("a", StartKind::Cold, runs(&[100, 200, 300, 400])).unwrap().median_ms, 250);
    }

    #[test]
    fn the_extremes_and_the_ninetieth_percentile() {
        let r = StartupReport::new("a", StartKind::Warm, runs(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 1000])).unwrap();
        assert_eq!((r.min_ms, r.max_ms, r.p90_ms), (10, 1000, 90));
        let one = StartupReport::new("a", StartKind::Hot, runs(&[42])).unwrap();
        assert_eq!((one.min_ms, one.median_ms, one.p90_ms, one.max_ms), (42, 42, 42, 42));
    }

    #[test]
    fn no_runs_is_no_report() {
        assert!(StartupReport::new("a", StartKind::Cold, Vec::new()).is_none());
    }

    #[test]
    fn kinds_are_read_by_name() {
        assert_eq!(StartKind::parse("COLD"), Some(StartKind::Cold));
        assert_eq!(StartKind::parse("warm"), Some(StartKind::Warm));
        assert_eq!(StartKind::parse("lukewarm"), None);
        assert_eq!(serde_json::to_string(&StartKind::Hot).unwrap(), "\"hot\"");
    }
}
