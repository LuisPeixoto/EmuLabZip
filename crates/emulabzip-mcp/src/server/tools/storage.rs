//! The storage tools.

use crate::server::*;

#[tool_router(router = storage_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "Save an app's data (databases, preferences, files; not caches) to a state file kept by EmuLabZip, to compare later with diff_state. Stops the app. Putting a state back is not offered here."
    )]
    async fn save_state(&self, Parameters(a): Parameters<args::SaveState>) -> CallToolResult {
        reply(
            async {
                let name = match &a.name {
                    Some(n) if valid_session_id(n) => n.clone(),
                    Some(n) => return Err(format!("`{n}` is not a plain name")),
                    None => {
                        let ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis());
                        format!("{}-{ms}", slug(&a.app))
                    }
                };
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let dir = states_dir();
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let out = dir.join(format!("{name}.{}", emulabzip_storage::snapshot::EXTENSION));
                let target = self.target(&a.device).await?;
                let meta = emulabzip_storage::snapshot::save(&self.lab, &target, &a.app, &out, false)
                    .await
                    .map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "name": name, "file": out.display().to_string(), "state": meta }))
            }
            .await,
        )
    }

    #[tool(description = "The state files EmuLabZip has saved, newest first, by the names diff_state takes.")]
    async fn list_states(&self) -> CallToolResult {
        reply(states_list())
    }

    #[tool(
        description = "What changed in the structure of an app's data between two saved states, or between a saved state and the app now: tables, columns, indexes and preference keys added, removed or changed."
    )]
    async fn diff_state(&self, Parameters(a): Parameters<args::DiffState>) -> CallToolResult {
        reply(
            async {
                let before = state_file(&a.before)?;
                let changes = match &a.after {
                    Some(after) => emulabzip_storage::snapshot::compare(&before, &state_file(after)?),
                    None => {
                        let app = emulabzip_storage::snapshot::open(&before).map_err(|e| e.to_string())?.meta.app;
                        let target = self.target(&a.device).await?;
                        let now = std::env::temp_dir().join(format!(
                            "emulabzip-mcp-now-{}.{}",
                            std::process::id(),
                            emulabzip_storage::snapshot::EXTENSION
                        ));
                        let _ = std::fs::remove_file(&now);
                        let saved = emulabzip_storage::snapshot::save(&self.lab, &target, &app, &now, false).await;
                        let result = saved.and_then(|_| emulabzip_storage::snapshot::compare(&before, &now));
                        let _ = std::fs::remove_file(&now);
                        result
                    }
                }
                .map_err(|e| e.to_string())?;
                json(&serde_json::json!({
                    "changes": changes.iter().map(|c| c.line()).collect::<Vec<_>>(),
                    "detail": changes,
                }))
            }
            .await,
        )
    }

    #[tool(
        description = "List an app's storage roots, or a folder in one. Android needs a debuggable app for the private `data` root."
    )]
    async fn list_files(&self, Parameters(a): Parameters<args::Files>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                match a.root.as_deref() {
                    None if a.path.is_none() => {
                        json(&self.lab.storage_roots(&target, &a.app).await.map_err(|e| e.to_string())?)
                    }
                    root => {
                        let entries = self
                            .lab
                            .list_files(&target, &a.app, root.unwrap_or("data"), a.path.as_deref().unwrap_or(""))
                            .await;
                        json(&entries.map_err(|e| e.to_string())?)
                    }
                }
            }
            .await,
        )
    }

    #[tool(description = "Read one file of an app: text as it is, binary files as base64. Files over 256 KB are cut.")]
    async fn read_file(&self, Parameters(a): Parameters<args::Files>) -> CallToolResult {
        reply(self.read_file_inner(a).await)
    }
}
