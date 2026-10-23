//! Perfetto trace configurations, in the text form `perfetto --txt -c` reads.

use emulabzip_core::{Error, PerfettoPreset, Result};

/// How much memory the trace buffer may use on the device, in kilobytes.
pub const BUFFER_KB: u32 = 65_536;

/// Only characters a package name can have, so an app name can never change the configuration.
fn check_package(app: &str) -> Result<&str> {
    let ok =
        !app.is_empty() && app.len() <= 255 && app.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
    if ok { Ok(app) } else { Err(Error::invalid("package name", app)) }
}

fn buffers() -> String {
    format!(
        "buffers {{ size_kb: {BUFFER_KB} fill_policy: RING_BUFFER }}\n\
         buffers {{ size_kb: 8192 fill_policy: RING_BUFFER }}\n"
    )
}

fn footer(session: &str, duration_s: u32) -> String {
    format!(
        "duration_ms: {}\n\
         write_into_file: true\n\
         file_write_period_ms: 2500\n\
         flush_period_ms: 5000\n\
         max_file_size_bytes: 1000000000\n\
         unique_session_name: \"{session}\"\n",
        u64::from(duration_s) * 1000
    )
}

fn ftrace(events: &[&str], categories: &[&str], app: Option<&str>) -> String {
    let mut s = String::from("data_sources {\n  config {\n    name: \"linux.ftrace\"\n    ftrace_config {\n");
    for e in events {
        s.push_str(&format!("      ftrace_events: \"{e}\"\n"));
    }
    for c in categories {
        s.push_str(&format!("      atrace_categories: \"{c}\"\n"));
    }
    if let Some(app) = app {
        s.push_str(&format!("      atrace_apps: \"{app}\"\n"));
    }
    s.push_str("    }\n  }\n}\n");
    s
}

fn process_stats(poll_ms: u32) -> String {
    format!(
        "data_sources {{\n  config {{\n    name: \"linux.process_stats\"\n    target_buffer: 1\n    \
         process_stats_config {{ scan_all_processes_on_start: true proc_stats_poll_ms: {poll_ms} }}\n  }}\n}}\n"
    )
}

const SCHED: &[&str] = &[
    "sched/sched_switch",
    "sched/sched_waking",
    "sched/sched_wakeup_new",
    "sched/sched_process_exit",
    "sched/sched_process_free",
    "task/task_newtask",
    "task/task_rename",
];

/// The configuration for a preset. `session` names the recording, so it can be stopped early.
///
/// `custom` is used as it is for [`PerfettoPreset::Custom`], with the duration and session name
/// added when it does not set them.
pub fn config(
    preset: PerfettoPreset,
    session: &str,
    duration_s: u32,
    app: Option<&str>,
    custom: Option<&str>,
) -> Result<String> {
    let app = app.map(check_package).transpose()?;
    let body = match preset {
        PerfettoPreset::Cpu => {
            let mut events: Vec<&str> = SCHED.to_vec();
            events.extend(["power/cpu_frequency", "power/cpu_idle", "power/suspend_resume"]);
            let mut s = buffers();
            s += &ftrace(&events, &["am", "wm", "sched", "freq", "idle"], None);
            s += &process_stats(1000);
            s += "data_sources {\n  config {\n    name: \"linux.sys_stats\"\n    target_buffer: 1\n    sys_stats_config { cpufreq_period_ms: 500 }\n  }\n}\n";
            s
        }
        PerfettoPreset::Frames => {
            let mut s = buffers();
            s += &ftrace(SCHED, &["gfx", "view", "input", "am", "wm", "res", "sched", "freq", "binder_driver"], app);
            s += "data_sources {\n  config { name: \"android.surfaceflinger.frametimeline\" }\n}\n";
            s += &process_stats(1000);
            s
        }
        PerfettoPreset::Memory => {
            let mut s = buffers();
            s += &process_stats(250);
            s += "data_sources {\n  config {\n    name: \"linux.sys_stats\"\n    target_buffer: 1\n    sys_stats_config {\n      \
                  meminfo_period_ms: 250\n      meminfo_counters: MEMINFO_MEM_TOTAL\n      meminfo_counters: MEMINFO_MEM_FREE\n      \
                  meminfo_counters: MEMINFO_MEM_AVAILABLE\n      meminfo_counters: MEMINFO_BUFFERS\n      meminfo_counters: MEMINFO_CACHED\n      \
                  meminfo_counters: MEMINFO_SWAP_TOTAL\n      meminfo_counters: MEMINFO_SWAP_FREE\n      vmstat_period_ms: 1000\n    }\n  }\n}\n";
            // Native allocations need the app to be profileable or debuggable.
            if let Some(app) = app {
                s += &format!(
                    "data_sources {{\n  config {{\n    name: \"android.heapprofd\"\n    target_buffer: 0\n    heapprofd_config {{\n      \
                     sampling_interval_bytes: 4096\n      process_cmdline: \"{app}\"\n      shmem_size_bytes: 8388608\n      block_client: true\n    }}\n  }}\n}}\n"
                );
            }
            s
        }
        PerfettoPreset::Custom => {
            let text = custom
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or_else(|| Error::invalid("configuration", "empty"))?;
            let mut s = text.to_string();
            s.push('\n');
            if !text.contains("duration_ms") {
                s += &format!("duration_ms: {}\n", u64::from(duration_s) * 1000);
            }
            if !text.contains("write_into_file") {
                s += "write_into_file: true\nfile_write_period_ms: 2500\nflush_period_ms: 5000\n";
            }
            if !text.contains("unique_session_name") {
                s += &format!("unique_session_name: \"{session}\"\n");
            }
            return Ok(s);
        }
    };
    Ok(body + &footer(session, duration_s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(p: PerfettoPreset, app: Option<&str>) -> String {
        config(p, "emulabzip-1", 10, app, None).unwrap()
    }

    #[test]
    fn every_preset_names_its_session_and_duration() {
        for p in [PerfettoPreset::Cpu, PerfettoPreset::Frames, PerfettoPreset::Memory] {
            let c = make(p, None);
            assert!(c.contains("duration_ms: 10000"), "{p:?}");
            assert!(c.contains("unique_session_name: \"emulabzip-1\""), "{p:?}");
            assert!(c.contains("write_into_file: true"), "{p:?}");
            assert_eq!(c.matches('{').count(), c.matches('}').count(), "balanced braces in {p:?}");
        }
    }

    #[test]
    fn the_cpu_preset_records_scheduling_and_frequency() {
        let c = make(PerfettoPreset::Cpu, None);
        for needle in ["sched/sched_switch", "power/cpu_frequency", "linux.process_stats", "cpufreq_period_ms"] {
            assert!(c.contains(needle), "{needle}");
        }
    }

    #[test]
    fn the_frames_preset_records_frame_timelines_and_the_app() {
        let c = make(PerfettoPreset::Frames, Some("com.example.app"));
        assert!(c.contains("android.surfaceflinger.frametimeline"));
        assert!(c.contains("atrace_categories: \"gfx\""));
        assert!(c.contains("atrace_apps: \"com.example.app\""));
    }

    #[test]
    fn the_memory_preset_profiles_native_allocations_only_for_an_app() {
        assert!(!make(PerfettoPreset::Memory, None).contains("heapprofd"));
        let c = make(PerfettoPreset::Memory, Some("com.example.app"));
        assert!(c.contains("process_cmdline: \"com.example.app\""));
        assert!(c.contains("MEMINFO_MEM_AVAILABLE"));
    }

    #[test]
    fn an_app_name_cannot_change_the_configuration() {
        for bad in ["", "a\" } data_sources {", "com.example;rm", "x y", "é"] {
            assert!(config(PerfettoPreset::Frames, "s", 5, Some(bad), None).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_custom_configuration_gets_what_it_lacks() {
        let c = config(PerfettoPreset::Custom, "emulabzip-9", 20, None, Some("buffers { size_kb: 1024 }")).unwrap();
        assert!(c.starts_with("buffers { size_kb: 1024 }"));
        assert!(c.contains("duration_ms: 20000") && c.contains("emulabzip-9") && c.contains("write_into_file"));
        // What the user set is left alone.
        let own = config(
            PerfettoPreset::Custom,
            "s",
            20,
            None,
            Some("duration_ms: 5000\nwrite_into_file: true\nunique_session_name: \"mine\""),
        )
        .unwrap();
        assert_eq!(own.matches("duration_ms").count(), 1);
        assert_eq!(own.matches("unique_session_name").count(), 1);
        assert!(config(PerfettoPreset::Custom, "s", 5, None, Some("   ")).is_err());
    }
}
