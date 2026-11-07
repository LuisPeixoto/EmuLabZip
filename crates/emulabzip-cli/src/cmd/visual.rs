//! `emulabzip visual`: screenshots compared with a baseline, between two files, or between two sessions.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use emulabzip_audit::visual::{self, Comparison, Options, Region};
use emulabzip_core::{Control, Lab, StatusBar};
use emulabzip_session::config::{Config, VisualConfig};

use super::ci::Exit;
use crate::output;

/// Where baselines are kept, and how they are compared: the `[visual]` section of the project's `emulabzip.toml`, with
/// the command line over it.
pub struct Settings {
    pub dir: PathBuf,
    pub threshold: f64,
    pub options: Options,
}

pub fn settings(dir: Option<PathBuf>, threshold: Option<f64>, config: Option<&Path>) -> Result<Settings> {
    if let Some(t) = threshold
        && !(0.0..=100.0).contains(&t)
    {
        bail!("a threshold is a percentage between 0 and 100, not {t}");
    }
    let file = match config {
        Some(path) => Some(path.to_path_buf()),
        None => Config::find(Path::new(".")),
    };
    let visual: VisualConfig = match file {
        Some(path) => Config::load(&path)?.visual.unwrap_or_default(),
        None => VisualConfig::default(),
    };
    let ignore = visual.ignore.iter().map(|r| Region { x: r.x, y: r.y, width: r.width, height: r.height }).collect();
    Ok(Settings {
        dir: dir.unwrap_or_else(|| PathBuf::from(visual.baseline_dir.as_deref().unwrap_or("visual"))),
        threshold: threshold.unwrap_or(visual.threshold_percent),
        options: Options { tolerance: visual.tolerance, ignore },
    })
}

/// The screen, with a clean status bar (9:41, full battery, full signal) when asked, so the clock and the battery do not
/// show up as differences. The status bar is put back whatever happens.
async fn screenshot(lab: &Lab, device: Option<&str>, clean_status_bar: bool) -> Result<Vec<u8>> {
    let device = lab.resolve(device).await?;
    let target = device.target();
    if clean_status_bar {
        let bar = StatusBar { time: Some("9:41".into()), battery: Some(100), wifi: Some(true), cellular: Some(true) };
        lab.control(&target, &Control::StatusBar { status_bar: bar }).await?;
    }
    let shot = lab.screenshot(&target).await;
    if clean_status_bar {
        let _ = lab.control(&target, &Control::ClearStatusBar).await;
    }
    Ok(shot?)
}

fn describe(c: &Comparison, threshold: f64) -> String {
    let place =
        c.bounds.map(|b| format!(", in a {}×{} area at {},{}", b.width, b.height, b.x, b.y)).unwrap_or_default();
    format!(
        "{:.3}% of the screen changed ({} of {} pixels{place}); the limit is {threshold}%",
        c.percent, c.changed, c.compared
    )
}

fn report(name: &str, c: &Comparison, threshold: f64, diff: Option<&Path>, json: bool) -> Result<bool> {
    let over = c.exceeds(threshold);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({ "name": name, "comparison": c, "threshold": threshold, "exceeds": over })
            )?
        );
    } else if over {
        println!("{name}: {}", describe(c, threshold));
        if let Some(d) = diff {
            println!("{}", output::dim(&format!("The difference is drawn in {}", d.display())));
        }
    } else if c.changed == 0 {
        println!("{name}: identical");
    } else {
        println!("{name}: within the limit ({})", describe(c, threshold));
    }
    Ok(over)
}

fn write_diff(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes).with_context(|| format!("could not write {}", path.display()))
}

pub async fn capture(
    lab: &Lab,
    device: Option<&str>,
    name: &str,
    dir: Option<PathBuf>,
    status_bar: bool,
    replace: bool,
) -> Result<()> {
    if !visual::is_baseline_name(name) {
        bail!("`{name}` is not a baseline name: use letters, digits, . _ and -");
    }
    let s = settings(dir, None, None)?;
    let existing = visual::baseline_path(&s.dir, name)?;
    if existing.exists() && !replace {
        bail!("{} exists already; `emulabzip visual update {name} --yes` replaces it", existing.display());
    }
    let shot = screenshot(lab, device, status_bar).await?;
    let path = visual::save_baseline(&s.dir, name, &shot, replace)?;
    println!("Saved {}", path.display());
    Ok(())
}

pub async fn update(
    lab: &Lab,
    device: Option<&str>,
    name: &str,
    dir: Option<PathBuf>,
    status_bar: bool,
    yes: bool,
) -> Result<()> {
    if !yes {
        bail!("Accepting the screen as the new baseline replaces the old one. Add --yes to go ahead.");
    }
    capture(lab, device, name, dir, status_bar, true).await
}

#[allow(clippy::too_many_arguments)]
pub async fn compare(
    lab: &Lab,
    device: Option<&str>,
    name: &str,
    dir: Option<PathBuf>,
    threshold: Option<f64>,
    config: Option<PathBuf>,
    diff: Option<PathBuf>,
    status_bar: bool,
    json: bool,
) -> Result<()> {
    let s = settings(dir, threshold, config.as_deref())?;
    let baseline = visual::load_baseline(&s.dir, name)?;
    let shot = screenshot(lab, device, status_bar).await?;
    let done = visual::compare(&baseline, &shot, &s.options)?;
    let over = done.comparison.exceeds(s.threshold);
    let diff_path = diff.unwrap_or_else(|| s.dir.join("diffs").join(format!("{name}.png")));
    if done.comparison.changed > 0 {
        write_diff(&diff_path, &done.diff)?;
    }
    report(name, &done.comparison, s.threshold, (done.comparison.changed > 0).then_some(diff_path.as_path()), json)?;
    if over { Err(Exit(1).into()) } else { Ok(()) }
}

pub fn list(dir: Option<PathBuf>, json: bool) -> Result<()> {
    let s = settings(dir, None, None)?;
    let found = visual::list_baselines(&s.dir);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &found.iter().map(|(n, b)| serde_json::json!({ "name": n, "bytes": b })).collect::<Vec<_>>()
            )?
        );
        return Ok(());
    }
    if found.is_empty() {
        println!("No baselines in {}. `emulabzip visual capture <name>` makes one.", s.dir.display());
    }
    for (name, bytes) in found {
        println!("{name}  {}", output::dim(&output::size(bytes)));
    }
    Ok(())
}

/// Two screenshot files.
pub fn files(
    a: &Path,
    b: &Path,
    threshold: Option<f64>,
    config: Option<PathBuf>,
    diff: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let s = settings(None, threshold, config.as_deref())?;
    let read = |p: &Path| std::fs::read(p).with_context(|| format!("could not read {}", p.display()));
    let done = visual::compare(&read(a)?, &read(b)?, &s.options)?;
    let over = done.comparison.exceeds(s.threshold);
    if let (Some(path), true) = (&diff, done.comparison.changed > 0) {
        write_diff(path, &done.diff)?;
    }
    report(&format!("{} against {}", a.display(), b.display()), &done.comparison, s.threshold, diff.as_deref(), json)?;
    if over { Err(Exit(1).into()) } else { Ok(()) }
}

/// The screenshots of two recorded sessions, compared in the order they were taken.
pub fn sessions(
    a: &Path,
    b: &Path,
    threshold: Option<f64>,
    config: Option<PathBuf>,
    out: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let s = settings(None, threshold, config.as_deref())?;
    let (sa, sb) = (emulabzip_session::Session::open(a)?, emulabzip_session::Session::open(b)?);
    if sa.screens.is_empty() || sb.screens.is_empty() {
        bail!("both sessions need screenshots: record them with `--screens <seconds>`");
    }
    let mut results = Vec::new();
    let mut any_over = false;
    for (i, (na, nb)) in sa.screens.iter().zip(&sb.screens).enumerate() {
        let done = visual::compare(&sa.screen(na)?, &sb.screen(nb)?, &s.options);
        match done {
            Ok(d) => {
                let over = d.comparison.exceeds(s.threshold);
                any_over |= over;
                if let (Some(dir), true) = (&out, d.comparison.changed > 0) {
                    write_diff(&dir.join(format!("screen-{:03}.png", i + 1)), &d.diff)?;
                }
                results.push((i + 1, Some(d.comparison), None));
            }
            Err(e) => {
                any_over = true;
                results.push((i + 1, None, Some(e.to_string())));
            }
        }
    }
    if sa.screens.len() != sb.screens.len() {
        eprintln!(
            "{}",
            output::dim(&format!(
                "The sessions have {} and {} screenshots; the first {} were compared.",
                sa.screens.len(),
                sb.screens.len(),
                results.len()
            ))
        );
    }
    if json {
        let list: Vec<_> =
            results.iter().map(|(n, c, e)| serde_json::json!({ "screen": n, "comparison": c, "error": e })).collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else {
        for (n, c, e) in &results {
            match (c, e) {
                (Some(c), _) => println!(
                    "screen {n}: {}",
                    if c.changed == 0 { "identical".to_string() } else { describe(c, s.threshold) }
                ),
                (_, Some(e)) => println!("screen {n}: {e}"),
                _ => {}
            }
        }
    }
    if any_over { Err(Exit(1).into()) } else { Ok(()) }
}
