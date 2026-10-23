//! Tauri shell: owns the backends and exposes them to the interface as commands. Live data
//! (logs, samples) is streamed over IPC channels in batches.

mod commands;
mod files;
mod network;
mod profiling;
mod state;

use tauri::{Manager, RunEvent};

use crate::state::AppState;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_env("EMULABZIP_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .setup(|_| {
            // If an earlier run was killed while it was capturing a simulator, the Mac's proxy
            // still points at EmuLabZip. Put it back before anything else.
            tauri::async_runtime::spawn(async {
                match emulabzip_apple::AppleBackend::new().restore_system_proxy().await {
                    Ok(true) => tracing::warn!("restored the Mac's proxy settings left behind by an earlier run"),
                    Ok(false) => {}
                    Err(e) => tracing::warn!("could not restore the Mac's proxy settings: {e}"),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::host_info,
            commands::list_devices,
            commands::device_info,
            commands::list_apps,
            commands::screenshot,
            commands::save_png,
            commands::save_text,
            commands::start_logs,
            commands::stop_session,
            profiling::start_perf,
            profiling::memory_report,
            profiling::memory_diff,
            profiling::save_heap_dump,
            files::storage_roots,
            files::list_files,
            files::open_file,
            files::sqlite_rows,
            files::sqlite_query,
            files::save_file,
            network::net_state,
            network::net_start,
            network::net_stop,
            network::net_status,
            network::net_connect,
            network::net_disconnect,
            network::net_install_ca,
            network::net_clear_bypass,
            network::net_subscribe,
            network::net_clear,
            network::net_flow,
            network::net_export_har,
        ])
        .build(tauri::generate_context!())
        .expect("error while building EmuLabZip");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Stop log streams so no `log stream` or adb socket outlives the app, and put every
            // device (and the Mac's proxy) back the way it was.
            let state = handle.state::<AppState>();
            state.stop_all();
            tauri::async_runtime::block_on(state.shutdown_network());
        }
    });
}
