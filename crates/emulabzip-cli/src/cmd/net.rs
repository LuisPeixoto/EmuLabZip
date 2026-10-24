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

fn load_ca() -> Result<Arc<CertificateAuthority>> {
    let dir = default_dir();
    Ok(Arc::new(
        CertificateAuthority::load_or_create(&dir).with_context(|| format!("could not use {}", dir.display()))?,
    ))
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

    let result = stream(&store, args.json).await;
    finish(lab, target.as_ref()).await;
    drop(proxy);

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

async fn stream(store: &FlowStore, json: bool) -> Result<()> {
    let mut events = store.subscribe();
    let mut printed = HashSet::new();
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(FlowEvent::Added { flow } | FlowEvent::Updated { flow }) => {
                    // A flow is shown once, when it is finished.
                    if matches!(flow.state, FlowState::Completed | FlowState::Failed) && printed.insert(flow.id) {
                        if json { println!("{}", serde_json::to_string(&flow)?) } else { println!("{}", output::flow_line(&flow)) }
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
