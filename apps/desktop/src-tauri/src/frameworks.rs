//! Commands for the Frameworks view: which UI frameworks an app uses, and looking inside Flutter and React Native.

use emulabzip_core::{Framework, Target};
use emulabzip_frameworks::flutter::{self, FlutterMemory, FrameStats, VmUri, WidgetNode};
use emulabzip_frameworks::react_native::{self, ConsoleLine, HermesHeap, METRO_PORT, ReactNode};
use serde::Serialize;
use tauri::State;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

/// The longest a recording runs, so the window cannot be held for long by a wrong number.
const MAX_SECONDS: u64 = 120;

#[tauri::command]
pub async fn frameworks_detect(state: State<'_, AppState>, target: Target, app: String) -> CmdResult<Vec<Framework>> {
    state.lab.frameworks(&target, &app).await.map_err(err)
}

fn uri(text: Option<String>) -> CmdResult<Option<VmUri>> {
    text.map(|t| VmUri::parse(&t).ok_or_else(|| format!("`{t}` is not an address like http://127.0.0.1:41233/token=/")))
        .transpose()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlutterTree {
    pub root: WidgetNode,
    pub count: usize,
    /// Widgets know where in the source they were made.
    pub tracked: bool,
}

#[tauri::command]
pub async fn flutter_tree(
    state: State<'_, AppState>,
    target: Target,
    vm_uri: Option<String>,
) -> CmdResult<FlutterTree> {
    let flutter = flutter::connect_app(&state.lab, &target, uri(vm_uri)?).await.map_err(err)?;
    let root = flutter.widget_tree().await.map_err(err)?;
    Ok(FlutterTree { count: root.count(), root, tracked: flutter.creation_tracked() })
}

#[tauri::command]
pub async fn flutter_frames(
    state: State<'_, AppState>,
    target: Target,
    vm_uri: Option<String>,
    seconds: u64,
) -> CmdResult<FrameStats> {
    if seconds == 0 || seconds > MAX_SECONDS {
        return Err(format!("Record between 1 and {MAX_SECONDS} seconds."));
    }
    let flutter = flutter::connect_app(&state.lab, &target, uri(vm_uri)?).await.map_err(err)?;
    flutter.frames(seconds).await.map_err(err)
}

#[tauri::command]
pub async fn flutter_memory(
    state: State<'_, AppState>,
    target: Target,
    vm_uri: Option<String>,
) -> CmdResult<FlutterMemory> {
    let flutter = flutter::connect_app(&state.lab, &target, uri(vm_uri)?).await.map_err(err)?;
    flutter.memory(25).await.map_err(err)
}

fn metro(port: Option<u16>) -> u16 {
    port.unwrap_or(METRO_PORT)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactTree {
    pub root: ReactNode,
    pub count: usize,
}

#[tauri::command]
pub async fn rn_tree(app: String, port: Option<u16>) -> CmdResult<ReactTree> {
    let rn = react_native::connect_app(&app, metro(port)).await.map_err(err)?;
    let root = rn.tree().await.map_err(err)?;
    Ok(ReactTree { count: root.count(), root })
}

#[tauri::command]
pub async fn rn_heap(app: String, port: Option<u16>) -> CmdResult<HermesHeap> {
    let rn = react_native::connect_app(&app, metro(port)).await.map_err(err)?;
    rn.heap().await.map_err(err)
}

#[tauri::command]
pub async fn rn_console(app: String, port: Option<u16>, seconds: u64) -> CmdResult<Vec<ConsoleLine>> {
    if seconds == 0 || seconds > MAX_SECONDS {
        return Err(format!("Listen for between 1 and {MAX_SECONDS} seconds."));
    }
    let rn = react_native::connect_app(&app, metro(port)).await.map_err(err)?;
    rn.console(seconds).await.map_err(err)
}
