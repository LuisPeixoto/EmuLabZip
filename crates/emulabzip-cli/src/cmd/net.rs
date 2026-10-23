//! `emulabzip net` and `emulabzip ca`

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use emulabzip_core::{Lab, Target};
use emulabzip_net::flow::FlowState;
use emulabzip_net::store::FlowEvent;
use emulabzip_net::{CertificateAuthority, FlowStore, Proxy, ProxyConfig, default_dir, to_har};

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
}

/// Runs the proxy, points the device at it and prints each flow as it finishes, until Ctrl+C.
pub async fn run(lab: &Lab, args: NetArgs) -> Result<()> {
    let ca = load_ca()?;
    let store = FlowStore::new(emulabzip_net::store::DEFAULT_CAPACITY);
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let proxy =
        Proxy::start(ProxyConfig { addr, ..Default::default() }, ca, store.clone()).await.with_context(|| {
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
                    if flow.state != FlowState::Pending && printed.insert(flow.id) {
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
