//! Live performance samples.

use serde::{Deserialize, Serialize};

/// What to sample and how often.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfOptions {
    /// Package name or bundle id. Without an app only device-wide values are sampled.
    pub app: Option<String>,
    /// Time between samples, in milliseconds. Values below 250 are raised to 250.
    pub interval_ms: u32,
}

impl Default for PerfOptions {
    fn default() -> Self {
        PerfOptions { app: None, interval_ms: 1000 }
    }
}

impl PerfOptions {
    pub fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(u64::from(self.interval_ms.max(250)))
    }
}

/// One point in time. Every value is optional: a platform reports what it can measure, and the
/// first sample has no CPU values because CPU usage needs two readings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfSample {
    /// Milliseconds since the stream started.
    pub t_ms: u64,
    /// Process id of the app when it is running.
    pub pid: Option<u32>,
    /// App CPU usage, as a percentage of the whole device (all cores).
    pub app_cpu: Option<f32>,
    /// Device CPU usage, as a percentage of all cores.
    pub device_cpu: Option<f32>,
    /// App resident memory in kilobytes.
    pub app_memory_kb: Option<u64>,
    pub device_memory_used_kb: Option<u64>,
    pub device_memory_total_kb: Option<u64>,
    /// Frames rendered per second by the app since the previous sample.
    pub fps: Option<f32>,
    /// Share of janky frames since the previous sample, in percent.
    pub jank_percent: Option<f32>,
    pub battery_level: Option<u8>,
    pub battery_temp_c: Option<f32>,
}

/// CPU usage between two readings of cumulative counters, in percent.
///
/// `busy` and `total` are deltas in the same unit (jiffies, or microseconds of CPU time).
pub fn percent(busy: u64, total: u64) -> Option<f32> {
    (total > 0).then(|| (busy as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_percentages() {
        assert_eq!(percent(25, 100), Some(25.0));
        assert_eq!(percent(0, 0), None);
        assert_eq!(percent(150, 100), Some(100.0));
    }

    #[test]
    fn clamps_the_interval() {
        let o = PerfOptions { app: None, interval_ms: 10 };
        assert_eq!(o.interval().as_millis(), 250);
    }

    #[test]
    fn serializes_in_camel_case() {
        let s = PerfSample { app_cpu: Some(1.5), ..Default::default() };
        let json = serde_json::to_value(s).unwrap();
        assert_eq!(json["appCpu"], 1.5);
        assert!(json["fps"].is_null());
    }
}
