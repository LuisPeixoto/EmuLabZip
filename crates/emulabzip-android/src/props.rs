//! System properties, as printed by `getprop`.

use std::collections::BTreeMap;

/// Parsed `getprop` output: `[ro.build.version.sdk]: [35]` per line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Props(BTreeMap<String, String>);

impl Props {
    pub fn parse(text: &str) -> Self {
        let mut map = BTreeMap::new();
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix('[') else { continue };
            let Some((key, value)) = rest.split_once("]: [") else { continue };
            let value = value.trim_end().strip_suffix(']').unwrap_or(value);
            map.insert(key.to_string(), value.to_string());
        }
        Props(map)
    }

    /// The value of a property, or `None` when it is missing or empty.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str).filter(|v| !v.is_empty())
    }

    /// The first non-empty property among `keys`.
    pub fn first(&self, keys: &[&str]) -> Option<&str> {
        keys.iter().find_map(|k| self.get(k))
    }

    pub fn api_level(&self) -> Option<u32> {
        self.get("ro.build.version.sdk").and_then(|v| v.parse().ok())
    }

    pub fn is_emulator(&self) -> bool {
        self.first(&["ro.boot.qemu", "ro.kernel.qemu"]) == Some("1")
            || self.get("ro.hardware").is_some_and(|h| h == "ranchu" || h == "goldfish")
    }

    /// The AVD name for emulators. Recent images expose it as a property.
    pub fn avd_name(&self) -> Option<&str> {
        self.first(&["ro.boot.qemu.avd_name", "ro.kernel.qemu.avd_name"])
    }

    pub fn is_booted(&self) -> bool {
        self.get("sys.boot_completed") == Some("1")
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[ro.boot.qemu]: [1]\n\
        [ro.boot.qemu.avd_name]: [Pixel_9]\n\
        [ro.build.version.sdk]: [35]\n\
        [ro.build.version.release]: [15]\n\
        [ro.product.model]: [sdk_gphone64_arm64]\n\
        [ro.product.cpu.abilist]: [arm64-v8a]\n\
        [persist.sys.timezone]: []\n\
        [sys.boot_completed]: [1]\n\
        not a property line\n";

    #[test]
    fn parses_properties() {
        let p = Props::parse(SAMPLE);
        assert_eq!(p.len(), 8);
        assert_eq!(p.get("ro.build.version.release"), Some("15"));
        assert_eq!(p.api_level(), Some(35));
        assert_eq!(p.avd_name(), Some("Pixel_9"));
        assert!(p.is_emulator());
        assert!(p.is_booted());
    }

    #[test]
    fn treats_empty_values_as_missing() {
        let p = Props::parse(SAMPLE);
        assert_eq!(p.get("persist.sys.timezone"), None);
        assert_eq!(p.first(&["persist.sys.timezone", "ro.product.model"]), Some("sdk_gphone64_arm64"));
    }

    #[test]
    fn keeps_brackets_inside_values() {
        let p = Props::parse("[x]: [a [b] c]\n");
        assert_eq!(p.get("x"), Some("a [b] c"));
    }
}
