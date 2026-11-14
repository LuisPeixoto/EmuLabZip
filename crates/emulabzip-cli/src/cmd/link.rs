//! `emulabzip link`: open a deep link, check that the domain and the app agree about it, and keep a history.

use anyhow::{Result, bail};
use emulabzip_core::control::is_app_id;
use emulabzip_core::links::{self, Finding, Level};
use emulabzip_core::{Control, Lab};
use emulabzip_net::links::{Report, check};
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::output;

fn mark(level: Level) -> String {
    match level {
        Level::Pass => "✓".if_supports_color(Stdout, |t| t.green()).to_string(),
        Level::Info => "·".if_supports_color(Stdout, |t| t.dimmed()).to_string(),
        Level::Warn => "!".if_supports_color(Stdout, |t| t.yellow()).to_string(),
        Level::Fail => "✗".if_supports_color(Stdout, |t| t.red()).to_string(),
    }
}

pub fn print_finding(f: &Finding) {
    println!("{} {}", mark(f.level), output::heading(&f.title));
    println!("    {}", f.detail);
    if let Some(fix) = &f.fix {
        println!("    {} {fix}", output::dim("fix:"));
    }
}

pub fn print_report(r: &Report) {
    println!("{} {}", output::heading(&r.url), output::dim(&format!("({:?})", r.platform).to_lowercase()));
    if let Some(file) = &r.file_url {
        println!("{}", output::dim(&format!("domain file: {file}")));
    }
    r.findings.iter().for_each(print_finding);
    if !r.domains.is_empty() {
        println!("\n{}", output::heading("Domains the device decided about"));
        for d in &r.domains {
            println!("  {:<40} {}", d.domain, d.state);
        }
    }
}

/// Checks a link and an app before any device is looked for.
fn validate(url: &str, app: Option<&str>) -> Result<()> {
    Control::OpenUrl { url: url.to_string() }.validate()?;
    if let Some(app) = app
        && !is_app_id(app)
    {
        bail!("`{app}` is not a package name or bundle id");
    }
    Ok(())
}

pub async fn open(lab: &Lab, device: Option<&str>, url: &str, app: Option<&str>) -> Result<()> {
    validate(url, app)?;
    let device = lab.resolve(device).await?;
    lab.control(&device.target(), &Control::OpenUrl { url: url.to_string() }).await?;
    let _ = links::note_link(url, app, None);
    println!("Link opened {}", output::dim(&format!("({})", device.name)));
    Ok(())
}

pub async fn run_check(lab: &Lab, device: Option<&str>, url: &str, app: Option<&str>, json: bool) -> Result<()> {
    validate(url, app)?;
    let device = lab.resolve(device).await?;
    let report = check(lab, &device.target(), url, app).await?;
    let _ = links::note_link(url, app, Some(report.worst));
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    if report.worst == Level::Fail {
        bail!("the link check found problems");
    }
    Ok(())
}

pub async fn verify(lab: &Lab, device: Option<&str>, app: &str) -> Result<()> {
    validate("https://example.com", Some(app))?;
    let device = lab.resolve(device).await?;
    lab.control(&device.target(), &Control::VerifyAppLinks { app: app.to_string() }).await?;
    println!(
        "The device was asked to verify {app} again. Check in a few seconds: emulabzip link check URL --app {app}"
    );
    Ok(())
}

pub fn history(json: bool) -> Result<()> {
    let list = links::load_history();
    if json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    if list.is_empty() {
        println!("No links yet. `emulabzip link open URL` and `link check URL` remember them.");
    }
    for h in &list {
        let result = h.result.map_or(" ".to_string(), mark);
        let pin = if h.pinned { "📌 " } else { "" };
        println!("{result} {pin}{}  {}", h.url, output::dim(h.app.as_deref().unwrap_or("")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_and_an_app_are_checked_before_any_device_is_looked_up() {
        assert!(validate("https://example.com/x", Some("dev.x.app")).is_ok());
        assert!(validate("javascript:alert(1)", None).is_err());
        assert!(validate("https://example.com", Some("no way")).is_err());
    }
}
