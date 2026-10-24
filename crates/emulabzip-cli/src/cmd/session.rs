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
}

/// The proxy of a recording that watches the network, and the device pointed at it.
pub struct Network {
    pub store: Arc<FlowStore>,
    proxy: ProxyHandle,
    target: Target,
}

impl Network {
    pub async fn start(lab: &Lab, device: &Device, port: u16) -> Result<Network> {
        let store = FlowStore::new(emulabzip_net::store::DEFAULT_CAPACITY);
        let ca = super::net::load_ca()?;
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let proxy = Proxy::start(ProxyConfig { addr, ..Default::default() }, ca, store.clone())
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
                eprintln!("{}", output::dim(&format!("  {} s: {} log lines, {} samples, {} crashes", started.elapsed().as_secs(), s.logs, s.samples, s.crashes)));
            }
        }
    }
    let written = recorder.finish().await;
    if let Some(n) = network {
        n.stop(lab).await;
    }
    let path = written?;
    println!("Saved {}", path.display());
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
