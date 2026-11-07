//! `emulabzip state`: save an app's data to one file, put it back, look inside, compare two.

use std::path::Path;

use anyhow::{Result, bail};
use emulabzip_core::Lab;
use emulabzip_storage::snapshot;

use crate::output;

/// Refuses what needs a yes before any device is looked up.
pub fn need_yes(yes: bool, what: &str) -> Result<()> {
    if !yes {
        bail!("{what} changes the device. Add --yes to go ahead.");
    }
    Ok(())
}

pub async fn save(lab: &Lab, device: Option<&str>, app: &str, out: &Path, external: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let meta = snapshot::save(lab, &device.target(), app, out, external).await?;
    println!(
        "Saved {} from {}: {} files, {} → {}",
        meta.app,
        meta.device,
        meta.files,
        output::size(meta.bytes),
        out.display()
    );
    Ok(())
}

pub async fn restore(lab: &Lab, device: Option<&str>, file: &Path, yes: bool) -> Result<()> {
    need_yes(yes, "Restoring a state replaces the app's data and")?;
    // The file says what it is for before the device is asked about.
    let state = snapshot::open(file)?;
    let device = lab.resolve(device).await?;
    let restored = snapshot::restore(lab, &device.target(), file, true).await?;
    for warning in &restored.warnings {
        eprintln!("warning: {warning}");
    }
    println!("Restored {} on {}.", state.meta.app, device.name);
    println!(
        "{}",
        output::dim(&format!("The data it replaced is in {} (restore that file to undo).", restored.backup.display()))
    );
    Ok(())
}

pub fn show(file: &Path, json: bool) -> Result<()> {
    let state = snapshot::open(file)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&state.meta)?);
        return Ok(());
    }
    let m = &state.meta;
    println!("{} {}", output::heading(&m.app), m.app_version.as_deref().unwrap_or(""));
    println!(
        "{} · {} · {} files, {}{}",
        m.platform,
        m.device,
        m.files,
        output::size(m.bytes),
        if m.external { " · with external storage" } else { "" }
    );
    for f in &state.schemas()?.files {
        let what =
            if f.tables.is_empty() { format!("{} keys", f.keys.len()) } else { format!("{} tables", f.tables.len()) };
        println!("  {}  {}", f.path, output::dim(&what));
    }
    Ok(())
}

pub async fn diff(lab: &Lab, device: Option<&str>, a: &Path, b: Option<&Path>, json: bool) -> Result<()> {
    let changes = match b {
        Some(b) => snapshot::compare(a, b)?,
        None => {
            // Against the app as it is now: take a state to compare, and remove it again.
            let before = snapshot::open(a)?;
            let device = lab.resolve(device).await?;
            let now =
                std::env::temp_dir().join(format!("emulabzip-now-{}.{}", std::process::id(), snapshot::EXTENSION));
            let _ = std::fs::remove_file(&now);
            snapshot::save(lab, &device.target(), &before.meta.app, &now, false).await?;
            let result = snapshot::compare(a, &now);
            let _ = std::fs::remove_file(&now);
            result?
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&changes)?);
    } else if changes.is_empty() {
        println!("No difference in the structure of the data.");
    } else {
        for c in &changes {
            println!("{}", c.line());
        }
    }
    Ok(())
}
