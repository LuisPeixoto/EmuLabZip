//! Parsing `simctl list devices -j`.

use std::collections::BTreeMap;

use emulabzip_core::{Device, DeviceKind, DeviceState, Error, Platform, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DeviceList {
    devices: BTreeMap<String, Vec<SimDevice>>,
}

/// One simulator, as `simctl` describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimDevice {
    pub udid: String,
    pub name: String,
    /// `Booted`, `Shutdown`, `Booting`, `Shutting Down`, `Creating`.
    pub state: String,
    #[serde(default)]
    pub is_available: bool,
    #[serde(default)]
    pub device_type_identifier: Option<String>,
    #[serde(default)]
    pub data_path: Option<String>,
    #[serde(default)]
    pub data_path_size: Option<u64>,
    #[serde(default)]
    pub log_path: Option<String>,
    #[serde(default)]
    pub last_booted_at: Option<String>,
    /// Filled in from the runtime key, for example `iOS 26.2`.
    #[serde(skip)]
    pub runtime: Option<Runtime>,
}

/// The OS a simulator runs, decoded from a runtime identifier such as
/// `com.apple.CoreSimulator.SimRuntime.iOS-26-2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runtime {
    pub os: String,
    pub version: String,
}

impl Runtime {
    pub fn from_identifier(id: &str) -> Option<Self> {
        let tail = id.rsplit('.').next()?;
        let (os, version) = tail.split_once('-')?;
        Some(Runtime { os: os.to_string(), version: version.replace('-', ".") })
    }
}

impl SimDevice {
    /// A readable model name from `com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro`.
    pub fn device_type(&self) -> Option<String> {
        let id = self.device_type_identifier.as_deref()?;
        Some(id.rsplit('.').next()?.replace('-', " "))
    }

    pub fn to_device(&self) -> Device {
        Device {
            id: self.udid.clone(),
            name: self.name.clone(),
            platform: Platform::Ios,
            kind: DeviceKind::Simulator,
            state: match self.state.as_str() {
                "Booted" => DeviceState::Online,
                "Booting" => DeviceState::Booting,
                _ => DeviceState::Offline,
            },
            os_version: self.runtime.as_ref().map(|r| r.version.clone()),
            api_level: None,
            model: self.device_type(),
            abi: Some(std::env::consts::ARCH.replace("aarch64", "arm64")),
        }
    }
}

/// Every available iOS simulator in the JSON list, booted or not.
pub fn parse(json: &[u8]) -> Result<Vec<SimDevice>> {
    let list: DeviceList = serde_json::from_slice(json).map_err(|e| Error::parse("simulator list", e.to_string()))?;
    let mut out = Vec::new();
    for (runtime_id, devices) in list.devices {
        let runtime = Runtime::from_identifier(&runtime_id);
        // watchOS, tvOS and visionOS simulators use different tooling; iPad runs the iOS runtime.
        if runtime.as_ref().is_none_or(|r| r.os != "iOS") {
            continue;
        }
        out.extend(devices.into_iter().filter(|d| d.is_available).map(|mut d| {
            d.runtime = runtime.clone();
            d
        }));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
      "devices": {
        "com.apple.CoreSimulator.SimRuntime.iOS-26-2": [
          {"udid": "F8CE84C1", "name": "iPhone 17", "state": "Booted", "isAvailable": true,
           "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-17",
           "dataPath": "/Users/me/Library/Developer/CoreSimulator/Devices/F8CE84C1/data", "dataPathSize": 19926409216},
          {"udid": "AAAA", "name": "iPad Air", "state": "Shutdown", "isAvailable": true},
          {"udid": "BBBB", "name": "Broken", "state": "Shutdown", "isAvailable": false}
        ],
        "com.apple.CoreSimulator.SimRuntime.watchOS-11-0": [
          {"udid": "CCCC", "name": "Apple Watch", "state": "Booted", "isAvailable": true}
        ]
      }
    }"#;

    #[test]
    fn decodes_runtime_identifiers() {
        let r = Runtime::from_identifier("com.apple.CoreSimulator.SimRuntime.iOS-26-2").unwrap();
        assert_eq!((r.os.as_str(), r.version.as_str()), ("iOS", "26.2"));
        assert_eq!(Runtime::from_identifier("nonsense"), None);
    }

    #[test]
    fn keeps_available_ios_simulators() {
        let devices = parse(JSON.as_bytes()).unwrap();
        assert_eq!(devices.iter().map(|d| d.udid.as_str()).collect::<Vec<_>>(), ["F8CE84C1", "AAAA"]);
    }

    #[test]
    fn converts_to_the_shared_model() {
        let d = parse(JSON.as_bytes()).unwrap().remove(0).to_device();
        assert_eq!(d.state, DeviceState::Online);
        assert_eq!(d.os_label(), "iOS 26.2");
        assert_eq!(d.model.as_deref(), Some("iPhone 17"));
        assert_eq!(d.kind, DeviceKind::Simulator);
    }
}
