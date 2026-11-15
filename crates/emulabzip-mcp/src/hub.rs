//! What the server keeps between calls: the network proxy and the devices pointed at it.
//!
//! A tool call is a request and an answer, but the proxy has to keep running while the assistant uses the app, and the
//! requests it saw have to be there when the assistant asks for them. The [`Hub`] holds those, and puts the devices back
//! when the session ends.

use std::net::SocketAddr;
use std::sync::Arc;

use emulabzip_core::{Lab, Target};
use emulabzip_net::{
    Breakpoints, CertificateAuthority, Conditions, FlowStore, Proxy, ProxyConfig, ProxyHandle, ReplaySet, RuleSet,
};
use tokio::sync::Mutex;

/// The port the proxy listens on when the assistant does not say.
pub const DEFAULT_PORT: u16 = 9090;

/// The proxy, and everything that outlives starting and stopping it.
pub struct Net {
    pub proxy: Option<ProxyHandle>,
    pub port: u16,
    pub ca: Option<Arc<CertificateAuthority>>,
    /// Devices whose settings this session changed, so they can be put back.
    pub connected: Vec<Target>,
    /// Rules are kept in a file shared with the desktop app, so a rule made there is in force here too.
    pub rules: Arc<RuleSet>,
    pub breakpoints: Arc<Breakpoints>,
    pub conditions: Conditions,
    pub replay: Option<Arc<ReplaySet>>,
    pub http3: bool,
}

impl Net {
    fn new() -> Net {
        Net {
            proxy: None,
            port: 0,
            ca: None,
            connected: Vec::new(),
            rules: Arc::new(RuleSet::load_default()),
            breakpoints: Arc::new(Breakpoints::new()),
            conditions: Conditions::default(),
            replay: None,
            http3: false,
        }
    }

    /// The certificate authority, made on first use.
    pub fn authority(&mut self) -> Result<Arc<CertificateAuthority>, String> {
        if let Some(ca) = &self.ca {
            return Ok(ca.clone());
        }
        let ca =
            Arc::new(CertificateAuthority::load_or_create(emulabzip_net::default_dir()).map_err(|e| e.to_string())?);
        self.ca = Some(ca.clone());
        Ok(ca)
    }
}

/// State shared by every tool of one server.
pub struct Hub {
    pub net: Mutex<Net>,
    /// The background watches, by id.
    pub watches: std::sync::Mutex<std::collections::HashMap<u64, crate::watch::Watch>>,
    pub next_watch: std::sync::atomic::AtomicU64,
    /// What the proxy saw. It outlives the proxy, so the requests are still there after it stops.
    pub flows: Arc<FlowStore>,
}

impl Hub {
    pub fn new() -> Arc<Hub> {
        Arc::new(Hub {
            net: Mutex::new(Net::new()),
            watches: Default::default(),
            next_watch: std::sync::atomic::AtomicU64::new(1),
            flows: FlowStore::new(emulabzip_net::store::DEFAULT_CAPACITY),
        })
    }

    /// Starts the proxy on the loopback interface, unless it is running.
    pub async fn start_proxy(&self, net: &mut Net, port: Option<u16>) -> Result<(), String> {
        if net.proxy.is_some() {
            return Ok(());
        }
        let port = port.unwrap_or(if net.port == 0 { DEFAULT_PORT } else { net.port });
        let ca = net.authority()?;
        let config = ProxyConfig {
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            rules: net.rules.clone(),
            breakpoints: net.breakpoints.clone(),
            http3: net.http3,
            conditions: net.conditions.clone(),
            replay: net.replay.clone(),
            ..Default::default()
        };
        let handle = Proxy::start(config, ca, self.flows.clone())
            .await
            .map_err(|e| format!("Could not listen on port {port}. Is another proxy using it? {e}"))?;
        net.port = handle.addr().port();
        net.proxy = Some(handle);
        Ok(())
    }

    /// Puts every device back and stops the proxy.
    pub async fn stop_proxy(&self, lab: &Lab, net: &mut Net) {
        for target in std::mem::take(&mut net.connected) {
            if let Err(e) = lab.disconnect_network(&target).await {
                tracing_warn(&format!("could not disconnect {}: {e}", target.id));
            }
        }
        net.proxy = None;
    }

    /// Called when the session ends: nothing this server changed is left behind.
    pub async fn shutdown(&self, lab: &Lab) {
        self.watches.lock().unwrap().clear();
        let mut net = self.net.lock().await;
        self.stop_proxy(lab, &mut net).await;
    }
}

fn tracing_warn(message: &str) {
    eprintln!("emulabzip mcp: {message}");
}
