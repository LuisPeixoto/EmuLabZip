//! `emulabzip flow`: record what is touched on a device, play it again, and export it to Maestro or a script.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use emulabzip_core::Lab;
use emulabzip_core::automation::{self, Flow, Step};
use emulabzip_core::control::is_app_id;

use crate::output;

/// The longest a recording runs unless it is stopped, so a forgotten one ends.
pub const MAX_SECONDS: u64 = 3_600;

/// A flow named on the command line: a path to a file, or the id of a saved flow.
pub fn open(name: &str) -> Result<Flow> {
    let path = Path::new(name);
    if name.ends_with(".json") || name.contains('/') || name.contains('\\') {
        return Ok(automation::read(path)?);
    }
    automation::load(name)
        .with_context(|| format!("no saved flow is called `{name}`; `emulabzip flow list` shows them"))
}

pub fn describe(step: &Step) -> String {
    match step {
        Step::Launch => "launch the app".into(),
        Step::Tap { x, y, label } => match label {
            Some(l) => format!("tap {} ({x}, {y})", l.text.as_deref().or(l.id.as_deref()).unwrap_or("")),
            None => format!("tap ({x}, {y})"),
        },
        Step::LongPress { x, y, ms, .. } => format!("long press ({x}, {y}) for {ms} ms"),
        Step::Swipe { x1, y1, x2, y2, .. } => format!("swipe ({x1}, {y1}) to ({x2}, {y2})"),
        Step::Text { text } => format!("type {text:?}"),
        Step::Key { key } => format!("press {key}"),
        Step::Wait { ms } => format!("wait {ms} ms"),
    }
}

pub fn check_record(name: &str, app: Option<&str>, seconds: Option<u64>) -> Result<()> {
    if name.trim().is_empty() || name.len() > 100 {
        bail!("the name is between 1 and 100 characters");
    }
    if let Some(app) = app
        && !is_app_id(app)
    {
        bail!("`{app}` is not a package name or bundle id");
    }
    if let Some(s) = seconds
        && (s == 0 || s > MAX_SECONDS)
    {
        bail!("--seconds is between 1 and {MAX_SECONDS}");
    }
    Ok(())
}

pub async fn record(
    lab: &Lab,
    device: Option<&str>,
    name: &str,
    app: Option<&str>,
    launch: bool,
    seconds: Option<u64>,
    out: Option<PathBuf>,
) -> Result<()> {
    check_record(name, app, seconds)?;
    let device = lab.resolve(device).await?;
    eprintln!(
        "{}",
        output::dim("Touch the screen of the device (on the emulator window). Press Ctrl+C when you are done.")
    );
    let stop = async {
        match seconds {
            Some(s) => tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(s)) => {},
                _ = tokio::signal::ctrl_c() => {},
            },
            None => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    };
    let flow = automation::record(lab, &device.target(), name, app, launch, stop, |step| {
        eprintln!("  {}", describe(step));
    })
    .await?;
    if flow.steps.iter().all(|s| matches!(s, Step::Launch | Step::Wait { .. })) {
        bail!("nothing was touched, so there is nothing to save");
    }
    let path = match out {
        Some(path) => {
            std::fs::write(&path, flow.to_json())?;
            path
        }
        None => automation::save(&flow)?,
    };
    println!("Saved {} steps to {}", flow.steps.len(), path.display());
    Ok(())
}

pub fn list(json: bool) -> Result<()> {
    let flows = automation::list();
    if json {
        println!("{}", serde_json::to_string_pretty(&flows)?);
        return Ok(());
    }
    if flows.is_empty() {
        println!("No flows yet. `emulabzip flow record --name NAME --app APP` makes one.");
    }
    for f in &flows {
        println!(
            "{}  {}  {}",
            output::heading(&f.id),
            f.app.as_deref().unwrap_or("-"),
            output::dim(&format!("{} steps", f.steps))
        );
    }
    Ok(())
}

pub fn show(name: &str, json: bool) -> Result<()> {
    let flow = open(name)?;
    if json {
        print!("{}", flow.to_json());
        return Ok(());
    }
    println!("{} {}", output::heading(&flow.name), output::dim(flow.app.as_deref().unwrap_or("")));
    for (i, step) in flow.steps.iter().enumerate() {
        println!("  {:>3}  {}", i + 1, describe(step));
    }
    Ok(())
}

pub fn export(name: &str, maestro: bool, out: Option<PathBuf>) -> Result<()> {
    let flow = open(name)?;
    let text = if maestro { flow.to_maestro() } else { flow.to_script() };
    match out {
        Some(path) => {
            std::fs::write(&path, &text)?;
            #[cfg(unix)]
            if !maestro {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
            }
            eprintln!("Saved {}", path.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}

pub async fn play(lab: &Lab, device: Option<&str>, name: &str) -> Result<()> {
    // Read and check the flow before any device is looked for.
    let flow = open(name)?;
    let device = lab.resolve(device).await?;
    let total = flow.steps.len();
    automation::play(
        lab,
        &device.target(),
        &flow,
        async {
            let _ = tokio::signal::ctrl_c().await;
        },
        |i, step| eprintln!("  {:>3}/{total}  {}", i + 1, describe(step)),
    )
    .await?;
    println!("Played {} {}", flow.name, output::dim(&format!("({})", device.name)));
    Ok(())
}

pub async fn remove(name: &str) -> Result<()> {
    automation::remove(name)?;
    println!("Removed {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_is_checked_before_any_device_is_looked_up() {
        assert!(check_record("Login", Some("dev.x.app"), Some(60)).is_ok());
        assert!(check_record("  ", None, None).is_err());
        assert!(check_record("x", Some("not an app"), None).is_err());
        assert!(check_record("x", None, Some(0)).is_err());
        assert!(check_record("x", None, Some(MAX_SECONDS + 1)).is_err());
    }

    #[test]
    fn a_flow_named_with_a_path_or_a_wrong_id_is_refused() {
        assert!(open("../../etc/passwd").is_err());
        assert!(open("no-such-flow").is_err());
        assert!(open("/no/such/file.json").is_err());
    }

    #[test]
    fn steps_are_described_in_words() {
        assert_eq!(describe(&Step::Tap { x: 1, y: 2, label: None }), "tap (1, 2)");
        assert_eq!(describe(&Step::Wait { ms: 500 }), "wait 500 ms");
        assert_eq!(describe(&Step::Launch), "launch the app");
    }
}
