//! The network tools: the proxy, what it saw, and the rules that change it.

use emulabzip_net::flow::{Flow, FlowKind, FlowState, FlowSummary};
use emulabzip_net::rules::{Action, Rule, glob_match};
use emulabzip_net::{Conditions, Edits, Resend, Resume, to_har};

use crate::server::*;

const DEFAULT_BODY: usize = 8_000;
const MAX_BODY: usize = 40_000;

/// The folder an assistant may serve files from in a rule.
fn mocks_dir() -> std::path::PathBuf {
    emulabzip_core::data_dir().join("mocks")
}

fn exports_dir() -> std::path::PathBuf {
    emulabzip_core::data_dir().join("exports")
}

/// A file name made of letters, digits and `-_.`, without a path.
fn file_name(name: &str) -> Result<&str, String> {
    if valid_session_id(name) && !name.starts_with('.') {
        Ok(name)
    } else {
        Err(format!("`{name}` is not a file name: use letters, digits, - and _"))
    }
}

/// Checks the rules an assistant wants in force. What would read or run something it was not given is refused here.
fn check_rules(values: Vec<serde_json::Value>) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::with_capacity(values.len());
    for (i, v) in values.into_iter().enumerate() {
        let rule: Rule = serde_json::from_value(v).map_err(|e| format!("rule {} is not valid: {e}", i + 1))?;
        match &rule.action {
            Action::MapLocal { path, .. } => {
                let root = std::fs::canonicalize(mocks_dir()).map_err(|_| {
                    format!(
                        "Rule “{}” serves a file, but {} does not exist. Put the files there first.",
                        rule.name,
                        mocks_dir().display()
                    )
                })?;
                let real =
                    std::fs::canonicalize(path).map_err(|_| format!("Rule “{}”: {path} does not exist.", rule.name))?;
                if !real.starts_with(&root) {
                    return Err(format!(
                        "Rule “{}”: an assistant can serve only files inside {}. Use the app's Rules dialog for others.",
                        rule.name,
                        root.display()
                    ));
                }
            }
            Action::Script { path } => {
                let script =
                    emulabzip_net::script::Script::load(std::path::Path::new(path)).map_err(|e| e.to_string())?;
                if !emulabzip_net::script::Approvals::load().is_approved(&script.sha256) {
                    return Err(format!(
                        "Rule “{}”: the script {} has not been approved. Only the user can approve a script: `emulabzip script approve`.",
                        rule.name, script.name
                    ));
                }
            }
            Action::Validate { spec } => {
                spec_path(spec)?;
            }
            _ => {}
        }
        rules.push(rule);
    }
    Ok(rules)
}

/// An OpenAPI file an assistant may have read: a plain file with the right extension.
fn spec_path(file: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(file);
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("yaml" | "yml" | "json")) {
        return Err(format!("`{file}` is not a .yaml, .yml or .json file"));
    }
    if !path.is_absolute() || !path.is_file() {
        return Err(format!("there is no file at {file}; give its full path"));
    }
    Ok(path)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FlowRow {
    id: u64,
    method: String,
    url: String,
    status: Option<u16>,
    state: String,
    content_type: Option<String>,
    size: u64,
    ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract: Option<String>,
}

fn row(f: &FlowSummary) -> FlowRow {
    FlowRow {
        id: f.id,
        method: if f.kind == FlowKind::Tunnel { "TUNNEL".into() } else { f.method.clone() },
        url: if f.kind == FlowKind::Tunnel { format!("{}:{}", f.host, f.port) } else { f.url.clone() },
        status: f.status,
        state: format!("{:?}", f.state).to_lowercase(),
        content_type: f.content_type.clone(),
        size: f.response_size,
        ms: f.duration_ms,
        error: f.error.clone(),
        rule: f.rule.clone(),
        source: f.source.clone(),
        operation: f.operation.clone(),
        contract: f.contract.clone().filter(|c| c == "violations").map(|_| format!("{} violations", f.violations)),
    }
}

/// Whether a request passes the filters.
fn passes(f: &FlowSummary, a: &args::NetFlows) -> bool {
    let failed = f.state == FlowState::Failed
        || (f.status.is_none() && f.state == FlowState::Completed && f.kind != FlowKind::Tunnel);
    if let Some(h) = a.host.as_deref().filter(|h| !h.is_empty())
        && !glob_match(h, &f.host, true)
    {
        return false;
    }
    if let Some(p) = a.path.as_deref().filter(|p| !p.is_empty())
        && !glob_match(p, f.path.split('?').next().unwrap_or(&f.path), false)
    {
        return false;
    }
    if let Some(m) = a.method.as_deref().filter(|m| !m.is_empty())
        && !f.method.eq_ignore_ascii_case(m)
    {
        return false;
    }
    if let Some(t) = a.text.as_deref().filter(|t| !t.is_empty()) {
        let t = t.to_ascii_lowercase();
        if !f.url.to_ascii_lowercase().contains(&t)
            && !f.operation.as_deref().is_some_and(|o| o.to_ascii_lowercase().contains(&t))
        {
            return false;
        }
    }
    match a.status.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("") => true,
        Some("2xx") => f.status.is_some_and(|s| (200..300).contains(&s)),
        Some("3xx") => f.status.is_some_and(|s| (300..400).contains(&s)),
        Some("4xx") => f.status.is_some_and(|s| (400..500).contains(&s)),
        Some("5xx") => f.status.is_some_and(|s| (500..600).contains(&s)),
        Some("failed") => failed,
        Some("errors") => failed || f.status.is_some_and(|s| s >= 400),
        Some(_) => true,
    }
}

/// A body as an assistant reads it: decoded, text or JSON as text, an image as an image, anything else described.
enum Body {
    Text { text: String, truncated: bool, size: u64 },
    Image { mime: &'static str, bytes: Vec<u8> },
    Binary { size: u64, hex: String },
    None,
}

fn read_body(headers: &[(String, String)], stored: &[u8], size: u64, truncated: bool, cap: usize) -> Body {
    if size == 0 || stored.is_empty() {
        return Body::None;
    }
    let encoding = Flow::header(headers, "content-encoding");
    let decoded = emulabzip_net::decode::decode(encoding, stored).unwrap_or_else(|_| stored.to_vec());
    if let Some(mime) = emulabzip_storage::image_mime(&decoded) {
        return Body::Image { mime, bytes: decoded };
    }
    let kind = emulabzip_storage::detect("", &decoded);
    let looks_text = std::str::from_utf8(&decoded[..decoded.len().min(4096)]).is_ok_and(|s| !s.contains('\0'));
    if !looks_text || matches!(kind, emulabzip_storage::FileKind::Sqlite) {
        let hex: String = decoded.iter().take(48).map(|b| format!("{b:02x}")).collect();
        return Body::Binary { size: decoded.len() as u64, hex };
    }
    let mut text = String::from_utf8_lossy(&decoded).into_owned();
    let json = Flow::header(headers, "content-type").is_some_and(|c| c.contains("json"));
    if json && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
        text = serde_json::to_string_pretty(&v).unwrap_or(text);
    }
    let cut = text.chars().count() > cap;
    if cut {
        text = text.chars().take(cap).collect();
    }
    Body::Text { text, truncated: truncated || cut, size: decoded.len() as u64 }
}

fn body_json(body: &Body) -> serde_json::Value {
    match body {
        Body::None => serde_json::Value::Null,
        Body::Text { text, truncated, size } => {
            serde_json::json!({ "text": text, "bytes": size, "truncated": truncated })
        }
        Body::Image { mime, bytes } => {
            serde_json::json!({ "image": mime, "bytes": bytes.len(), "note": "returned as an image block after the text" })
        }
        Body::Binary { size, hex } => serde_json::json!({ "binary": true, "bytes": size, "firstBytesHex": hex }),
    }
}

#[tool_router(router = network_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "Start the network proxy on this computer, so a device's HTTPS traffic can be read. It listens on 127.0.0.1. Next: net_connect_device to point a device at it, and net_install_ca if HTTPS shows as connections that could not be read. Nothing is changed on a device by this call.",
        title = "Start the network proxy",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = true)
    )]
    async fn net_start(&self, Parameters(a): Parameters<args::NetStart>) -> CallToolResult {
        reply(
            async {
                if a.port == Some(0) {
                    return Err("port 0 is not a port".to_string());
                }
                let mut net = self.hub.net.lock().await;
                if let Some(h) = a.http3 {
                    net.http3 = h;
                    if let Some(p) = &net.proxy {
                        p.set_http3(h);
                    }
                }
                self.hub.start_proxy(&mut net, a.port).await?;
                json(&net_status_of(&net, &self.hub))
            }
            .await,
        )
    }

    #[tool(
        description = "Stop the proxy and put every device this session pointed at it back to normal. The requests it saw are kept until net_clear.",
        title = "Stop the network proxy",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_stop(&self) -> CallToolResult {
        reply(
            async {
                let mut net = self.hub.net.lock().await;
                self.hub.stop_proxy(&self.lab, &mut net).await;
                json(&net_status_of(&net, &self.hub))
            }
            .await,
        )
    }

    #[tool(
        description = "Whether the proxy is running and on which port, the devices pointed at it, how many requests were seen, the rules, the network conditions and any recording being replayed.",
        title = "Network proxy status",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn net_status(&self) -> CallToolResult {
        let net = self.hub.net.lock().await;
        reply(json(&net_status_of(&net, &self.hub)))
    }

    #[tool(
        description = "Point a device at the proxy (an Android device's global proxy setting through adb, or the iOS Simulator's Mac proxy), starting the proxy first if needed. The session puts it back when it ends, or with net_disconnect_device.",
        title = "Send a device's traffic through the proxy",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = true)
    )]
    async fn net_connect_device(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let (device, target) = self.device(&a.device).await?;
                let mut net = self.hub.net.lock().await;
                self.hub.start_proxy(&mut net, None).await?;
                let status = self.lab.connect_network(&target, net.port).await.map_err(|e| e.to_string())?;
                if !net.connected.contains(&target) {
                    net.connected.push(target);
                }
                json(&serde_json::json!({ "device": device.name, "port": net.port, "status": status }))
            }
            .await,
        )
    }

    #[tool(
        description = "Put a device's network settings back to normal.",
        title = "Stop sending a device's traffic through the proxy",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_disconnect_device(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let (device, target) = self.device(&a.device).await?;
                let mut net = self.hub.net.lock().await;
                net.connected.retain(|t| t != &target);
                let status = self.lab.disconnect_network(&target).await.map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "device": device.name, "status": status }))
            }
            .await,
        )
    }

    #[tool(
        description = "Make a device trust the proxy's certificate, so HTTPS can be read: the system store on an emulator without Google Play, the simulator's keychain on iOS. Elsewhere it says what the user has to do on the device.",
        title = "Install the proxy certificate",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_install_ca(&self, Parameters(a): Parameters<args::Device>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                let mut net = self.hub.net.lock().await;
                let ca = net.authority()?;
                let result =
                    self.lab.install_ca(&target, ca.cert_pem(), ca.cert_der()).await.map_err(|e| e.to_string())?;
                if result.done
                    && let Some(p) = &net.proxy
                {
                    p.clear_bypass();
                }
                json(&result)
            }
            .await,
        )
    }

    #[tool(
        description = "The requests the proxy saw, as short rows: id, method, address, status, size, time, and why one failed, which rule touched it and any contract violations. Filter by host, path, method, status class or text. To read only what is new, pass the `nextSinceId` of the last answer as since_id. Use net_flow for headers and bodies.",
        title = "List captured requests",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn net_flows(&self, Parameters(a): Parameters<args::NetFlows>) -> CallToolResult {
        let limit = a.limit.unwrap_or(50).clamp(1, 200);
        let all = self.hub.flows.summaries();
        let total = all.len();
        let mut matching: Vec<&FlowSummary> =
            all.iter().filter(|f| a.since_id.is_none_or(|s| f.id > s)).filter(|f| passes(f, &a)).collect();
        let matched = matching.len();
        if a.since_id.is_none() && matching.len() > limit {
            matching = matching.split_off(matching.len() - limit);
        } else {
            matching.truncate(limit);
        }
        let next = matching.last().map(|f| f.id).or(a.since_id).unwrap_or(0);
        reply(json(&serde_json::json!({
            "captured": total,
            "matching": matched,
            "shown": matching.len(),
            "nextSinceId": next,
            "flows": matching.iter().map(|f| row(f)).collect::<Vec<_>>(),
        })))
    }

    #[tool(
        description = "One captured request in full: headers, timing, the error, which rule changed it, an OpenAPI contract result, and the request and response bodies decoded (JSON pretty-printed, images returned as images). GraphQL operations and gRPC methods are named in net_flows.",
        title = "Read a captured request",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn net_flow(&self, Parameters(a): Parameters<args::NetFlow>) -> CallToolResult {
        let Some(flow) = self.hub.flows.get(a.id) else {
            return reply(Err(format!("no request has the id {}; net_flows lists them", a.id)));
        };
        let limit = a.max_chars.unwrap_or(DEFAULT_BODY).clamp(200, MAX_BODY);
        let want = a.bodies.unwrap_or(true);
        let (request, response) = if want {
            (
                read_body(&flow.request_headers, &flow.request_body, flow.request_size, flow.request_truncated, limit),
                read_body(
                    &flow.response_headers,
                    &flow.response_body,
                    flow.response_size,
                    flow.response_truncated,
                    limit,
                ),
            )
        } else {
            (Body::None, Body::None)
        };
        let text = serde_json::json!({
            "id": flow.id,
            "method": flow.method,
            "url": flow.url,
            "status": flow.status,
            "state": format!("{:?}", flow.state).to_lowercase(),
            "httpVersion": flow.http_version,
            "serverProtocol": flow.server_protocol,
            "requestHeaders": flow.request_headers,
            "responseHeaders": flow.response_headers,
            "responseTrailers": flow.response_trailers,
            "timing": { "ttfbMs": flow.timing.ttfb_ms, "totalMs": flow.timing.total_ms },
            "error": flow.error,
            "rule": flow.rule,
            "mocked": flow.mocked,
            "mappedTo": flow.mapped_to,
            "source": flow.source,
            "operation": flow.operation,
            "contract": flow.validation,
            "scriptNotes": flow.script_notes,
            "requestBody": body_json(&request),
            "responseBody": body_json(&response),
        });
        let mut blocks =
            vec![ContentBlock::text(cap(serde_json::to_string_pretty(&text).unwrap_or_default(), MAX_TEXT))];
        for body in [&request, &response] {
            if let Body::Image { mime, bytes } = body {
                blocks.push(ContentBlock::image(base64::engine::general_purpose::STANDARD.encode(bytes), *mime));
            }
        }
        CallToolResult::success(blocks)
    }

    #[tool(
        description = "Forget the captured requests.",
        title = "Clear captured requests",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_clear(&self) -> CallToolResult {
        self.hub.flows.clear();
        reply(Ok("Captured requests cleared".into()))
    }

    #[tool(
        description = "Save the captured requests as a HAR file in EmuLabZip's exports folder, named `<name>.har`. Returns its path. net_replay can answer the app from it later.",
        title = "Export captured requests as HAR",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_export_har(&self, Parameters(a): Parameters<args::NetExport>) -> CallToolResult {
        reply(
            async {
                let name = file_name(&a.name)?;
                let flows = self.hub.flows.all();
                let har = to_har(&flows);
                let count = har["log"]["entries"].as_array().map_or(0, Vec::len);
                if count == 0 {
                    return Err("There are no requests to export yet.".to_string());
                }
                let dir = exports_dir();
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let path = dir.join(format!("{name}.har"));
                std::fs::write(&path, serde_json::to_vec_pretty(&har).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
                json(&serde_json::json!({ "path": path, "requests": count }))
            }
            .await,
        )
    }

    #[tool(
        description = "The rules in force: mocks, delays, blocks, breakpoints, map to a file or another server, OpenAPI checks and scripts. The first rule that matches a request decides, unless it says thenContinue.",
        title = "List network rules",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn net_rules_get(&self) -> CallToolResult {
        let net = self.hub.net.lock().await;
        reply(json(&net.rules.rules()))
    }

    #[tool(
        description = "Replace the rules in force (send the whole list; net_rules_get shows the current one). They apply to the next request and are kept in a file the desktop app shares. Serving a file works only for files in EmuLabZip's `mocks` folder, and a script only when the user approved it. Mocks, delays, blocks and holds do not need the proxy to be running yet.",
        title = "Set network rules",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_rules_set(&self, Parameters(a): Parameters<args::NetRules>) -> CallToolResult {
        reply(
            async {
                if a.rules.len() > 200 {
                    return Err("at most 200 rules".to_string());
                }
                let rules = check_rules(a.rules)?;
                let net = self.hub.net.lock().await;
                net.rules.set(rules).map_err(|e| e.to_string())?;
                json(&net.rules.rules())
            }
            .await,
        )
    }

    #[tool(
        description = "Slow, lag or cut the network the proxy gives the device, from a preset (fast-3g, slow-3g, slow-wifi, high-latency, packet-loss, offline) or with numbers, or `off` for the normal network.",
        title = "Set network conditions",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_conditions(&self, Parameters(a): Parameters<args::NetConditions>) -> CallToolResult {
        reply(
            async {
                let mut c = if a.off == Some(true) {
                    Conditions::default()
                } else if let Some(p) = a.preset.as_deref() {
                    Conditions::preset(p).ok_or_else(|| format!("`{p}` is not a preset: use fast-3g, slow-3g, slow-wifi, high-latency, packet-loss or offline"))?
                } else {
                    Conditions::default()
                };
                if a.off != Some(true) {
                    if a.down_kbps.is_some() {
                        c.down_kbps = a.down_kbps;
                    }
                    if a.up_kbps.is_some() {
                        c.up_kbps = a.up_kbps;
                    }
                    if let Some(l) = a.latency_ms {
                        c.latency_ms = l;
                    }
                    if let Some(l) = a.loss_percent {
                        c.loss_percent = l;
                    }
                }
                c.check()?;
                let mut net = self.hub.net.lock().await;
                if let Some(p) = &net.proxy {
                    p.set_conditions(c.clone())?;
                }
                net.conditions = c.clone();
                json(&serde_json::json!({ "conditions": c, "summary": c.summary() }))
            }
            .await,
        )
    }

    #[tool(
        description = "Answer the app's requests from a recording, so it runs offline and the same every time: a recorded session (list_sessions) or a HAR file from net_export_har. A request that is not in the recording fails with a 504, unless passthrough lets it reach the server. Rules still win over the recording.",
        title = "Replay a recording",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_replay(&self, Parameters(a): Parameters<args::NetReplay>) -> CallToolResult {
        reply(
            async {
                let path = match (&a.session, &a.har) {
                    (Some(id), None) => {
                        if !valid_session_id(id) {
                            return Err(format!("`{id}` is not a session name: use one from list_sessions"));
                        }
                        let dir = sessions_dir();
                        [format!("{id}.ezsession"), format!("{id}.ezsession.partial")]
                            .into_iter()
                            .map(|n| dir.join(n))
                            .find(|p| p.exists())
                            .ok_or_else(|| format!("there is no session called `{id}`"))?
                    }
                    (None, Some(name)) => exports_dir().join(format!("{}.har", file_name(name)?)),
                    _ => return Err("give a session, or a har name, not both".into()),
                };
                if !path.exists() {
                    return Err(format!("there is no recording at {}", path.display()));
                }
                let passthrough = a.passthrough.unwrap_or(false);
                let set =
                    tokio::task::spawn_blocking(move || emulabzip_session::replay::load(&path, passthrough, false))
                        .await
                        .map_err(|e| e.to_string())?
                        .map_err(|e| e.to_string())?;
                let set = std::sync::Arc::new(set);
                let mut net = self.hub.net.lock().await;
                net.replay = Some(set.clone());
                if let Some(p) = &net.proxy {
                    p.set_replay_set(Some(set));
                }
                json(&net_status_of(&net, &self.hub))
            }
            .await,
        )
    }

    #[tool(
        description = "Go back to asking the real servers.",
        title = "Stop replaying",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn net_replay_stop(&self) -> CallToolResult {
        let mut net = self.hub.net.lock().await;
        net.replay = None;
        if let Some(p) = &net.proxy {
            p.set_replay_set(None);
        }
        reply(json(&net_status_of(&net, &self.hub)))
    }

    #[tool(
        description = "Send a captured request again, with changes if asked (method, address, headers, body). The new request appears in net_flows; its id is returned. The proxy has to be running.",
        title = "Send a request again",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = true)
    )]
    async fn net_resend(&self, Parameters(a): Parameters<args::NetResend>) -> CallToolResult {
        reply(
            async {
                if self.hub.flows.get(a.id).is_none() {
                    return Err(format!("no request has the id {}; net_flows lists them", a.id));
                }
                let edits = Resend {
                    method: a.method,
                    url: a.url,
                    headers: a.headers,
                    body: a.body_text.map(String::into_bytes),
                };
                let net = self.hub.net.lock().await;
                let proxy = net.proxy.as_ref().ok_or("The proxy is not running. net_start starts it.")?;
                let id = proxy.replay(a.id, edits).await?;
                json(&serde_json::json!({ "newRequestId": id }))
            }
            .await,
        )
    }

    #[tool(
        description = "The requests and responses held at a breakpoint right now (a rule with the action breakpoint), with their headers and bodies. A held message goes on by itself after two minutes.",
        title = "List held requests",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn net_breakpoints(&self) -> CallToolResult {
        let net = self.hub.net.lock().await;
        reply(json(&net.breakpoints.pending()))
    }

    #[tool(
        description = "Let a held message go on, optionally with edited headers, body or status, or fail the request with abort.",
        title = "Release a held request",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = true)
    )]
    async fn net_resume(&self, Parameters(a): Parameters<args::NetResume>) -> CallToolResult {
        reply(
            async {
                let answer = if a.abort == Some(true) {
                    Resume::Abort
                } else {
                    Resume::Continue(Edits {
                        headers: a.headers,
                        body: a.body_text.map(String::into_bytes),
                        status: a.status,
                    })
                };
                let net = self.hub.net.lock().await;
                if net.breakpoints.resume(a.id, answer) {
                    Ok("Released".to_string())
                } else {
                    Err("That message is no longer held.".to_string())
                }
            }
            .await,
        )
    }

    #[tool(
        description = "Release everything held at a breakpoint, unchanged.",
        title = "Release all held requests",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = true)
    )]
    async fn net_resume_all(&self) -> CallToolResult {
        self.hub.net.lock().await.breakpoints.resume_all();
        reply(Ok("Everything held was released".into()))
    }

    #[tool(
        description = "The analytics events (Segment, Amplitude, Mixpanel, Firebase) found in the captured requests, decoded: the event's name, kind, properties and user. The proxy has to have seen the app's traffic.",
        title = "Analytics events in captured requests",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn analytics_events(&self, Parameters(a): Parameters<args::AnalyticsEvents>) -> CallToolResult {
        let wanted: Vec<String> = a.services.unwrap_or_default().into_iter().map(|s| s.to_ascii_lowercase()).collect();
        let limit = a.limit.unwrap_or(100).clamp(1, 500);
        let mut out = Vec::new();
        for flow in self.hub.flows.all() {
            for event in emulabzip_net::analytics::events(&flow) {
                let service = format!("{:?}", event.service).to_ascii_lowercase();
                if wanted.is_empty() || wanted.contains(&service) {
                    out.push(serde_json::json!({ "request": flow.id, "event": event }));
                }
            }
        }
        let total = out.len();
        out.truncate(limit);
        reply(json(&serde_json::json!({ "total": total, "shown": out.len(), "events": out })))
    }

    #[tool(
        description = "Check the captured responses against an OpenAPI 3 file (JSON or YAML, given by its full path on this computer): each response that does not match, with where in the body and why. Responses the spec does not describe are counted separately.",
        title = "Check captured responses against OpenAPI",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn openapi_check(&self, Parameters(a): Parameters<args::OpenapiCheck>) -> CallToolResult {
        reply(
            async {
                let path = spec_path(&a.spec)?;
                let spec = emulabzip_net::openapi::Spec::load(&path).map_err(|e| e.to_string())?;
                let (mut ok, mut unknown, mut unchecked) = (0usize, 0usize, 0usize);
                let mut problems = Vec::new();
                for f in self.hub.flows.all() {
                    if f.kind != FlowKind::Http || f.status.is_none() || a.host.as_deref().is_some_and(|h| !glob_match(h, &f.host, true)) {
                        continue;
                    }
                    let encoding = Flow::header(&f.response_headers, "content-encoding");
                    let body = emulabzip_net::decode::decode(encoding, &f.response_body).unwrap_or_else(|_| f.response_body.clone());
                    let v = spec.check(&f.method, &f.path, f.status.unwrap_or(0), f.content_type().as_deref(), &body);
                    match v.outcome.as_str() {
                        "ok" => ok += 1,
                        "unknown" => unknown += 1,
                        "unchecked" => unchecked += 1,
                        _ => problems.push(serde_json::json!({ "request": f.id, "url": f.url, "status": f.status, "operation": v.operation, "violations": v.violations })),
                    }
                }
                json(&serde_json::json!({
                    "spec": spec.name(),
                    "operations": spec.operation_count(),
                    "match": ok,
                    "doNotMatch": problems.len(),
                    "notDescribed": unknown,
                    "notChecked": unchecked,
                    "problems": problems,
                }))
            }
            .await,
        )
    }
}

fn net_status_of(net: &crate::hub::Net, hub: &crate::hub::Hub) -> serde_json::Value {
    serde_json::json!({
        "running": net.proxy.is_some(),
        "port": net.proxy.as_ref().map(|p| p.addr().port()),
        "connectedDevices": net.connected.iter().map(|t| format!("{:?}:{}", t.platform, t.id).to_lowercase()).collect::<Vec<_>>(),
        "requests": hub.flows.summaries().len(),
        "rules": net.rules.rules().len(),
        "conditions": net.conditions,
        "http3": net.http3,
        "replay": net.replay.as_ref().map(|r| r.stats()),
        "heldAtBreakpoints": net.breakpoints.pending().len(),
        "certificate": net.ca.as_ref().and_then(|c| c.cert_path()),
    })
}
