//! Changing a file that lives on a device: stop the app, pull the file, change it here, put it back, check it, and keep
//! what was there so the change can be undone.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use emulabzip_core::{Error, Lab, Result, Target};
use serde::{Deserialize, Serialize};

use super::sqlite::Editor;

/// Where the files an edit replaced are kept.
pub fn backup_dir() -> PathBuf {
    emulabzip_core::data_dir().join("states").join("edits")
}

/// What an edit replaced, kept so it can be put back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub dir: PathBuf,
    pub app: String,
    pub root: String,
    /// Each file the edit touched, by its path in the root, and whether it existed (then a copy is in `dir`).
    pub files: Vec<BackedUp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackedUp {
    pub path: String,
    pub existed: bool,
}

fn stamp() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{nanos:x}")
}

impl Backup {
    /// Reads a backup back from its folder.
    pub fn load(dir: &Path) -> Result<Backup> {
        let text = std::fs::read_to_string(dir.join("backup.json"))
            .map_err(|e| Error::failed(format!("{} is not a backup of an edit: {e}", dir.display())))?;
        serde_json::from_str(&text).map_err(|e| Error::parse("an edit backup", e.to_string()))
    }
}

async fn read_if_there(lab: &Lab, target: &Target, app: &str, root: &str, path: &str) -> Option<Vec<u8>> {
    lab.read_file(target, app, root, path).await.ok()
}

/// Keeps `paths` as they are now. Missing ones are noted as such.
async fn back_up(
    lab: &Lab,
    target: &Target,
    app: &str,
    root: &str,
    paths: &[String],
    backups: &Path,
) -> Result<Backup> {
    let dir = backups.join(format!("{app}-{}", stamp()));
    std::fs::create_dir_all(&dir)?;
    let mut files = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        match read_if_there(lab, target, app, root, path).await {
            Some(bytes) => {
                std::fs::write(dir.join(format!("{i}.bin")), bytes)?;
                files.push(BackedUp { path: path.clone(), existed: true });
            }
            None => files.push(BackedUp { path: path.clone(), existed: false }),
        }
    }
    let backup = Backup { dir: dir.clone(), app: app.to_string(), root: root.to_string(), files };
    std::fs::write(
        dir.join("backup.json"),
        serde_json::to_vec_pretty(&backup).map_err(|e| Error::failed(e.to_string()))?,
    )?;
    Ok(backup)
}

/// Changes a file with `change`, which gets its bytes and returns the new ones. The app is stopped first, and the
/// file is read back afterwards to see it is what was written.
pub async fn edit_file(
    lab: &Lab,
    target: &Target,
    app: &str,
    root: &str,
    path: &str,
    confirm: bool,
    backups: &Path,
    change: impl FnOnce(&[u8]) -> Result<Vec<u8>>,
) -> Result<Backup> {
    if !confirm {
        return Err(Error::NeedsConfirmation("Editing a file of the app"));
    }
    let before = lab.read_file(target, app, root, path).await?;
    // The edit is made before anything on the device is touched, so one that does not work leaves it alone.
    let after = change(&before)?;
    lab.stop_app(target, app).await?;
    let backup = back_up(lab, target, app, root, &[path.to_string()], backups).await?;
    lab.write_file(target, app, root, path, &after, true).await?;
    let now = lab.read_file(target, app, root, path).await?;
    if now != after {
        // Put the old file back rather than leave a half-written one.
        let _ = lab.write_file(target, app, root, path, &before, true).await;
        return Err(Error::failed("The file on the device is not what was written, so the old one was put back."));
    }
    Ok(backup)
}

/// Changes a SQLite database with `change`. The database and its write-ahead log are pulled, changed here, folded into one
/// file and put back, and the log on the device is removed so it cannot be replayed over the edit.
pub async fn edit_database(
    lab: &Lab,
    target: &Target,
    app: &str,
    root: &str,
    path: &str,
    confirm: bool,
    backups: &Path,
    change: impl FnOnce(&mut Editor) -> Result<()>,
) -> Result<Backup> {
    if !confirm {
        return Err(Error::NeedsConfirmation("Editing a database of the app"));
    }
    lab.stop_app(target, app).await?;
    let paths = [path.to_string(), format!("{path}-wal"), format!("{path}-shm")];
    let main = lab.read_file(target, app, root, path).await?;

    let work = std::env::temp_dir().join(format!("emulabzip-edit-{}-{}", std::process::id(), stamp()));
    std::fs::create_dir_all(&work)?;
    let local = work.join("db");
    let result = async {
        std::fs::write(&local, &main)?;
        for suffix in ["-wal", "-shm"] {
            if let Some(bytes) = read_if_there(lab, target, app, root, &format!("{path}{suffix}")).await {
                std::fs::write(work.join(format!("db{suffix}")), bytes)?;
            }
        }
        let mut editor = Editor::open(&local)?;
        change(&mut editor)?;
        editor.finish()?;
        let after = std::fs::read(&local)?;

        let backup = back_up(lab, target, app, root, &paths, backups).await?;
        lab.write_file(target, app, root, path, &after, true).await?;
        // The log belongs to the old file: left in place it would be replayed over the new one.
        lab.remove_file(target, app, root, &paths[1], true).await?;
        lab.remove_file(target, app, root, &paths[2], true).await?;
        if lab.read_file(target, app, root, path).await? != after {
            let _ = undo(lab, target, &backup, true).await;
            return Err(Error::failed(
                "The database on the device is not what was written, so the old one was put back.",
            ));
        }
        Ok(backup)
    }
    .await;
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Puts back what an edit replaced.
pub async fn undo(lab: &Lab, target: &Target, backup: &Backup, confirm: bool) -> Result<()> {
    if !confirm {
        return Err(Error::NeedsConfirmation("Undoing an edit"));
    }
    lab.stop_app(target, &backup.app).await?;
    for (i, file) in backup.files.iter().enumerate() {
        if file.existed {
            let bytes = std::fs::read(backup.dir.join(format!("{i}.bin")))?;
            lab.write_file(target, &backup.app, &backup.root, &file.path, &bytes, true).await?;
        } else {
            lab.remove_file(target, &backup.app, &backup.root, &file.path, true).await?;
        }
    }
    Ok(())
}
