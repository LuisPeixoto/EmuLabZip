//! The crashes tools.

use crate::server::*;

#[tool_router(router = crashes_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "List the crashes, native crashes and ANRs a device has recorded, newest first.",
        title = "List crashes",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_crashes(&self, Parameters(a): Parameters<args::Crashes>) -> CallToolResult {
        reply(
            async {
                let kind = match a.kind.as_deref() {
                    None => None,
                    Some("crash") => Some(emulabzip_core::CrashKind::Crash),
                    Some("native") => Some(emulabzip_core::CrashKind::NativeCrash),
                    Some("anr") => Some(emulabzip_core::CrashKind::Anr),
                    Some(other) => return Err(format!("`{other}` is not a kind: use crash, native or anr")),
                };
                let list = self
                    .lab
                    .crashes(&self.target(&a.device).await?, a.app.as_deref())
                    .await
                    .map_err(|e| e.to_string())?;
                json(&list.into_iter().filter(|c| kind.is_none_or(|k| c.kind == k)).collect::<Vec<_>>())
            }
            .await,
        )
    }

    #[tool(
        description = "One crash with its exception or signal, the stack of the thread that crashed (and others), and hints. Names scrambled by R8 or ProGuard, JavaScript positions and native iOS addresses are made readable with the mapping.txt, source maps and dSYMs stored for the app and version (`symbolication` says which were used, and what is missing).",
        title = "Read a crash",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_crash(&self, Parameters(a): Parameters<args::Crash>) -> CallToolResult {
        reply(
            async {
                let mut report =
                    self.lab.crash(&self.target(&a.device).await?, &a.id).await.map_err(|e| e.to_string())?;
                if !a.raw.unwrap_or(false) {
                    report.raw.clear();
                }
                if a.original.unwrap_or(false) { json(&report) } else { json(&readable(report)) }
            }
            .await,
        )
    }
}
