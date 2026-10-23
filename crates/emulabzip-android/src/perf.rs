//! Performance sampling: CPU from `/proc`, memory, frames from `dumpsys gfxinfo`, and battery.
//!
//! These are the same sources Android Studio's transport daemon reads, queried with one shell
//! call per sample instead of an on-device agent.

use std::time::Instant;

use emulabzip_adb::{AdbClient, quote};
use emulabzip_core::perf::percent;
use emulabzip_core::{PerfOptions, PerfSample, Stream};

use crate::info::{SEPARATOR, key_values};

/// The shell script for one sample. With an app, its pid is resolved on every sample so a
/// restarted app is picked up again.
pub fn command(app: Option<&str>) -> String {
    let mut s = String::new();
    match app {
        Some(app) => {
            let pkg = quote(app);
            s.push_str(&format!(
                "pid=$(pidof {pkg} 2>/dev/null | cut -d' ' -f1); echo \"pid=$pid\"; echo {SEPARATOR}; "
            ));
            s.push_str(&format!("head -n1 /proc/stat; echo {SEPARATOR}; "));
            s.push_str(&format!("[ -n \"$pid\" ] && cat /proc/$pid/stat; echo {SEPARATOR}; "));
            s.push_str(&format!("[ -n \"$pid\" ] && grep VmRSS /proc/$pid/status; echo {SEPARATOR}; "));
            s.push_str(&format!("grep -E 'MemTotal|MemAvailable' /proc/meminfo; echo {SEPARATOR}; "));
            s.push_str(&format!(
                "[ -n \"$pid\" ] && dumpsys gfxinfo {pkg} | grep -E 'Total frames rendered|Janky frames:'; echo {SEPARATOR}; "
            ));
        }
        None => {
            s.push_str(&format!(
                "echo pid=; echo {SEPARATOR}; head -n1 /proc/stat; echo {SEPARATOR}; echo {SEPARATOR}; "
            ));
            s.push_str(&format!("echo {SEPARATOR}; grep -E 'MemTotal|MemAvailable' /proc/meminfo; echo {SEPARATOR}; "));
            s.push_str(&format!("echo {SEPARATOR}; "));
        }
    }
    s.push_str("dumpsys battery | grep -E 'level|temperature'");
    s
}

/// The parsed output of [`command`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    pub pid: Option<u32>,
    /// Total and idle jiffies across all cores.
    pub cpu_total: Option<(u64, u64)>,
    /// User plus system jiffies of the app.
    pub app_jiffies: Option<u64>,
    pub app_rss_kb: Option<u64>,
    pub mem_total_kb: Option<u64>,
    pub mem_available_kb: Option<u64>,
    /// Cumulative frames rendered and janky frames.
    pub frames: Option<(u64, u64)>,
    pub battery_level: Option<u8>,
    pub battery_temp_tenths: Option<i32>,
}

/// `cpu  17941 7791 24539 711536 518 4393 679 0 0 0`: total and idle (idle + iowait) jiffies.
pub fn parse_proc_stat(line: &str) -> Option<(u64, u64)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "cpu" {
        return None;
    }
    let values: Vec<u64> = parts.take(8).filter_map(|v| v.parse().ok()).collect();
    if values.len() < 5 {
        return None;
    }
    Some((values.iter().sum(), values[3] + values[4]))
}

/// User plus system time from `/proc/<pid>/stat`. The process name may contain spaces and
/// parentheses, so fields are counted after the last `)`.
pub fn parse_pid_stat(line: &str) -> Option<u64> {
    let rest = &line[line.rfind(')')? + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // Field 3 (state) is index 0 here, so utime (14) and stime (15) are 11 and 12.
    Some(fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?)
}

fn kb(value: &str) -> Option<u64> {
    value.split_whitespace().next()?.parse().ok()
}

pub fn parse(output: &str) -> Reading {
    let parts: Vec<&str> = output.split(SEPARATOR).map(str::trim).collect();
    let part = |i: usize| parts.get(i).copied().unwrap_or("");
    let mut r = Reading {
        pid: part(0).strip_prefix("pid=").and_then(|p| p.trim().parse().ok()),
        cpu_total: part(1).lines().next().and_then(parse_proc_stat),
        app_jiffies: part(2).lines().next().and_then(parse_pid_stat),
        ..Default::default()
    };
    for (k, v) in key_values(part(3)) {
        if k == "VmRSS" {
            r.app_rss_kb = kb(v);
        }
    }
    for (k, v) in key_values(part(4)) {
        match k {
            "MemTotal" => r.mem_total_kb = kb(v),
            "MemAvailable" => r.mem_available_kb = kb(v),
            _ => {}
        }
    }
    let mut total = None;
    let mut janky = None;
    for (k, v) in key_values(part(5)) {
        match k {
            "Total frames rendered" => total = kb(v),
            "Janky frames" => janky = kb(v),
            _ => {}
        }
    }
    r.frames = total.zip(janky);
    for (k, v) in key_values(part(6)) {
        match k {
            "level" => r.battery_level = v.parse().ok(),
            "temperature" => r.battery_temp_tenths = v.parse().ok(),
            _ => {}
        }
    }
    r
}

/// Turns successive readings into samples. CPU and frame rates need the previous reading.
#[derive(Debug)]
pub struct Sampler {
    start: Instant,
    prev: Option<(Instant, Reading)>,
}

impl Default for Sampler {
    fn default() -> Self {
        Sampler::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        Sampler { start: Instant::now(), prev: None }
    }

    pub fn next(&mut self, now: Instant, r: Reading) -> PerfSample {
        let mut s = PerfSample {
            t_ms: now.duration_since(self.start).as_millis() as u64,
            pid: r.pid,
            app_memory_kb: r.app_rss_kb,
            device_memory_total_kb: r.mem_total_kb,
            device_memory_used_kb: r.mem_total_kb.zip(r.mem_available_kb).map(|(t, a)| t.saturating_sub(a)),
            battery_level: r.battery_level,
            battery_temp_c: r.battery_temp_tenths.map(|t| t as f32 / 10.0),
            ..Default::default()
        };

        if let Some((then, prev)) = &self.prev {
            let elapsed = now.duration_since(*then).as_secs_f32();
            if let (Some((t1, i1)), Some((t0, i0))) = (r.cpu_total, prev.cpu_total)
                && t1 > t0
            {
                let total = t1 - t0;
                s.device_cpu = percent(total.saturating_sub(i1.saturating_sub(i0)), total);
                // Only compare app counters from the same process.
                if r.pid.is_some()
                    && r.pid == prev.pid
                    && let (Some(a1), Some(a0)) = (r.app_jiffies, prev.app_jiffies)
                {
                    s.app_cpu = percent(a1.saturating_sub(a0), total);
                }
            }
            if r.pid.is_some()
                && r.pid == prev.pid
                && let (Some((f1, j1)), Some((f0, j0))) = (r.frames, prev.frames)
                && f1 >= f0
                && elapsed > 0.0
            {
                let frames = f1 - f0;
                s.fps = Some(frames as f32 / elapsed);
                s.jank_percent =
                    Some(if frames == 0 { 0.0 } else { j1.saturating_sub(j0) as f32 / frames as f32 * 100.0 });
            }
        }
        self.prev = Some((now, r));
        s
    }
}

/// Samples the device every `options.interval()` until the stream is dropped.
pub fn stream(adb: AdbClient, serial: String, options: PerfOptions) -> Stream<PerfSample> {
    Stream::spawn(16, move |tx| async move {
        let cmd = command(options.app.as_deref());
        let mut sampler = Sampler::new();
        let mut tick = tokio::time::interval(options.interval());
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let Ok(out) = adb.shell(&serial, &cmd).await else { break };
            let sample = sampler.next(Instant::now(), parse(&out));
            if tx.send(sample).await.is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn output(pid: u32, cpu: (u64, u64), app: u64, frames: (u64, u64)) -> String {
        let s = SEPARATOR;
        let busy = cpu.0 - cpu.1;
        format!(
            "pid={pid}\n{s}\ncpu  {busy} 0 0 {} 0 0 0 0 0 0\n{s}\n{pid} (ndroid.settings) S 461 461 0 0 -1 4194624 28663 0 5613 0 {app} 0 0 0 20\n{s}\nVmRSS:\t  184156 kB\n{s}\nMemTotal: 4013812 kB\nMemAvailable: 2166592 kB\n{s}\nTotal frames rendered: {}\nJanky frames: {} (13.73%)\n{s}\n  level: 87\n  temperature: 250\n",
            cpu.1, frames.0, frames.1
        )
    }

    #[test]
    fn parses_proc_stat_and_pid_stat() {
        assert_eq!(parse_proc_stat("cpu  10 0 10 70 10 0 0 0 0 0"), Some((100, 80)));
        assert_eq!(parse_proc_stat("cpu0 1 2 3 4 5"), None);
        let line = "1158 (Some (odd) name) S 461 461 0 0 -1 4194624 28663 0 5613 0 215 195 0 0 20";
        assert_eq!(parse_pid_stat(line), Some(410));
    }

    #[test]
    fn parses_a_reading() {
        let r = parse(&output(1158, (1000, 800), 50, (102, 14)));
        assert_eq!(r.pid, Some(1158));
        assert_eq!(r.cpu_total, Some((1000, 800)));
        assert_eq!(r.app_jiffies, Some(50));
        assert_eq!(r.app_rss_kb, Some(184156));
        assert_eq!(r.mem_total_kb, Some(4013812));
        assert_eq!(r.frames, Some((102, 14)));
        assert_eq!(r.battery_level, Some(87));
        assert_eq!(r.battery_temp_tenths, Some(250));
    }

    #[test]
    fn parses_a_reading_without_app() {
        let s = SEPARATOR;
        let out = format!(
            "pid=\n{s}\ncpu  1 0 0 9 0 0 0 0\n{s}\n{s}\n{s}\nMemTotal: 100 kB\nMemAvailable: 40 kB\n{s}\n{s}\nlevel: 50\n"
        );
        let r = parse(&out);
        assert_eq!(r.pid, None);
        assert_eq!(r.app_jiffies, None);
        assert_eq!(r.mem_available_kb, Some(40));
        assert_eq!(r.battery_level, Some(50));
    }

    #[test]
    fn computes_rates_between_readings() {
        let mut s = Sampler::new();
        let t0 = Instant::now();
        let first = s.next(t0, parse(&output(7, (1000, 800), 100, (100, 10))));
        assert_eq!(first.app_cpu, None);
        assert_eq!(first.device_memory_used_kb, Some(4013812 - 2166592));
        assert_eq!(first.battery_temp_c, Some(25.0));

        // 200 jiffies passed, 100 of them idle; the app used 50; 60 frames in 1 s, 6 janky.
        let second = s.next(t0 + Duration::from_secs(1), parse(&output(7, (1200, 900), 150, (160, 16))));
        assert_eq!(second.device_cpu, Some(50.0));
        assert_eq!(second.app_cpu, Some(25.0));
        assert_eq!(second.fps.map(f32::round), Some(60.0));
        assert_eq!(second.jank_percent, Some(10.0));
    }

    #[test]
    fn a_restarted_app_resets_its_rates() {
        let mut s = Sampler::new();
        let t0 = Instant::now();
        s.next(t0, parse(&output(7, (1000, 800), 100, (100, 10))));
        let after = s.next(t0 + Duration::from_secs(1), parse(&output(8, (1200, 900), 5, (3, 0))));
        assert_eq!(after.app_cpu, None);
        assert_eq!(after.fps, None);
        assert_eq!(after.device_cpu, Some(50.0));
    }

    #[test]
    fn quotes_the_package_in_the_command() {
        assert!(command(Some("com.example.app")).contains("pidof com.example.app"));
        assert!(command(Some("x; reboot")).contains("pidof 'x; reboot'"));
        assert!(!command(None).contains("pidof"));
    }
}
