# EmuLabZip for assistants

EmuLabZip reads and controls Android emulators and iOS simulators. Every tool takes an optional `device` (id or name);
leave it out when one device is running. Start with `list_devices`.

## Find the right tool by task

- **See the screen**: `screenshot`, `get_layout`, `audit_accessibility`, `check_text`.
- **Drive the app**: `launch_app`, `tap`, `swipe`, `input_text`, `press_key`, `open_url`, `play_flow`.
- **Read what happened**: `read_logs`, `list_crashes` then `get_crash`, `symbolicate_stack`, `perf_snapshot`.
- **Keep watching**: `watch_start` (logs, perf, touches, WebView and React Native streams), `watch_read`, `watch_stop`.
- **Network**: `net_start`, `net_connect_device`, `net_flows`, `net_flow`, `net_rules_set`, `net_export_har`.
- **Record and replay**: `session_start`, `session_stop`, `session_summary`, `session_timeline`, `session_compare`.
- **App data (read only)**: `storage_roots`, `read_database`, `read_preferences`, `list_files`, `read_file`.
- **Performance**: `record_trace`, `trace_summary`, `measure_startup`, `memory_diff`.
- **Before release**: `audit_security`, `analyze_package`, `package_budget`, `i18n_sweep`, `visual_compare`.
- **Reports**: `create_bug_report`.

## Safety

- Tools that wipe, delete, restore or rewrite data are only offered when the person started the server with
  `--allow-writes`, and each call then asks them to confirm.
- Approving scripts and plugins, and running `ci`, are never offered: they stay with the person.
- Say what you are about to do before acting on a device, and put things back as you found them (for example
  `net_disconnect_device`).

## Resources

`emulabzip://devices`, `sessions`, `flows`, `states`, `links/history`, `rules`, `network/flows` and `guide`, plus
`emulabzip://sessions/{id}`, `emulabzip://flows/{id}` and `emulabzip://network/flows/{id}`.
