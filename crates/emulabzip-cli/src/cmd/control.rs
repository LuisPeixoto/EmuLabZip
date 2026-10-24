//! `emulabzip control`

use std::path::PathBuf;

use anyhow::{Context, Result};
use emulabzip_core::{Appearance, Control, Grant, Lab, StatusBar};

use crate::output;

/// Applies a control to the device and says what was done.
pub async fn apply(lab: &Lab, device: Option<&str>, control: Control, done: &str) -> Result<()> {
    let device = lab.resolve(device).await?;
    lab.control(&device.target(), &control).await?;
    println!("{done} {}", output::dim(&format!("({})", device.name)));
    Ok(())
}

pub fn status_bar(time: Option<String>, battery: Option<u8>, wifi: Option<bool>, cellular: Option<bool>) -> StatusBar {
    StatusBar { time, battery, wifi, cellular }
}

pub fn appearance(dark: bool) -> Appearance {
    if dark { Appearance::Dark } else { Appearance::Light }
}

pub fn grant(action: &str) -> Result<Grant, String> {
    match action {
        "grant" => Ok(Grant::Grant),
        "revoke" => Ok(Grant::Revoke),
        "reset" => Ok(Grant::Reset),
        other => Err(format!("`{other}` is not an action: use grant, revoke or reset")),
    }
}

pub fn read_payload(path: Option<PathBuf>) -> Result<Option<String>> {
    path.map(|p| std::fs::read_to_string(&p).with_context(|| format!("could not read {}", p.display()))).transpose()
}

pub async fn info(lab: &Lab, device: Option<&str>, app: Option<&str>, json: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let info = lab.controls(&device.target(), app).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }
    println!("{}", output::heading(&device.name));
    if let Some(a) = info.appearance {
        println!("  appearance  {}", if a == Appearance::Dark { "dark" } else { "light" });
    }
    println!("  controls    {}", info.supported.join(", "));
    if !info.permissions.is_empty() {
        println!("\n{}", output::heading("Permissions"));
        for p in &info.permissions {
            println!("  {} {}", if p.granted { "granted" } else { "denied " }, p.name);
        }
    }
    for n in &info.notes {
        println!("{}", output::dim(&format!("\n{n}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_grant_actions() {
        assert_eq!(grant("revoke"), Ok(Grant::Revoke));
        assert!(grant("allow").is_err());
    }

    #[test]
    fn reads_a_payload_file() {
        assert_eq!(read_payload(None).unwrap(), None);
        assert!(read_payload(Some(PathBuf::from("/no/such/file.json"))).is_err());
    }
}
