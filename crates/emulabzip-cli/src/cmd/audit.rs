//! `emulabzip audit security`: what an app declares, sends, logs and stores that it should not.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use emulabzip_audit::collect::{Sources, collect};
use emulabzip_audit::security::{self, traffic::Request};
use emulabzip_audit::{Area, Audit, Severity};
use emulabzip_core::Lab;

use super::ci::Exit;
use crate::output;

/// Requests from a HAR file, such as one `emulabzip net --har` or a session wrote.
pub fn requests_from_har(path: &Path) -> Result<Vec<Request>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    let har: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not a HAR file", path.display()))?;
    let entries = har["log"]["entries"].as_array().with_context(|| format!("{} has no log.entries", path.display()))?;
    Ok(entries
        .iter()
        .filter_map(|e| {
            let r = &e["request"];
            Some(Request {
                method: r["method"].as_str()?.to_string(),
                url: r["url"].as_str()?.to_string(),
                body: r["postData"]["text"].as_str().map(str::to_string),
            })
        })
        .collect())
}

pub struct SecurityArgs {
    pub app: String,
    pub har: Option<PathBuf>,
    /// Which areas to look at; all when empty.
    pub only: Vec<String>,
    pub json: bool,
    pub fail_on: Option<Severity>,
}

fn sources(only: &[String], requests: Option<Vec<Request>>) -> Result<Sources> {
    if only.is_empty() {
        return Ok(Sources::all(requests));
    }
    let mut s = Sources { requests, ..Sources::default() };
    for area in only {
        match area.as_str() {
            "manifest" => s.manifest = true,
            "logs" | "log" => s.logs = true,
            "storage" => s.storage = true,
            "traffic" => {}
            other => bail!("`{other}` is not an area: use manifest, logs, storage or traffic"),
        }
    }
    Ok(s)
}

pub async fn security(lab: &Lab, device: Option<&str>, args: SecurityArgs) -> Result<()> {
    if !emulabzip_core::control::is_app_id(&args.app) {
        bail!("`{}` is not a package name or bundle id", args.app);
    }
    let requests = args.har.as_deref().map(requests_from_har).transpose()?;
    let what = sources(&args.only, requests)?;
    let device = lab.resolve(device).await?;
    let input = collect(lab, &device.target(), &args.app, what).await;
    let audit = security::audit(&input);
    print_audit(&audit, args.json)?;
    match args.fail_on {
        Some(level) if audit.fails(level) => Err(Exit(1).into()),
        _ => Ok(()),
    }
}

pub fn print_audit(audit: &Audit, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(audit)?);
        return Ok(());
    }
    for f in &audit.findings {
        let tag = match f.severity {
            Severity::Error => "error  ",
            Severity::Warning => "warning",
            Severity::Info => "note   ",
        };
        println!("{tag} {:<28} {}", f.rule, f.message);
        let mut detail = Vec::new();
        if let Some(l) = &f.location {
            detail.push(l.clone());
        }
        if let Some(e) = &f.evidence {
            detail.push(format!("saw: {e}"));
        }
        detail.push(f.fix.clone());
        println!("        {}", output::dim(&detail.join(" — ")));
    }
    if audit.findings.is_empty() {
        println!("Nothing found in what was looked at.");
    }
    let looked: Vec<_> = audit.checked.iter().map(|a| a.label()).collect();
    eprintln!(
        "{}",
        output::dim(&format!(
            "Looked at: {}. {} errors, {} warnings, {} notes.",
            if looked.is_empty() { "nothing".to_string() } else { looked.join(", ") },
            audit.count(Severity::Error),
            audit.count(Severity::Warning),
            audit.count(Severity::Info)
        ))
    );
    for (area, why) in &audit.skipped {
        eprintln!("{}", output::dim(&format!("Could not read the {}: {why}", area.label())));
    }
    let _ = Area::Log;
    Ok(())
}
