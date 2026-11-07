//! Editing files on a fake device whose storage is a map in memory. No device is used.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use emulabzip_core::{
    App, Backend, Device, DeviceKind, DeviceState, Error, InfoSection, Lab, LogEntry, LogOptions, Platform, Result,
    Stream, Target,
};
use emulabzip_storage::edit::device::{self, Backup};
use emulabzip_storage::edit::prefs::{self, PrefValue};
use emulabzip_storage::edit::sqlite::{Cell, RowKey};
use rusqlite::Connection;

type Files = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct Fake {
    files: Files,
    stopped: Arc<Mutex<u32>>,
    /// Writes to this path are silently dropped, to see a bad write noticed.
    broken: Option<String>,
}

fn key(root: &str, path: &str) -> String {
    format!("{root}/{path}")
}

#[async_trait]
impl Backend for Fake {
    fn platform(&self) -> Platform {
        Platform::Android
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn devices(&self) -> Result<Vec<Device>> {
        Ok(vec![Device {
            id: "fake-1".into(),
            name: "Fake".into(),
            platform: Platform::Android,
            kind: DeviceKind::Emulator,
            state: DeviceState::Online,
            os_version: None,
            api_level: None,
            model: None,
            abi: None,
        }])
    }

    async fn info(&self, _: &str) -> Result<Vec<InfoSection>> {
        Ok(vec![])
    }

    async fn apps(&self, _: &str) -> Result<Vec<App>> {
        Ok(vec![])
    }

    async fn logs(&self, _: &str, _: LogOptions) -> Result<Stream<LogEntry>> {
        Err(Error::Unsupported("logs"))
    }

    async fn screenshot(&self, _: &str) -> Result<Vec<u8>> {
        Err(Error::Unsupported("screenshots"))
    }

    async fn stop_app(&self, _: &str, _: &str) -> Result<()> {
        *self.stopped.lock().unwrap() += 1;
        Ok(())
    }

    async fn read_file(&self, _: &str, _: &str, root: &str, path: &str) -> Result<Vec<u8>> {
        self.files.lock().unwrap().get(&key(root, path)).cloned().ok_or_else(|| Error::NotFound(path.into()))
    }

    async fn write_file(&self, _: &str, _: &str, root: &str, path: &str, bytes: &[u8]) -> Result<()> {
        if self.broken.as_deref() != Some(path) {
            self.files.lock().unwrap().insert(key(root, path), bytes.to_vec());
        }
        Ok(())
    }

    async fn remove_file(&self, _: &str, _: &str, root: &str, path: &str) -> Result<()> {
        self.files.lock().unwrap().remove(&key(root, path));
        Ok(())
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-editdev-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rig(files: &[(&str, Vec<u8>)], broken: Option<&str>) -> (Lab, Target, Files, Arc<Mutex<u32>>) {
    let map: Files = Arc::new(Mutex::new(files.iter().map(|(p, b)| (key("data", p), b.clone())).collect()));
    let stopped = Arc::new(Mutex::new(0));
    let lab =
        Lab::new().with(Fake { files: map.clone(), stopped: stopped.clone(), broken: broken.map(str::to_string) });
    (lab, Target { platform: Platform::Android, id: "fake-1".into() }, map, stopped)
}

const PREFS: &str = "<?xml version='1.0' encoding='utf-8' standalone='yes' ?>\n<map>\n    <int name=\"launches\" value=\"3\" />\n</map>\n";

#[tokio::test]
async fn a_file_is_edited_checked_and_can_be_put_back() {
    let dir = scratch("file");
    let (lab, t, files, stopped) = rig(&[("shared_prefs/p.xml", PREFS.as_bytes().to_vec())], None);
    let backup = device::edit_file(&lab, &t, "dev.x", "data", "shared_prefs/p.xml", true, &dir, |bytes| {
        Ok(prefs::set(std::str::from_utf8(bytes).unwrap(), "launches", PrefValue::Int(4))?.into_bytes())
    })
    .await
    .unwrap();
    assert_eq!(*stopped.lock().unwrap(), 1, "the app is stopped first");
    let now = String::from_utf8(files.lock().unwrap()["data/shared_prefs/p.xml"].clone()).unwrap();
    assert!(now.contains("value=\"4\""));

    // The backup is on disk, can be read back from its folder, and puts the old file back.
    let loaded = Backup::load(&backup.dir).unwrap();
    assert_eq!(loaded, backup);
    device::undo(&lab, &t, &loaded, true).await.unwrap();
    assert_eq!(files.lock().unwrap()["data/shared_prefs/p.xml"], PREFS.as_bytes());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn an_edit_that_does_not_work_leaves_the_device_alone() {
    let dir = scratch("fail");
    let (lab, t, files, stopped) = rig(&[("a.xml", PREFS.as_bytes().to_vec())], None);
    let err = device::edit_file(&lab, &t, "dev.x", "data", "a.xml", true, &dir, |_| Err(Error::failed("no")))
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "no");
    assert_eq!(*stopped.lock().unwrap(), 0, "the app was not even stopped");
    assert_eq!(files.lock().unwrap()["data/a.xml"], PREFS.as_bytes());
    // Not confirmed: refused, nothing touched.
    let err = device::edit_file(&lab, &t, "dev.x", "data", "a.xml", false, &dir, |b| Ok(b.to_vec())).await.unwrap_err();
    assert!(matches!(err, Error::NeedsConfirmation(_)));
    // A file that is not there.
    assert!(device::edit_file(&lab, &t, "dev.x", "data", "nope.xml", true, &dir, |b| Ok(b.to_vec())).await.is_err());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_write_that_did_not_take_is_noticed_and_the_old_file_stays() {
    let dir = scratch("broken");
    let (lab, t, files, _) = rig(&[("a.xml", PREFS.as_bytes().to_vec())], Some("a.xml"));
    let err = device::edit_file(&lab, &t, "dev.x", "data", "a.xml", true, &dir, |_| Ok(b"different".to_vec()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not what was written"), "{err}");
    assert_eq!(files.lock().unwrap()["data/a.xml"], PREFS.as_bytes());
    std::fs::remove_dir_all(dir).ok();
}

fn database_bytes(dir: &Path, wal: bool) -> (Vec<u8>, Option<Vec<u8>>) {
    let path = dir.join("make.db");
    let conn = Connection::open(&path).unwrap();
    if wal {
        conn.pragma_update(None, "journal_mode", "wal").unwrap();
        conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    }
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT); INSERT INTO t (v) VALUES ('one'), ('two');")
        .unwrap();
    // Everything above sits in the log when it is on: copy the files before the connection folds them in.
    let main = std::fs::read(&path).unwrap();
    let wal_bytes = std::fs::read(path.with_extension("db-wal")).ok();
    let main_now = if wal { main } else { std::fs::read(&path).unwrap() };
    std::mem::forget(conn);
    (main_now, wal_bytes)
}

#[tokio::test]
async fn a_database_is_edited_with_its_log_folded_in_and_removed_from_the_device() {
    let dir = scratch("db");
    let (main, wal) = database_bytes(&dir, true);
    let wal = wal.expect("a log");
    assert!(!wal.is_empty());
    let (lab, t, files, _) = rig(
        &[("databases/app.db", main.clone()), ("databases/app.db-wal", wal), ("databases/app.db-shm", vec![0; 32768])],
        None,
    );
    let backup = device::edit_database(&lab, &t, "dev.x", "data", "databases/app.db", true, &dir, |e| {
        e.update("t", &RowKey::Rowid(2), "v", &Cell::Text("edited".into()))
    })
    .await
    .unwrap();
    {
        let map = files.lock().unwrap();
        assert!(
            !map.contains_key("data/databases/app.db-wal") && !map.contains_key("data/databases/app.db-shm"),
            "the log is gone"
        );
        // The file alone holds the rows the log held, and the edit.
        let alone = dir.join("alone.db");
        std::fs::write(&alone, &map["data/databases/app.db"]).unwrap();
        let conn = Connection::open(&alone).unwrap();
        let rows: Vec<String> = conn
            .prepare("SELECT v FROM t ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(rows, ["one", "edited"]);
    }
    // Undo puts the three files back as they were, the log included.
    device::undo(&lab, &t, &backup, true).await.unwrap();
    let map = files.lock().unwrap();
    assert_eq!(map["data/databases/app.db"], main);
    assert!(map.contains_key("data/databases/app.db-wal") && map.contains_key("data/databases/app.db-shm"));
    drop(map);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_database_edit_that_fails_changes_nothing_on_the_device() {
    let dir = scratch("dbfail");
    let (main, _) = database_bytes(&dir, false);
    let (lab, t, files, _) = rig(&[("databases/app.db", main.clone())], None);
    let err = device::edit_database(&lab, &t, "dev.x", "data", "databases/app.db", true, &dir, |e| {
        e.update("t", &RowKey::Rowid(99), "v", &Cell::Text("x".into()))
    })
    .await
    .unwrap_err();
    assert!(err.to_string().contains("instead of one"), "{err}");
    assert_eq!(files.lock().unwrap()["data/databases/app.db"], main);
    let err = device::edit_database(&lab, &t, "dev.x", "data", "databases/app.db", false, &dir, |_| Ok(()))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NeedsConfirmation(_)));
    std::fs::remove_dir_all(dir).ok();
}
