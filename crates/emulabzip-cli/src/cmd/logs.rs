//! `emulabzip logs`

use std::time::Duration;

use anyhow::{Context, Result};
use emulabzip_core::{Lab, LogFilter, LogLevel, LogOptions, Target};

use crate::output;

pub struct Args {
    pub device: Option<String>,
    pub app: Option<String>,
    pub level: Option<LogLevel>,
    pub tag: Option<String>,
    pub grep: Option<String>,
    pub regex: bool,
    pub tail: u32,
    pub json: bool,
}

/// Process ids of `app`, refreshed periodically because apps restart and get a new pid.
async fn app_pids(lab: &Lab, target: &Target, app: &str) -> Vec<u32> {
    lab.apps(target)
        .await
        .map(|apps| apps.iter().filter(|a| a.id == app).filter_map(|a| a.pid).collect())
        .unwrap_or_default()
}

pub async fn run(lab: &Lab, args: Args) -> Result<()> {
    let device = lab.resolve(args.device.as_deref()).await?;
    let target = device.target();
    let mut filter =
        LogFilter { min_level: args.level, pids: Vec::new(), tag: args.tag, text: args.grep, regex: args.regex };
    if let Some(app) = &args.app {
        filter.pids = app_pids(lab, &target, app).await;
        if filter.pids.is_empty() {
            eprintln!("{}", output::dim(&format!("{app} is not running yet; waiting for it to start…")));
        }
    }
    let mut matcher = filter.compile().context("invalid --grep pattern")?;

    if !args.json {
        eprintln!(
            "{}",
            output::dim(&format!("Following {} ({}). Press Ctrl+C to stop.", device.name, device.os_label()))
        );
    }
    let mut stream = lab.logs(&target, LogOptions { tail: args.tail, verbose: true }).await?;
    let mut refresh = tokio::time::interval(Duration::from_secs(3));
    refresh.tick().await;

    loop {
        tokio::select! {
            entry = stream.next() => {
                let Some(entry) = entry else { break };
                // With --app and no process yet, drop everything until the app shows up.
                if args.app.is_some() && filter.pids.is_empty() {
                    continue;
                }
                if !matcher.matches(&entry) {
                    continue;
                }
                if args.json {
                    println!("{}", serde_json::to_string(&entry)?);
                } else {
                    println!("{}", output::log_line(&entry));
                }
            }
            _ = refresh.tick(), if args.app.is_some() => {
                let pids = app_pids(lab, &target, args.app.as_deref().unwrap_or_default()).await;
                if pids != filter.pids {
                    filter.pids = pids;
                    matcher = filter.compile()?;
                }
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}
