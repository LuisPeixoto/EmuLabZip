//! Reading what an app keeps: databases, preferences and the other files of its sandbox. Nothing here writes.

use emulabzip_storage::sqlite::Database;
use emulabzip_storage::{FileKind, detect};

use crate::server::*;

/// Any parsed value, or the reason it could not be read, as JSON.
fn to_json<T: Serialize, E: std::fmt::Display>(read: Result<T, E>) -> Result<serde_json::Value, String> {
    read.map_err(|e| e.to_string()).and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}

const DEFAULT_ROWS: u64 = 50;
const MAX_ROWS: u64 = 500;
const MAX_FILE_FOR_DATABASE: usize = 64 * 1024 * 1024;

/// A scratch folder of its own for one call, removed afterwards.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, String> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("emulabzip-mcp-db-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tool_router(router = storage_data_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "The places an app keeps files that can be read: `data`, `external`, `cache` on Android (a debuggable app), the data, bundle and app group containers on iOS. Use a root name with list_files, read_database and read_preferences.",
        title = "List an app's storage roots",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn storage_roots(&self, Parameters(a): Parameters<args::StorageApp>) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let target = self.target(&a.device).await?;
                json(&self.lab.storage_roots(&target, &a.app).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Read a database in an app's sandbox without changing it: SQLite (with its write-ahead log, so recent writes show), a Realm file, or a Core Data store. Without `table` it lists the tables (or classes, or entities) with their columns and row counts; with `table` it reads rows; with `sql` it runs one read-only statement and refuses anything that would change data. Also recognizes React Native's AsyncStorage database.",
        title = "Read an app's database",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn read_database(&self, Parameters(a): Parameters<args::ReadDatabase>) -> CallToolResult {
        reply(self.read_database_inner(a).await)
    }

    #[tool(
        description = "Read an app's preferences or settings file as structured data: Android SharedPreferences, Preferences DataStore (and Proto DataStore), MMKV, React Native AsyncStorage, an iOS property list, or a JSON or text file. The kind is detected. For databases use read_database.",
        title = "Read an app's preferences",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn read_preferences(&self, Parameters(a): Parameters<args::ReadPreferences>) -> CallToolResult {
        reply(self.read_preferences_inner(a).await)
    }
}

impl EmuLabServer {
    async fn read_database_inner(&self, a: args::ReadDatabase) -> Result<String, String> {
        if !emulabzip_core::control::is_app_id(&a.app) {
            return Err(format!("`{}` is not a package name or bundle id", a.app));
        }
        let target = self.target(&a.device).await?;
        let bytes = self.lab.read_file(&target, &a.app, &a.root, &a.path).await.map_err(|e| e.to_string())?;
        if bytes.len() > MAX_FILE_FOR_DATABASE {
            return Err(format!(
                "that file is {} MB; at most {} MB are read",
                bytes.len() / 1024 / 1024,
                MAX_FILE_FOR_DATABASE / 1024 / 1024
            ));
        }
        let name = a.path.rsplit('/').next().unwrap_or(&a.path).to_string();
        let limit = a.limit.unwrap_or(DEFAULT_ROWS).clamp(1, MAX_ROWS);
        let offset = a.offset.unwrap_or(0);
        match detect(&a.path, &bytes) {
            FileKind::Sqlite => {
                let scratch = Scratch::new()?;
                let local = scratch.0.join(&name);
                std::fs::write(&local, &bytes).map_err(|e| e.to_string())?;
                // The write-ahead log holds the newest writes.
                for suffix in ["-wal", "-shm"] {
                    if let Ok(extra) =
                        self.lab.read_file(&target, &a.app, &a.root, &format!("{}{suffix}", a.path)).await
                    {
                        let _ = std::fs::write(scratch.0.join(format!("{name}{suffix}")), extra);
                    }
                }
                let db = Database::open(&local).map_err(|e| e.to_string())?;
                if let Some(sql) = &a.sql {
                    if sql.len() > 10_000 {
                        return Err("the statement is longer than 10000 characters".into());
                    }
                    let rows = db.query(sql, limit).map_err(|e| e.to_string())?;
                    return json(&rows);
                }
                if let Some(table) = &a.table {
                    return json(&db.rows(table, offset, limit).map_err(|e| e.to_string())?);
                }
                let mut out =
                    serde_json::json!({ "kind": "sqlite", "tables": db.tables().map_err(|e| e.to_string())? });
                if emulabzip_storage::asyncstorage::is_android_store(&db)
                    && let Ok(kv) = emulabzip_storage::asyncstorage::from_database(&db)
                {
                    out["asyncStorage"] = serde_json::to_value(kv).unwrap_or_default();
                }
                if emulabzip_storage::coredata::is_core_data(&db)
                    && let Ok(entities) = emulabzip_storage::coredata::entities(&db)
                {
                    out["coreDataEntities"] = serde_json::to_value(&entities).unwrap_or_default();
                    if let Some(entity) = &a.table
                        && let Some(found) = entities.iter().find(|e| &e.name == entity)
                    {
                        return json(
                            &emulabzip_storage::coredata::rows(&db, found, offset, limit).map_err(|e| e.to_string())?,
                        );
                    }
                }
                json(&out)
            }
            FileKind::Realm => match &a.table {
                None => json(
                    &serde_json::json!({ "kind": "realm", "realm": emulabzip_storage::realm::read(&bytes).map_err(|e| e.to_string())? }),
                ),
                Some(class) => {
                    json(&emulabzip_storage::realm::rows(&bytes, class, offset, limit).map_err(|e| e.to_string())?)
                }
            },
            other => Err(format!(
                "that file is not a database ({other:?}); read_preferences reads preferences and other files"
            )),
        }
    }

    async fn read_preferences_inner(&self, a: args::ReadPreferences) -> Result<String, String> {
        if !emulabzip_core::control::is_app_id(&a.app) {
            return Err(format!("`{}` is not a package name or bundle id", a.app));
        }
        let target = self.target(&a.device).await?;
        let bytes = self.lab.read_file(&target, &a.app, &a.root, &a.path).await.map_err(|e| e.to_string())?;
        let kind = detect(&a.path, &bytes);
        let value = match kind {
            FileKind::SharedPrefs => to_json(emulabzip_storage::prefs::parse(&String::from_utf8_lossy(&bytes))),
            FileKind::DataStore => to_json(emulabzip_storage::datastore::parse_preferences(&bytes)),
            FileKind::ProtoDataStore => emulabzip_storage::datastore::parse_proto(&bytes).map_err(|e| e.to_string()),
            FileKind::Mmkv => to_json(emulabzip_storage::mmkv::parse(&bytes)),
            FileKind::AsyncStorage => {
                to_json(emulabzip_storage::asyncstorage::from_manifest(&String::from_utf8_lossy(&bytes)))
            }
            FileKind::Plist => emulabzip_storage::plist::to_json(&bytes).map_err(|e| e.to_string()),
            FileKind::Json | FileKind::Text => {
                let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_FILE)]).into_owned();
                Ok(serde_json::from_str::<serde_json::Value>(&text).unwrap_or(serde_json::Value::String(text)))
            }
            FileKind::Sqlite | FileKind::Realm => return Err("that file is a database; read_database reads it".into()),
            _ => return Err(format!("that is a {kind:?} file; read_file returns it as it is")),
        }?;
        json(&serde_json::json!({ "kind": format!("{kind:?}"), "size": bytes.len(), "value": value }))
    }
}
