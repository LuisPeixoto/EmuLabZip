//! Commands that change what an app keeps: rows, preferences and property lists. Each needs `confirm`, which the
//! interface sets only after asking the person, stops the app, keeps the old file and reads the new one back.

use emulabzip_core::Target;
use emulabzip_storage::edit::device::{Backup, Place, backup_dir, edit_database, edit_file, undo};
use emulabzip_storage::edit::plist::{self, PlistValue};
use emulabzip_storage::edit::prefs::{self, PrefValue};
use emulabzip_storage::edit::sqlite::{Cell, RowKey};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

/// Which file, on which device.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Where {
    pub target: Target,
    pub app: String,
    pub root: String,
    pub path: String,
}

impl Where {
    fn place(&self) -> Place<'_> {
        Place { app: &self.app, root: &self.root, path: &self.path }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edited {
    /// The folder `edit_undo` takes to put the old file back.
    pub backup: String,
    /// The row an insert made.
    pub rowid: Option<i64>,
}

fn done(backup: &Backup, rowid: Option<i64>) -> Edited {
    Edited { backup: backup.dir.display().to_string(), rowid }
}

#[tauri::command]
pub async fn edit_cells(
    state: State<'_, AppState>,
    at: Where,
    table: String,
    key: RowKey,
    changes: Vec<(String, Cell)>,
    confirm: bool,
) -> CmdResult<Edited> {
    let backup = edit_database(&state.lab, &at.target, at.place(), confirm, &backup_dir(), |e| {
        for (column, value) in &changes {
            e.update(&table, &key, column, value)?;
        }
        Ok(())
    })
    .await
    .map_err(err)?;
    Ok(done(&backup, None))
}

#[tauri::command]
pub async fn edit_insert(
    state: State<'_, AppState>,
    at: Where,
    table: String,
    values: Vec<(String, Cell)>,
    confirm: bool,
) -> CmdResult<Edited> {
    let mut rowid = 0;
    let backup = edit_database(&state.lab, &at.target, at.place(), confirm, &backup_dir(), |e| {
        rowid = e.insert(&table, &values)?;
        Ok(())
    })
    .await
    .map_err(err)?;
    Ok(done(&backup, Some(rowid)))
}

#[tauri::command]
pub async fn edit_delete(
    state: State<'_, AppState>,
    at: Where,
    table: String,
    key: RowKey,
    confirm: bool,
) -> CmdResult<Edited> {
    let backup = edit_database(&state.lab, &at.target, at.place(), confirm, &backup_dir(), |e| e.delete(&table, &key))
        .await
        .map_err(err)?;
    Ok(done(&backup, None))
}

/// Sets a SharedPreferences value, or removes the key when `value` is `None`.
#[tauri::command]
pub async fn edit_pref(
    state: State<'_, AppState>,
    at: Where,
    key: String,
    value: Option<PrefValue>,
    confirm: bool,
) -> CmdResult<Edited> {
    let backup = edit_file(&state.lab, &at.target, at.place(), confirm, &backup_dir(), |bytes| {
        let xml = String::from_utf8(bytes.to_vec())
            .map_err(|_| emulabzip_core::Error::failed("The preferences file is not text."))?;
        let out = match &value {
            Some(v) => prefs::set(&xml, &key, v.clone())?,
            None => prefs::remove(&xml, &key)?,
        };
        Ok(out.into_bytes())
    })
    .await
    .map_err(err)?;
    Ok(done(&backup, None))
}

/// Sets a value in a property list, or removes it when `value` is `None`. `keys` is the path to it.
#[tauri::command]
pub async fn edit_plist(
    state: State<'_, AppState>,
    at: Where,
    keys: Vec<String>,
    value: Option<PlistValue>,
    confirm: bool,
) -> CmdResult<Edited> {
    let backup = edit_file(&state.lab, &at.target, at.place(), confirm, &backup_dir(), |bytes| match &value {
        Some(v) => plist::set(bytes, &keys, v),
        None => plist::remove(bytes, &keys),
    })
    .await
    .map_err(err)?;
    Ok(done(&backup, None))
}

#[tauri::command]
pub async fn edit_undo(state: State<'_, AppState>, target: Target, backup: String, confirm: bool) -> CmdResult<()> {
    // Only a folder under the backups folder: a path from the interface is not trusted to point elsewhere.
    let dir = std::path::PathBuf::from(&backup);
    let root = backup_dir();
    if dir.parent() != Some(root.as_path()) {
        return Err("That is not a backup EmuLabZip made.".into());
    }
    let backup = Backup::load(&dir).map_err(err)?;
    undo(&state.lab, &target, &backup, confirm).await.map_err(err)
}
