//! `emulabzip ci`: record a session around a command, such as a test run.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use emulabzip_core::Lab;
use emulabzip_session::{Options, Recorder, Session, markdown, summarize};
use tokio::process::Command;

use super::session::Network;
use crate::output;

/// An exit code to end with, after what was needed has been printed.
#[derive(Debug)]
pub struct Exit(pub u8);

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit code {}", self.0)
    }
}

impl std::error::Error for Exit {}

pub struct Args {
    pub out: PathBuf,
    pub app: Option<String>,
    pub network: bool,
    pub port: u16,
    pub screens: Option<u64>,
    pub fail_on_crash: bool,
    pub summary: Option<PathBuf>,
    pub command: Vec<String>,
}

pub async fn run(lab: &Lab, device: Option<&str>, args: Args) -> Result<()> {
    let Some((program, rest)) = args.command.split_first() else {
        bail!(
            "give the command to run after --, for example: emulabzip ci -o run.ezsession -- ./gradlew connectedCheck"
        );
    };
    if args.out.exists() {
        bail!("{} exists already. Pick another name, or delete it.", args.out.display());
    }
    let device = lab.resolve(device).await?;
    let network = if args.network { Some(Network::start(lab, &device, args.port).await?) } else { None };
    let options = Options {
        app: args.app.clone(),
        screens_every: args.screens.map(Duration::from_secs),
        flows: network.as_ref().map(|n| n.store.clone()),
        ..Default::default()
    };
    let recorder = match Recorder::start(lab.clone(), &device, args.out.clone(), options).await {
        Ok(r) => r,
        Err(e) => {
            if let Some(n) = network {
                n.stop(lab).await;
            }
            return Err(e.into());
        }
    };
    eprintln!("{}", output::dim(&format!("Recording {} while running: {}", device.name, args.command.join(" "))));

    let mut child = Command::new(program)
        .args(rest)
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("could not run `{program}`"));
    let code = match &mut child {
        Ok(child) => tokio::select! {
            status = child.wait() => status.map(|s| s.code().unwrap_or(1)).unwrap_or(1),
            _ = tokio::signal::ctrl_c() => {
                let _ = child.kill().await;
                130
            }
        },
        Err(_) => 127,
    };
    let crashed = recorder.new_crashes();
    let written = recorder.finish().await;
    if let Some(n) = network {
        n.stop(lab).await;
    }
    child?;
    let path = written?;

    let session = Session::open(&path)?;
    let summary = summarize(&session);
    let report = markdown(&summary);
    let mut notes = Vec::new();
    if let Some(file) = &args.summary {
        std::fs::write(file, &report).with_context(|| format!("could not write {}", file.display()))?;
        notes.push(format!("summary in {}", file.display()));
    }
    // GitHub Actions shows this file on the run's page.
    if let Some(step) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(step) {
            let _ = writeln!(f, "{report}");
        }
    }
    eprintln!(
        "Saved {} ({} crashes, {} log errors{})",
        path.display(),
        summary.crashes.len(),
        summary.logs.errors,
        notes.iter().map(|n| format!(", {n}")).collect::<String>()
    );

    if code != 0 {
        return Err(Exit(code.clamp(1, 255) as u8).into());
    }
    if args.fail_on_crash && !crashed.is_empty() {
        eprintln!("error: {} crash(es) happened during the run, so it fails (--fail-on-crash).", crashed.len());
        for c in &summary.crashes {
            eprintln!("  {}: {}", c.kind, c.title);
        }
        return Err(Exit(1).into());
    }
    Ok(())
}
