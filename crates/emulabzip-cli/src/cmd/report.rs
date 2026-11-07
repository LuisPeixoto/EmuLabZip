//! `emulabzip report`: one file with what a bug report needs, and a link that opens the issue.

use std::path::PathBuf;

use anyhow::{Result, bail};
use emulabzip_core::Lab;
use emulabzip_report::issue::{Tracker, issue_url};
use emulabzip_report::{BugReport, Options, html, markdown};
use emulabzip_session::Session;
use emulabzip_symbols::store::Store;

use crate::output;

pub struct Args {
    pub session: Option<PathBuf>,
    pub app: Option<String>,
    pub output: PathBuf,
    pub title: Option<String>,
    pub description: Option<String>,
    pub issue: Option<String>,
    pub without: Vec<String>,
    pub open: bool,
}

pub fn parse_without(text: &str) -> Result<String, String> {
    match text {
        "screenshot" | "logs" | "requests" | "crashes" => Ok(text.to_string()),
        other => Err(format!("`{other}` is not a part of the report: use screenshot, logs, requests or crashes")),
    }
}

fn options(args: &Args) -> Options {
    let mut o = Options::everything();
    o.title = args.title.clone().unwrap_or_default();
    o.description = args.description.clone().unwrap_or_default();
    for w in &args.without {
        match w.as_str() {
            "screenshot" => o.include_screenshot = false,
            "logs" => o.include_logs = false,
            "requests" => o.include_requests = false,
            _ => o.include_crashes = false,
        }
    }
    o
}

fn default_title(r: &BugReport) -> String {
    match (r.crashes.first(), &r.app) {
        (Some(c), _) => c.title.clone(),
        (None, Some(app)) => format!("Bug in {app}"),
        _ => "Bug report".into(),
    }
}

pub async fn run(lab: &Lab, device: Option<&str>, args: Args) -> Result<()> {
    // Everything that can be checked without a device is checked first.
    let tracker = args.issue.as_deref().map(Tracker::parse).transpose()?;
    if args.session.is_some() && args.app.is_some() {
        bail!("--app is for a report of the device as it is now; a session already knows its app");
    }
    if let Some(parent) = args.output.parent().filter(|p| !p.as_os_str().is_empty())
        && !parent.is_dir()
    {
        bail!("{} is not a folder", parent.display());
    }
    let opts = options(&args);
    let store = Store::default();
    let mut report = match &args.session {
        Some(file) => {
            let session = Session::open(file).map_err(|e| anyhow::anyhow!("could not read {}: {e}", file.display()))?;
            BugReport::from_session(&session, &opts, &store)
        }
        None => {
            let device = lab.resolve(device).await?;
            BugReport::collect(lab, &device.target(), args.app.as_deref(), &[], &opts, &store).await
        }
    };
    if report.title.trim().is_empty() {
        report.title = default_title(&report);
    }
    std::fs::write(&args.output, html::html(&report))?;
    println!("{} {}", output::heading("Report written to"), args.output.display());
    for n in &report.notes {
        println!("  {}", output::dim(n));
    }
    if let Some(t) = &tracker {
        let url = issue_url(t, &report.title, &markdown::markdown(&report));
        println!("\n{}\n{url}", output::heading("Open the issue (attach the HTML file to it):"));
        if args.open {
            output::open_browser(&url)?;
        }
    }
    if args.open {
        output::open_browser(&args.output.canonicalize().unwrap_or(args.output.clone()).to_string_lossy())?;
    }
    Ok(())
}
