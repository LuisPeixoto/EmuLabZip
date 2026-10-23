//! Performance sampling for simulator apps.
//!
//! A simulator app is an ordinary macOS process, so its CPU time and resident memory are read on
//! the host with `ps`. Frame rate and battery do not exist for simulators.

use std::time::Instant;

use emulabzip_core::{PerfOptions, PerfSample, Stream};
use tokio::process::Command;

use crate::{apps, simctl};

/// Parses `ps -o cputime=`: `0:01.67`, `12:03.50`, `1:02:03.45` or `2-01:02:03.45`, in seconds.
pub fn parse_cputime(text: &str) -> Option<f64> {
    let text = text.trim();
    let (days, rest) = match text.split_once('-') {
        Some((d, rest)) => (d.parse::<f64>().ok()?, rest),
        None => (0.0, text),
    };
    let mut seconds = 0.0;
    for part in rest.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(days * 86_400.0 + seconds)
}

/// Parses `ps -o cputime=,rss=` for one process: CPU seconds and resident kilobytes.
pub fn parse_ps(text: &str) -> Option<(f64, u64)> {
    let mut parts = text.split_whitespace();
    Some((parse_cputime(parts.next()?)?, parts.next()?.parse().ok()?))
}

async fn ps(pid: u32) -> Option<(f64, u64)> {
    let out = Command::new("ps").args(["-p", &pid.to_string(), "-o", "cputime=,rss="]).output().await.ok()?;
    out.status.success().then(|| parse_ps(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// The pid of a running app, from `launchctl list` inside the simulator.
pub async fn app_pid(udid: &str, bundle: &str) -> Option<u32> {
    let out = simctl::run_text(["spawn", udid, "launchctl", "list"]).await.ok()?;
    apps::parse_running(&out).get(bundle).copied()
}

/// Turns successive `ps` readings into samples.
#[derive(Debug)]
pub struct Sampler {
    start: Instant,
    cores: f64,
    prev: Option<(Instant, u32, f64)>,
}

impl Sampler {
    pub fn new(cores: usize) -> Self {
        Sampler { start: Instant::now(), cores: cores.max(1) as f64, prev: None }
    }

    pub fn next(&mut self, now: Instant, pid: Option<u32>, reading: Option<(f64, u64)>) -> PerfSample {
        let mut s = PerfSample { t_ms: now.duration_since(self.start).as_millis() as u64, pid, ..Default::default() };
        match (pid, reading) {
            (Some(pid), Some((cpu, rss))) => {
                s.app_memory_kb = Some(rss);
                if let Some((then, prev_pid, prev_cpu)) = self.prev
                    && prev_pid == pid
                {
                    let wall = now.duration_since(then).as_secs_f64();
                    if wall > 0.0 {
                        let share = (cpu - prev_cpu).max(0.0) / (wall * self.cores) * 100.0;
                        s.app_cpu = Some(share.clamp(0.0, 100.0) as f32);
                    }
                }
                self.prev = Some((now, pid, cpu));
            }
            _ => self.prev = None,
        }
        s
    }
}

pub fn stream(udid: String, options: PerfOptions) -> Stream<PerfSample> {
    Stream::spawn(16, move |tx| async move {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let mut sampler = Sampler::new(cores);
        let mut tick = tokio::time::interval(options.interval());
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut pid = None;
        let mut ticks = 0u32;
        loop {
            tick.tick().await;
            // Resolve the pid when unknown, and every few samples to notice restarts.
            if let Some(app) = &options.app
                && (pid.is_none() || ticks.is_multiple_of(5))
            {
                pid = app_pid(&udid, app).await;
            }
            ticks = ticks.wrapping_add(1);
            let reading = match pid {
                Some(p) => ps(p).await,
                None => None,
            };
            if reading.is_none() {
                pid = None;
            }
            if tx.send(sampler.next(Instant::now(), pid, reading)).await.is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn parses_cpu_times() {
        assert_eq!(parse_cputime("0:01.67"), Some(1.67));
        assert_eq!(parse_cputime("12:03.50"), Some(723.5));
        assert_eq!(parse_cputime("1:02:03.45"), Some(3723.45));
        assert_eq!(parse_cputime("2-00:00:01.00"), Some(172_801.0));
        assert_eq!(parse_cputime("soon"), None);
    }

    #[test]
    fn parses_ps_output() {
        assert_eq!(parse_ps("  0:01.67 150756\n"), Some((1.67, 150756)));
        assert_eq!(parse_ps(""), None);
    }

    #[test]
    fn computes_cpu_share_of_all_cores() {
        let mut s = Sampler::new(4);
        let t0 = Instant::now();
        assert_eq!(s.next(t0, Some(7), Some((10.0, 1000))).app_cpu, None);
        // One core fully busy for one second out of four cores is 25%.
        let second = s.next(t0 + Duration::from_secs(1), Some(7), Some((11.0, 1200)));
        assert_eq!(second.app_cpu, Some(25.0));
        assert_eq!(second.app_memory_kb, Some(1200));
    }

    #[test]
    fn a_new_process_starts_over() {
        let mut s = Sampler::new(2);
        let t0 = Instant::now();
        s.next(t0, Some(7), Some((10.0, 1000)));
        assert_eq!(s.next(t0 + Duration::from_secs(1), Some(8), Some((0.5, 900))).app_cpu, None);
        assert_eq!(s.next(t0 + Duration::from_secs(2), None, None).app_memory_kb, None);
    }
}
