//! The automation tools.

use crate::server::*;

#[tool_router(router = automation_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "List the plugins installed on this computer: programs that collect their own data from a device. Only enabled plugins can be run.",
        title = "List plugins",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_plugins(&self) -> CallToolResult {
        let (plugins, problems) = plugin::discover();
        let approvals = plugin::Approvals::load();
        let listed: Vec<_> = plugins
            .iter()
            .map(|p| serde_json::json!({ "name": p.manifest.name, "description": p.manifest.description, "mode": p.manifest.mode, "platforms": p.manifest.platforms, "enabled": approvals.is_approved(p) }))
            .collect();
        let problems: Vec<String> = problems.iter().map(|p| format!("{}: {}", p.dir.display(), p.message)).collect();
        reply(json(&serde_json::json!({ "plugins": listed, "problems": problems })))
    }

    #[tool(
        description = "Run an enabled plugin on a device and return what it reported: metrics, tables, text and log lines. Plugins that the user has not enabled are refused.",
        title = "Run a plugin",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn run_plugin(&self, Parameters(a): Parameters<args::RunPlugin>) -> CallToolResult {
        reply(self.run_plugin_inner(a).await)
    }

    #[tool(
        description = "Check a deep link: whether the domain's assetlinks.json (Android) or apple-app-site-association (iOS) lists the app, whether the app's signing key or team id matches, what the device decided about the domain, and which app opens the link. Returns findings with what to fix. Reads one file from the link's own domain over https.",
        title = "Check a deep link",
        annotations(read_only_hint = true, open_world_hint = true)
    )]
    async fn check_link(&self, Parameters(a): Parameters<args::CheckLink>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                let report = emulabzip_net::links::check(&self.lab, &target, &a.url, a.app.as_deref())
                    .await
                    .map_err(|e| e.to_string())?;
                let _ = emulabzip_core::links::note_link(&a.url, a.app.as_deref(), Some(report.worst));
                json(&report)
            }
            .await,
        )
    }

    #[tool(
        description = "The saved flows: runs of touches, text and key presses recorded from a device (emulabzip flow record, or the Automation view), by id with their app and number of steps.",
        title = "List saved flows",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_flows(&self) -> CallToolResult {
        reply(json(&emulabzip_core::automation::list()))
    }

    #[tool(
        description = "Play a saved flow on an Android device: the taps, swipes, text and key presses that were recorded, scaled to this screen. Flows that would take longer than two minutes are refused here; play them from the command line.",
        title = "Play a saved flow",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn play_flow(&self, Parameters(a): Parameters<args::PlayFlow>) -> CallToolResult {
        reply(
            async {
                // An id of a saved flow, never a path.
                let flow = emulabzip_core::automation::load(&a.flow).map_err(|e| e.to_string())?;
                if flow.duration_ms() > 120_000 {
                    return Err("That flow takes more than two minutes. Play it with `emulabzip flow play`.".into());
                }
                let target = self.target(&a.device).await?;
                emulabzip_core::automation::play(&self.lab, &target, &flow, std::future::pending(), |_, _| {})
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("Played {} ({} steps)", flow.name, flow.steps.len()))
            }
            .await,
        )
    }
}
