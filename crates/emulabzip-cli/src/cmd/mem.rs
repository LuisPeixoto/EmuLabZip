//! `emulabzip mem` and `emulabzip heapdump`

use std::path::PathBuf;

use anyhow::{Context, Result};
use emulabzip_core::Lab;

use crate::output;

pub async fn run(lab: &Lab, device: Option<&str>, app: &str, json: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let report = lab.memory(&device.target(), app).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!(
        "{}  {}",
        output::heading(&format!("{:.1} MB", report.total_kb as f64 / 1024.0)),
        output::dim(&format!("total {} · {} · pid {}", report.metric, report.app, report.pid))
    );
    let largest = report.categories.first().map_or(1, |c| c.kb.max(1));
    let width = report.categories.iter().map(|c| c.name.chars().count()).max().unwrap_or(0);
    for c in &report.categories {
        let bar = "█".repeat(((c.kb as f64 / largest as f64) * 30.0).ceil() as usize);
        println!("  {:<width$}  {:>10}  {}", c.name, format!("{:.1} MB", c.kb as f64 / 1024.0), output::dim(&bar));
    }
    if !report.details.is_empty() {
        println!();
        for (k, v) in &report.details {
            println!("  {}  {v}", output::dim(&format!("{k:<width$}")));
        }
    }
    Ok(())
}

pub async fn heap_dump(lab: &Lab, device: Option<&str>, app: &str, out: Option<PathBuf>) -> Result<()> {
    let device = lab.resolve(device).await?;
    eprintln!("{}", output::dim(&format!("Capturing the heap of {app}…")));
    let dump = lab.heap_dump(&device.target(), app).await?;
    let path = out.unwrap_or_else(|| PathBuf::from(&dump.file_name));
    tokio::fs::write(&path, &dump.bytes).await.with_context(|| format!("could not write {}", path.display()))?;
    println!("Saved {} ({:.1} MB)", path.display(), dump.bytes.len() as f64 / 1_048_576.0);
    Ok(())
}
