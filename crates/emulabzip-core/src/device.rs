//! Devices, apps and descriptive information shared by every backend.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The operating system family a device runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Android,
    Ios,
}

impl Platform {
    pub fn label(self) -> &'static str {
        match self {
            Platform::Android => "Android",
            Platform::Ios => "iOS",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Whether the target is virtual (emulator / simulator) or real hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Emulator,
    Simulator,
    Physical,
}

/// Connection state as reported by `adb` or `simctl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceState {
    /// Ready to be inspected.
    Online,
    /// Attached but still booting.
    Booting,
    /// Known to the host but not reachable (adb `offline`).
    Offline,
    /// Waiting for the user to accept the USB debugging prompt.
    Unauthorized,
}

impl DeviceState {
    pub fn is_ready(self) -> bool {
        self == DeviceState::Online
    }
}

/// A device that EmuLabZip can inspect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// adb serial (`emulator-5554`) or simulator UDID.
    pub id: String,
    /// Human friendly name: the AVD name, the simulator name or the device model.
    pub name: String,
    pub platform: Platform,
    pub kind: DeviceKind,
    pub state: DeviceState,
    /// Marketing OS version, for example `15` or `26.2`.
    pub os_version: Option<String>,
    /// Android API level. Always `None` on iOS.
    pub api_level: Option<u32>,
    pub model: Option<String>,
    /// CPU architecture, for example `arm64-v8a` or `arm64`.
    pub abi: Option<String>,
}

impl Device {
    pub fn target(&self) -> Target {
        Target { platform: self.platform, id: self.id.clone() }
    }

    /// One line description, for example `Android 15 (API 35)`.
    pub fn os_label(&self) -> String {
        match (&self.os_version, self.api_level) {
            (Some(v), Some(api)) => format!("{} {v} (API {api})", self.platform),
            (Some(v), None) => format!("{} {v}", self.platform),
            (None, Some(api)) => format!("{} API {api}", self.platform),
            (None, None) => self.platform.to_string(),
        }
    }
}

/// Addresses one device on one platform. This is what the UI and the CLI pass around.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub platform: Platform,
    pub id: String,
}

/// An installed application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    /// Android package name or iOS bundle identifier.
    pub id: String,
    /// Display name when the platform reports one, otherwise the id.
    pub name: String,
    pub version: Option<String>,
    /// Process id when the app is running.
    pub pid: Option<u32>,
    /// `true` for apps installed by the user, `false` for system apps.
    pub user: bool,
}

/// A titled group of key / value facts, rendered as a card in the Overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoSection {
    pub title: String,
    pub items: Vec<(String, String)>,
}

impl InfoSection {
    pub fn new(title: impl Into<String>) -> Self {
        InfoSection { title: title.into(), items: Vec::new() }
    }

    /// Adds an item, skipping empty values so cards never show blank rows.
    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let value = value.into();
        if !value.trim().is_empty() {
            self.items.push((key.into(), value));
        }
    }

    pub fn push_opt(&mut self, key: impl Into<String>, value: Option<impl Into<String>>) {
        if let Some(value) = value {
            self.push(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> Device {
        Device {
            id: "emulator-5554".into(),
            name: "Pixel_9".into(),
            platform: Platform::Android,
            kind: DeviceKind::Emulator,
            state: DeviceState::Online,
            os_version: Some("15".into()),
            api_level: Some(35),
            model: None,
            abi: None,
        }
    }

    #[test]
    fn os_label_combines_version_and_api() {
        assert_eq!(device().os_label(), "Android 15 (API 35)");
    }

    #[test]
    fn serializes_in_camel_case() {
        let json = serde_json::to_value(device()).unwrap();
        assert_eq!(json["osVersion"], "15");
        assert_eq!(json["platform"], "android");
        assert_eq!(json["state"], "online");
    }

    #[test]
    fn info_section_skips_blank_values() {
        let mut s = InfoSection::new("Build");
        s.push("Brand", "google");
        s.push("Empty", "  ");
        s.push_opt("Missing", None::<String>);
        assert_eq!(s.items, vec![("Brand".to_string(), "google".to_string())]);
    }
}
