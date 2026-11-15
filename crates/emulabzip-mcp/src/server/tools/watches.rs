//! Watches: follow something the device does in the background, and read what came in whenever it suits.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use emulabzip_net::cdp::CdpFlows;

use crate::server::*;
use crate::watch::{MAX_WATCHES, Ring, Watch, idle};

const KINDS: &str = "logs, perf, touches, webview_console, webview_network, react_console, react_actions";

/// Ends a ring with a reason, and a watch that has not been read for a while.
fn finish(ring: &Mutex<Ring>, why: &str) {
    ring.lock().unwrap().ended.get_or_insert_with(|| why.to_string());
}

/// A reply stays well under the size a tool result is cut at, so it is always whole JSON.
const WATCH_REPLY_BYTES: usize = 40_000;

#[tool_router(router = watches_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "Follow something in the background and read it later with watch_read: `logs` (with level, tag and text filters), `perf` samples, `touches` on an Android screen, a WebView's `webview_console` or `webview_network` (its requests also appear in net_flows), a React Native app's `react_console` or `react_actions` (Redux). Use it when the thing happens while you act: start the watch, tap or type, then read. Returns an id. At most 8 run at once, each keeps its last 5000 items, and one nobody reads for ten minutes stops.",
        title = "Start a watch",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn watch_start(&self, Parameters(a): Parameters<args::WatchStart>) -> CallToolResult {
        reply(self.watch_start_inner(a).await)
    }

    #[tool(
        description = "What a watch has gathered since `since` (everything kept without it), at most `limit` items, oldest first. Returns `next`: pass it as `since` the next time. `missed` is true when items were pushed out before you read them. `ended` says why a watch that stopped did.",
        title = "Read a watch",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watch_read(&self, Parameters(a): Parameters<args::WatchRead>) -> CallToolResult {
        reply(
            async {
                let watches = self.hub.watches.lock().unwrap();
                let w =
                    watches.get(&a.id).ok_or_else(|| format!("no watch has the id {}; watch_list shows them", a.id))?;
                *w.last_read.lock().unwrap() = Instant::now();
                let ring = w.ring.lock().unwrap();
                let (items, next, missed) =
                    ring.read(a.since, a.limit.unwrap_or(200).clamp(1, 1000), WATCH_REPLY_BYTES);
                json(&serde_json::json!({
                    "watch": a.id,
                    "kind": w.kind,
                    "items": items,
                    "next": next,
                    "missed": missed,
                    "dropped": ring.dropped(),
                    "running": !w.finished(),
                    "ended": ring.ended,
                }))
            }
            .await,
        )
    }

    #[tool(
        description = "Stop a watch and forget what it gathered.",
        title = "Stop a watch",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn watch_stop(&self, Parameters(a): Parameters<args::WatchId>) -> CallToolResult {
        let removed = self.hub.watches.lock().unwrap().remove(&a.id).is_some();
        reply(if removed {
            Ok(format!("Watch {} stopped", a.id))
        } else {
            Err(format!("no watch has the id {}", a.id))
        })
    }

    #[tool(
        description = "The watches that exist: id, kind, device, how long, how many items are waiting, and whether it still runs.",
        title = "List watches",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watch_list(&self) -> CallToolResult {
        let watches = self.hub.watches.lock().unwrap();
        let mut list: Vec<_> = watches.values().map(Watch::info).collect();
        list.sort_by_key(|w| w.id);
        reply(json(&list))
    }
}

impl EmuLabServer {
    async fn watch_start_inner(&self, a: args::WatchStart) -> Result<String, String> {
        // What can be checked without a device is checked first.
        if !KINDS.split(", ").any(|k| k == a.kind) {
            return Err(format!("`{}` is not a kind of watch: use one of {KINDS}", a.kind));
        }
        let needs_app = a.kind.starts_with("react_");
        if needs_app && a.app.is_none() {
            return Err(format!("{} needs `app`", a.kind));
        }
        if let Some(app) = &a.app
            && !emulabzip_core::control::is_app_id(app)
        {
            return Err(format!("`{app}` is not a package name or bundle id"));
        }
        let level = a
            .level
            .as_deref()
            .map(str::parse::<LogLevel>)
            .transpose()
            .map_err(|_| "level must be verbose, debug, info, warn, error or fatal".to_string())?;
        if let Some(s) = &a.socket
            && !emulabzip_core::backend::is_socket_name(s)
        {
            return Err(format!("`{s}` is not a socket name"));
        }
        let (device, target) = self.device(&a.device).await?;
        {
            let mut watches = self.hub.watches.lock().unwrap();
            // A watch whose task ended, or that nobody reads, is forgotten before the limit is counted.
            watches.retain(|_, w| !w.finished() && !idle(&w.last_read));
            if watches.len() >= MAX_WATCHES {
                return Err(format!("{MAX_WATCHES} watches are running; watch_stop one first (watch_list shows them)"));
            }
        }
        let ring = Arc::new(Mutex::new(Ring::default()));
        let last_read = Arc::new(Mutex::new(Instant::now()));
        let lab = self.lab.clone();
        let flows = self.hub.flows.clone();
        let (ring2, last2) = (ring.clone(), last_read.clone());
        let kind = a.kind.clone();
        let detail = a.app.clone().unwrap_or_default();

        // Everything that can fail on starting does so here, so the assistant hears about it now.
        let task = match kind.as_str() {
            "logs" => {
                let mut filter = LogFilter {
                    min_level: level,
                    pids: Vec::new(),
                    tag: a.tag.clone(),
                    text: a.grep.clone(),
                    regex: false,
                };
                if let Some(app) = &a.app {
                    let apps = self.lab.apps(&target).await.map_err(|e| e.to_string())?;
                    filter.pids = apps.iter().filter(|x| &x.id == app).filter_map(|x| x.pid).collect();
                    if filter.pids.is_empty() {
                        return Err(format!("{app} is not running, so it has no log lines to follow"));
                    }
                }
                let matcher = filter.compile().map_err(|e| e.to_string())?;
                let mut stream =
                    lab.logs(&target, LogOptions { tail: 0, verbose: true }).await.map_err(|e| e.to_string())?;
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            entry = stream.next() => match entry {
                                Some(e) => if matcher.matches(&e) { ring2.lock().unwrap().push(serde_json::Value::String(e.to_line())); },
                                None => { finish(&ring2, "the log ended"); break }
                            },
                            () = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                                if idle(&last2) { finish(&ring2, "nobody read it for ten minutes"); break }
                            }
                        }
                    }
                })
            }
            "perf" => {
                let interval = a.interval_seconds.unwrap_or(1).clamp(1, 60) * 1000;
                let mut stream = lab
                    .perf(&target, PerfOptions { app: a.app.clone(), interval_ms: interval })
                    .await
                    .map_err(|e| e.to_string())?;
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            s = stream.next() => match s {
                                Some(s) => ring2.lock().unwrap().push(serde_json::to_value(&s).unwrap_or_default()),
                                None => { finish(&ring2, "the samples ended"); break }
                            },
                            () = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                                if idle(&last2) { finish(&ring2, "nobody read it for ten minutes"); break }
                            }
                        }
                    }
                })
            }
            "touches" => {
                let mut stream = lab.input(&target).await.map_err(|e| e.to_string())?;
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            e = stream.next() => match e {
                                Some(e) => ring2.lock().unwrap().push(serde_json::to_value(e).unwrap_or_default()),
                                None => { finish(&ring2, "the touches ended"); break }
                            },
                            () = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                                if idle(&last2) { finish(&ring2, "nobody read it for ten minutes"); break }
                            }
                        }
                    }
                })
            }
            "webview_console" | "webview_network" => {
                let (_, page, view) = emulabzip_frameworks::webview::connect(
                    &lab,
                    &target,
                    a.app.as_deref(),
                    a.socket.as_deref(),
                    a.page.as_deref(),
                )
                .await
                .map_err(|e| e.to_string())?;
                let network = kind == "webview_network";
                let mut events = view.events();
                view.start(network).await.map_err(|e| e.to_string())?;
                let _ = page;
                tokio::spawn(async move {
                    let mut cdp = CdpFlows::new(flows);
                    loop {
                        tokio::select! {
                            event = events.recv() => {
                                let event = match event {
                                    Ok(e) => e,
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                                    Err(_) => { finish(&ring2, "the page was closed"); break }
                                };
                                if network {
                                    if let Some(need) = cdp.on_event(&event)
                                        && let Ok(Ok((body, b64))) = tokio::time::timeout(std::time::Duration::from_secs(3), view.response_body(&need.request_id)).await
                                    {
                                        cdp.set_body(need.flow, &body, b64);
                                    }
                                    if event["method"] == "Network.loadingFinished" || event["method"] == "Network.loadingFailed" {
                                        ring2.lock().unwrap().push(serde_json::json!({ "request": event["params"]["requestId"], "event": event["method"] }));
                                    }
                                } else if let Some(line) = emulabzip_frameworks::webview::parse_event(&event) {
                                    ring2.lock().unwrap().push(serde_json::to_value(&line).unwrap_or_default());
                                }
                            }
                            () = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                                if idle(&last2) { finish(&ring2, "nobody read it for ten minutes"); break }
                            }
                        }
                    }
                })
            }
            "react_console" => {
                let app = a.app.clone().unwrap_or_default();
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&app, port).await.map_err(|e| e.to_string())?;
                tokio::spawn(async move {
                    loop {
                        if idle(&last2) {
                            finish(&ring2, "nobody read it for ten minutes");
                            break;
                        }
                        match rn.console(5).await {
                            Ok(lines) => {
                                let mut ring = ring2.lock().unwrap();
                                lines.iter().for_each(|l| ring.push(serde_json::to_value(l).unwrap_or_default()));
                            }
                            Err(e) => {
                                finish(&ring2, &e.to_string());
                                break;
                            }
                        }
                    }
                })
            }
            _ => {
                let app = a.app.clone().unwrap_or_default();
                let port = a.metro_port.unwrap_or(emulabzip_frameworks::react_native::METRO_PORT);
                let rn =
                    emulabzip_frameworks::react_native::connect_app(&app, port).await.map_err(|e| e.to_string())?;
                rn.watch_actions().await.map_err(|e| e.to_string())?;
                tokio::spawn(async move {
                    let mut since = 0u64;
                    loop {
                        if idle(&last2) {
                            let _ = rn.stop_actions().await;
                            finish(&ring2, "nobody read it for ten minutes");
                            break;
                        }
                        match rn.actions_since(since).await {
                            Ok((_, actions)) => {
                                let mut ring = ring2.lock().unwrap();
                                for action in actions {
                                    since = since.max(action.seq);
                                    ring.push(serde_json::to_value(&action).unwrap_or_default());
                                }
                            }
                            Err(e) => {
                                finish(&ring2, &e.to_string());
                                break;
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                })
            }
        };
        let id = self.hub.next_watch.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let watch = Watch {
            id,
            kind: a.kind.clone(),
            device: device.name.clone(),
            detail,
            started: Instant::now(),
            ring,
            last_read,
            task,
        };
        self.hub.watches.lock().unwrap().insert(id, watch);
        json(&serde_json::json!({
            "watch": id,
            "kind": a.kind,
            "device": device.name,
            "next": "act on the device, then call watch_read with this id (and `since` from its `next`)",
        }))
    }
}
