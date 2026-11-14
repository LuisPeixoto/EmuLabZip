//! Commands for the WebView panel: the pages inside an app's WebView, their console, and their requests.

use std::time::Duration;

use emulabzip_core::{DevtoolsSocket, Target};
use emulabzip_frameworks::webview::{self, Evaluated, Page, WebLine, WebView};
use emulabzip_net::cdp::CdpFlows;
use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

/// How long a response body is waited for. A page that is busy must not hold up the console.
const BODY_WAIT: Duration = Duration::from_secs(3);

#[derive(Serialize)]
pub struct WebViewInfo {
    pub socket: DevtoolsSocket,
    pub pages: Vec<Page>,
}

/// The WebViews and browsers that can be inspected, with their pages.
#[tauri::command]
pub async fn webview_list(state: State<'_, AppState>, target: Target) -> CmdResult<Vec<WebViewInfo>> {
    let sockets = state.lab.devtools_sockets(&target).await.map_err(err)?;
    let mut out = Vec::with_capacity(sockets.len());
    for socket in sockets {
        let pages = webview::pages(&state.lab, &target, &socket.socket).await.unwrap_or_default();
        out.push(WebViewInfo { socket, pages });
    }
    Ok(out)
}

/// What the interface hears from a page.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WebMessage {
    Line(WebLine),
    /// The connection ended: the page was closed or the app stopped.
    Ended,
}

async fn page_of(state: &AppState, target: &Target, socket: &str, page: &str) -> CmdResult<(Page, WebView)> {
    let pages = webview::pages(&state.lab, target, socket).await.map_err(err)?;
    let found = pages.into_iter().find(|p| p.id == page).ok_or("That page is gone. Refresh the list.")?;
    let view = WebView::attach(&state.lab, target, socket, &found).await.map_err(err)?;
    Ok((found, view))
}

/// Starts following a page. Console lines arrive on `on_event`; with `network`, its requests also appear in the
/// Network view, marked as coming from the WebView.
#[tauri::command]
pub async fn webview_start(
    state: State<'_, AppState>,
    target: Target,
    socket: String,
    page: String,
    network: bool,
    on_event: Channel<WebMessage>,
) -> CmdResult<u64> {
    let (_, view) = page_of(&state, &target, &socket, &page).await?;
    let mut events = view.events();
    view.start(network).await.map_err(err)?;
    let store = state.flows.clone();
    let task = tauri::async_runtime::spawn(async move {
        let mut flows = CdpFlows::new(store);
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            };
            if let Some(line) = webview::parse_event(&event)
                && on_event.send(WebMessage::Line(line)).is_err()
            {
                return;
            }
            if network
                && let Some(need) = flows.on_event(&event)
                && let Ok(Ok((body, base64))) =
                    tokio::time::timeout(BODY_WAIT, view.response_body(&need.request_id)).await
            {
                flows.set_body(need.flow, &body, base64);
            }
        }
        let _ = on_event.send(WebMessage::Ended);
    });
    Ok(state.add_session(task))
}

/// Runs an expression in a page.
#[tauri::command]
pub async fn webview_eval(
    state: State<'_, AppState>,
    target: Target,
    socket: String,
    page: String,
    expression: String,
) -> CmdResult<Evaluated> {
    let (_, view) = page_of(&state, &target, &socket, &page).await?;
    view.evaluate(&expression).await.map_err(err)
}

#[tauri::command]
pub async fn webview_reload(state: State<'_, AppState>, target: Target, socket: String, page: String) -> CmdResult<()> {
    let (_, view) = page_of(&state, &target, &socket, &page).await?;
    view.reload().await.map_err(err)
}

/// Opens Safari, where the Develop menu lists the simulator's web pages for Web Inspector.
#[tauri::command]
pub async fn open_safari() -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    {
        let status = tokio::process::Command::new("open").args(["-a", "Safari"]).status().await.map_err(err)?;
        if status.success() { Ok(()) } else { Err("Safari could not be opened.".into()) }
    }
    #[cfg(not(target_os = "macos"))]
    Err("Safari's Web Inspector needs a Mac.".into())
}
