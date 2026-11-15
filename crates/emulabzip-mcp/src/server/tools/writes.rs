//! Tools that destroy or rewrite data. They are only offered with `--allow-writes`, and each call asks the person to
//! confirm in words that say exactly what will happen.

use emulabzip_core::SnapshotAction;
use rmcp::RoleServer;
use rmcp::service::RequestContext;

use crate::server::*;

const MAX_WRITE: usize = 1024 * 1024;

#[tool_router(router = writes_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "Put a saved state (from save_state) back into its app: the app's data is replaced. The data as it is now is saved first, to the automatic backups, so this can be undone by restoring that backup. Asks the person to confirm.",
        title = "Restore an app's state",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn restore_state(
        &self,
        Parameters(a): Parameters<args::RestoreState>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                let file = state_file(&a.state)?;
                let (device, target) = self.device(&a.device).await?;
                self.authorize(
                    &context,
                    &format!(
                        "Replace the data of the app in state \"{}\" on {} with that saved state? The data as it is now is backed up first.",
                        a.state, device.name
                    ),
                )
                .await?;
                let done = emulabzip_storage::snapshot::restore(&self.lab, &target, &file, true)
                    .await
                    .map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "restored": a.state, "backup": done.backup.display().to_string(), "warnings": done.warnings }))
            }
            .await,
        )
    }

    #[tool(
        description = "Erase a device: every app and all its data are deleted, and it starts fresh. Asks the person to confirm.",
        title = "Wipe a device",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn wipe_device(
        &self,
        Parameters(a): Parameters<args::DeviceRef>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                let device = self.find_device(&a.device, a.platform.as_deref()).await?;
                self.authorize(&context, &format!("Wipe {}? All its apps and data are deleted.", device.name)).await?;
                self.lab.wipe_device(&device.target(), true).await.map_err(|e| e.to_string())?;
                Ok(format!("Wiped {}", device.name))
            }
            .await,
        )
    }

    #[tool(
        description = "Delete an emulator or simulator for good, with its apps and data. Asks the person to confirm.",
        title = "Delete a device",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn delete_device(
        &self,
        Parameters(a): Parameters<args::DeviceRef>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                let device = self.find_device(&a.device, a.platform.as_deref()).await?;
                self.authorize(&context, &format!("Delete {} for good, with all its apps and data?", device.name))
                    .await?;
                self.lab.delete_device(&device.target(), true).await.map_err(|e| e.to_string())?;
                Ok(format!("Deleted {}", device.name))
            }
            .await,
        )
    }

    #[tool(
        description = "Load an emulator snapshot: the emulator goes back to it, and the state it is in now is thrown away. Asks the person to confirm.",
        title = "Load an emulator snapshot",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn load_emulator_snapshot(
        &self,
        Parameters(a): Parameters<args::EmulatorSnapshotArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(self.snapshot_action(&context, a, SnapshotAction::Load).await)
    }

    #[tool(
        description = "Delete an emulator snapshot. Asks the person to confirm.",
        title = "Delete an emulator snapshot",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn delete_emulator_snapshot(
        &self,
        Parameters(a): Parameters<args::EmulatorSnapshotArgs>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(self.snapshot_action(&context, a, SnapshotAction::Delete).await)
    }

    #[tool(
        description = "Replace a file in an app's own storage with text. Save the state first (save_state) if the old content matters. Asks the person to confirm.",
        title = "Write a file in an app's storage",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn write_file(
        &self,
        Parameters(a): Parameters<args::WriteFile>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                if a.text.len() > MAX_WRITE {
                    return Err("the text is longer than 1 MB".to_string());
                }
                let (device, target) = self.device(&a.device).await?;
                self.authorize(
                    &context,
                    &format!(
                        "Replace {}/{} of {} on {} with {} bytes of new content?",
                        a.root,
                        a.path,
                        a.app,
                        device.name,
                        a.text.len()
                    ),
                )
                .await?;
                self.lab
                    .write_file(&target, &a.app, &a.root, &a.path, a.text.as_bytes(), true)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("Wrote {}/{}", a.root, a.path))
            }
            .await,
        )
    }

    #[tool(
        description = "Run JavaScript in a WebView page and return the result. Script can change the page's data and the app's storage. Asks the person to confirm, showing the script.",
        title = "Run script in a web page",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn webview_eval(
        &self,
        Parameters(a): Parameters<args::WebviewEval>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                if a.expression.trim().is_empty() {
                    return Err("the expression is empty".to_string());
                }
                let target = self.target(&a.device).await?;
                let shown: String = a.expression.chars().take(400).collect();
                self.authorize(&context, &format!("Run this script in the page?\n\n{shown}")).await?;
                let (_, _, view) = emulabzip_frameworks::webview::connect(
                    &self.lab,
                    &target,
                    a.app.as_deref(),
                    a.socket.as_deref(),
                    a.page.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
                json(&view.evaluate(&a.expression).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Delete a recorded session for good. Asks the person to confirm.",
        title = "Delete a session",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn delete_session(
        &self,
        Parameters(a): Parameters<args::SessionRef>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                if !valid_session_id(&a.id) {
                    return Err(format!("`{}` is not a session name", a.id));
                }
                let path = sessions_dir().join(format!("{}.{}", a.id, emulabzip_session::EXTENSION));
                if !path.is_file() {
                    return Err(format!("no session is called {}; list_sessions shows them", a.id));
                }
                self.authorize(&context, &format!("Delete the session \"{}\" for good?", a.id)).await?;
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
                Ok(format!("Deleted {}", a.id))
            }
            .await,
        )
    }

    #[tool(
        description = "Delete a saved flow. Asks the person to confirm.",
        title = "Delete a flow",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn remove_flow(
        &self,
        Parameters(a): Parameters<args::FlowId>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                emulabzip_core::automation::load(&a.flow).map_err(|e| e.to_string())?;
                self.authorize(&context, &format!("Delete the flow \"{}\" for good?", a.flow)).await?;
                emulabzip_core::automation::remove(&a.flow).map_err(|e| e.to_string())?;
                Ok(format!("Deleted {}", a.flow))
            }
            .await,
        )
    }

    #[tool(
        description = "Delete the mapping, source map or dSYM stored for an app version. Crashes of that build read less clearly afterwards. Asks the person to confirm.",
        title = "Remove stored mappings",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn remove_mapping(
        &self,
        Parameters(a): Parameters<args::RemoveMapping>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                if !emulabzip_core::control::is_app_id(&a.app) {
                    return Err(format!("`{}` is not a package name or bundle id", a.app));
                }
                let store = emulabzip_symbols::store::Store::default();
                if store.find(&a.app, Some(&a.version)).is_empty() {
                    return Err(format!("nothing is stored for {} {}", a.app, a.version));
                }
                self.authorize(&context, &format!("Delete the stored mappings of {} {}?", a.app, a.version)).await?;
                let n = store.remove(&a.app, &a.version, None).map_err(|e| e.to_string())?;
                Ok(format!("Removed {n} file(s)"))
            }
            .await,
        )
    }

    #[tool(
        description = "Take a link out of the deep link history. Asks the person to confirm.",
        title = "Forget a deep link",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn forget_link(
        &self,
        Parameters(a): Parameters<args::ForgetLink>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                self.authorize(&context, &format!("Forget the link {} from the history?", a.url)).await?;
                emulabzip_core::links::forget_link(&a.url).map_err(|e| e.to_string())?;
                Ok(format!("Forgot {}", a.url))
            }
            .await,
        )
    }

    #[tool(
        description = "Replace a visual baseline with the screen as it is now. The old picture is lost. Asks the person to confirm.",
        title = "Update a visual baseline",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn visual_update(
        &self,
        Parameters(a): Parameters<args::VisualCapture>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        reply(
            async {
                if !emulabzip_audit::visual::is_baseline_name(&a.name) {
                    return Err(format!("`{}` is not a baseline name: use letters, digits, . _ and -", a.name));
                }
                let target = self.target(&a.device).await?;
                self.authorize(&context, &format!("Replace the baseline \"{}\" with the screen as it is now?", a.name))
                    .await?;
                let dir = super::more::visual_dir()?;
                let shot = self.lab.screenshot(&target).await.map_err(|e| e.to_string())?;
                emulabzip_audit::visual::save_baseline(&dir, &a.name, &shot, true).map_err(|e| e.to_string())?;
                Ok(format!("Updated {}", a.name))
            }
            .await,
        )
    }
}

impl EmuLabServer {
    async fn snapshot_action(
        &self,
        context: &RequestContext<RoleServer>,
        a: args::EmulatorSnapshotArgs,
        action: SnapshotAction,
    ) -> Result<String, String> {
        if !emulabzip_core::lifecycle::is_device_name(&a.name) {
            return Err(format!("`{}` is not a snapshot name", a.name));
        }
        let (device, target) = self.device(&a.device).await?;
        let (what, done) = match action {
            SnapshotAction::Load => (
                format!("Load the snapshot \"{}\" on {}? The state it is in now is thrown away.", a.name, device.name),
                "Loaded",
            ),
            _ => (format!("Delete the snapshot \"{}\" of {}?", a.name, device.name), "Deleted"),
        };
        self.authorize(context, &what).await?;
        self.lab.emulator_snapshot(&target, action, &a.name, true).await.map_err(|e| e.to_string())?;
        Ok(format!("{done} {}", a.name))
    }
}
