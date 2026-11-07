//! The Devices view: emulators and simulators running or not, and making, starting, stopping, wiping and deleting them.

use emulabzip_core::{
    BootOptions, CreateDevice, Device, DeviceOptions, EmulatorSnapshot, Platform, SnapshotAction, Target,
};
use serde::Serialize;
use tauri::State;

use crate::commands::{CmdResult, err};
use crate::state::AppState;

#[derive(Serialize)]
pub struct AllDevices {
    pub devices: Vec<Device>,
    pub problems: Vec<String>,
}

#[tauri::command]
pub async fn devices_all(state: State<'_, AppState>) -> CmdResult<AllDevices> {
    let (devices, errors) = state.lab.all_devices().await;
    Ok(AllDevices { devices, problems: errors.iter().map(|(p, e)| format!("{p}: {e}")).collect() })
}

#[tauri::command]
pub async fn device_options(state: State<'_, AppState>, platform: Platform) -> CmdResult<DeviceOptions> {
    state.lab.device_options(platform).await.map_err(err)
}

#[tauri::command]
pub async fn device_create(state: State<'_, AppState>, platform: Platform, request: CreateDevice) -> CmdResult<Device> {
    state.lab.create_device(platform, &request).await.map_err(err)
}

/// Starting from factory state (`wipe`) throws the data away, so it needs `confirm`.
#[tauri::command]
pub async fn device_boot(
    state: State<'_, AppState>,
    target: Target,
    options: BootOptions,
    confirm: bool,
) -> CmdResult<Device> {
    if options.wipe && !confirm {
        return Err("Starting from factory state erases the device. Confirm it first.".into());
    }
    state.lab.boot_device(&target, &options).await.map_err(err)
}

#[tauri::command]
pub async fn device_shutdown(state: State<'_, AppState>, target: Target) -> CmdResult<()> {
    state.lab.shutdown_device(&target).await.map_err(err)
}

#[tauri::command]
pub async fn device_wipe(state: State<'_, AppState>, target: Target, confirm: bool) -> CmdResult<()> {
    state.lab.wipe_device(&target, confirm).await.map_err(err)
}

#[tauri::command]
pub async fn device_delete(state: State<'_, AppState>, target: Target, confirm: bool) -> CmdResult<()> {
    state.lab.delete_device(&target, confirm).await.map_err(err)
}

#[tauri::command]
pub async fn emulator_snapshots(state: State<'_, AppState>, target: Target) -> CmdResult<Vec<EmulatorSnapshot>> {
    state.lab.emulator_snapshots(&target).await.map_err(err)
}

#[tauri::command]
pub async fn emulator_snapshot(
    state: State<'_, AppState>,
    target: Target,
    action: SnapshotAction,
    name: String,
    confirm: bool,
) -> CmdResult<()> {
    state.lab.emulator_snapshot(&target, action, &name, confirm).await.map_err(err)
}
