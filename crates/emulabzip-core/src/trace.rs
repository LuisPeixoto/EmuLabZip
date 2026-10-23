//! Recording traces: what the system and an app did over a few seconds.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A ready-made Perfetto configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PerfettoPreset {
    /// Scheduling, CPU frequency and idle states, and the processes that ran.
    Cpu,
    /// Frame timelines from SurfaceFlinger plus the view, graphics and input slices around them.
    Frames,
    /// Memory counters over time, plus native allocations of one app.
    Memory,
    /// The user's own text configuration.
    Custom,
}

/// What to record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TraceRequest {
    /// A system trace recorded by Perfetto (Android).
    #[serde(rename_all = "camelCase")]
    Perfetto {
        preset: PerfettoPreset,
        /// The configuration in text form. Only used by [`PerfettoPreset::Custom`].
        config: Option<String>,
        duration_s: u32,
        /// Package to look at more closely (native allocations, atrace for that app).
        app: Option<String>,
    },
    /// CPU samples with call stacks, for a flame graph (simpleperf on Android).
    #[serde(rename_all = "camelCase")]
    CpuProfile { app: Option<String>, duration_s: u32, system_wide: bool },
    /// An Instruments template, recorded with `xctrace` (iOS Simulator).
    #[serde(rename_all = "camelCase")]
    Instruments { template: String, app: Option<String>, duration_s: u32 },
}

impl TraceRequest {
    pub fn duration_s(&self) -> u32 {
        match self {
            TraceRequest::Perfetto { duration_s, .. }
            | TraceRequest::CpuProfile { duration_s, .. }
            | TraceRequest::Instruments { duration_s, .. } => *duration_s,
        }
    }

    pub fn app(&self) -> Option<&str> {
        match self {
            TraceRequest::Perfetto { app, .. }
            | TraceRequest::CpuProfile { app, .. }
            | TraceRequest::Instruments { app, .. } => app.as_deref(),
        }
    }
}

/// How a recording is stored, which decides how it is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TraceFormat {
    /// A Perfetto protobuf trace (`.pftrace`).
    Perfetto,
    /// A simpleperf report in protobuf form (`.pb`).
    SimpleperfReport,
    /// An Instruments document (`.trace`), opened by Xcode.
    InstrumentsBundle,
}

/// A recording on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceFile {
    pub path: PathBuf,
    pub format: TraceFormat,
    /// Files saved next to it, such as the raw simpleperf `perf.data` or an `xctrace` export.
    pub extras: Vec<PathBuf>,
}

/// The longest recording that is accepted, in seconds. Traces grow fast.
pub const MAX_DURATION_S: u32 = 300;

/// Rejects durations that are zero or too long, with a message for the user.
pub fn check_duration(seconds: u32) -> Result<(), String> {
    match seconds {
        0 => Err("The duration must be at least one second.".into()),
        s if s > MAX_DURATION_S => Err(format!("A recording can last up to {MAX_DURATION_S} seconds.")),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_through_json() {
        let r = TraceRequest::Perfetto {
            preset: PerfettoPreset::Frames,
            config: None,
            duration_s: 10,
            app: Some("com.example".into()),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["kind"], "perfetto");
        assert_eq!(json["durationS"], 10);
        assert_eq!(json["preset"], "frames");
        assert_eq!(serde_json::from_value::<TraceRequest>(json).unwrap(), r);
    }

    #[test]
    fn exposes_the_common_fields() {
        let r = TraceRequest::CpuProfile { app: Some("a.b".into()), duration_s: 7, system_wide: false };
        assert_eq!((r.duration_s(), r.app()), (7, Some("a.b")));
    }

    #[test]
    fn limits_the_duration() {
        assert!(check_duration(0).is_err());
        assert!(check_duration(1).is_ok());
        assert!(check_duration(MAX_DURATION_S).is_ok());
        assert!(check_duration(MAX_DURATION_S + 1).unwrap_err().contains("300"));
    }
}
