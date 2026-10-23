//! State shared by every command.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use emulabzip_android::AndroidBackend;
use emulabzip_apple::AppleBackend;
use emulabzip_core::Lab;
use parking_lot::Mutex;
use tauri::async_runtime::JoinHandle;

pub struct AppState {
    pub lab: Lab,
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
