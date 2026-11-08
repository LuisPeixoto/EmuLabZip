//! Tauri shell: owns the backends and exposes them to the interface as commands. Live data
//! (logs, samples) is streamed over IPC channels in batches.

mod commands;
mod controls;
mod crashes;
mod devices;
mod edit;
mod files;
mod frameworks;
mod layout;
mod network;
mod plugins;
mod profiling;
mod quality;
mod report;
mod sessions;
mod state;
mod states;
mod themes;
mod traces;

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
            profiling::startup_measure,
            profiling::energy_begin,
            profiling::energy_end,
            profiling::pick_heap_dump,
            profiling::heap_analyze,
            files::storage_roots,
            files::list_files,
            files::open_file,
            files::sqlite_rows,
            files::sqlite_query,
            files::save_file,
            files::sqlite_keyed_rows,
            files::realm_rows,
            files::coredata_rows,
            edit::edit_cells,
            edit::edit_insert,
            edit::edit_delete,
            edit::edit_pref,
            edit::edit_plist,
            edit::edit_undo,
            states::state_save,
            states::state_pick,
            states::state_info,
            states::state_restore,
            states::state_compare,
            states::state_compare_now,
            devices::devices_all,
            devices::device_options,
            devices::device_create,
            devices::device_boot,
            devices::device_shutdown,
            devices::device_wipe,
            devices::device_delete,
            devices::emulator_snapshots,
            devices::emulator_snapshot,
            themes::theme_import,
            themes::theme_export,
            quality::audit_security,
            quality::text_check,
            quality::colour_simulate,
            quality::package_pick,
            quality::package_analyze,
            quality::package_diff,
            quality::visual_list,
            quality::visual_capture,
            quality::visual_compare,
            quality::analytics_events,
            layout::layout_read,
            sessions::session_status,
            sessions::session_start,
            sessions::session_stop,
            sessions::session_list,
            sessions::session_open,
            sessions::session_request,
            sessions::session_screen,
            sessions::session_video,
            sessions::session_delete,
            sessions::session_reveal,
            sessions::compare_info,
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
            network::net_rules_get,
            network::net_rules_set,
            network::net_breakpoints_subscribe,
            network::net_resume,
            network::net_resume_all,
            network::net_set_http3,
            network::net_set_hide_http3,
            network::net_replay_load,
            network::net_replay_stop,
            network::net_condition_presets,
            network::net_set_conditions,
            network::net_pick_protos,
            network::net_clear_protos,
            network::net_resend,
            frameworks::frameworks_detect,
            frameworks::flutter_tree,
            frameworks::flutter_frames,
            frameworks::flutter_memory,
            frameworks::rn_tree,
            frameworks::rn_heap,
            frameworks::rn_console,
            frameworks::rn_state,
            frameworks::rn_state_watch,
            frameworks::rn_state_stop,
            network::net_import_pcap,
            controls::controls_info,
            controls::control_apply,
            crashes::crash_list,
            crashes::crash_get,
            crashes::crash_save,
            crashes::mapping_add,
            report::report_save,
            plugins::plugin_list,
            plugins::plugin_enable,
            plugins::plugin_run,
            plugins::plugin_stop,
            plugins::plugin_open_folder,
            traces::trace_start,
            traces::trace_stop,
            traces::trace_templates,
            traces::trace_list,
            traces::trace_delete,
            traces::trace_processor_status,
            traces::trace_install_processor,
            traces::trace_open,
            traces::trace_flame,
            traces::trace_folded,
            traces::trace_query,
            traces::trace_open_perfetto_ui,
            traces::trace_open_external,
            traces::trace_save,
        ])
        .build(tauri::generate_context!())
        .expect("error while building EmuLabZip");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Stop log streams so no `log stream` or adb socket outlives the app, and put every
            // device (and the Mac's proxy) back the way it was.
            let state = handle.state::<AppState>();
            tauri::async_runtime::block_on(state.shutdown_traces());
            state.stop_all();
            tauri::async_runtime::block_on(state.shutdown_network());
        }
    });
}
