//! How much battery an app used over a stretch of time.

use serde::{Deserialize, Serialize};

/// One part of the power an app drew: the processor, the radio, the screen...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyComponent {
    pub name: String,
    pub mah: f64,
}

/// A wake lock: the app kept the device awake for a while.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wakelock {
    pub tag: String,
    pub ms: u64,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergyReport {
    pub app: String,
    /// How long the measurement ran.
    pub seconds: u64,
    /// The system's estimate of the app's use, in milliampere-hours. `None` when the platform has no model.
    pub estimated_mah: Option<f64>,
    /// Largest first.
    pub components: Vec<EnergyComponent>,
    pub cpu_ms: u64,
    pub wakelocks: Vec<Wakelock>,
    pub alarms: u64,
    pub jobs: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    /// What to keep in mind when reading the numbers.
    pub notes: Vec<String>,
}

/// The longest a measurement may run, in seconds.
pub const MAX_SECONDS: u64 = 1800;

impl EnergyReport {
    /// Sorts components, largest first, and drops the ones that drew nothing.
    pub fn tidy(mut self) -> Self {
        self.components.retain(|c| c.mah > 0.0);
        self.components.sort_by(|a, b| b.mah.total_cmp(&a.mah).then_with(|| a.name.cmp(&b.name)));
        self.wakelocks.sort_by(|a, b| b.ms.cmp(&a.ms).then_with(|| a.tag.cmp(&b.tag)));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components_are_sorted_and_the_empty_ones_dropped() {
        let r = EnergyReport {
            app: "a".into(),
            seconds: 30,
            estimated_mah: Some(1.2),
            components: vec![
                EnergyComponent { name: "wifi".into(), mah: 0.1 },
                EnergyComponent { name: "cpu".into(), mah: 0.9 },
                EnergyComponent { name: "gps".into(), mah: 0.0 },
            ],
            cpu_ms: 0,
            wakelocks: vec![
                Wakelock { tag: "b".into(), ms: 5, count: 1 },
                Wakelock { tag: "a".into(), ms: 50, count: 2 },
            ],
            alarms: 0,
            jobs: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            notes: vec![],
        }
        .tidy();
        assert_eq!(r.components.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["cpu", "wifi"]);
        assert_eq!(r.wakelocks[0].tag, "a");
    }
}
