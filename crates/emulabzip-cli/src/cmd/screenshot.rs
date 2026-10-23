//! `emulabzip screenshot`

use std::path::PathBuf;

use anyhow::{Context, Result};
use emulabzip_core::Lab;

pub async fn run(lab: &Lab, device: Option<&str>, out: Option<PathBuf>) -> Result<()> {
    let device = lab.resolve(device).await?;
    let png = lab.screenshot(&device.target()).await?;
    let path = out.unwrap_or_else(|| {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let name: String = device.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        PathBuf::from(format!("{name}-{secs}.png"))
    });
    tokio::fs::write(&path, &png).await.with_context(|| format!("could not write {}", path.display()))?;
    println!("Saved {} ({} KB)", path.display(), png.len() / 1024);
    Ok(())
}
