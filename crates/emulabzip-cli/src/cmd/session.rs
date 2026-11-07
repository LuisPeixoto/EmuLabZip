//! `emulabzip session`: record what happens on a device, and read it back.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use emulabzip_core::{Device, Lab, Target};
use emulabzip_net::{FlowStore, Proxy, ProxyConfig, ProxyHandle};
use emulabzip_session::{EXTENSION, Options, Recorder, Session, markdown, summarize};

use crate::output;

pub struct RecordArgs {
    pub out: PathBuf,
    pub app: Option<String>,
    pub network: bool,
    pub port: u16,
    pub screens: Option<u64>,
    pub duration: Option<u64>,
    /// Record the screen, in pieces of this many seconds.
    pub video: Option<u32>,
    /// Record touches (Android).
    pub taps: bool,
    /// Answer the app's requests from this recording while recording the new one.
    pub replay: Option<super::net::ReplayArgs>,
}

/// What `--video` accepts: whole seconds from 1 to 180, the longest `screenrecord` records at a time.
pub fn parse_video_secs(text: &str) -> Result<u32, String> {
    match text.parse::<u32>() {
        Ok(n) if (1..=emulabzip_core::MAX_SEGMENT_SECS).contains(&n) => Ok(n),
        _ => Err(format!("use a number of seconds from 1 to {}", emulabzip_core::MAX_SEGMENT_SECS)),
    }
}

/// The proxy of a recording that watches the network, and the device pointed at it.
pub struct Network {
    pub store: Arc<FlowStore>,
    proxy: ProxyHandle,
    target: Target,
}

impl Network {
    pub async fn start(
        lab: &Lab,
        device: &Device,
        port: u16,
        replay: Option<Arc<emulabzip_net::ReplaySet>>,
    ) -> Result<Network> {
        let store = FlowStore::new(emulabzip_net::store::DEFAULT_CAPACITY);
        let ca = super::net::load_ca()?;
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let proxy = Proxy::start(ProxyConfig { addr, replay, ..Default::default() }, ca, store.clone())
            .await
            .with_context(|| format!("could not listen on {addr}. Pick another port with --port"))?;
        let target = device.target();
        lab.connect_network(&target, proxy.addr().port()).await?;
        Ok(Network { store, proxy, target })
    }

    /// Puts the device back. Always called, so a failed run never leaves a device on a dead proxy.
    pub async fn stop(self, lab: &Lab) {
        if let Err(e) = lab.disconnect_network(&self.target).await {
            eprintln!("warning: could not disconnect {}: {e}", self.target.id);
        }
        drop(self.proxy);
    }
}

fn check_out(out: &Path) -> Result<()> {
    if out.extension().is_none_or(|e| e != EXTENSION) {
        bail!("the output file should end in .{EXTENSION}, for example run.{EXTENSION}");
    }
    if out.exists() {
        bail!("{} exists already. Pick another name, or delete it.", out.display());
    }
    Ok(())
}

pub async fn record(lab: &Lab, device: Option<&str>, args: RecordArgs) -> Result<()> {
    check_out(&args.out)?;
    if args.replay.is_some() && !args.network {
        bail!("--replay answers the app's requests through the proxy, so it needs --network");
    }
    // The recording is read before any device is looked for.
    let replay = args.replay.as_ref().map(super::net::load_replay).transpose()?;
    let device = lab.resolve(device).await?;
    if let Some(set) = &replay {
        eprintln!("{}", output::dim(&super::net::replay_banner(set)));
    }
    let network =
        if args.network { Some(Network::start(lab, &device, args.port, replay.clone()).await?) } else { None };
    let options = Options {
        app: args.app.clone(),
        screens_every: args.screens.map(Duration::from_secs),
        flows: network.as_ref().map(|n| n.store.clone()),
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
    eprintln!(
        "{}",
        output::dim(&format!(
            "Recording {}. Press Ctrl+C to stop{}.",
            device.name,
            args.duration.map(|s| format!(", or wait {s} s")).unwrap_or_default()
        ))
    );

    let started = std::time::Instant::now();
    // Without a duration this waits for ever: only Ctrl+C ends it.
    let end = tokio::time::sleep(args.duration.map_or(Duration::MAX / 4, Duration::from_secs));
    tokio::pin!(end);
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = &mut end => break,
            _ = tick.tick() => {
                let s = recorder.stats();
                let mut line = format!("  {} s: {} log lines, {} samples, {} crashes", started.elapsed().as_secs(), s.logs, s.samples, s.crashes);
                if args.taps {
                    line += &format!(", {} touches", s.taps);
                }
                if args.video.is_some() {
                    line += &format!(", {} video pieces", s.video_pieces);
                }
                eprintln!("{}", output::dim(&line));
            }
        }
    }
    let written = recorder.finish().await;
    if let Some(n) = network {
        n.stop(lab).await;
    }
    if let Some(set) = &replay {
        super::net::print_replay_stats(set);
    }
    let path = written?;
    println!("Saved {}", path.display());
    Ok(())
}

fn clock(ms: u64) -> String {
    let total = ms / 1000;
    format!("+{:02}:{:02}.{:03}", total / 60, total % 60, ms % 1000)
}

/// `emulabzip session timeline`: everything the session recorded, on one time axis.
pub fn timeline(path: &Path, json: bool, lanes: &[emulabzip_session::Lane]) -> Result<()> {
    let session = Session::open(path)?;
    let timeline = emulabzip_session::timeline::build(&session);
    let events = timeline.only(lanes);
    if json {
        let shown =
            emulabzip_session::Timeline { events: events.iter().map(|e| (*e).clone()).collect(), ..timeline.clone() };
        println!("{}", serde_json::to_string_pretty(&shown)?);
        return Ok(());
    }
    if events.is_empty() {
        println!("Nothing was recorded on {}.", if lanes.is_empty() { "the timeline" } else { "these lanes" });
    }
    for e in &events {
        let dur = e.dur_ms.filter(|d| *d > 0).map(|d| format!("  ({d} ms)")).unwrap_or_default();
        let line = format!("{}  {:<9} {}{dur}", clock(e.t_ms), e.lane.name(), e.label);
        match e.severity {
            emulabzip_session::Severity::Info => println!("{line}"),
            _ => println!(
                "{} {}",
                line,
                output::dim(if e.severity == emulabzip_session::Severity::Error { "!!" } else { "!" })
            ),
        }
    }
    let counts: Vec<String> = timeline
        .lanes
        .iter()
        .map(|l| {
            if l.shown < l.total {
                format!("{} {} (showing {})", l.total, l.lane.name(), l.shown)
            } else {
                format!("{} {}", l.total, l.lane.name())
            }
        })
        .collect();
    eprintln!("{}", output::dim(&format!("{} in {}.", counts.join(", "), clock(timeline.duration_ms))));
    if timeline.clock == emulabzip_session::ClockSource::Estimated {
        eprintln!("{}", output::dim("This recording did not measure the device's clock, so log times are estimated."));
    }
    Ok(())
}

/// `emulabzip session video`: writes the pieces of the screen recording into a folder.
pub fn video(path: &Path, out: &Path) -> Result<()> {
    let session = Session::open(path)?;
    if session.video.is_empty() {
        bail!("this session has no screen recording. Record with `emulabzip session record --video`.");
    }
    for piece in &session.video {
        let written = session.extract_video(&piece.file, out)?;
        println!(
            "{}  {}",
            written.display(),
            output::dim(&format!(
                "{} to {}",
                clock(piece.start_ms.saturating_sub(session.meta.started_ms)),
                clock(piece.end_ms.saturating_sub(session.meta.started_ms))
            ))
        );
    }
    Ok(())
}

/// What got better or worse between two sessions.
pub fn compare(baseline: &Path, run: &Path, tolerance: f64, json: bool) -> Result<()> {
    let (a, b) = (summarize(&Session::open(baseline)?), summarize(&Session::open(run)?));
    let changes = emulabzip_session::baseline::compare(&a, &b, tolerance);
    if json {
        println!("{}", serde_json::to_string_pretty(&changes)?);
    } else if changes.is_empty() {
        println!("These two sessions have no number in common to compare.");
    } else {
        for c in &changes {
            let mark = if c.regression {
                "worse "
            } else if c.after < c.before {
                "better"
            } else {
                "same  "
            };
            let pct = c.percent.map(|p| format!("{p:+.0}%")).unwrap_or_else(|| "new".into());
            println!(
                "{mark}  {:<20} {:>12} → {:<12} {}",
                emulabzip_session::budget::label(&c.metric),
                emulabzip_session::budget::number(c.before, &c.unit),
                emulabzip_session::budget::number(c.after, &c.unit),
                output::dim(&pct)
            );
        }
        eprintln!("{}", output::dim(&format!("A number counts as worse when it grew by more than {tolerance:.0}%.")));
    }
    if changes.iter().any(|c| c.regression) {
        return Err(super::ci::Exit(1).into());
    }
    Ok(())
}

pub fn show(path: &Path, json: bool, as_markdown: bool) -> Result<()> {
    let session = Session::open(path)?;
    let summary = summarize(&session);
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else if as_markdown {
        print!("{}", markdown(&summary));
    } else {
        print_text(&summary);
    }
    Ok(())
}

fn secs(ms: u64) -> String {
    format!("{:.1} s", ms as f64 / 1000.0)
}

fn print_text(s: &emulabzip_session::Summary) {
    println!(
        "{}",
        output::heading(&format!(
            "{}{}",
            s.device.name,
            s.app.as_ref().map(|a| format!("  ·  {a}")).unwrap_or_default()
        ))
    );
    if let Some(ms) = s.duration_ms {
        println!("  {}  {}", output::dim("Duration  "), secs(ms));
    }
    if s.partial {
        println!("  {}", output::dim("The recording was cut short; this is what it had when it stopped."));
    }
    println!(
        "  {}  {} errors, {} warnings, {} lines",
        output::dim("Logs      "),
        s.logs.errors,
        s.logs.warnings,
        s.logs.total
    );
    println!(
        "  {}  {}",
        output::dim("Crashes   "),
        if s.crashes.is_empty() { "none".to_string() } else { s.crashes.len().to_string() }
    );
    if let Some(n) = &s.network {
        println!("  {}  {} ({} failed)", output::dim("Requests  "), n.requests, n.failed);
    }
    if let Some(p) = &s.perf {
        if let (Some(avg), Some(peak)) = (p.cpu_avg, p.cpu_peak) {
            println!("  {}  {avg:.0}% average, {peak:.0}% peak", output::dim("CPU       "));
        }
        if let Some(kb) = p.memory_peak_kb {
            println!("  {}  {:.0} MB peak", output::dim("Memory    "), kb as f64 / 1024.0);
        }
    }
    for c in &s.crashes {
        println!("\n{} {}: {}", output::dim("crash"), c.kind, c.title);
    }
    if !s.top_errors.is_empty() {
        println!("\n{}", output::heading("Most repeated errors"));
        for e in &s.top_errors {
            println!("  {:>5}×  {}", e.count, e.message);
        }
    }
    if let Some(n) = s.network.as_ref().filter(|n| !n.slowest.is_empty()) {
        println!("\n{}", output::heading("Slowest requests"));
        for r in &n.slowest {
            println!(
                "  {:>7.0} ms  {:<6} {}  {}",
                r.ms,
                r.method,
                r.url,
                output::dim(&if r.status == 0 { "failed".into() } else { r.status.to_string() })
            );
        }
    }
}
