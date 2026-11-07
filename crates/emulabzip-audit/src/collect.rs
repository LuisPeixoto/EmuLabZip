//! Gathering what the security audit looks at from a device, through [`Lab`].

use std::path::PathBuf;
use std::time::Duration;

use emulabzip_core::{Lab, LogOptions, Platform, Target};
use emulabzip_storage::{FileKind, detect};
use serde_json::Value;

use crate::finding::Area;
use crate::security::Input;
use crate::security::storage::{Column, Stored};
use crate::security::traffic::Request;

/// Files bigger than this are not read.
const MAX_FILE: u64 = 4 * 1024 * 1024;
/// A sandbox with more files than this is cut short.
const MAX_FILES: usize = 600;
const MAX_DEPTH: usize = 5;
/// What the app keeps there is not its own data.
const SKIP_DIRS: &[&str] = &["cache", "code_cache", "lib", "tmp", "SystemData", "Caches"];

/// What to look at.
#[derive(Debug, Default)]
pub struct Sources {
    pub manifest: bool,
    pub logs: bool,
    pub storage: bool,
    /// Requests the proxy recorded, when it was running.
    pub requests: Option<Vec<Request>>,
}

impl Sources {
    pub fn all(requests: Option<Vec<Request>>) -> Sources {
        Sources { manifest: true, logs: true, storage: true, requests }
    }
}

pub async fn collect(lab: &Lab, target: &Target, app: &str, what: Sources) -> Input {
    let mut input = Input { requests: what.requests, ..Input::default() };

    if what.manifest {
        match target.platform {
            Platform::Android => match lab.package_dump(target, app).await {
                Ok(text) => input.dumpsys = Some(text),
                Err(e) => input.unreadable.push((Area::Manifest, e.to_string())),
            },
            Platform::Ios => {
                match lab.read_file(target, app, "bundle", "Info.plist").await {
                    Ok(bytes) => match emulabzip_storage::plist::to_json(&bytes) {
                        Ok(v) => input.info_plist = Some(v),
                        Err(e) => input.unreadable.push((Area::Manifest, format!("Info.plist: {e}"))),
                    },
                    Err(e) => input.unreadable.push((Area::Manifest, format!("Info.plist: {e}"))),
                }
                // An app may have no privacy manifest; that is a finding, not a failure.
                input.privacy_manifest = lab
                    .read_file(target, app, "bundle", "PrivacyInfo.xcprivacy")
                    .await
                    .ok()
                    .and_then(|b| emulabzip_storage::plist::to_json(&b).ok());
            }
        }
    }

    if what.logs {
        match read_log(lab, target, app).await {
            Ok(lines) => input.log_lines = Some(lines),
            Err(e) => input.unreadable.push((Area::Log, e.to_string())),
        }
    }

    if what.storage {
        match read_storage(lab, target, app).await {
            Ok((stored, columns)) => {
                input.stored = Some(stored);
                input.columns = Some(columns);
            }
            Err(e) => input.unreadable.push((Area::Storage, e.to_string())),
        }
    }
    input
}

/// The log for a moment: what is replayed, and then a short wait for anything more. Lines of the app are kept when it
/// can be told which they are.
async fn read_log(lab: &Lab, target: &Target, app: &str) -> emulabzip_core::Result<Vec<String>> {
    let mut stream = lab.logs(target, LogOptions { tail: 1500, verbose: true }).await?;
    let mut entries = Vec::new();
    // The replay can start slowly and can take a while: wait for the first line, then until the log goes quiet.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    let mut wait = Duration::from_secs(4);
    while entries.len() < 8000 && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(wait, stream.next()).await {
            Ok(Some(e)) => {
                entries.push(e);
                wait = Duration::from_millis(700);
            }
            _ => break,
        }
    }
    // The app's own lines are the ones from its process, and the ones that mention it; the rest is the system talking.
    let pid = lab.apps(target).await.ok().and_then(|apps| apps.into_iter().find(|a| a.id == app)).and_then(|a| a.pid);
    let mine: Vec<String> = entries
        .iter()
        .filter(|e| pid.is_some_and(|p| e.pid == p) || e.message.contains(app) || e.tag.contains(app))
        .map(|e| format!("{}: {}", e.tag, e.message))
        .collect();
    if mine.is_empty() {
        // The log of a whole device is mostly other people's: scanning it would report their secrets and mistake numbers for
        // cards. Better to say that the app's own lines could not be told.
        return Err(emulabzip_core::Error::failed(
            "The app has written nothing to the log and is not running, so its lines cannot be told from the system's. Start the app and use it, then audit again.",
        ));
    }
    Ok(mine)
}

async fn read_storage(lab: &Lab, target: &Target, app: &str) -> emulabzip_core::Result<(Vec<Stored>, Vec<Column>)> {
    let mut files = Vec::new();
    walk(lab, target, app, "data", "", 0, &mut files).await?;
    let mut stored = Vec::new();
    let mut columns = Vec::new();
    for (path, size) in files {
        if size > MAX_FILE {
            continue;
        }
        let Ok(bytes) = lab.read_file(target, app, "data", &path).await else { continue };
        match detect(&path, &bytes) {
            FileKind::SharedPrefs => {
                if let Ok(prefs) = emulabzip_storage::prefs::parse(&String::from_utf8_lossy(&bytes)) {
                    for p in prefs {
                        let value = if p.items.is_empty() { p.value } else { p.items.join(", ") };
                        stored.push(Stored { file: path.clone(), key: p.key, value });
                    }
                }
            }
            FileKind::DataStore => {
                if let Ok(prefs) = emulabzip_storage::datastore::parse_preferences(&bytes) {
                    stored.extend(prefs.into_iter().map(|p| Stored { file: path.clone(), key: p.key, value: p.value }));
                }
            }
            FileKind::Mmkv => {
                if let Ok(entries) = emulabzip_storage::mmkv::parse(&bytes) {
                    stored.extend(entries.into_iter().map(|e| Stored {
                        file: path.clone(),
                        key: e.key,
                        value: e.value,
                    }));
                }
            }
            FileKind::Plist => {
                if let Ok(json) = emulabzip_storage::plist::to_json(&bytes) {
                    flatten(&path, "", &json, &mut stored);
                }
            }
            FileKind::Sqlite => {
                columns.extend(sqlite_columns(&path, &bytes));
            }
            _ => {}
        }
    }
    Ok((stored, columns))
}

/// Every file under `dir`, with its size, skipping caches.
async fn walk(
    lab: &Lab,
    target: &Target,
    app: &str,
    root: &str,
    dir: &str,
    depth: usize,
    out: &mut Vec<(String, u64)>,
) -> emulabzip_core::Result<()> {
    let entries = lab.list_files(target, app, root, dir).await?;
    for e in entries {
        if out.len() >= MAX_FILES {
            return Ok(());
        }
        if e.is_dir {
            if depth < MAX_DEPTH && !SKIP_DIRS.contains(&e.name.as_str()) {
                // A folder that cannot be listed is left out; the root itself failing is the error above.
                let _ = Box::pin(walk(lab, target, app, root, &e.path, depth + 1, out)).await;
            }
        } else {
            out.push((e.path, e.size));
        }
    }
    Ok(())
}

fn flatten(file: &str, prefix: &str, value: &Value, out: &mut Vec<Stored>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}/{k}") };
                flatten(file, &key, v, out);
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                flatten(file, &format!("{prefix}/{i}"), v, out);
            }
        }
        Value::String(s) => out.push(Stored { file: file.to_string(), key: prefix.to_string(), value: s.clone() }),
        Value::Number(n) => out.push(Stored { file: file.to_string(), key: prefix.to_string(), value: n.to_string() }),
        _ => {}
    }
}

fn sqlite_columns(file: &str, bytes: &[u8]) -> Vec<Column> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir: PathBuf = std::env::temp_dir().join(format!("emulabzip-audit-{}-{n}", std::process::id()));
    if std::fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }
    let local = dir.join("db");
    let found = (|| {
        std::fs::write(&local, bytes).ok()?;
        let db = emulabzip_storage::sqlite::Database::open(&local).ok()?;
        let tables = db.tables().ok()?;
        Some(
            tables
                .into_iter()
                .filter(|t| t.kind == "table" && !t.name.starts_with("sqlite_") && t.name != "android_metadata")
                .flat_map(|t| {
                    let name = t.name.clone();
                    t.columns.into_iter().map(move |c| Column {
                        file: file.to_string(),
                        table: name.clone(),
                        column: c.name,
                    })
                })
                .collect::<Vec<_>>(),
        )
    })();
    let _ = std::fs::remove_dir_all(&dir);
    found.unwrap_or_default()
}
