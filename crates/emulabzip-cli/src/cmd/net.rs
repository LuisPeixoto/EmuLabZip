//! `emulabzip net` and `emulabzip ca`

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use emulabzip_core::{Lab, Target};
use emulabzip_net::flow::FlowState;
use emulabzip_net::store::FlowEvent;
use emulabzip_net::{Action, CertificateAuthority, FlowStore, Proxy, ProxyConfig, Rule, RuleSet, default_dir, to_har};

use crate::output;

pub(crate) fn load_ca() -> Result<Arc<CertificateAuthority>> {
    let dir = default_dir();
    Ok(Arc::new(
        CertificateAuthority::load_or_create(&dir).with_context(|| format!("could not use {}", dir.display()))?,
    ))
}

/// The network conditions asked for: a preset, then any number changed by hand.
pub struct ConditionArgs {
    pub preset: Option<String>,
    pub down_kbps: Option<u32>,
    pub up_kbps: Option<u32>,
    pub latency_ms: Option<u32>,
    pub loss_percent: Option<f32>,
}

impl ConditionArgs {
    /// `None` when nothing was asked for.
    pub fn build(&self) -> Result<Option<emulabzip_net::Conditions>> {
        let asked = self.preset.is_some()
            || self.down_kbps.is_some()
            || self.up_kbps.is_some()
            || self.latency_ms.is_some()
            || self.loss_percent.is_some();
        if !asked {
            return Ok(None);
        }
        let mut c = match &self.preset {
            Some(id) => emulabzip_net::Conditions::preset(id).with_context(|| {
                let names: Vec<&str> = emulabzip_net::PRESETS.iter().map(|(i, _)| *i).collect();
                format!("`{id}` is not a preset: use {}", names.join(", "))
            })?,
            None => emulabzip_net::Conditions::default(),
        };
        if self.down_kbps.is_some()
            || self.up_kbps.is_some()
            || self.latency_ms.is_some()
            || self.loss_percent.is_some()
        {
            c.name = None;
        }
        c.down_kbps = self.down_kbps.or(c.down_kbps);
        c.up_kbps = self.up_kbps.or(c.up_kbps);
        c.latency_ms = self.latency_ms.unwrap_or(c.latency_ms);
        c.loss_percent = self.loss_percent.unwrap_or(c.loss_percent);
        c.check().map_err(anyhow::Error::msg)?;
        Ok(Some(c))
    }
}

/// A recording to answer from, and how.
#[derive(Debug, Clone)]
pub struct ReplayArgs {
    pub file: PathBuf,
    pub passthrough: bool,
    pub timing: bool,
}

/// Reads the recording. Called before any proxy starts or device is looked for.
pub fn load_replay(args: &ReplayArgs) -> Result<Arc<emulabzip_net::ReplaySet>> {
    let set = emulabzip_session::replay::load(&args.file, args.passthrough, args.timing)?;
    Ok(Arc::new(set))
}

/// A line that says what is being replayed and what happens to everything else.
pub fn replay_banner(set: &emulabzip_net::ReplaySet) -> String {
    format!(
        "Replaying {}: {} requests. {}",
        set.source(),
        set.len(),
        if set.is_strict() {
            "A request that is not in it fails with 504 and never reaches the internet."
        } else {
            "A request that is not in it goes to the real server."
        }
    )
}

/// What a replay did, for the end of a run: how many were answered and which requests were not in the recording.
pub fn print_replay_stats(set: &emulabzip_net::ReplaySet) {
    let s = set.stats();
    eprintln!(
        "{}",
        output::dim(&format!(
            "Replay of {}: {} answered from the recording, {} not in it.",
            s.source, s.hits, s.misses
        ))
    );
    for line in &s.missed {
        eprintln!("  {}", output::dim(&format!("not recorded: {line}")));
    }
}

pub struct NetArgs {
    pub device: Option<String>,
    pub port: u16,
    pub no_connect: bool,
    pub har: Option<PathBuf>,
    pub json: bool,
    pub rules: Option<PathBuf>,
    pub http3: bool,
    pub show_h3: bool,
    pub conditions: ConditionArgs,
    pub replay: Option<ReplayArgs>,
    /// Print the analytics events of each request instead of the request: the services to show, all when empty.
    pub analytics: Option<Vec<emulabzip_net::analytics::Service>>,
}

/// Reads rules from a JSON file: a list, or an object with a `rules` list. Breakpoints are left
/// out, because nobody is there to continue what they hold.
fn load_rules(path: &std::path::Path) -> Result<(Arc<RuleSet>, Vec<String>)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", path.display()))?;
    let list = match value {
        serde_json::Value::Object(mut o) if o.contains_key("rules") => o.remove("rules").unwrap_or_default(),
        other => other,
    };
    let mut rules: Vec<Rule> = serde_json::from_value(list)
        .with_context(|| format!("{} does not hold a list of rules (see docs/mocks.md)", path.display()))?;
    let mut skipped = Vec::new();
    rules.retain(|r| {
        let holds = matches!(r.action, Action::Breakpoint { .. });
        if holds {
            skipped.push(r.name.clone());
        }
        !holds
    });
    let set = Arc::new(RuleSet::new());
    set.set(rules).with_context(|| format!("a rule in {} is not valid", path.display()))?;
    Ok((set, skipped))
}

/// Runs the proxy, points the device at it and prints each flow as it finishes, until Ctrl+C.
pub async fn run(lab: &Lab, args: NetArgs) -> Result<()> {
    // Checked before the proxy starts or any device is looked for.
    let conditions = args.conditions.build()?;
    let replay = args.replay.as_ref().map(load_replay).transpose()?;
    let ca = load_ca()?;
    let store = FlowStore::new(emulabzip_net::store::DEFAULT_CAPACITY);
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let mut config = ProxyConfig { addr, http3: args.http3, hide_http3: !args.show_h3, ..Default::default() };
    if let Some(path) = &args.rules {
        let (rules, skipped) = load_rules(path)?;
        for name in skipped {
            eprintln!("warning: the rule “{name}” holds requests at a breakpoint, which needs the app. It is skipped.");
        }
        if !args.json {
            eprintln!("{}", output::dim(&format!("{} rules from {}.", rules.rules().len(), path.display())));
        }
        config.rules = rules;
    }
    if let Some(set) = &replay {
        if !args.json {
            eprintln!("{}", output::dim(&replay_banner(set)));
        }
        config.replay = Some(set.clone());
    }
    if let Some(c) = &conditions {
        config.conditions = c.clone();
        if !args.json {
            eprintln!("{}", output::dim(&format!("Network conditions: {}.", c.summary())));
        }
    }
    let proxy = Proxy::start(config, ca, store.clone()).await.with_context(|| {
        format!("could not listen on {addr}. Is another EmuLabZip running? Pick a port with --port")
    })?;

    // Everything after the device is connected goes through `finish`, so an error or Ctrl+C
    // never leaves a device (or the Mac) pointing at a proxy that is gone.
    let target = if args.no_connect { None } else { Some(lab.resolve(args.device.as_deref()).await?.target()) };
    if let Some(target) = &target {
        let status = lab.connect_network(target, args.port).await?;
        if !args.json {
            eprintln!("{}", output::dim(&format!("Connected {:?} to 127.0.0.1:{}.", target.id, args.port)));
            if let Some(note) = status.note {
                eprintln!("{}", output::dim(&note));
            }
        }
    } else if !args.json {
        eprintln!(
            "{}",
            output::dim(&format!(
                "Listening on {}. Point a client's proxy there; the certificate is at http://{}/ca.crt",
                proxy.addr(),
                proxy.addr()
            ))
        );
    }
    if !args.json {
        eprintln!("{}", output::dim("Press Ctrl+C to stop."));
    }

    let result = stream(&store, args.json, args.analytics.as_deref()).await;
    finish(lab, target.as_ref()).await;
    drop(proxy);
    if let Some(set) = &replay
        && !args.json
    {
        print_replay_stats(set);
    }

    if let Some(path) = &args.har {
        let har = to_har(&store.all());
        let entries = har["log"]["entries"].as_array().map_or(0, Vec::len);
        tokio::fs::write(path, serde_json::to_vec_pretty(&har)?)
            .await
            .with_context(|| format!("could not write {}", path.display()))?;
        eprintln!("Saved {entries} requests to {}", path.display());
    }
    result
}

/// `emulabzip net import`: reads a capture file and prints what it holds.
pub async fn import(file: &std::path::Path, har: Option<PathBuf>, json: bool, dns: bool) -> Result<()> {
    let bytes = tokio::fs::read(file).await.with_context(|| format!("could not read {}", file.display()))?;
    let capture = emulabzip_net::pcap::import(&bytes).with_context(|| format!("could not read {}", file.display()))?;
    let store = FlowStore::new(emulabzip_net::store::DEFAULT_CAPACITY);
    for flow in capture.flows {
        store.begin(flow);
    }
    if json {
        for flow in store.summaries() {
            println!("{}", serde_json::to_string(&flow)?);
        }
    } else {
        for flow in store.summaries() {
            println!("{}", output::flow_line(&flow));
        }
    }
    if dns && !json {
        println!("\n{}", output::heading("DNS"));
        for l in &capture.dns {
            let answer = if l.answers.is_empty() { "—".to_string() } else { l.answers.join(", ") };
            println!(
                "{:<6} {}  {}  {}",
                l.kind,
                l.name,
                answer,
                output::dim(l.status.as_deref().unwrap_or("no answer"))
            );
        }
    }
    let s = &capture.summary;
    if !json {
        eprintln!(
            "{}",
            output::dim(&format!(
                "{} packets: {} requests, {} encrypted connections, {} other connections, {} DNS lookups.",
                s.packets, s.http_requests, s.tls_connections, s.other_connections, s.dns_lookups
            ))
        );
        if s.gaps {
            eprintln!("{}", output::dim("Some packets are missing from the capture, so some bodies are incomplete."));
        }
    }
    if let Some(path) = har {
        let har = to_har(&store.all());
        let entries = har["log"]["entries"].as_array().map_or(0, Vec::len);
        tokio::fs::write(&path, serde_json::to_vec_pretty(&har)?)
            .await
            .with_context(|| format!("could not write {}", path.display()))?;
        eprintln!("Saved {entries} requests to {}", path.display());
    }
    Ok(())
}

async fn finish(lab: &Lab, target: Option<&Target>) {
    if let Some(target) = target {
        match lab.disconnect_network(target).await {
            Ok(_) => eprintln!("{}", output::dim("Disconnected.")),
            Err(e) => eprintln!("warning: could not disconnect {}: {e}", target.id),
        }
    }
}

async fn stream(store: &FlowStore, json: bool, analytics: Option<&[emulabzip_net::analytics::Service]>) -> Result<()> {
    let mut events = store.subscribe();
    let mut printed = HashSet::new();
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(FlowEvent::Added { flow } | FlowEvent::Updated { flow }) => {
                    // A flow is shown once, when it is finished.
                    if matches!(flow.state, FlowState::Completed | FlowState::Failed) && printed.insert(flow.id) {
                        if let Some(services) = analytics {
                            if let Some(full) = store.get(flow.id) {
                                super::analytics::print_flow(&full, services, json)?;
                            }
                        } else if json { println!("{}", serde_json::to_string(&flow)?) } else { println!("{}", output::flow_line(&flow)) }
                    }
                }
                Ok(FlowEvent::Cleared) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
            },
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}

/// `emulabzip ca install`: make the device trust the proxy's certificate.
pub async fn install_ca(lab: &Lab, device: Option<&str>) -> Result<()> {
    let ca = load_ca()?;
    let device = lab.resolve(device).await?;
    let result = lab.install_ca(&device.target(), ca.cert_pem(), ca.cert_der()).await?;
    println!("{}", result.message);
    for (i, step) in result.steps.iter().enumerate() {
        println!("  {}. {step}", i + 1);
    }
    if let Some(snippet) = &result.snippet {
        println!("\n{}\n{snippet}", output::heading("network_security_config.xml"));
    }
    Ok(())
}

/// `emulabzip ca path`: where the certificate is, creating it on first use.
pub fn path() -> Result<()> {
    let ca = load_ca()?;
    println!("{}", ca.cert_path().context("the certificate has no file")?.display());
    Ok(())
}
