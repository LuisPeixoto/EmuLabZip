//! The bug report: one HTML file, and a link that opens the issue.

use emulabzip_core::Target;
use emulabzip_report::issue::{Tracker, issue_url};
use emulabzip_report::{BugReport, Options, html, markdown};
use emulabzip_session::Session;
use emulabzip_symbols::store::Store;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::{CmdResult, ask_save_path, err};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRequest {
    pub title: String,
    pub description: String,
    pub screenshot: bool,
    pub logs: bool,
    pub requests: bool,
    pub crashes: bool,
    /// The piece of the screen recording around the crash, when the session has one. Only a session has video.
    #[serde(default)]
    pub video: bool,
    /// `github:owner/repo`, `gitlab:[host/]group/project` or `jira:site/PROJECTID/ISSUETYPEID`; empty for none.
    pub issue: String,
    /// A recorded session's file, to make the report from it instead of from the device as it is now.
    pub session: Option<String>,
    pub app: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSaved {
    pub path: String,
    pub issue_url: Option<String>,
    pub notes: Vec<String>,
}

/// Builds the report, asks where to save it, and opens the pre-filled issue in the browser. `None` when the user
/// cancels the save.
#[tauri::command]
pub async fn report_save(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Option<Target>,
    request: ReportRequest,
) -> CmdResult<Option<ReportSaved>> {
    // What can be checked without the device is checked first.
    let tracker = match request.issue.trim() {
        "" => None,
        spec => Some(Tracker::parse(spec).map_err(err)?),
    };
    let options = Options {
        title: request.title.trim().to_string(),
        description: request.description.trim().to_string(),
        include_screenshot: request.screenshot,
        include_logs: request.logs,
        include_requests: request.requests,
        include_crashes: request.crashes,
        include_video: request.video,
    };
    let mut report = match &request.session {
        Some(path) => {
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                let session = Session::open(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
                Ok::<_, String>(BugReport::from_session(&session, &options, &Store::default()))
            })
            .await
            .map_err(err)??
        }
        None => {
            let target = target.ok_or("Pick a device first.")?;
            let flows = state.flows.all();
            BugReport::collect(&state.lab, &target, request.app.as_deref(), &flows, &options, &Store::default()).await
        }
    };
    if report.title.trim().is_empty() {
        report.title = match (report.crashes.first(), &report.app) {
            (Some(c), _) => c.title.clone(),
            (None, Some(a)) => format!("Bug in {a}"),
            _ => "Bug report".into(),
        };
    }
    let Some(path) = ask_save_path(&app, "emulabzip-report.html", ("Bug report", &["html"])).await else {
        return Ok(None);
    };
    tokio::fs::write(&path, html::html(&report)).await.map_err(err)?;
    // A clip too big to be part of the file goes next to it, under the name the file refers to.
    if let Some(clip) = report.video_sidecar() {
        tokio::fs::write(path.with_file_name(emulabzip_report::VIDEO_FILE), clip).await.map_err(err)?;
    }
    let issue_url = tracker.map(|t| issue_url(&t, &report.title, &markdown::markdown(&report)));
    if let Some(url) = &issue_url {
        app.opener().open_url(url, None::<&str>).map_err(err)?;
    }
    // Show the file, so it can be attached to the issue.
    let _ = app.opener().reveal_item_in_dir(&path);
    Ok(Some(ReportSaved { path: path.display().to_string(), issue_url, notes: report.notes }))
}
