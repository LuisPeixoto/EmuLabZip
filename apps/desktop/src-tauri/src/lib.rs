//! Tauri shell: owns the backends and exposes them to the interface as commands. Live data
//! (logs, samples) is streamed over IPC channels in batches.

mod commands;
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
        ])
        .build(tauri::generate_context!())
        .expect("error while building EmuLabZip");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Stop log streams so no `log stream` or adb socket outlives the app.
            handle.state::<AppState>().stop_all();
        }
    });
}
