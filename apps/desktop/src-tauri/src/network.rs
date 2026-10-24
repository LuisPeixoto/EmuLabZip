//! Commands for the Network tool: the proxy, routing devices to it, and looking at what it saw.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use emulabzip_core::{CaInstall, NetworkStatus, Target};
use emulabzip_net::decode::decode;
use emulabzip_net::flow::{Flow, FlowSummary};
use emulabzip_net::pcap::dns::Lookup;
use emulabzip_net::pcap::{Summary, import};
use emulabzip_net::store::FlowEvent;
use emulabzip_net::{
    BreakpointEvent, CertificateAuthority, Edits, Proxy, ProxyConfig, Resume, Rule, default_dir, to_har,
};
use emulabzip_storage::{FileKind, detect, image_mime};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::commands::{CmdResult, ask_open_path, ask_save_path, err};
use crate::files::{HEX_BYTES, MAX_TEXT, hex_dump};
use crate::state::{AppState, NetRuntime};

const DEFAULT_PORT: u16 = 9090;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetState {
    pub running: bool,
    pub port: Option<u16>,
    pub ca_path: Option<String>,
    /// Hosts that rejected the certificate and are passed through.
    pub bypassed: Vec<String>,
    /// Devices pointed at the proxy by this app, as `platform:id`.
    pub connected: Vec<String>,
    /// Requests go to servers over HTTP/3 when they offer it.
    pub http3: bool,
    /// HTTP/3 is removed from what the device is told.
    pub hide_http3: bool,
}

fn ca(rt: &mut NetRuntime) -> CmdResult<Arc<CertificateAuthority>> {
    if let Some(ca) = &rt.ca {
        return Ok(ca.clone());
    }
    let ca = Arc::new(CertificateAuthority::load_or_create(default_dir()).map_err(err)?);
    rt.ca = Some(ca.clone());
    Ok(ca)
}

fn snapshot(rt: &NetRuntime) -> NetState {
    NetState {
        running: rt.proxy.is_some(),
        port: rt.proxy.as_ref().map(|p| p.addr().port()),
        ca_path: rt.ca.as_ref().and_then(|c| c.cert_path()).map(|p| p.display().to_string()),
        bypassed: rt.proxy.as_ref().map(|p| p.bypassed()).unwrap_or_default(),
        connected: rt.connected.iter().map(|t| format!("{:?}:{}", t.platform, t.id).to_lowercase()).collect(),
        http3: rt.http3,
        hide_http3: rt.hide_http3,
    }
}

async fn start(state: &AppState, rt: &mut NetRuntime, port: u16) -> CmdResult<()> {
    if rt.proxy.is_some() {
        return Ok(());
    }
    let ca = ca(rt)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let config = ProxyConfig {
        addr,
        rules: rt.rules.clone(),
        breakpoints: rt.breakpoints.clone(),
        http3: rt.http3,
        hide_http3: rt.hide_http3,
        ..Default::default()
    };
    let handle = Proxy::start(config, ca, state.flows.clone())
        .await
        .map_err(|e| format!("Could not listen on port {port}. Is another proxy using it? {e}"))?;
    rt.port = handle.addr().port();
    rt.proxy = Some(handle);
    Ok(())
}

/// The state of the proxy. Also makes sure the certificate exists, so its file can be shown.
#[tauri::command]
pub async fn net_state(state: State<'_, AppState>) -> CmdResult<NetState> {
    let mut rt = state.net.lock().await;
    ca(&mut rt)?;
    Ok(snapshot(&rt))
}

#[tauri::command]
pub async fn net_start(state: State<'_, AppState>, port: u16) -> CmdResult<NetState> {
    let mut rt = state.net.lock().await;
    start(&state, &mut rt, port).await?;
    Ok(snapshot(&rt))
}

/// Stops the proxy after putting every device back.
#[tauri::command]
pub async fn net_stop(state: State<'_, AppState>) -> CmdResult<NetState> {
    let mut rt = state.net.lock().await;
    for target in std::mem::take(&mut rt.connected) {
        if let Err(e) = state.lab.disconnect_network(&target).await {
            tracing::warn!("could not disconnect {}: {e}", target.id);
        }
    }
    rt.proxy = None;
    Ok(snapshot(&rt))
}

#[tauri::command]
pub async fn net_status(state: State<'_, AppState>, target: Target) -> CmdResult<NetworkStatus> {
    state.lab.network_status(&target).await.map_err(err)
}

/// Points a device at the proxy, starting the proxy first when it is not running.
#[tauri::command]
pub async fn net_connect(state: State<'_, AppState>, target: Target) -> CmdResult<NetworkStatus> {
    let mut rt = state.net.lock().await;
    let port = if rt.port == 0 { DEFAULT_PORT } else { rt.port };
    start(&state, &mut rt, port).await?;
    let status = state.lab.connect_network(&target, rt.port).await.map_err(err)?;
    if !rt.connected.contains(&target) {
        rt.connected.push(target);
    }
    Ok(status)
}

#[tauri::command]
pub async fn net_disconnect(state: State<'_, AppState>, target: Target) -> CmdResult<NetworkStatus> {
    let mut rt = state.net.lock().await;
    rt.connected.retain(|t| t != &target);
    state.lab.disconnect_network(&target).await.map_err(err)
}

/// Makes the device trust the certificate. Hosts that rejected it before are tried again.
#[tauri::command]
pub async fn net_install_ca(state: State<'_, AppState>, target: Target) -> CmdResult<CaInstall> {
    let mut rt = state.net.lock().await;
    let ca = ca(&mut rt)?;
    let result = state.lab.install_ca(&target, ca.cert_pem(), ca.cert_der()).await.map_err(err)?;
    if result.done
        && let Some(proxy) = &rt.proxy
    {
        proxy.clear_bypass();
    }
    Ok(result)
}

#[tauri::command]
pub async fn net_clear_bypass(state: State<'_, AppState>) -> CmdResult<NetState> {
    let rt = state.net.lock().await;
    if let Some(proxy) = &rt.proxy {
        proxy.clear_bypass();
    }
    Ok(snapshot(&rt))
}

// ---------------------------------------------------------------------------------------------
// Flows
// ---------------------------------------------------------------------------------------------

/// Changes to the flow list since the last batch. `cleared` means the list starts over.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetBatch {
    pub cleared: bool,
    pub flows: Vec<FlowSummary>,
}

/// Keeps the last version of each flow, in the order flows first appeared.
fn dedupe(flows: Vec<FlowSummary>) -> Vec<FlowSummary> {
    let mut out: Vec<FlowSummary> = Vec::with_capacity(flows.len());
    let mut index = std::collections::HashMap::new();
    for f in flows {
        match index.get(&f.id) {
            Some(&i) => out[i] = f,
            None => {
                index.insert(f.id, out.len());
                out.push(f);
            }
        }
    }
    out
}

/// Sends the current list, then every change in batches of 100 ms.
#[tauri::command]
pub async fn net_subscribe(state: State<'_, AppState>, on_batch: Channel<NetBatch>) -> CmdResult<u64> {
    let store = state.flows.clone();
    let mut events = store.subscribe();
    let _ = on_batch.send(NetBatch { cleared: true, flows: store.summaries() });
    let task = tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        let (mut pending, mut cleared) = (Vec::new(), false);
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(FlowEvent::Added { flow } | FlowEvent::Updated { flow }) => pending.push(flow),
                    Ok(FlowEvent::Cleared) => (pending, cleared) = (Vec::new(), true),
                    // Too many changes at once: start over from what the store holds.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => (pending, cleared) = (store.summaries(), true),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                _ = tick.tick() => {
                    if cleared || !pending.is_empty() {
                        let batch = NetBatch { cleared, flows: dedupe(std::mem::take(&mut pending)) };
                        cleared = false;
                        if on_batch.send(batch).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });
    Ok(state.add_session(task))
}

#[tauri::command]
pub fn net_clear(state: State<'_, AppState>) {
    state.flows.clear();
}

/// A body ready to show: decoded, and rendered the way its type suggests.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BodyView {
    /// Bytes on the wire.
    pub size: u64,
    pub decoded_size: u64,
    /// Only the first part was stored.
    pub truncated: bool,
    pub encoding: Option<String>,
    pub kind: Option<FileKind>,
    pub text: Option<String>,
    pub image: Option<String>,
    pub hex: Option<String>,
    pub error: Option<String>,
}

fn kind_for(content_type: Option<&str>, bytes: &[u8]) -> FileKind {
    let ct = content_type.unwrap_or_default().to_ascii_lowercase();
    if ct.contains("json") {
        FileKind::Json
    } else if ct.starts_with("image/") && image_mime(bytes).is_some() {
        FileKind::Image
    } else {
        detect("", bytes)
    }
}

pub fn body_view(stored: &[u8], size: u64, truncated: bool, headers: &[(String, String)]) -> Option<BodyView> {
    if size == 0 {
        return None;
    }
    let encoding = Flow::header(headers, "content-encoding").map(str::to_string);
    let content_type = Flow::header(headers, "content-type").map(str::to_string);
    let mut view = BodyView { size, truncated, encoding: encoding.clone(), ..Default::default() };

    let decoded = match decode(encoding.as_deref(), stored) {
        Ok(d) => d,
        Err(e) => {
            // A body cut short cannot be decompressed; show what is there.
            view.error = Some(if truncated {
                "The body is larger than what is stored, so it cannot be decoded.".to_string()
            } else {
                format!("Could not decode the {} body: {e}", encoding.as_deref().unwrap_or("compressed"))
            });
            stored.to_vec()
        }
    };
    view.decoded_size = decoded.len() as u64;
    let kind = kind_for(content_type.as_deref(), &decoded);
    view.kind = Some(kind);
    match kind {
        FileKind::Image => {
            let mime = image_mime(&decoded).unwrap_or("image/png");
            view.image =
                Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&decoded)));
        }
        FileKind::Binary | FileKind::Sqlite => view.hex = Some(hex_dump(&decoded[..decoded.len().min(HEX_BYTES)])),
        _ => {
            let shown = &decoded[..decoded.len().min(MAX_TEXT)];
            let text = String::from_utf8_lossy(shown).into_owned();
            view.text = Some(match kind {
                FileKind::Json => serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| serde_json::to_string_pretty(&v).ok())
                    .unwrap_or(text),
                _ => text,
            });
            view.truncated |= decoded.len() > MAX_TEXT;
        }
    }
    Some(view)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowDetail {
    pub flow: Flow,
    pub request: Option<BodyView>,
    pub response: Option<BodyView>,
}

#[tauri::command]
pub fn net_flow(state: State<'_, AppState>, id: u64) -> CmdResult<FlowDetail> {
    let flow = state.flows.get(id).ok_or("That request is no longer in the list.")?;
    let request = body_view(&flow.request_body, flow.request_size, flow.request_truncated, &flow.request_headers);
    let response = body_view(&flow.response_body, flow.response_size, flow.response_truncated, &flow.response_headers);
    Ok(FlowDetail { flow, request, response })
}

/// Saves the list as a HAR file, to a place the user picks. Returns the path, or `None` if cancelled.
#[tauri::command]
pub async fn net_export_har(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let har = to_har(&state.flows.all());
    let count = har["log"]["entries"].as_array().map_or(0, Vec::len);
    if count == 0 {
        return Err("There are no requests to export yet.".into());
    }
    let Some(path) = ask_save_path(&app, "emulabzip-capture.har", ("HTTP Archive", &["har"])).await else {
        return Ok(None);
    };
    tokio::fs::write(&path, serde_json::to_vec_pretty(&har).map_err(err)?).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}

// ---------------------------------------------------------------------------------------------
// Rules and breakpoints
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn net_rules_get(state: State<'_, AppState>) -> CmdResult<Vec<Rule>> {
    Ok(state.net.lock().await.rules.rules())
}

/// Replaces the rules. They apply to the next request, and are kept for the next launch.
#[tauri::command]
pub async fn net_rules_set(state: State<'_, AppState>, rules: Vec<Rule>) -> CmdResult<Vec<Rule>> {
    let rt = state.net.lock().await;
    rt.rules.set(rules).map_err(err)?;
    Ok(rt.rules.rules())
}

/// Sends what is held now, then every change.
#[tauri::command]
pub async fn net_breakpoints_subscribe(
    state: State<'_, AppState>,
    on_event: Channel<BreakpointEvent>,
) -> CmdResult<u64> {
    let breakpoints = state.net.lock().await.breakpoints.clone();
    let mut events = breakpoints.subscribe();
    for paused in breakpoints.pending() {
        let _ = on_event.send(BreakpointEvent::Paused { paused: Box::new(paused) });
    }
    let task = tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    if on_event.send(event).is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Whatever was missed is shown again from what is still held.
                    for paused in breakpoints.pending() {
                        let _ = on_event.send(BreakpointEvent::Paused { paused: Box::new(paused) });
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    Ok(state.add_session(task))
}

/// What the user changed in a held message. Fields left out stay as they were.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EditsInput {
    pub headers: Option<Vec<(String, String)>>,
    pub body_text: Option<String>,
    pub body_base64: Option<String>,
    pub status: Option<u16>,
}

fn edits(input: EditsInput) -> CmdResult<Edits> {
    let body = match (input.body_text, input.body_base64) {
        (Some(text), _) => Some(text.into_bytes()),
        (None, Some(b64)) => Some(base64::engine::general_purpose::STANDARD.decode(b64).map_err(err)?),
        (None, None) => None,
    };
    Ok(Edits { headers: input.headers, body, status: input.status })
}

/// Lets a held message go on, with edits, or fails it.
#[tauri::command]
pub async fn net_resume(state: State<'_, AppState>, id: u64, abort: bool, edits_input: EditsInput) -> CmdResult<()> {
    let breakpoints = state.net.lock().await.breakpoints.clone();
    let answer = if abort { Resume::Abort } else { Resume::Continue(edits(edits_input)?) };
    if breakpoints.resume(id, answer) { Ok(()) } else { Err("That request is no longer held.".into()) }
}

#[tauri::command]
pub async fn net_resume_all(state: State<'_, AppState>) -> CmdResult<()> {
    state.net.lock().await.breakpoints.resume_all();
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// HTTP/3
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn net_set_http3(state: State<'_, AppState>, enabled: bool) -> CmdResult<NetState> {
    let mut rt = state.net.lock().await;
    rt.http3 = enabled;
    if let Some(proxy) = &rt.proxy {
        proxy.set_http3(enabled);
    }
    Ok(snapshot(&rt))
}

#[tauri::command]
pub async fn net_set_hide_http3(state: State<'_, AppState>, hide: bool) -> CmdResult<NetState> {
    let mut rt = state.net.lock().await;
    rt.hide_http3 = hide;
    if let Some(proxy) = &rt.proxy {
        proxy.set_hide_http3(hide);
    }
    Ok(snapshot(&rt))
}

// ---------------------------------------------------------------------------------------------
// Packet captures
// ---------------------------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub file: String,
    pub summary: Summary,
    pub dns: Vec<Lookup>,
}

/// Asks for a `.pcap` or `.pcapng` file and adds what it holds to the list. `None` when cancelled.
#[tauri::command]
pub async fn net_import_pcap(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<Imported>> {
    let Some(path) = ask_open_path(&app, ("Packet capture", &["pcap", "pcapng", "cap"])).await else {
        return Ok(None);
    };
    let bytes = tokio::fs::read(&path).await.map_err(err)?;
    let capture = tokio::task::spawn_blocking(move || import(&bytes)).await.map_err(err)?.map_err(err)?;
    for flow in capture.flows {
        state.flows.begin(flow);
    }
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Some(Imported { file, summary: capture.summary, dns: capture.dns }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn shows_nothing_for_an_empty_body() {
        assert!(body_view(b"", 0, false, &[]).is_none());
    }

    #[test]
    fn pretty_prints_json_bodies() {
        let v =
            body_view(br#"{"a":1,"b":[true]}"#, 18, false, &headers(&[("content-type", "application/json")])).unwrap();
        assert_eq!(v.kind, Some(FileKind::Json));
        assert!(v.text.unwrap().contains("\n  \"a\": 1"));
    }

    #[test]
    fn decodes_compressed_bodies_and_reports_both_sizes() {
        use std::io::Write;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(b"hello hello hello hello hello").unwrap();
        let gz = gz.finish().unwrap();
        let v = body_view(&gz, gz.len() as u64, false, &headers(&[("content-encoding", "gzip")])).unwrap();
        assert_eq!(v.text.as_deref(), Some("hello hello hello hello hello"));
        assert_eq!((v.size, v.decoded_size), (gz.len() as u64, 29));
    }

    #[test]
    fn explains_a_compressed_body_that_was_cut_short() {
        let v = body_view(&[0x1f, 0x8b, 8, 0], 99_999_999, true, &headers(&[("content-encoding", "gzip")])).unwrap();
        assert!(v.error.unwrap().contains("larger than what is stored"));
        assert!(v.hex.is_some() || v.text.is_some());
    }

    #[test]
    fn shows_images_and_binary_data_differently() {
        let png = [0x89, b'P', b'N', b'G', 0, 0, 0, 0];
        assert!(
            body_view(&png, 8, false, &headers(&[("content-type", "image/png")]))
                .unwrap()
                .image
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert!(body_view(&[0, 1, 2, 255], 4, false, &headers(&[])).unwrap().hex.is_some());
    }

    #[test]
    fn keeps_the_last_version_of_each_flow() {
        let mk = |id, status| {
            let mut f = Flow::new(emulabzip_net::FlowKind::Http, "GET", "https", "a.com", 443, "/");
            f.id = id;
            f.status = status;
            f.summary()
        };
        let out = dedupe(vec![mk(1, None), mk(2, None), mk(1, Some(200))]);
        assert_eq!(out.iter().map(|f| (f.id, f.status)).collect::<Vec<_>>(), [(1, Some(200)), (2, None)]);
    }
}
