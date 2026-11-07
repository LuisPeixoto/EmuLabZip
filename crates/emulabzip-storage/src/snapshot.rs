//! An app's data kept as one `.ezstate` file, and the steps that save it and put it back.
//!
//! The file is a zip: `state.json` says what it is, `data.tar` holds the app's private data (and `external.tar` what it
//! keeps on shared storage), and `schemas.json` describes the databases and preferences inside, so two snapshots can be
//! compared without unpacking either. Putting a snapshot back replaces the app's data, so it saves the data as it is
//! first, to a file of its own, and a restore can be undone.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use emulabzip_core::tarball;
use emulabzip_core::{DATA_TAR, EXTERNAL_TAR, Error, Lab, Platform, Result, StateFiles, Target};
use serde::{Deserialize, Serialize};

/// The version of the layout. Readers refuse a newer one.
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "ezstate";
pub const META: &str = "state.json";
pub const SCHEMAS: &str = "schemas.json";

/// The most an archive entry other than the two tar files may hold.
const MAX_SMALL: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateMeta {
    pub version: u32,
    /// The EmuLabZip that wrote it.
    pub tool: String,
    pub platform: Platform,
    pub app: String,
    /// The version of the app the data belongs to.
    pub app_version: Option<String>,
    /// The device it was taken from, by name.
    pub device: String,
    /// Milliseconds since the Unix epoch.
    pub taken_ms: u64,
    pub files: u64,
    pub bytes: u64,
    /// The file also holds what the app keeps on shared storage.
    pub external: bool,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("a state file", detail)
}

/// Writes a `.ezstate` file. It is written next to its final name and moved into place, so a half-written file never has
/// the real name.
pub fn write(out: &Path, meta: &StateMeta, files: &StateFiles, schemas: Option<&[u8]>) -> Result<()> {
    let tmp = out.with_extension("ezstate.tmp");
    {
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(File::create(&tmp)?));
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut put = |name: &str, options, bytes: &mut dyn Read| -> Result<()> {
            zip.start_file(name, options).map_err(|e| bad(e.to_string()))?;
            std::io::copy(bytes, &mut zip)?;
            Ok(())
        };
        let meta_json = serde_json::to_vec_pretty(meta).map_err(|e| bad(e.to_string()))?;
        put(META, deflated, &mut meta_json.as_slice())?;
        put(DATA_TAR, stored, &mut File::open(&files.data)?)?;
        if let Some(external) = &files.external {
            put(EXTERNAL_TAR, stored, &mut File::open(external)?)?;
        }
        if let Some(schemas) = schemas {
            put(SCHEMAS, deflated, &mut &*schemas)?;
        }
        zip.finish().map_err(|e| bad(e.to_string()))?.flush()?;
    }
    std::fs::rename(&tmp, out)?;
    Ok(())
}

/// An opened `.ezstate` file.
#[derive(Debug)]
pub struct StateFile {
    pub meta: StateMeta,
    path: PathBuf,
}

/// Opens a `.ezstate` file and reads what it says it is. Nothing is unpacked.
pub fn open(path: &Path) -> Result<StateFile> {
    let file = File::open(path).map_err(|e| Error::failed(format!("could not open {}: {e}", path.display())))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| bad(format!("{} is not a state file", path.display())))?;
    let meta: StateMeta = {
        let mut entry = zip.by_name(META).map_err(|_| bad(format!("{} has no {META}", path.display())))?;
        let mut bytes = Vec::new();
        (&mut entry).take(MAX_SMALL as u64).read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes).map_err(|e| bad(format!("{META}: {e}")))?
    };
    if meta.version > VERSION {
        return Err(Error::failed(format!(
            "This state was written by a newer EmuLabZip (format {}, this one reads up to {VERSION}). Update EmuLabZip to open it.",
            meta.version
        )));
    }
    if zip.by_name(DATA_TAR).is_err() {
        return Err(bad(format!("{} has no {DATA_TAR}", path.display())));
    }
    Ok(StateFile { meta, path: path.to_path_buf() })
}

impl StateFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Unpacks the tar files into `dir`, to hand to a backend.
    pub fn extract(&self, dir: &Path) -> Result<StateFiles> {
        std::fs::create_dir_all(dir)?;
        let mut zip = zip::ZipArchive::new(File::open(&self.path)?).map_err(|e| bad(e.to_string()))?;
        let mut copy = |name: &str| -> Result<Option<PathBuf>> {
            let Ok(mut entry) = zip.by_name(name) else { return Ok(None) };
            let to = dir.join(name);
            std::io::copy(&mut entry, &mut File::create(&to)?)?;
            Ok(Some(to))
        };
        let data = copy(DATA_TAR)?.ok_or_else(|| bad(format!("no {DATA_TAR}")))?;
        let external = copy(EXTERNAL_TAR)?;
        Ok(StateFiles { data, external, app_version: self.meta.app_version.clone() })
    }

    /// The `schemas.json` of the file, when it has one.
    pub fn schemas_json(&self) -> Result<Option<Vec<u8>>> {
        let mut zip = zip::ZipArchive::new(File::open(&self.path)?).map_err(|e| bad(e.to_string()))?;
        let Ok(entry) = zip.by_name(SCHEMAS) else { return Ok(None) };
        let mut bytes = Vec::new();
        entry.take(MAX_SMALL as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > MAX_SMALL {
            return Err(bad(format!("{SCHEMAS} is too large")));
        }
        Ok(Some(bytes))
    }
}

fn scratch() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("emulabzip-state-{}-{:x}", std::process::id(), now_ms()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Saves an app's data from a device into a `.ezstate` file. The app is stopped first. `schemas` describes what is inside
/// (see the schema scan), when the caller has it.
pub async fn save(lab: &Lab, target: &Target, app: &str, out: &Path, include_external: bool) -> Result<StateMeta> {
    if out.extension().is_none_or(|e| e != EXTENSION) {
        return Err(Error::failed(format!("The file should end in .{EXTENSION}, for example app.{EXTENSION}")));
    }
    if out.exists() {
        return Err(Error::failed(format!("{} exists already. Pick another name, or delete it.", out.display())));
    }
    let device = lab.resolve(Some(&target.id)).await?;
    let dir = scratch()?;
    let result = async {
        let files = lab.save_state(target, app, &dir, include_external).await?;
        let entries = tarball::check(&files.data)?;
        let bytes = entries.iter().filter(|e| !e.is_dir).map(|e| e.size).sum();
        let meta = StateMeta {
            version: VERSION,
            tool: format!("EmuLabZip {}", env!("CARGO_PKG_VERSION")),
            platform: target.platform,
            app: app.to_string(),
            app_version: files.app_version.clone(),
            device: device.name.clone(),
            taken_ms: now_ms(),
            files: entries.iter().filter(|e| !e.is_dir).count() as u64,
            bytes,
            external: files.external.is_some(),
        };
        let schemas = crate::schema::scan_json(&files.data).ok();
        write(out, &meta, &files, schemas.as_deref())?;
        Ok(meta)
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// What came of putting a snapshot back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The data as it was just before, saved so the restore can be undone.
    pub backup: PathBuf,
    /// Things worth a look: another version of the app, for example.
    pub warnings: Vec<String>,
}

/// Where the automatic backups go.
pub fn backup_dir() -> PathBuf {
    emulabzip_core::data_dir().join("states").join("auto")
}

/// Puts a snapshot back. It has to be for this app on this platform. The data as it is now is saved first, into
/// [`backup_dir`]; if that fails, nothing is replaced.
pub async fn restore(lab: &Lab, target: &Target, file: &Path, confirm: bool) -> Result<Restored> {
    restore_with_backup(lab, target, file, confirm, &backup_dir()).await
}

/// [`restore`], with the backup going to `backups` instead of the usual folder.
pub async fn restore_with_backup(
    lab: &Lab,
    target: &Target,
    file: &Path,
    confirm: bool,
    backups: &Path,
) -> Result<Restored> {
    let state = open(file)?;
    if state.meta.platform != target.platform {
        return Err(Error::failed(format!(
            "This state was taken on {} and cannot be put back on {}.",
            state.meta.platform, target.platform
        )));
    }
    if !confirm {
        return Err(Error::NeedsConfirmation("Restoring an app's state"));
    }
    let app = state.meta.app.clone();
    let mut warnings = Vec::new();
    // The version the app has now, from the device, to say when it differs.
    if let Ok(apps) = lab.apps(target).await
        && let Some(now) = apps.iter().find(|a| a.id == app).and_then(|a| a.version.clone())
        && state.meta.app_version.as_ref().is_some_and(|saved| *saved != now)
    {
        warnings.push(format!(
            "The state is from {} {}, and the app on the device is {now}. A database from another version may not suit it.",
            app,
            state.meta.app_version.as_deref().unwrap_or("?")
        ));
    }

    let dir = scratch()?;
    let result = async {
        std::fs::create_dir_all(backups)?;
        let backup = backups.join(format!("{app}-{}.{EXTENSION}", now_ms()));
        save(lab, target, &app, &backup, state.meta.external).await.map_err(|e| {
            Error::failed(format!("The data as it is now could not be saved first, so nothing was replaced: {e}"))
        })?;
        let files = state.extract(&dir)?;
        lab.restore_state(target, &app, &files.data, files.external.as_deref(), true).await?;
        Ok(Restored { backup, warnings })
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result
}
