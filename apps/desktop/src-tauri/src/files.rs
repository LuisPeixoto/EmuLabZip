//! Commands for the Storage tool: browsing an app's files and previewing them.

use base64::Engine;
use emulabzip_core::{FileEntry, StorageRoot, Target};
use emulabzip_storage::asyncstorage::KeyValue;
use emulabzip_storage::coredata::Entity;
use emulabzip_storage::mmkv::MmkvEntry;
use emulabzip_storage::prefs::Pref;
use emulabzip_storage::sqlite::{Database, KeyedRows, Rows, Table};
use emulabzip_storage::{FileKind, detect, image_mime};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::{CmdResult, ask_save_path, err};
use crate::state::{AppState, scratch_dir};

/// Text previews stop here; the rest of the file is still available through Save.
pub(crate) const MAX_TEXT: usize = 2 * 1024 * 1024;
pub(crate) const HEX_BYTES: usize = 4096;

#[tauri::command]
pub async fn storage_roots(state: State<'_, AppState>, target: Target, app: String) -> CmdResult<Vec<StorageRoot>> {
    state.lab.storage_roots(&target, &app).await.map_err(err)
}

#[tauri::command]
pub async fn list_files(
    state: State<'_, AppState>,
    target: Target,
    app: String,
    root: String,
    path: String,
) -> CmdResult<Vec<FileEntry>> {
    state.lab.list_files(&target, &app, &root, &path).await.map_err(err)
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub kind: Option<FileKind>,
    pub text: Option<String>,
    pub truncated: bool,
    pub image: Option<String>,
    pub prefs: Option<Vec<Pref>>,
    pub plist: Option<serde_json::Value>,
    pub tables: Option<Vec<Table>>,
    /// MMKV keys, with the type each value was guessed to be.
    pub mmkv: Option<Vec<MmkvEntry>>,
    /// React Native AsyncStorage keys and values.
    pub kv: Option<Vec<KeyValue>>,
    /// A Realm file: its classes. The rows come from `realm_rows`, with `id`.
    pub realm: Option<emulabzip_storage::realm::Realm>,
    /// A Core Data store: its entities. The rows come from `coredata_rows`, with `id`.
    pub entities: Option<Vec<Entity>>,
    pub hex: Option<String>,
    /// The viewer could not read the file; the hex view is shown instead.
    pub error: Option<String>,
}

/// Classic hex dump: offset, 16 bytes, ASCII.
pub fn hex_dump(bytes: &[u8]) -> String {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            let ascii: String =
                chunk.iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect();
            format!("{:08x}  {:<47}  |{}|", i * 16, hex.join(" "), ascii)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tauri::command]
pub async fn open_file(
    state: State<'_, AppState>,
    target: Target,
    app: String,
    root: String,
    path: String,
) -> CmdResult<Preview> {
    let bytes = state.lab.read_file(&target, &app, &root, &path).await.map_err(err)?;
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    let kind = detect(&path, &bytes);
    let mut p = Preview {
        id: state.next_id(),
        name: name.clone(),
        size: bytes.len() as u64,
        kind: Some(kind),
        ..Default::default()
    };

    let text = |bytes: &[u8]| {
        let cut = bytes.len().min(MAX_TEXT);
        (Some(String::from_utf8_lossy(&bytes[..cut]).into_owned()), bytes.len() > cut)
    };
    match kind {
        FileKind::Sqlite => {
            // Copy the database with its write-ahead log so recent writes are visible.
            let dir = scratch_dir().join(p.id.to_string());
            tokio::fs::create_dir_all(&dir).await.map_err(err)?;
            let local = dir.join(&name);
            tokio::fs::write(&local, &bytes).await.map_err(err)?;
            for suffix in ["-wal", "-shm"] {
                if let Ok(extra) = state.lab.read_file(&target, &app, &root, &format!("{path}{suffix}")).await {
                    let _ = tokio::fs::write(dir.join(format!("{name}{suffix}")), extra).await;
                }
            }
            match Database::open(&local).and_then(|db| {
                if emulabzip_storage::asyncstorage::is_android_store(&db) {
                    p.kv = emulabzip_storage::asyncstorage::from_database(&db).ok();
                }
                if emulabzip_storage::coredata::is_core_data(&db) {
                    p.entities = emulabzip_storage::coredata::entities(&db).ok();
                }
                db.tables()
            }) {
                Ok(tables) => {
                    p.tables = Some(tables);
                    state.add_database(p.id, local);
                }
                Err(e) => p.error = Some(e.to_string()),
            }
        }
        FileKind::SharedPrefs => match emulabzip_storage::prefs::parse(&String::from_utf8_lossy(&bytes)) {
            Ok(prefs) => p.prefs = Some(prefs),
            Err(e) => {
                (p.text, p.truncated) = text(&bytes);
                p.error = Some(e.to_string());
            }
        },
        FileKind::Plist => match emulabzip_storage::plist::to_json(&bytes) {
            Ok(v) => p.plist = Some(v),
            Err(e) => p.error = Some(e.to_string()),
        },
        FileKind::Image => {
            let mime = image_mime(&bytes).unwrap_or("image/png");
            p.image = Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)));
        }
        FileKind::Json | FileKind::Text => (p.text, p.truncated) = text(&bytes),
        FileKind::DataStore => match emulabzip_storage::datastore::parse_preferences(&bytes) {
            Ok(prefs) => p.prefs = Some(prefs),
            Err(e) => p.error = Some(e.to_string()),
        },
        FileKind::ProtoDataStore => match emulabzip_storage::datastore::parse_proto(&bytes) {
            Ok(v) => p.plist = Some(v),
            Err(e) => p.error = Some(e.to_string()),
        },
        FileKind::Mmkv => match emulabzip_storage::mmkv::parse(&bytes) {
            Ok(entries) => p.mmkv = Some(entries),
            Err(e) => p.error = Some(e.to_string()),
        },
        FileKind::AsyncStorage => {
            match emulabzip_storage::asyncstorage::from_manifest(&String::from_utf8_lossy(&bytes)) {
                Ok(kv) => p.kv = Some(kv),
                Err(e) => p.error = Some(e.to_string()),
            }
        }
        FileKind::Realm => match emulabzip_storage::realm::read(&bytes) {
            Ok(realm) => {
                // The rows are read from a copy, a page at a time.
                let dir = scratch_dir().join(p.id.to_string());
                tokio::fs::create_dir_all(&dir).await.map_err(err)?;
                let local = dir.join(&name);
                tokio::fs::write(&local, &bytes).await.map_err(err)?;
                state.add_database(p.id, local);
                p.realm = Some(realm);
            }
            Err(e) => p.error = Some(e.to_string()),
        },
        FileKind::Binary => {}
    }
    if p.text.is_none()
        && p.image.is_none()
        && p.prefs.is_none()
        && p.plist.is_none()
        && p.tables.is_none()
        && p.mmkv.is_none()
        && p.kv.is_none()
        && p.realm.is_none()
    {
        p.hex = Some(hex_dump(&bytes[..bytes.len().min(HEX_BYTES)]));
        p.truncated = bytes.len() > HEX_BYTES;
    }
    Ok(p)
}

fn database(state: &AppState, id: u64) -> CmdResult<Database> {
    let path = state.database(id).ok_or("The database is no longer open. Open the file again.")?;
    Database::open(&path).map_err(err)
}

#[tauri::command]
pub async fn sqlite_rows(
    state: State<'_, AppState>,
    id: u64,
    table: String,
    offset: u64,
    limit: u64,
) -> CmdResult<Rows> {
    database(&state, id)?.rows(&table, offset, limit).map_err(err)
}

#[tauri::command]
pub async fn sqlite_query(state: State<'_, AppState>, id: u64, sql: String) -> CmdResult<Rows> {
    database(&state, id)?.query(&sql, 1000).map_err(err)
}

/// The rows of a table with what is needed to change them: each row's key, and why a table cannot be changed.
#[tauri::command]
pub async fn sqlite_keyed_rows(
    state: State<'_, AppState>,
    id: u64,
    table: String,
    offset: u64,
    limit: u64,
) -> CmdResult<KeyedRows> {
    database(&state, id)?.rows_keyed(&table, offset, limit).map_err(err)
}

#[tauri::command]
pub async fn realm_rows(
    state: State<'_, AppState>,
    id: u64,
    class: String,
    offset: u64,
    limit: u64,
) -> CmdResult<emulabzip_storage::realm::Rows> {
    let path = state.database(id).ok_or("The file is no longer open. Open it again.")?;
    let bytes = tokio::fs::read(&path).await.map_err(err)?;
    emulabzip_storage::realm::rows(&bytes, &class, offset, limit).map_err(err)
}

#[tauri::command]
pub async fn coredata_rows(
    state: State<'_, AppState>,
    id: u64,
    entity: String,
    offset: u64,
    limit: u64,
) -> CmdResult<Rows> {
    let db = database(&state, id)?;
    let entities = emulabzip_storage::coredata::entities(&db).map_err(err)?;
    let found = entities.iter().find(|e| e.name == entity).ok_or("That entity is not in the store.")?;
    emulabzip_storage::coredata::rows(&db, found, offset, limit).map_err(err)
}

/// Saves a file from the device to a path the user picks.
#[tauri::command]
pub async fn save_file(
    handle: AppHandle,
    state: State<'_, AppState>,
    target: Target,
    app: String,
    root: String,
    path: String,
) -> CmdResult<Option<String>> {
    let bytes = state.lab.read_file(&target, &app, &root, &path).await.map_err(err)?;
    let name = path.rsplit('/').next().unwrap_or("file");
    let Some(dest) = ask_save_path(&handle, name, ("All files", &["*"])).await else { return Ok(None) };
    tokio::fs::write(&dest, bytes).await.map_err(err)?;
    Ok(Some(dest.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_a_hex_dump() {
        let dump = hex_dump(b"\x89PNG\r\n\x1a\nhello world, 0123");
        let lines: Vec<&str> = dump.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("00000000  89 50 4e 47 0d 0a 1a 0a 68 65 6c 6c 6f 20 77 6f  |.PNG....hello wo|"));
        assert!(lines[1].starts_with("00000010  72 6c 64"));
    }
}
