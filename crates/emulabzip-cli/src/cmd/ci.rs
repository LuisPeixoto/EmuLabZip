//! `emulabzip ci`: record a session around a command, such as a test run.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use emulabzip_core::Lab;
use emulabzip_session::{Config, Options, Recorder, Session, baseline, budget, markdown, summarize};
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
    /// The `emulabzip.toml` to hold the run to. `./emulabzip.toml` is used when there is one.
    pub config: Option<PathBuf>,
    /// An earlier session to compare with.
    pub baseline: Option<PathBuf>,
    pub fail_on_regression: bool,
    pub video: Option<u32>,
    pub taps: bool,
    pub replay: Option<super::net::ReplayArgs>,
    /// Fail the run when the app asked for something the replayed recording does not hold.
    pub fail_on_miss: bool,
    /// A build (APK, AAB or IPA) to hold to the `[size]` limits of emulabzip.toml.
    pub size: Option<PathBuf>,
    /// An earlier build to measure growth against.
    pub size_baseline: Option<PathBuf>,
    /// Audit the app's security and privacy after the command, and fail on a finding at least this serious.
    pub fail_on_security: Option<emulabzip_core::Severity>,
    /// Compare the screen after the command with this baseline.
    pub visual: Option<String>,
    pub fail_on_visual: bool,
    pub command: Vec<String>,
}

/// A build's size, checked before anything runs: a file that cannot be read should not cost a test run.
fn size_check(args: &Args, config: &Config) -> Result<Option<Vec<emulabzip_audit::package::Breach>>> {
    let Some(file) = &args.size else { return Ok(None) };
    let Some(limits) = &config.size else {
        bail!("--size checks the limits of the [size] section of emulabzip.toml, and there is none");
    };
    let package = emulabzip_audit::package::analyze_file(file)?;
    let baseline = args.size_baseline.as_deref().map(emulabzip_audit::package::analyze_file).transpose()?;
    let limits = super::size::limits(limits);
    Ok(Some(emulabzip_audit::package::check(&package, &limits, baseline.as_ref())))
}

/// The requests the proxy recorded, as the security audit reads them.
fn requests_of(flows: &[emulabzip_net::flow::Flow]) -> Vec<emulabzip_audit::security::traffic::Request> {
    flows
        .iter()
        .map(|f| emulabzip_audit::security::traffic::Request {
            method: f.method.clone(),
            url: f.url.clone(),
            body: std::str::from_utf8(&f.request_body).ok().map(str::to_string),
        })
        .collect()
}

/// The settings of the project, and the session to compare with: both read before any device is touched.
fn settings(args: &Args) -> Result<(Config, Option<emulabzip_session::Summary>)> {
    let config = match &args.config {
        Some(path) => Config::load(path)?,
        None => match Config::find(&std::env::current_dir()?) {
            Some(path) => Config::load(&path)?,
            None => Config::default(),
        },
    };
    if config.startup.is_some() && args.app.is_none() {
        bail!("emulabzip.toml asks for the start time to be measured, which needs the app: pass --app");
    }
    if args.fail_on_security.is_some() && args.app.is_none() {
        bail!("--fail-on-security audits an app, so it needs --app");
    }
    if args.fail_on_visual && args.visual.is_none() {
        bail!("--fail-on-visual needs --visual <baseline name> to compare with");
    }
    if args.size_baseline.is_some() && args.size.is_none() {
        bail!("--size-baseline needs --size <build> to measure");
    }
    if args.fail_on_regression && args.baseline.is_none() {
        bail!("--fail-on-regression needs --baseline <earlier.ezsession> to compare with");
    }
    let baseline = match &args.baseline {
        Some(path) => Some(summarize(
            &Session::open(path).with_context(|| format!("could not read the baseline {}", path.display()))?,
        )),
        None => None,
    };
    Ok((config, baseline))
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
    if args.replay.is_some() && !args.network {
        bail!("--replay answers the app's requests through the proxy, so it needs --network");
    }
    if args.fail_on_miss && args.replay.is_none() {
        bail!("--fail-on-miss needs --replay <recording> to check against");
    }
    let replay = args.replay.as_ref().map(super::net::load_replay).transpose()?;
    let (config, baseline_summary) = settings(&args)?;
    let size_breaches = size_check(&args, &config)?;
    let visual_settings = match &args.visual {
        Some(name) => {
            let s = super::visual::settings(None, None, args.config.as_deref())?;
            let baseline = emulabzip_audit::visual::load_baseline(&s.dir, name)?;
            Some((s, baseline))
        }
        None => None,
    };
    let device = lab.resolve(device).await?;
    // The start time is measured first, so the openings are not part of the recording.
    let startup = match (&config.startup, &args.app) {
        (Some(s), Some(app)) => {
            eprintln!(
                "{}",
                output::dim(&format!("Measuring the start time of {app} ({} runs, {})…", s.runs, s.kind.as_str()))
            );
            let report = lab.startup(&device.target(), app, s.kind()?, s.runs).await?;
            super::startup::print_startup(&report);
            Some(report)
        }
        _ => None,
    };
    if let Some(set) = &replay {
        eprintln!("{}", output::dim(&super::net::replay_banner(set)));
    }
    let network =
        if args.network { Some(Network::start(lab, &device, args.port, replay.clone()).await?) } else { None };
    let options = Options {
        app: args.app.clone(),
        screens_every: args.screens.map(Duration::from_secs),
        flows: network.as_ref().map(|n| n.store.clone()),
        startup,
        video: args.video,
        taps: args.taps,
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
    // What the checks after the run look at is read while the device and the proxy are still in place.
    let flows = network.as_ref().map(|n| n.store.all()).unwrap_or_default();
    let security = match (args.fail_on_security, &args.app) {
        (Some(_), Some(app)) => {
            let input = emulabzip_audit::collect::collect(
                lab,
                &device.target(),
                app,
                emulabzip_audit::collect::Sources::all(network.as_ref().map(|_| requests_of(&flows))),
            )
            .await;
            Some(emulabzip_audit::security::audit(&input))
        }
        _ => None,
    };
    let visual = match &visual_settings {
        Some((s, baseline)) => match lab.screenshot(&device.target()).await {
            Ok(shot) => Some(emulabzip_audit::visual::compare(baseline, &shot, &s.options).map(|c| c.comparison)),
            Err(e) => Some(Err(e)),
        },
        None => None,
    };
    let written = recorder.finish().await;
    if let Some(n) = network {
        n.stop(lab).await;
    }
    child?;
    let path = written?;

    let session = Session::open(&path)?;
    let summary = summarize(&session);
    let violations = budget::check(&summary, &config.budget);
    let tolerance = config.baseline.tolerance_percent;
    let changes = baseline_summary.as_ref().map(|b| baseline::compare(b, &summary, tolerance)).unwrap_or_default();
    let regressions = baseline::regressions(&changes).len();
    let report = format!(
        "{}{}{}{}",
        markdown(&summary),
        budget::markdown(&violations, budget::count(&config.budget)),
        baseline::markdown(&changes, tolerance),
        quality_markdown(size_breaches.as_deref(), security.as_ref(), visual.as_ref())
    );
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

    if let Some(set) = &replay {
        super::net::print_replay_stats(set);
    }
    if code != 0 {
        return Err(Exit(code.clamp(1, 255) as u8).into());
    }
    if args.fail_on_miss
        && let Some(set) = &replay
        && set.stats().misses > 0
    {
        eprintln!(
            "error: the app asked for {} thing(s) that are not in the recording, so the run fails (--fail-on-miss).",
            set.stats().misses
        );
        return Err(Exit(1).into());
    }
    if args.fail_on_crash && !crashed.is_empty() {
        eprintln!("error: {} crash(es) happened during the run, so it fails (--fail-on-crash).", crashed.len());
        for c in &summary.crashes {
            eprintln!("  {}: {}", c.kind, c.title);
        }
        return Err(Exit(1).into());
    }
    if !violations.is_empty() {
        eprintln!("error: {} limit(s) of emulabzip.toml were broken:", violations.len());
        for v in &violations {
            eprintln!(
                "  {}: allowed {}, was {}",
                budget::label(&v.metric),
                budget::number(v.limit, &v.unit),
                budget::number(v.actual, &v.unit)
            );
        }
        return Err(Exit(1).into());
    }
    if let Some(breaches) = size_breaches.as_deref().filter(|b| !b.is_empty()) {
        eprintln!("error: the build is over {} size limit(s) of emulabzip.toml:", breaches.len());
        for b in breaches {
            eprintln!("  {}: allowed {}, was {}", b.what, b.limit, b.actual);
        }
        return Err(Exit(1).into());
    }
    if let (Some(level), Some(audit)) = (args.fail_on_security, &security)
        && audit.fails(level)
    {
        let bad: Vec<_> = audit.findings.iter().filter(|f| f.severity >= level).collect();
        eprintln!(
            "error: the security audit found {} problem(s) at {} or worse (--fail-on-security):",
            bad.len(),
            level.label()
        );
        for f in bad {
            eprintln!("  {}: {}", f.rule, f.message);
        }
        return Err(Exit(1).into());
    }
    if args.fail_on_visual
        && let (Some(name), Some((s, _))) = (&args.visual, &visual_settings)
    {
        match &visual {
            Some(Ok(c)) if c.exceeds(s.threshold) => {
                eprintln!(
                    "error: the screen differs from the baseline {name}: {:.3}% changed, the limit is {}% (--fail-on-visual).",
                    c.percent, s.threshold
                );
                return Err(Exit(1).into());
            }
            Some(Err(e)) => {
                eprintln!("error: the screen could not be compared with the baseline {name}: {e}");
                return Err(Exit(1).into());
            }
            _ => {}
        }
    }
    if args.fail_on_regression && regressions > 0 {
        eprintln!(
            "error: {regressions} number(s) got worse than the baseline by more than {tolerance:.0}% (--fail-on-regression):"
        );
        for c in baseline::regressions(&changes) {
            eprintln!(
                "  {}: {} before, {} now",
                budget::label(&c.metric),
                budget::number(c.before, &c.unit),
                budget::number(c.after, &c.unit)
            );
        }
        return Err(Exit(1).into());
    }
    Ok(())
}

/// The checks after the run, for the summary: only the ones that were asked for.
fn quality_markdown(
    size: Option<&[emulabzip_audit::package::Breach]>,
    security: Option<&emulabzip_audit::Audit>,
    visual: Option<&emulabzip_core::Result<emulabzip_audit::visual::Comparison>>,
) -> String {
    let mut out = String::new();
    if let Some(breaches) = size {
        out.push_str("\n### Build size\n\n");
        if breaches.is_empty() {
            out.push_str("Within every `[size]` limit.\n");
        } else {
            out.push_str("| Limit | Allowed | Actual |\n|---|---|---|\n");
            for b in breaches {
                out.push_str(&format!("| {} | {} | {} |\n", b.what, b.limit, b.actual));
            }
        }
    }
    if let Some(audit) = security {
        out.push_str("\n### Security and privacy\n\n");
        if audit.findings.is_empty() {
            out.push_str("Nothing found.\n");
        } else {
            out.push_str("| Severity | Rule | What |\n|---|---|---|\n");
            for f in audit.findings.iter().take(30) {
                out.push_str(&format!("| {} | {} | {} |\n", f.severity.label(), f.rule, f.message.replace('|', "\\|")));
            }
        }
    }
    if let Some(result) = visual {
        out.push_str("\n### Visual comparison\n\n");
        match result {
            Ok(c) if c.changed == 0 => out.push_str("The screen is identical to the baseline.\n"),
            Ok(c) => out.push_str(&format!("{:.3}% of the screen changed ({} pixels).\n", c.percent, c.changed)),
            Err(e) => out.push_str(&format!("Could not compare: {e}\n")),
        }
    }
    out
}
