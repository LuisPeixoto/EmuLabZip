//! Commands for the Automation view: recording touches into flows, playing them, and exporting them.

use std::sync::Mutex;

use emulabzip_core::Target;
use emulabzip_core::automation::{self, Flow, FlowEntry, Step};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::oneshot;

use crate::commands::{CmdResult, ask_save_path, err};
use crate::state::AppState;

/// What the interface hears while a flow records or plays.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FlowMessage {
    /// A step was recorded, or is about to play. `index` is its place in the flow when playing.
    Step {
        index: usize,
        step: Step,
    },
    /// Recording ended and the flow was saved under `id`.
    Saved {
        id: String,
        steps: usize,
    },
    /// Playing reached the end.
    Done,
    /// Nothing was touched, so nothing was saved.
    Empty,
    Failed {
        message: String,
    },
}

/// The recording in progress, if any. Only one runs at a time.
static RECORDING: Mutex<Option<oneshot::Sender<()>>> = Mutex::new(None);

#[tauri::command]
pub fn flow_list() -> Vec<FlowEntry> {
    automation::list()
}

#[tauri::command]
pub fn flow_get(id: String) -> CmdResult<Flow> {
    automation::load(&id).map_err(err)
}

/// Keeps an edited flow. The id is made from its name, so a new name makes a new flow.
#[tauri::command]
pub fn flow_save(flow: Flow) -> CmdResult<String> {
    let path = automation::save(&flow).map_err(err)?;
    Ok(path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
}

#[tauri::command]
pub fn flow_remove(id: String) -> CmdResult<()> {
    automation::remove(&id).map_err(err)
}

/// Saves a flow as a Maestro file or a shell script, where the person says. `None` when they cancel.
#[tauri::command]
pub async fn flow_export(app: AppHandle, id: String, kind: String) -> CmdResult<Option<String>> {
    let flow = automation::load(&id).map_err(err)?;
    let (text, name, filter) = match kind.as_str() {
        "maestro" => (flow.to_maestro(), format!("{id}.yaml"), ("Maestro flow", &["yaml", "yml"][..])),
        "script" => (flow.to_script(), format!("{id}.sh"), ("Shell script", &["sh"][..])),
        other => return Err(format!("`{other}` is not an export: use maestro or script.")),
    };
    let Some(path) = ask_save_path(&app, &name, filter).await else { return Ok(None) };
    tokio::fs::write(&path, text).await.map_err(err)?;
    #[cfg(unix)]
    if kind == "script" {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).await;
    }
    Ok(Some(path.display().to_string()))
}

/// Starts recording the touches on the device. Steps arrive on `on_message`; `flow_record_stop` ends it, and the
/// flow is saved then.
#[tauri::command]
pub async fn flow_record_start(
    state: State<'_, AppState>,
    target: Target,
    name: String,
    app: Option<String>,
    launch: bool,
    on_message: Channel<FlowMessage>,
) -> CmdResult<u64> {
    if name.trim().is_empty() || name.len() > 100 {
        return Err("The name is between 1 and 100 characters.".into());
    }
    if let Some(app) = &app
        && !emulabzip_core::control::is_app_id(app)
    {
        return Err(format!("`{app}` is not a package name or bundle id."));
    }
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    {
        let mut slot = RECORDING.lock().unwrap();
        if slot.is_some() {
            return Err("A recording is already running. Stop it first.".into());
        }
        *slot = Some(stop_tx);
    }
    let lab = state.lab.clone();
    let task = tauri::async_runtime::spawn(async move {
        let sink = on_message.clone();
        let mut index = 0usize;
        let result = automation::record(
            &lab,
            &target,
            &name,
            app.as_deref(),
            launch,
            async {
                let _ = stop_rx.await;
            },
            |step| {
                let _ = sink.send(FlowMessage::Step { index, step: step.clone() });
                index += 1;
            },
        )
        .await;
        RECORDING.lock().unwrap().take();
        let message = match result {
            Ok(flow) if flow.steps.iter().all(|s| matches!(s, Step::Launch | Step::Wait { .. })) => FlowMessage::Empty,
            Ok(flow) => match automation::save(&flow) {
                Ok(path) => FlowMessage::Saved {
                    id: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    steps: flow.steps.len(),
                },
                Err(e) => FlowMessage::Failed { message: e.to_string() },
            },
            Err(e) => FlowMessage::Failed { message: e.to_string() },
        };
        let _ = on_message.send(message);
    });
    Ok(state.add_session(task))
}

/// Ends the recording in progress. The flow is saved and announced on the recording's channel.
#[tauri::command]
pub fn flow_record_stop() {
    if let Some(stop) = RECORDING.lock().unwrap().take() {
        let _ = stop.send(());
    }
}

/// Plays a saved flow. Each step is announced before it runs; stop it with `stop_session`.
#[tauri::command]
pub async fn flow_play(
    state: State<'_, AppState>,
    target: Target,
    id: String,
    on_message: Channel<FlowMessage>,
) -> CmdResult<u64> {
    let flow = automation::load(&id).map_err(err)?;
    let lab = state.lab.clone();
    let task = tauri::async_runtime::spawn(async move {
        let sink = on_message.clone();
        let result = automation::play(&lab, &target, &flow, std::future::pending(), |index, step| {
            let _ = sink.send(FlowMessage::Step { index, step: step.clone() });
        })
        .await;
        let _ = on_message.send(match result {
            Ok(()) => FlowMessage::Done,
            Err(e) => FlowMessage::Failed { message: e.to_string() },
        });
    });
    Ok(state.add_session(task))
}
