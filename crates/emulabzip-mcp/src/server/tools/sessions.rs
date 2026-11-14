//! The sessions tools.

use crate::server::*;

#[tool_router(router = sessions_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "List the sessions recorded with EmuLabZip (`emulabzip session record`, `emulabzip ci`, or the app), newest first."
    )]
    async fn list_sessions(&self) -> CallToolResult {
        reply(sessions_list())
    }

    #[tool(
        description = "Summarize a recorded session: duration, log errors and the most repeated ones, crashes, requests and the slowest, CPU and memory peaks."
    )]
    async fn session_summary(&self, Parameters(a): Parameters<args::Session>) -> CallToolResult {
        reply(session_summary(&a.id))
    }

    #[tool(
        description = "Everything a recorded session captured on one time axis, oldest first: touches, log warnings and errors, requests, slow frames (a stutter is one event), garbage collections, crashes, screenshots and pieces of screen video. Times are milliseconds from the start of the recording. Pick lanes to keep it short."
    )]
    async fn session_timeline(&self, Parameters(a): Parameters<args::SessionTimeline>) -> CallToolResult {
        reply(session_timeline(&a))
    }

    #[tool(
        description = "Write a bug report as one self-contained HTML file in EmuLabZip's reports folder: a screenshot, the device, the warnings and errors of the log, the failed requests and the readable crashes, from the device as it is now or from a recorded session. Returns the file's path, a Markdown summary for an issue, and optionally a link that opens a pre-filled issue. Nothing is sent anywhere."
    )]
    async fn create_bug_report(&self, Parameters(a): Parameters<args::BugReport>) -> CallToolResult {
        reply(self.bug_report(a).await)
    }
}
