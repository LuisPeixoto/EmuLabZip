//! `emulabzip apps`

use anyhow::Result;
use emulabzip_core::Lab;

use crate::output::{self, cell};

pub async fn run(lab: &Lab, device: Option<&str>, all: bool, running: bool, json: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let apps: Vec<_> = lab
        .apps(&device.target())
        .await?
        .into_iter()
        .filter(|a| all || a.user || (running && a.pid.is_some()))
        .filter(|a| !running || a.pid.is_some())
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&apps)?);
        return Ok(());
    }
    if apps.is_empty() {
        println!("No apps matched. Use --all to include system apps.");
        return Ok(());
    }
    let rows: Vec<_> = apps
        .iter()
        .map(|a| {
            vec![
                cell(a.id.clone()),
                cell(if a.name == a.id { String::new() } else { a.name.clone() }),
                cell(a.version.clone().unwrap_or_default()),
                cell(a.pid.map(|p| p.to_string()).unwrap_or_default()),
                cell(if a.user { "user" } else { "system" }),
            ]
        })
        .collect();
    print!("{}", output::table(&["ID", "NAME", "VERSION", "PID", "TYPE"], &rows));
    Ok(())
}
