//! Recording a session from the assistant: start it, use the app, stop it, and read it.

use std::time::Duration;

use emulabzip_session::recorder::{Options, Recorder};

use crate::server::*;

/// A recording in progress.
pub struct Active {
    pub recorder: Recorder,
    pub id: String,
    pub path: std::path::PathBuf,
    pub device: String,
    pub started: std::time::Instant,
    pub network: bool,
}

#[tool_router(router = recording_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "Start recording a session: the log, performance samples, crashes, and optionally the screen (video_seconds), touches, screenshots and the network, all on one time axis. Use the app, then session_stop; session_summary and session_timeline read it, and create_bug_report can pack it. Only one session records at a time.",
        title = "Start recording a session",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn session_start(&self, Parameters(a): Parameters<args::SessionStart>) -> CallToolResult {
        reply(
            async {
                let id = match &a.name {
                    Some(n) => {
                        if !valid_session_id(n) || n.starts_with('.') {
                            return Err(format!("`{n}` is not a session name: use letters, digits, - and _"));
                        }
                        n.clone()
                    }
                    None => format!(
                        "mcp-{}",
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
                    ),
                };
                if let Some(app) = &a.app
                    && !emulabzip_core::control::is_app_id(app)
                {
                    return Err(format!("`{app}` is not a package name or bundle id"));
                }
                if let Some(v) = a.video_seconds
                    && !(5..=60).contains(&v)
                {
                    return Err("video_seconds is between 5 and 60".into());
                }
                let dir = sessions_dir();
                let path = dir.join(format!("{id}.ezsession"));
                if path.exists() {
                    return Err(format!("a session called `{id}` already exists; choose another name"));
                }
                let (device, target) = self.device(&a.device).await?;
                let mut slot = self.hub.recording.lock().await;
                if slot.is_some() {
                    return Err("A session is already recording. session_stop ends it.".into());
                }
                let network = a.network.unwrap_or(false);
                if network {
                    let mut net = self.hub.net.lock().await;
                    self.hub.start_proxy(&mut net, None).await?;
                    let status = self.lab.connect_network(&target, net.port).await.map_err(|e| e.to_string())?;
                    let _ = status;
                    if !net.connected.contains(&target) {
                        net.connected.push(target.clone());
                    }
                    self.hub.flows.clear();
                }
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let options = Options {
                    app: a.app.clone(),
                    screens_every: a.screenshot_every_seconds.map(|s| Duration::from_secs(s.clamp(1, 600))),
                    flows: network.then(|| self.hub.flows.clone()),
                    video: a.video_seconds,
                    taps: a.touches.unwrap_or(false),
                    ..Default::default()
                };
                let recorder = Recorder::start(self.lab.clone(), &device, path.clone(), options)
                    .await
                    .map_err(|e| e.to_string())?;
                *slot = Some(Active {
                    recorder,
                    id: id.clone(),
                    path,
                    device: device.name.clone(),
                    started: std::time::Instant::now(),
                    network,
                });
                json(&serde_json::json!({
                    "session": id,
                    "device": device.name,
                    "recording": true,
                    "next": "use the app, then call session_stop",
                }))
            }
            .await,
        )
    }

    #[tool(
        description = "How the session that is recording is going: how long, how many log lines, samples, crashes, touches and video pieces so far.",
        title = "Recording status",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn session_status(&self) -> CallToolResult {
        let slot = self.hub.recording.lock().await;
        reply(match slot.as_ref() {
            None => Ok("No session is recording. session_start starts one.".to_string()),
            Some(r) => {
                let stats = r.recorder.stats();
                json(&serde_json::json!({
                    "session": r.id,
                    "device": r.device,
                    "seconds": r.started.elapsed().as_secs(),
                    "network": r.network,
                    "so_far": {
                        "logLines": stats.logs, "samples": stats.samples, "crashes": stats.crashes, "screenshots": stats.screens,
                        "touches": stats.taps, "slowFrames": stats.slow_frames, "videoPieces": stats.video_pieces,
                    },
                }))
            }
        })
    }

    #[tool(
        description = "Stop the session that is recording and save it. Returns its name and a summary: errors, crashes, slow requests and the numbers. Read more with session_timeline, or pack it with create_bug_report.",
        title = "Stop recording a session",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn session_stop(&self) -> CallToolResult {
        reply(
            async {
                let active = self
                    .hub
                    .recording
                    .lock()
                    .await
                    .take()
                    .ok_or("No session is recording. session_start starts one.")?;
                let path = active.recorder.finish().await.map_err(|e| e.to_string())?;
                let session = emulabzip_session::Session::open(&path).map_err(|e| e.to_string())?;
                json(&serde_json::json!({
                    "session": active.id,
                    "path": path,
                    "seconds": active.started.elapsed().as_secs(),
                    "summary": emulabzip_session::summarize(&session),
                }))
            }
            .await,
        )
    }

    #[tool(
        description = "Compare two recorded sessions: the numbers (start time, frame rate, memory, errors, slow requests…) that got worse by more than a tolerance. Use it to see whether a change made the app slower.",
        title = "Compare two sessions",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn session_compare(&self, Parameters(a): Parameters<args::SessionCompare>) -> CallToolResult {
        reply(
            async {
                let before = open_session(&a.baseline)?;
                let after = open_session(&a.run)?;
                let changes = emulabzip_session::baseline::compare(
                    &emulabzip_session::summarize(&before),
                    &emulabzip_session::summarize(&after),
                    a.tolerance_percent.unwrap_or(10.0).clamp(0.0, 1000.0),
                );
                json(&serde_json::json!({
                    "baseline": a.baseline,
                    "run": a.run,
                    "regressions": changes.iter().filter(|c| c.regression).count(),
                    "changes": changes,
                }))
            }
            .await,
        )
    }
}
