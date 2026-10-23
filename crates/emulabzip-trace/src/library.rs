//! The recordings kept on this computer, one folder each under `traces/` in the data folder:
//!
//! ```text
//! traces/20261023-184501-3f2a/
//!     meta.json        what was recorded, and when
//!     trace.pftrace    the recording (or profile.pb, or recording.trace)
//! ```

use std::path::PathBuf;

use emulabzip_core::{Error, Platform, Result, TraceFile, TraceFormat, TraceRequest, data_dir};
use serde::{Deserialize, Serialize};

const META: &str = "meta.json";

/// A saved recording. Files are named relative to the recording's folder, so the library can move
/// with the data folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String,
    pub platform: Platform,
    /// The name of the device it was recorded on.
    pub device: String,
    pub request: TraceRequest,
    pub format: TraceFormat,
    /// Milliseconds since the Unix epoch.
    pub created_ms: u64,
    pub file: String,
    pub extras: Vec<String>,
    pub size_bytes: u64,
}

pub fn root() -> PathBuf {
    data_dir().join("traces")
}

/// A fresh id: the time, then a few random letters so two recordings in one second differ.
pub fn new_id() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (days, rest) = (now.as_secs() / 86_400, now.as_secs() % 86_400);
    let (y, m, d) = civil(days as i64);
    let salt = now.subsec_nanos() ^ std::process::id().rotate_left(16);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}-{:04x}", rest / 3600, rest % 3600 / 60, rest % 60, salt & 0xffff)
}

/// Year, month and day of a day count since 1970-01-01 (proleptic Gregorian, UTC).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Only ids made by [`new_id`] are accepted, so an id can never point outside the library.
fn check_id(id: &str) -> Result<()> {
    if !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        Ok(())
    } else {
        Err(Error::invalid("recording id", id))
    }
}

pub fn folder(id: &str) -> Result<PathBuf> {
    check_id(id)?;
    Ok(root().join(id))
}

impl Recording {
    /// The folder the recording is in.
    pub fn dir(&self) -> PathBuf {
        root().join(&self.id)
    }

    pub fn path(&self) -> PathBuf {
        self.dir().join(&self.file)
    }

    pub fn extra_paths(&self) -> Vec<PathBuf> {
        self.extras.iter().map(|e| self.dir().join(e)).collect()
    }

    /// Describes a finished recording and writes `meta.json` next to its files.
    pub fn save(
        id: &str,
        platform: Platform,
        device: &str,
        request: &TraceRequest,
        file: &TraceFile,
    ) -> Result<Recording> {
        let dir = folder(id)?;
        let name = |p: &std::path::Path| {
            p.strip_prefix(&dir)
                .ok()
                .and_then(|r| r.to_str())
                .map(str::to_string)
                .ok_or_else(|| Error::failed("the recording was saved outside its folder"))
        };
        let extras = file.extras.iter().map(|e| name(e)).collect::<Result<Vec<_>>>()?;
        let main = name(&file.path)?;
        let size_bytes = std::iter::once(&file.path).chain(&file.extras).map(|p| size_of(p)).sum();
        let created_ms =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        let rec = Recording {
            id: id.to_string(),
            platform,
            device: device.to_string(),
            request: request.clone(),
            format: file.format,
            created_ms,
            file: main,
            extras,
            size_bytes,
        };
        std::fs::write(dir.join(META), serde_json::to_vec_pretty(&rec).map_err(|e| Error::failed(e.to_string()))?)?;
        Ok(rec)
    }
}

/// The size of a file, or of a folder's files (an Instruments document is a folder).
fn size_of(path: &std::path::Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => {
            std::fs::read_dir(path).map(|d| d.flatten().map(|e| size_of(&e.path())).sum()).unwrap_or(0)
        }
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

pub fn get(id: &str) -> Result<Recording> {
    let dir = folder(id)?;
    let bytes = std::fs::read(dir.join(META)).map_err(|_| Error::NotFound(format!("recording {id}")))?;
    serde_json::from_slice(&bytes).map_err(|e| Error::Parse { what: "recording", detail: e.to_string() })
}

/// Every recording, newest first. Folders without a readable `meta.json` (a recording that was
/// interrupted, or a stranger's folder) are left out.
pub fn list() -> Vec<Recording> {
    let mut all: Vec<Recording> = std::fs::read_dir(root())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|id| get(id).ok()))
        .collect();
    all.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then_with(|| b.id.cmp(&a.id)));
    all
}

pub fn delete(id: &str) -> Result<()> {
    let dir = folder(id)?;
    if !dir.join(META).is_file() {
        return Err(Error::NotFound(format!("recording {id}")));
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `test` with the data folder pointing at a throwaway folder. The data folder is read
    /// from the environment, so these tests take turns.
    fn with_home(test: impl FnOnce()) {
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join(format!("emulabzip-library-{}-{}", std::process::id(), new_id()));
        // SAFETY: only these tests change the variable, and they hold the lock above.
        unsafe { std::env::set_var("EMULABZIP_HOME", &home) };
        test();
        std::fs::remove_dir_all(home).ok();
    }

    fn record(id: &str) -> Recording {
        let dir = folder(id).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("trace.pftrace"), b"0123456789").unwrap();
        std::fs::write(dir.join("extra.txt"), b"abc").unwrap();
        let file = TraceFile {
            path: dir.join("trace.pftrace"),
            format: TraceFormat::Perfetto,
            extras: vec![dir.join("extra.txt")],
        };
        let request = TraceRequest::Perfetto {
            preset: emulabzip_core::PerfettoPreset::Cpu,
            config: None,
            duration_s: 5,
            app: None,
        };
        Recording::save(id, Platform::Android, "Pixel 9", &request, &file).unwrap()
    }

    #[test]
    fn dates_come_out_right() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_723), (2024, 1, 1));
        assert_eq!(civil(20_507), (2026, 2, 23));
        let id = new_id();
        assert!(check_id(&id).is_ok() && id.len() == 20, "{id}");
    }

    #[test]
    fn saves_lists_and_deletes_recordings() {
        with_home(|| {
            let a = record("20260101-000000-aaaa");
            assert_eq!(
                (a.file.as_str(), a.extras.as_slice(), a.size_bytes),
                ("trace.pftrace", ["extra.txt".to_string()].as_slice(), 13)
            );
            let b = record("20260102-000000-bbbb");
            assert_eq!(get(&a.id).unwrap(), a);
            let ids: Vec<String> = list().into_iter().map(|r| r.id).collect();
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&b.id));
            assert!(a.path().is_file() && a.extra_paths()[0].is_file());

            delete(&a.id).unwrap();
            assert!(get(&a.id).is_err() && !a.dir().exists());
            assert_eq!(list().len(), 1);
            assert!(matches!(delete(&a.id), Err(Error::NotFound(_))));
        });
    }

    #[test]
    fn ignores_folders_that_are_not_recordings() {
        with_home(|| {
            std::fs::create_dir_all(root().join("stray")).unwrap();
            std::fs::create_dir_all(root().join("broken")).unwrap();
            std::fs::write(root().join("broken").join(META), b"{").unwrap();
            assert!(list().is_empty());
        });
    }

    #[test]
    fn ids_cannot_leave_the_library() {
        for bad in ["", "..", "../x", "a/b", "a\\b", "a b", &"x".repeat(41)] {
            assert!(folder(bad).is_err(), "{bad:?}");
            assert!(delete(bad).is_err());
        }
    }
}
