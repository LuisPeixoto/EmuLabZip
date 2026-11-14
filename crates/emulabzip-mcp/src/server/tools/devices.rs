//! The devices tools.

use crate::server::*;

#[tool_router(router = devices_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "List the running Android emulators, iOS simulators and attached devices, with their ids, state and OS version.",
        title = "List running devices",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_devices(&self) -> CallToolResult {
        let (devices, errors) = self.lab.devices().await;
        let mut value = serde_json::json!({ "devices": devices });
        if !errors.is_empty() {
            value["problems"] = errors.iter().map(|(p, e)| format!("{p:?}: {e}")).collect::<Vec<_>>().into();
        }
        reply(json(&value))
    }

    #[tool(
        description = "Facts about a device: build, hardware, display, battery, memory and locale.",
        title = "Device facts",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn device_info(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(async { json(&self.lab.info(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?) }.await)
    }

    #[tool(
        description = "List the apps on a device, with their package names or bundle ids, versions and process ids when running.",
        title = "List apps",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_apps(&self, Parameters(a): Parameters<args::Apps>) -> CallToolResult {
        reply(
            async {
                let apps = self.lab.apps(&self.target(&a.device).await?).await.map_err(|e| e.to_string())?;
                let apps: Vec<_> = apps
                    .into_iter()
                    .filter(|app| a.all.unwrap_or(false) || app.user || app.pid.is_some())
                    .filter(|app| !a.running.unwrap_or(false) || app.pid.is_some())
                    .collect();
                json(&apps)
            }
            .await,
        )
    }

    #[tool(
        description = "Take a screenshot of a device. Returns the image.",
        title = "Take a screenshot",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn screenshot(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        match async { self.lab.screenshot(&self.target(&a.device).await?).await.map_err(|e| e.to_string()) }.await {
            Ok(png) => CallToolResult::success(vec![ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(png),
                "image/png",
            )]),
            Err(message) => reply(Err(message)),
        }
    }

    #[tool(
        description = "List every emulator and simulator, running or not (a stopped one has state \"shutdown\"), with ids to start them with boot_device.",
        title = "List all devices, running or not",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_all_devices(&self) -> CallToolResult {
        let (devices, errors) = self.lab.all_devices().await;
        let mut value = serde_json::json!({ "devices": devices });
        if !errors.is_empty() {
            value["problems"] = errors.iter().map(|(p, e)| format!("{p:?}: {e}")).collect::<Vec<_>>().into();
        }
        reply(json(&value))
    }

    #[tool(
        description = "Start an emulator or simulator that is not running, and wait until it is up. It never wipes the device.",
        title = "Start a device",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn boot_device(&self, Parameters(a): Parameters<args::BootDevice>) -> CallToolResult {
        reply(
            async {
                let device = self.find_device(&a.device, a.platform.as_deref()).await?;
                let options =
                    emulabzip_core::BootOptions { wipe: false, cold: false, headless: a.headless.unwrap_or(false) };
                json(&self.lab.boot_device(&device.target(), &options).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Stop a running emulator or simulator. What is on it is kept.",
        title = "Stop a device",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn shutdown_device(&self, Parameters(a): Parameters<args::ShutdownDevice>) -> CallToolResult {
        reply(
            async {
                let device = self.find_device(&a.device, a.platform.as_deref()).await?;
                self.lab.shutdown_device(&device.target()).await.map_err(|e| e.to_string())?;
                Ok(format!("{} is shut down.", device.name))
            }
            .await,
        )
    }

    #[tool(
        description = "The snapshots of a running Android emulator (its whole state, saved by the emulator itself).",
        title = "List emulator snapshots",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_emulator_snapshots(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                json(&self.lab.emulator_snapshots(&target).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Save the running Android emulator's whole state as a snapshot with this name. Loading or deleting snapshots is not offered here.",
        title = "Save an emulator snapshot",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn save_emulator_snapshot(&self, Parameters(a): Parameters<args::EmulatorSnapshot>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                self.lab
                    .emulator_snapshot(&target, emulabzip_core::SnapshotAction::Save, &a.name, false)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("Saved snapshot {}.", a.name))
            }
            .await,
        )
    }
}
