//! `emulabzip startup` and `emulabzip energy`

use std::time::Duration;

use anyhow::{Result, bail};
use emulabzip_core::control::is_app_id;
use emulabzip_core::energy::MAX_SECONDS;
use emulabzip_core::startup::MAX_RUNS;
use emulabzip_core::{EnergyReport, Lab, StartKind, StartupReport};

use crate::output;

pub fn parse_kind(text: &str) -> Result<StartKind, String> {
    StartKind::parse(text).ok_or_else(|| "use cold, warm or hot".to_string())
}

fn check_app(app: &str) -> Result<()> {
    if !is_app_id(app) {
        bail!("`{app}` is not a package name or bundle id");
    }
    Ok(())
}

pub fn print_startup(r: &StartupReport) {
    println!(
        "{} start of {}: median {} ms over {} {} (fastest {} ms, slowest {} ms, 90th percentile {} ms)",
        r.kind.label(),
        r.app,
        r.median_ms,
        r.runs.len(),
        if r.runs.len() == 1 { "run" } else { "runs" },
        r.min_ms,
        r.max_ms,
        r.p90_ms
    );
    for (i, run) in r.runs.iter().enumerate() {
        println!(
            "  {:>2}  {:>6} ms  {}",
            i + 1,
            run.total_ms,
            output::dim(run.launch_state.as_deref().map(|s| s.to_lowercase()).unwrap_or_default().as_str())
        );
    }
}

pub async fn startup(lab: &Lab, device: Option<&str>, app: &str, kind: StartKind, runs: u32, json: bool) -> Result<()> {
    // Checked before any device is looked for.
    check_app(app)?;
    if runs == 0 || runs > MAX_RUNS {
        bail!("--runs is between 1 and {MAX_RUNS}");
    }
    let device = lab.resolve(device).await?;
    eprintln!("{}", output::dim(&format!("Opening {app} {runs} times on {} ({} start)…", device.name, kind.label())));
    let report = lab.startup(&device.target(), app, kind, runs).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_startup(&report);
    }
    Ok(())
}

pub fn print_energy(r: &EnergyReport) {
    println!("{} over {} s", output::heading(&r.app), r.seconds);
    match r.estimated_mah {
        Some(mah) => println!("  Estimated use  {mah:.4} mAh"),
        None => println!("  Estimated use  {}", output::dim("none from the system")),
    }
    for c in &r.components {
        println!("    {:<14} {:.4} mAh", c.name, c.mah);
    }
    println!("  Processor      {:.1} s", r.cpu_ms as f64 / 1000.0);
    if r.network_rx_bytes + r.network_tx_bytes > 0 {
        println!(
            "  Network        {} received, {} sent",
            output::size(r.network_rx_bytes),
            output::size(r.network_tx_bytes)
        );
    }
    if r.alarms + r.jobs > 0 {
        println!("  Alarms, jobs   {}, {}", r.alarms, r.jobs);
    }
    for w in r.wakelocks.iter().take(5) {
        println!("  Wake lock      {} ({} ms, {} times)", w.tag, w.ms, w.count);
    }
    for n in &r.notes {
        println!("\n{}", output::dim(n));
    }
}

/// Counts the energy of an app for `seconds`, or until Ctrl+C. The device is always put back.
pub async fn energy(lab: &Lab, device: Option<&str>, app: &str, seconds: u64, json: bool) -> Result<()> {
    check_app(app)?;
    if seconds == 0 || seconds > MAX_SECONDS {
        bail!("--seconds is between 1 and {MAX_SECONDS}");
    }
    let target = lab.resolve(device).await?.target();
    lab.energy_begin(&target, app).await?;
    eprintln!(
        "{}",
        output::dim(&format!("Measuring {app} for {seconds} s. Use the app now; Ctrl+C reads what there is."))
    );
    let started = std::time::Instant::now();
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(seconds)) => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    // The read restores the device, whether it works or not.
    let report = lab.energy_end(&target, app, started.elapsed().as_secs()).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_energy(&report);
    }
    Ok(())
}
