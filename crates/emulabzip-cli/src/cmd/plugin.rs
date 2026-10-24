//! `emulabzip plugin`

use std::path::Path;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use emulabzip_core::Lab;
use emulabzip_plugin::{
    Approvals, Context, Event, Mode, Outcome, Plugin, discover, manifest, plugins_dir, runner, scaffold,
};
use serde_json::Value;
use tokio::sync::watch;

use crate::output;

fn find(name: &str) -> Result<Plugin> {
    let (plugins, problems) = discover();
    if let Some(p) = plugins.into_iter().find(|p| p.manifest.name == name) {
        return Ok(p);
    }
    match problems.iter().find(|p| p.dir.file_name().is_some_and(|n| n == name)) {
        Some(problem) => bail!("{name} cannot be used: {}", problem.message),
        None => bail!("there is no plugin called {name}. See `emulabzip plugin list`"),
    }
}

pub fn list(json: bool) -> Result<()> {
    let (plugins, problems) = discover();
    let approvals = Approvals::load();
    if json {
        let value: Vec<Value> = plugins
            .iter()
            .map(|p| serde_json::json!({ "manifest": p.manifest, "dir": p.dir, "enabled": approvals.is_approved(p) }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    if plugins.is_empty() && problems.is_empty() {
        println!(
            "{}",
            output::dim(&format!("No plugins in {}. Try `emulabzip plugin new my-plugin`.", plugins_dir().display()))
        );
    }
    for p in &plugins {
        let state = if approvals.is_approved(p) { "enabled " } else { "disabled" };
        let mode = if p.manifest.mode == Mode::Stream { "stream" } else { "once" };
        println!("{state}  {:<24} {:<7} {}", p.manifest.name, mode, p.manifest.description);
    }
    for p in &problems {
        println!("{}", output::dim(&format!("problem   {}: {}", p.dir.display(), p.message)));
    }
    Ok(())
}

pub fn new(name: &str) -> Result<()> {
    let dir = scaffold::scaffold(name)?;
    println!("Created {}", dir.display());
    println!(
        "{}",
        output::dim("Edit run.py, then `emulabzip plugin run` it. Enable it with `emulabzip plugin enable`.")
    );
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub fn install(folder: &Path, force: bool) -> Result<()> {
    let plugin = manifest::load(folder)
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("{} is not a plugin", folder.display()))?;
    let target = plugins_dir().join(&plugin.manifest.name);
    if target.exists() {
        if !force {
            bail!("{} is already installed. Use --force to replace it", plugin.manifest.name);
        }
        std::fs::remove_dir_all(&target)?;
    }
    copy_dir(folder, &target)?;
    println!("Installed {} in {}", plugin.manifest.name, target.display());
    println!("{}", output::dim("It is disabled until you enable it: `emulabzip plugin enable <name>`."));
    Ok(())
}

pub fn enable(name: &str, on: bool) -> Result<()> {
    let plugin = find(name)?;
    let mut approvals = Approvals::load();
    if on {
        println!(
            "{} runs `{}` as you, from {}.",
            plugin.manifest.name,
            plugin.manifest.command.join(" "),
            plugin.dir.display()
        );
        approvals.approve(&plugin)?;
        println!("Enabled {name}.");
    } else {
        approvals.revoke(name)?;
        println!("Disabled {name}.");
    }
    Ok(())
}

fn show(event: &Event, json: bool) {
    if json {
        if let Ok(line) = serde_json::to_string(event) {
            println!("{line}");
        }
        return;
    }
    let cell = |v: &Value| match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    match event {
        Event::Metric { name, value, unit } => {
            println!("{name} = {value}{}", unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default())
        }
        Event::Table { title, columns, rows } => {
            if let Some(t) = title {
                println!("{}", output::heading(t));
            }
            let widths: Vec<usize> = (0..columns.len())
                .map(|i| {
                    rows.iter()
                        .map(|r| r.get(i).map_or(0, |c| cell(c).chars().count()))
                        .chain([columns[i].chars().count()])
                        .max()
                        .unwrap_or(0)
                        .min(48)
                })
                .collect();
            let line = |cells: Vec<String>| {
                cells
                    .iter()
                    .zip(&widths)
                    .map(|(c, w)| format!("{:<w$}", c.chars().take(48).collect::<String>()))
                    .collect::<Vec<_>>()
                    .join("  ")
            };
            println!("{}", output::dim(&line(columns.clone())));
            for r in rows {
                println!("{}", line((0..columns.len()).map(|i| r.get(i).map(cell).unwrap_or_default()).collect()));
            }
        }
        Event::Text { title, body } => {
            if let Some(t) = title {
                println!("{}", output::heading(t));
            }
            println!("{body}");
        }
        Event::Log { level, message } => {
            eprintln!("{}", output::dim(&format!("[{}] {message}", level.as_deref().unwrap_or("info"))))
        }
        Event::Error { message } => eprintln!("error: {message}"),
    }
}

pub async fn run(
    lab: &Lab,
    device: Option<&str>,
    name: &str,
    app: Option<String>,
    seconds: Option<u64>,
    json: bool,
) -> Result<()> {
    let plugin = find(name)?;
    let device = lab.resolve(device).await?;
    if !plugin.manifest.supports(device.platform) {
        bail!("{name} does not support {}", device.platform.label());
    }
    let ctx = Context { device: device.id.clone(), platform: device.platform, app };
    let (stop, stopped) = watch::channel(false);
    let limit = match (plugin.manifest.mode, seconds) {
        (Mode::Stream, s) => s.map(Duration::from_secs),
        (Mode::Once, _) => None,
    };
    if plugin.manifest.mode == Mode::Stream && !json {
        eprintln!("{}", output::dim(&format!("Running {name} on {}. Press Ctrl+C to stop.", device.name)));
    }
    let failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let saw_error = failed.clone();
    let running = runner::run(&plugin, &ctx, stopped, move |e| {
        if matches!(e, Event::Error { .. }) {
            saw_error.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        show(&e, json);
    });
    tokio::pin!(running);
    let timer = async {
        match limit {
            Some(l) => tokio::time::sleep(l).await,
            None => std::future::pending().await,
        }
    };
    tokio::pin!(timer);
    let mut timed_out = false;
    let outcome = loop {
        tokio::select! {
            outcome = &mut running => break outcome?,
            // The timer is only polled until it fires: a finished sleep must not be awaited again.
            _ = &mut timer, if !timed_out => {
                timed_out = true;
                let _ = stop.send(true);
            }
            _ = tokio::signal::ctrl_c() => { let _ = stop.send(true); }
        }
    };
    if failed.load(std::sync::atomic::Ordering::Relaxed) || matches!(outcome, Outcome::Finished(Some(c)) if c != 0) {
        bail!("{name} failed");
    }
    Ok(())
}
