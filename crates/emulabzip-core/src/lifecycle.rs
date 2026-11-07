//! Creating, starting, stopping and wiping emulators and simulators, and the emulator's own snapshots.

use serde::{Deserialize, Serialize};

/// Text that is safe to hand to `avdmanager`, `emulator` or `simctl` as a name: letters, digits, `.`, `_` and `-`.
pub fn is_device_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A system image (`system-images;android-35;google_apis;x86_64`) or a runtime identifier
/// (`com.apple.CoreSimulator.SimRuntime.iOS-26-2`): letters, digits and `.`, `_`, `-`, `;`, `:`.
pub fn is_image_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 200
        && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ';' | ':'))
}

/// What a device is called to the lifecycle calls: a serial or UDID, or `avd:<name>` for an emulator that is not running.
pub fn is_device_ref(text: &str) -> bool {
    match text.strip_prefix("avd:") {
        Some(name) => is_device_name(name),
        None => {
            !text.is_empty()
                && text.len() <= 100
                && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
        }
    }
}

/// The AVD name of an `avd:<name>` reference.
pub fn avd_name(text: &str) -> Option<&str> {
    text.strip_prefix("avd:").filter(|n| is_device_name(n))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootOptions {
    /// Start from factory state: the data of the device is thrown away.
    pub wipe: bool,
    /// Do not resume from the quick-boot snapshot.
    pub cold: bool,
    /// No window (Android).
    pub headless: bool,
}

/// What a new device is made of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDevice {
    pub name: String,
    /// Android: a system image path. iOS: a runtime identifier.
    pub image: String,
    /// Android: a device definition id (`pixel_6`). iOS: a device type identifier.
    pub model: String,
}

impl CreateDevice {
    pub fn is_valid(&self) -> bool {
        is_device_name(&self.name) && is_image_id(&self.image) && is_image_id(&self.model)
    }
}

/// One thing a new device can be made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: String,
    pub label: String,
    /// A system image is on disk (Android), or a runtime is installed (iOS).
    pub installed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceOptions {
    pub images: Vec<Choice>,
    pub models: Vec<Choice>,
}

/// A snapshot the emulator keeps of its own state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorSnapshot {
    pub name: String,
    /// As the emulator prints it, for example `1.2 GB`.
    pub size: String,
    pub date: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotAction {
    Save,
    /// Throws away the state the emulator is in now.
    Load,
    Delete,
}

impl SnapshotAction {
    pub fn is_destructive(self) -> bool {
        !matches!(self, SnapshotAction::Save)
    }

    pub fn label(self) -> &'static str {
        match self {
            SnapshotAction::Save => "Saving an emulator snapshot",
            SnapshotAction::Load => "Loading an emulator snapshot",
            SnapshotAction::Delete => "Deleting an emulator snapshot",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_plain() {
        assert!(is_device_name("Pixel_9-test.1"));
        assert!(!is_device_name(""));
        assert!(!is_device_name("-rf"));
        assert!(!is_device_name("a b"));
        assert!(!is_device_name("a;b"));
        assert!(!is_device_name(&"x".repeat(65)));
    }

    #[test]
    fn images_may_carry_semicolons_but_nothing_a_shell_would_use() {
        assert!(is_image_id("system-images;android-35;google_apis;x86_64"));
        assert!(is_image_id("com.apple.CoreSimulator.SimRuntime.iOS-26-2"));
        assert!(!is_image_id("a b"));
        assert!(!is_image_id("a$(x)"));
        assert!(!is_image_id("a|b"));
    }

    #[test]
    fn a_device_ref_is_a_serial_a_udid_or_a_named_avd() {
        assert!(is_device_ref("emulator-5554"));
        assert!(is_device_ref("F8CE84C1-04D1-4E33-B52B-02B59E8A09C5"));
        assert!(is_device_ref("avd:Pixel_9"));
        assert!(!is_device_ref("avd:a b"));
        assert!(!is_device_ref("avd:"));
        assert!(!is_device_ref("a;b"));
        assert_eq!(avd_name("avd:Pixel_9"), Some("Pixel_9"));
        assert_eq!(avd_name("emulator-5554"), None);
    }

    #[test]
    fn only_saving_a_snapshot_is_harmless() {
        assert!(!SnapshotAction::Save.is_destructive());
        assert!(SnapshotAction::Load.is_destructive() && SnapshotAction::Delete.is_destructive());
    }
}
