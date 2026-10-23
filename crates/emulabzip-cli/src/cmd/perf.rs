//! `emulabzip perf`

use anyhow::Result;
use emulabzip_core::{Lab, PerfOptions, PerfSample};

use crate::output;

fn opt<T>(v: Option<T>, f: impl Fn(T) -> String) -> String {
    v.map(f).unwrap_or_else(|| "—".to_string())
}

fn mb(kb: u64) -> String {
    format!("{:.1} MB", kb as f64 / 1024.0)
}

fn row(s: &PerfSample) -> String {
    format!(
        "{:>7}  {:>8}  {:>8}  {:>10}  {:>10}  {:>6}  {:>6}  {:>7}",
        format!("{:.1}s", s.t_ms as f64 / 1000.0),
        opt(s.app_cpu, |v| format!("{v:.1}%")),
        opt(s.device_cpu, |v| format!("{v:.1}%")),
        opt(s.app_memory_kb, mb),
        opt(s.device_memory_used_kb, mb),
        opt(s.fps, |v| format!("{v:.0}")),
        opt(s.jank_percent, |v| format!("{v:.0}%")),
        opt(s.battery_level, |v| format!("{v}%")),
    )
}

pub async fn run(lab: &Lab, device: Option<&str>, app: Option<String>, interval_ms: u32, json: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let mut stream = lab.perf(&device.target(), PerfOptions { app: app.clone(), interval_ms }).await?;
    if !json {
        let subject = app.as_deref().unwrap_or("device-wide");
        eprintln!(
            "{}",
            output::dim(&format!(
                "Sampling {subject} on {} every {interval_ms} ms. Press Ctrl+C to stop.",
                device.name
            ))
        );
        println!(
            "{}",
            output::heading(&format!(
                "{:>7}  {:>8}  {:>8}  {:>10}  {:>10}  {:>6}  {:>6}  {:>7}",
                "TIME", "APP CPU", "CPU", "APP MEM", "MEM USED", "FPS", "JANK", "BATTERY"
            ))
        );
    }
    loop {
        tokio::select! {
            sample = stream.next() => {
                let Some(sample) = sample else { break };
                if json {
                    println!("{}", serde_json::to_string(&sample)?);
                } else {
                    println!("{}", row(&sample));
                }
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}
