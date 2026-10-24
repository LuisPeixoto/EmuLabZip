//! Reading a session back, from its `.ezsession` file or from the folder of one that was cut short.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use emulabzip_core::{CrashReport, Error, LogEntry, PerfSample, Result};
use serde::de::DeserializeOwned;

use crate::format::{CRASHES, LOGS, META, Meta, NETWORK, PERF, SCREENS, VERSION, bad};

/// The biggest single file read from a session. A bigger one is not a session of ours.
const MAX_FILE: u64 = 512 * 1024 * 1024;

enum Source {
    Dir(PathBuf),
    Zip(std::sync::Mutex<zip::ZipArchive<File>>),
}

pub struct Session {
    pub meta: Meta,
    pub logs: Vec<LogEntry>,
    pub perf: Vec<PerfSample>,
    /// The requests, as a HAR document, when the network was recorded.
    pub har: Option<serde_json::Value>,
    pub crashes: Vec<CrashReport>,
    /// The names of the screenshots, in order.
    pub screens: Vec<String>,
    /// The recording was cut short: it was read from a folder, or never got an end time.
    pub partial: bool,
    source: Source,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("meta", &self.meta)
            .field("logs", &self.logs.len())
            .field("perf", &self.perf.len())
            .finish()
    }
}

/// Lines of JSON, skipping what cannot be read: the last line of a killed run may be half written.
fn lines<T: DeserializeOwned>(text: &str) -> Vec<T> {
    text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

impl Session {
    /// Opens a `.ezsession` file, or the folder a recording that did not finish left behind.
    pub fn open(path: &Path) -> Result<Session> {
        let (source, partial_dir) = if path.is_dir() {
            (Source::Dir(path.to_path_buf()), true)
        } else {
            let file =
                File::open(path).map_err(|e| Error::failed(format!("could not open {}: {e}", path.display())))?;
            let zip = zip::ZipArchive::new(file)
                .map_err(|_| bad("session file", format!("{} is not a session", path.display())))?;
            (Source::Zip(std::sync::Mutex::new(zip)), false)
        };
        let mut session = Session {
            meta: Meta {
                version: 0,
                tool: String::new(),
                device: crate::format::DeviceMeta {
                    id: String::new(),
                    name: String::new(),
                    platform: emulabzip_core::Platform::Android,
                    os_version: None,
                    model: None,
                },
                app: None,
                started_ms: 0,
                ended_ms: None,
                network: false,
                screens_every_secs: None,
                logs_dropped: 0,
            },
            logs: Vec::new(),
            perf: Vec::new(),
            har: None,
            crashes: Vec::new(),
            screens: Vec::new(),
            partial: partial_dir,
            source,
        };
        let meta = session.read(META)?.ok_or_else(|| bad("session", format!("{} has no {META}", path.display())))?;
        session.meta = serde_json::from_slice(&meta).map_err(|e| bad("session.json", e.to_string()))?;
        if session.meta.version > VERSION {
            return Err(Error::failed(format!(
                "This session was written by a newer EmuLabZip (format {}, this one reads up to {VERSION}). Update EmuLabZip to open it.",
                session.meta.version
            )));
        }
        session.partial |= session.meta.ended_ms.is_none();
        if let Some(b) = session.read(LOGS)? {
            session.logs = lines(&String::from_utf8_lossy(&b));
        }
        if let Some(b) = session.read(PERF)? {
            session.perf = lines(&String::from_utf8_lossy(&b));
        }
        if let Some(b) = session.read(NETWORK)? {
            session.har = serde_json::from_slice(&b).ok();
        }
        for name in session.names(CRASHES) {
            if let Some(b) = session.read(&format!("{CRASHES}/{name}"))?
                && let Ok(report) = serde_json::from_slice(&b)
            {
                session.crashes.push(report);
            }
        }
        session.screens = session.names(SCREENS);
        Ok(session)
    }

    /// The file names inside one of the session's folders, sorted.
    fn names(&self, folder: &str) -> Vec<String> {
        let mut out: Vec<String> = match &self.source {
            Source::Dir(root) => fs::read_dir(root.join(folder))
                .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                .unwrap_or_default(),
            Source::Zip(zip) => {
                let zip = zip.lock().unwrap_or_else(|e| e.into_inner());
                let prefix = format!("{folder}/");
                (0..zip.len())
                    .filter_map(|i| zip.name_for_index(i)?.ok())
                    .filter_map(|n| n.strip_prefix(&prefix).map(str::to_string))
                    .filter(|n| !n.is_empty() && !n.contains('/'))
                    .collect()
            }
        };
        out.sort();
        out
    }

    /// One file of the session; `None` when the session has none by that name.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match &self.source {
            Source::Dir(root) => match fs::read(root.join(name)) {
                Ok(b) => Ok(Some(b)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            },
            Source::Zip(zip) => {
                let mut zip = zip.lock().unwrap_or_else(|e| e.into_inner());
                let Ok(file) = zip.by_name(name) else { return Ok(None) };
                if file.size() > MAX_FILE {
                    return Err(bad("session file", format!("{name} is too large")));
                }
                let mut out = Vec::with_capacity(file.size() as usize);
                file.take(MAX_FILE).read_to_end(&mut out)?;
                Ok(Some(out))
            }
        }
    }

    /// A screenshot by its name from [`Session::screens`].
    pub fn screen(&self, name: &str) -> Result<Vec<u8>> {
        // The name comes from the file's own listing, but is checked all the same.
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err(Error::invalid("screenshot name", name));
        }
        self.read(&format!("{SCREENS}/{name}"))?.ok_or_else(|| Error::NotFound(format!("no screenshot named {name}")))
    }
}
