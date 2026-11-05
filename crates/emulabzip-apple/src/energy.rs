//! What an iOS Simulator app used over a stretch of time.
//!
//! The simulator has no battery and no power model, so there is no figure in mAh. What can be said is
//! how much processor time the app used, which is what drains a real battery first, and whether the
//! Mac was slowing its processor down while it ran (which makes the numbers less comparable).

use std::time::Instant;

use emulabzip_core::{EnergyReport, Error, Result};
use tokio::process::Command;

use crate::perf::{app_pid, parse_ps};

/// The CPU limit from `pmset -g therm`: 100 when nothing is held back.
pub fn parse_cpu_limit(text: &str) -> Option<u32> {
    text.lines().find_map(|l| {
        let l = l.trim();
        l.strip_prefix("CPU_Scheduler_Limit")?.split('=').nth(1)?.trim().parse().ok()
    })
}

/// Where a measurement started: when, the app's process and its CPU time.
#[derive(Debug, Clone)]
pub struct Began {
    at: Instant,
    pid: u32,
    cpu_seconds: f64,
}

async fn cpu_seconds(pid: u32) -> Option<f64> {
    let out = Command::new("ps").args(["-p", &pid.to_string(), "-o", "cputime=,rss="]).output().await.ok()?;
    out.status.success().then(|| parse_ps(&String::from_utf8_lossy(&out.stdout)).map(|p| p.0)).flatten()
}

pub async fn begin(udid: &str, app: &str) -> Result<Began> {
    let pid = app_pid(udid, app).await.ok_or_else(|| Error::NotFound(format!("{app} is not running")))?;
    let cpu = cpu_seconds(pid).await.ok_or_else(|| Error::NotFound(format!("{app} is not running")))?;
    Ok(Began { at: Instant::now(), pid, cpu_seconds: cpu })
}

pub async fn end(began: Began, app: &str) -> Result<EnergyReport> {
    let now = cpu_seconds(began.pid)
        .await
        .ok_or_else(|| Error::NotFound(format!("{app} stopped running during the measurement")))?;
    let seconds = began.at.elapsed().as_secs();
    let limit = Command::new("pmset")
        .args(["-g", "therm"])
        .output()
        .await
        .ok()
        .and_then(|o| parse_cpu_limit(&String::from_utf8_lossy(&o.stdout)));
    let mut notes = vec!["The iOS Simulator has no battery and no power model, so there is no figure in mAh. The processor time is what the app used.".to_string()];
    if let Some(l) = limit.filter(|l| *l < 100) {
        notes.push(format!(
            "The Mac held its processor back to {l}% while this ran, so the time is not comparable with a cool run."
        ));
    }
    Ok(EnergyReport {
        app: app.to_string(),
        seconds,
        estimated_mah: None,
        components: Vec::new(),
        cpu_ms: ((now - began.cpu_seconds).max(0.0) * 1000.0) as u64,
        wakelocks: Vec::new(),
        alarms: 0,
        jobs: 0,
        network_rx_bytes: 0,
        network_tx_bytes: 0,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_cpu_limit_from_pmset() {
        let text = "Note: No thermal warning level has been recorded\n2026-11-05 19:31:09 -0300 CPU Power notify\n\tCPU_Scheduler_Limit \t= 100\n\tCPU_Available_CPUs \t= 16\n";
        assert_eq!(parse_cpu_limit(text), Some(100));
        assert_eq!(parse_cpu_limit("\tCPU_Scheduler_Limit \t= 70\n"), Some(70));
        assert_eq!(parse_cpu_limit("nothing here"), None);
    }
}
