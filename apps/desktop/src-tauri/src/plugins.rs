//! Commands for the Plugins tool. A plugin only runs after the user enabled it.

use emulabzip_core::Target;
use emulabzip_plugin::{Approvals, Context, Event, Manifest, Plugin, Problem, discover, plugins_dir, runner};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::watch;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRow {
    pub manifest: Manifest,
    pub dir: String,
    pub enabled: bool,
    /// What enabling it would let it do, in one line.
    pub runs: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginList {
    pub dir: String,
    pub plugins: Vec<PluginRow>,
    pub problems: Vec<Problem>,
}

#[tauri::command]
pub fn plugin_list() -> PluginList {
    let (plugins, problems) = discover();
    let approvals = Approvals::load();
    PluginList {
        dir: plugins_dir().display().to_string(),
        plugins: plugins
            .into_iter()
            .map(|p| PluginRow {
                enabled: approvals.is_approved(&p),
                runs: format!("{} (in {})", p.manifest.command.join(" "), p.dir.display()),
                dir: p.dir.display().to_string(),
                manifest: p.manifest,
            })
            .collect(),
        problems,
    }
}

fn find(name: &str) -> CmdResult<Plugin> {
    discover()
        .0
        .into_iter()
        .find(|p| p.manifest.name == name)
        .ok_or_else(|| format!("There is no plugin called {name}"))
}

/// Allows or stops allowing a plugin to run.
#[tauri::command]
pub fn plugin_enable(name: String, enabled: bool) -> CmdResult<()> {
    let plugin = find(&name)?;
    let mut approvals = Approvals::load();
    if enabled { approvals.approve(&plugin) } else { approvals.revoke(&name) }.map_err(err)
}

/// What a plugin run reports.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PluginEvent {
    Event { event: Event },
    Done { outcome: String },
    Failed { message: String },
}

/// Runs an enabled plugin on a device. Events arrive on `on_event`; the last one is `done` or
/// `failed`. Returns the session to pass to [`plugin_stop`].
#[tauri::command]
pub async fn plugin_run(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    target: Target,
    package: Option<String>,
    on_event: Channel<PluginEvent>,
) -> CmdResult<u64> {
    let plugin = find(&name)?;
    if !Approvals::load().is_approved(&plugin) {
        return Err(format!("{name} is not enabled"));
    }
    if !plugin.manifest.supports(target.platform) {
        return Err(format!("{name} does not support {}", target.platform.label()));
    }
    let ctx = Context { device: target.id.clone(), platform: target.platform, app: package };
    let (stop, stopped) = watch::channel(false);
    let session = state.next_id();
    state.plugin_stops.lock().insert(session, stop);
    let task = tauri::async_runtime::spawn(async move {
        let sink = on_event.clone();
        let result = runner::run(&plugin, &ctx, stopped, move |event| {
            let _ = sink.send(PluginEvent::Event { event });
        })
        .await;
        app.state::<AppState>().plugin_stops.lock().remove(&session);
        let _ = on_event.send(match result {
            Ok(outcome) => PluginEvent::Done { outcome: format!("{outcome:?}") },
            Err(e) => PluginEvent::Failed { message: e.to_string() },
        });
    });
    state.register_session(session, task);
    Ok(session)
}

#[tauri::command]
pub fn plugin_stop(state: State<'_, AppState>, session: u64) {
    if let Some(stop) = state.plugin_stops.lock().get(&session) {
        let _ = stop.send(true);
    }
}

/// Opens the plugins folder in the file manager, creating it first.
#[tauri::command]
pub fn plugin_open_folder(app: AppHandle) -> CmdResult<()> {
    let dir = plugins_dir();
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(err)
}
