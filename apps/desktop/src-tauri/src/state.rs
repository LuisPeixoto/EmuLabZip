//! State shared by every command.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use emulabzip_android::AndroidBackend;
use emulabzip_apple::AppleBackend;
use emulabzip_core::{Lab, Target};
use emulabzip_net::{CertificateAuthority, FlowStore, ProxyHandle};
use parking_lot::Mutex;
use tauri::async_runtime::JoinHandle;

/// The proxy and the devices pointed at it.
#[derive(Default)]
pub struct NetRuntime {
    pub proxy: Option<ProxyHandle>,
    pub port: u16,
    pub ca: Option<Arc<CertificateAuthority>>,
    /// Devices whose settings we changed, so they can be put back when the app quits.
    pub connected: Vec<Target>,
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
