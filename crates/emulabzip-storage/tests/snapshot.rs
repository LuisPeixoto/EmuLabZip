//! Saving and restoring an app's state through a fake device: the sandbox is a folder on this machine. No device is used.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use emulabzip_core::tarball;
use emulabzip_core::{
    App, Backend, DATA_TAR, Device, DeviceKind, DeviceState, Error, InfoSection, Lab, LogEntry, LogOptions, Platform,
    Result, StateFiles, Stream, Target,
};
use emulabzip_storage::snapshot::{self, restore_with_backup};

struct Fake {
    platform: Platform,
    sandbox: PathBuf,
    version: Arc<std::sync::Mutex<String>>,
}

#[async_trait]
impl Backend for Fake {
    fn platform(&self) -> Platform {
        self.platform
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn devices(&self) -> Result<Vec<Device>> {
        Ok(vec![Device {
            id: "fake-1".into(),
            name: "Fake Pixel".into(),
            platform: self.platform,
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
        let version = self.version.lock().unwrap().clone();
        Ok(vec![App { id: "dev.x".into(), name: "X".into(), version: Some(version), pid: None, user: true }])
    }

    async fn logs(&self, _: &str, _: LogOptions) -> Result<Stream<LogEntry>> {
        Err(Error::Unsupported("logs"))
    }

    async fn screenshot(&self, _: &str) -> Result<Vec<u8>> {
        Err(Error::Unsupported("screenshots"))
    }

    async fn save_state(&self, _: &str, _: &str, out_dir: &Path, _: bool) -> Result<StateFiles> {
        let data = out_dir.join(DATA_TAR);
        tarball::pack_dir(&self.sandbox, &data, &["cache"])?;
        Ok(StateFiles { data, external: None, app_version: Some(self.version.lock().unwrap().clone()) })
    }

    async fn restore_state(&self, _: &str, _: &str, data: &Path, _: Option<&Path>) -> Result<()> {
        tarball::empty_dir(&self.sandbox, &["cache"])?;
        tarball::unpack(data, &self.sandbox)?;
        Ok(())
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-snapshot-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rig(name: &str, platform: Platform) -> (PathBuf, Lab, Target, Arc<std::sync::Mutex<String>>) {
    let dir = scratch(name);
    let sandbox = dir.join("sandbox");
    std::fs::create_dir_all(sandbox.join("databases")).unwrap();
    std::fs::create_dir_all(sandbox.join("cache")).unwrap();
    std::fs::write(sandbox.join("databases/app.db"), b"v1").unwrap();
    std::fs::write(sandbox.join("prefs.xml"), b"<map/>").unwrap();
    std::fs::write(sandbox.join("cache/junk"), b"x").unwrap();
    let version = Arc::new(std::sync::Mutex::new("1.0 (1)".to_string()));
    let lab = Lab::new().with(Fake { platform, sandbox, version: version.clone() });
    (dir, lab, Target { platform, id: "fake-1".into() }, version)
}

#[tokio::test]
async fn a_state_is_saved_to_one_file_that_says_what_it_is() {
    let (dir, lab, target, _) = rig("save", Platform::Android);
    let out = dir.join("x.ezstate");
    let meta = snapshot::save(&lab, &target, "dev.x", &out, false).await.unwrap();
    assert_eq!((meta.app.as_str(), meta.device.as_str(), meta.files), ("dev.x", "Fake Pixel", 2));
    assert_eq!(meta.app_version.as_deref(), Some("1.0 (1)"));
    assert!(!meta.external && meta.bytes > 0);

    let opened = snapshot::open(&out).unwrap();
    assert_eq!(opened.meta, meta);
    // The file carries a description of what is inside, to compare snapshots with. The fake sandbox has `prefs.xml`.
    let schemas: emulabzip_storage::schema::Schemas =
        serde_json::from_slice(&opened.schemas_json().unwrap().unwrap()).unwrap();
    assert_eq!(
        schemas.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
        Vec::<&str>::new(),
        "a bare <map/> has no keys to describe, and databases/app.db is not a database"
    );
    let files = opened.extract(&dir.join("x")).unwrap();
    let names: Vec<_> = tarball::check(&files.data).unwrap().into_iter().map(|e| e.path).collect();
    assert!(names.contains(&"databases/app.db".to_string()) && !names.iter().any(|n| n.starts_with("cache")));

    // It is never written over, and a wrong name is explained.
    assert!(
        snapshot::save(&lab, &target, "dev.x", &out, false).await.unwrap_err().to_string().contains("exists already")
    );
    assert!(
        snapshot::save(&lab, &target, "dev.x", &dir.join("x.zip"), false)
            .await
            .unwrap_err()
            .to_string()
            .contains(".ezstate")
    );
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn restoring_needs_confirming_and_keeps_a_backup_of_what_it_replaces() {
    let (dir, lab, target, _) = rig("restore", Platform::Android);
    let out = dir.join("x.ezstate");
    snapshot::save(&lab, &target, "dev.x", &out, false).await.unwrap();
    let backups = dir.join("backups");

    std::fs::write(dir.join("sandbox/databases/app.db"), b"v2 changed").unwrap();
    std::fs::write(dir.join("sandbox/new.txt"), b"added").unwrap();

    assert!(matches!(
        restore_with_backup(&lab, &target, &out, false, &backups).await.unwrap_err(),
        Error::NeedsConfirmation(_)
    ));
    assert_eq!(std::fs::read(dir.join("sandbox/databases/app.db")).unwrap(), b"v2 changed", "nothing was replaced");

    let restored = restore_with_backup(&lab, &target, &out, true, &backups).await.unwrap();
    assert_eq!(std::fs::read(dir.join("sandbox/databases/app.db")).unwrap(), b"v1");
    assert!(!dir.join("sandbox/new.txt").exists());
    assert!(restored.warnings.is_empty());

    // The backup holds the data as it was before the restore, and can be put back.
    let backup = snapshot::open(&restored.backup).unwrap();
    let files = backup.extract(&dir.join("b")).unwrap();
    assert_eq!(tarball::read_file(&files.data, "databases/app.db", 100).unwrap().as_deref(), Some(&b"v2 changed"[..]));
    restore_with_backup(&lab, &target, &restored.backup, true, &backups).await.unwrap();
    assert_eq!(std::fs::read(dir.join("sandbox/databases/app.db")).unwrap(), b"v2 changed", "the restore was undone");
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn another_platform_is_refused_and_another_version_is_a_warning() {
    let (dir, lab, target, version) = rig("mismatch", Platform::Android);
    let out = dir.join("x.ezstate");
    snapshot::save(&lab, &target, "dev.x", &out, false).await.unwrap();

    let ios = Target { platform: Platform::Ios, id: "fake-1".into() };
    let other =
        Lab::new().with(Fake { platform: Platform::Ios, sandbox: dir.join("sandbox"), version: version.clone() });
    let err = restore_with_backup(&other, &ios, &out, true, &dir.join("b")).await.unwrap_err().to_string();
    assert!(err.contains("cannot be put back"), "{err}");

    *version.lock().unwrap() = "2.0 (2)".into();
    let restored = restore_with_backup(&lab, &target, &out, true, &dir.join("b")).await.unwrap();
    assert_eq!(restored.warnings.len(), 1);
    assert!(
        restored.warnings[0].contains("1.0 (1)") && restored.warnings[0].contains("2.0 (2)"),
        "{:?}",
        restored.warnings
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn what_is_not_a_state_file_is_explained() {
    let dir = scratch("junk");
    assert!(snapshot::open(&dir.join("missing.ezstate")).is_err());
    std::fs::write(dir.join("a.ezstate"), b"not a zip").unwrap();
    assert!(snapshot::open(&dir.join("a.ezstate")).unwrap_err().to_string().contains("not a state file"));
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn two_snapshots_are_compared_by_what_is_inside_them() {
    let (dir, lab, target, _) = rig("compare", Platform::Android);
    let make_db = |extra: bool| {
        let path = dir.join("sandbox/databases/app.db");
        let _ = std::fs::remove_file(&path);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT);").unwrap();
        if extra {
            conn.execute_batch("ALTER TABLE t ADD COLUMN note TEXT;").unwrap();
        }
    };
    make_db(false);
    snapshot::save(&lab, &target, "dev.x", &dir.join("a.ezstate"), false).await.unwrap();
    make_db(true);
    snapshot::save(&lab, &target, "dev.x", &dir.join("b.ezstate"), false).await.unwrap();
    let changes = snapshot::compare(&dir.join("a.ezstate"), &dir.join("b.ezstate")).unwrap();
    let lines: Vec<String> = changes.iter().map(|c| c.line()).collect();
    assert_eq!(lines, ["+ databases/app.db: column t.note  TEXT"]);
    assert!(snapshot::compare(&dir.join("a.ezstate"), &dir.join("a.ezstate")).unwrap().is_empty());
    std::fs::remove_dir_all(dir).ok();
}
