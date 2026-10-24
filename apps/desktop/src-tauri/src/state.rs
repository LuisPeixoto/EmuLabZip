//! State shared by every command.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use emulabzip_android::AndroidBackend;
use emulabzip_apple::AppleBackend;
use emulabzip_core::{Lab, Target};
use emulabzip_net::{Breakpoints, CertificateAuthority, FlowStore, ProxyHandle, RuleSet};
use emulabzip_trace::perfetto::processor::Processor;
use parking_lot::Mutex;
use tauri::async_runtime::JoinHandle;
use tokio::sync::watch;

use crate::traces::Loaded;

/// The proxy and the devices pointed at it.
pub struct NetRuntime {
    pub proxy: Option<ProxyHandle>,
    pub port: u16,
    pub ca: Option<Arc<CertificateAuthority>>,
    /// Devices whose settings we changed, so they can be put back when the app quits.
    pub connected: Vec<Target>,
    /// Rules and held requests outlive the proxy, so stopping it does not lose them.
    pub rules: Arc<RuleSet>,
    pub breakpoints: Arc<Breakpoints>,
    /// Send requests to servers over HTTP/3 when they offer it.
    pub http3: bool,
    /// Keep HTTP/3 out of what the device is told.
    pub hide_http3: bool,
}

impl Default for NetRuntime {
    fn default() -> Self {
        NetRuntime {
            proxy: None,
            port: 0,
            ca: None,
            connected: Vec::new(),
            rules: Arc::new(RuleSet::load_default()),
            breakpoints: Arc::new(Breakpoints::new()),
            http3: false,
            hide_http3: true,
        }
    }
}

pub struct AppState {
    pub lab: Lab,
    /// Every request the proxy has seen. It outlives the proxy, so stopping and starting again
    /// does not lose the list.
    pub flows: Arc<FlowStore>,
    pub net: tokio::sync::Mutex<NetRuntime>,
    /// Live streams (logs today, samplers later) by id, so the UI can stop them.
    sessions: Mutex<HashMap<u64, JoinHandle<()>>>,
    /// Database files copied from devices for the SQLite inspector, by preview id.
    databases: Mutex<HashMap<u64, PathBuf>>,
    /// Recordings in progress, by session, so they can be stopped early.
    pub trace_stops: Mutex<HashMap<u64, watch::Sender<bool>>>,
    /// Plugin runs in progress, by session, so they can be stopped.
    pub plugin_stops: Mutex<HashMap<u64, watch::Sender<bool>>>,
    /// The trace processor that has the open Perfetto recording loaded.
    pub processor: tokio::sync::Mutex<Option<(String, Arc<Processor>)>>,
    /// The parsed CPU profile of the open recording.
    pub profile: Mutex<Option<(String, Arc<Loaded>)>>,
    next_id: AtomicU64,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            lab: Lab::new().with(AndroidBackend::new()).with(AppleBackend::new()),
            flows: FlowStore::new(emulabzip_net::store::DEFAULT_CAPACITY),
            net: tokio::sync::Mutex::new(NetRuntime::default()),
            sessions: Mutex::new(HashMap::new()),
            databases: Mutex::new(HashMap::new()),
            trace_stops: Mutex::new(HashMap::new()),
            plugin_stops: Mutex::new(HashMap::new()),
            processor: tokio::sync::Mutex::new(None),
            profile: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn add_session(&self, task: JoinHandle<()>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut sessions = self.sessions.lock();
        // Forget sessions whose task already ended (device unplugged, window reloaded).
        sessions.retain(|_, t| !t.inner().is_finished());
        sessions.insert(id, task);
        id
    }

    /// Registers a session under an id that was handed out before its task existed.
    pub fn register_session(&self, id: u64, task: JoinHandle<()>) {
        self.sessions.lock().insert(id, task);
    }

    /// Lets go of whatever is open for a recording that is about to be deleted.
    pub async fn forget_trace(&self, id: &str) {
        let mut processor = self.processor.lock().await;
        if processor.as_ref().is_some_and(|(open, _)| open == id) {
            *processor = None;
        }
        let mut profile = self.profile.lock();
        if profile.as_ref().is_some_and(|(open, _)| open == id) {
            *profile = None;
        }
    }

    /// Asks every recording to stop and waits a few seconds for them to clean the device up,
    /// then stops the trace processor. Called on exit.
    pub async fn shutdown_traces(&self) {
        for stop in self.trace_stops.lock().values() {
            let _ = stop.send(true);
        }
        for _ in 0..40 {
            if self.trace_stops.lock().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        *self.processor.lock().await = None;
    }

    pub fn stop_session(&self, id: u64) {
        if let Some(task) = self.sessions.lock().remove(&id) {
            task.abort();
        }
    }

    pub fn stop_all(&self) {
        for (_, task) in self.sessions.lock().drain() {
            task.abort();
        }
        let _ = std::fs::remove_dir_all(scratch_dir());
    }

    /// Puts every device we changed back, and the Mac's proxy if it was ours. Called on exit.
    pub async fn shutdown_network(&self) {
        let mut net = self.net.lock().await;
        for target in std::mem::take(&mut net.connected) {
            if let Err(e) = self.lab.disconnect_network(&target).await {
                tracing::warn!("could not disconnect {}: {e}", target.id);
            }
        }
        net.proxy = None;
        // A Mac proxy left behind by an earlier run that was killed.
        if let Err(e) = AppleBackend::new().restore_system_proxy().await {
            tracing::warn!("could not restore the Mac's proxy settings: {e}");
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn add_database(&self, id: u64, path: PathBuf) {
        self.databases.lock().insert(id, path);
    }

    pub fn database(&self, id: u64) -> Option<PathBuf> {
        self.databases.lock().get(&id).cloned()
    }
}

impl Default for AppState {
    fn default() -> Self {
        AppState::new()
    }
}

/// A private folder for files copied from devices. Removed when the app exits.
pub fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join(format!("emulabzip-{}", std::process::id()))
}
