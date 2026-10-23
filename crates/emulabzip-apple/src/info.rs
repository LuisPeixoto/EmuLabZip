//! Simulator facts for the Overview.

use emulabzip_core::InfoSection;
use serde_json::Value;

use crate::devices::SimDevice;

/// Extra facts gathered with separate commands. Every field is optional: a failing command
/// only removes its row.
#[derive(Debug, Clone, Default)]
pub struct Extras {
    /// `simctl list runtimes -j`.
    pub runtimes: Option<Value>,
    /// `defaults read -g AppleLocale` inside the simulator.
    pub locale: Option<String>,
    /// `defaults read -g AppleLanguages`, an OpenStep array such as `("pt-BR","en-BR")`.
    pub languages: Option<String>,
    /// `sw_vers -productVersion` on the host.
    pub macos: Option<String>,
    /// First line of `xcodebuild -version`.
    pub xcode: Option<String>,
}

/// The build number of the runtime with `version`, for example `23C54` for iOS 26.2.
pub fn runtime_build(runtimes: &Value, version: &str) -> Option<String> {
    runtimes
        .get("runtimes")?
        .as_array()?
        .iter()
        .find(|r| r.get("version").and_then(Value::as_str) == Some(version))?
        .get("buildversion")?
        .as_str()
        .map(str::to_string)
}

/// `("pt-BR","en-BR")` becomes `pt-BR, en-BR`.
pub fn parse_languages(text: &str) -> String {
    text.trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .map(|l| l.trim().trim_matches('"'))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_bytes(bytes: u64) -> String {
    let gb = bytes as f64 / 1_073_741_824.0;
    if gb >= 1.0 { format!("{gb:.1} GB") } else { format!("{:.0} MB", bytes as f64 / 1_048_576.0) }
}

pub fn build_sections(sim: &SimDevice, extras: &Extras) -> Vec<InfoSection> {
    let mut device = InfoSection::new("Device");
    device.push("Name", sim.name.clone());
    device.push_opt("Model", sim.device_type());
    device.push("Type", "Simulator");
    device.push("UDID", sim.udid.clone());
    device.push("State", sim.state.clone());
    device.push_opt("Last booted", sim.last_booted_at.as_deref().map(|t| t.replace('T', " ").replace('Z', " UTC")));

    let mut software = InfoSection::new("Software");
    if let Some(rt) = &sim.runtime {
        software.push(rt.os.clone(), rt.version.clone());
        software.push_opt("Build", extras.runtimes.as_ref().and_then(|r| runtime_build(r, &rt.version)));
    }

    let mut storage = InfoSection::new("Storage");
    storage.push_opt("Data size", sim.data_path_size.map(format_bytes));
    storage.push_opt("Data path", sim.data_path.clone());
    storage.push_opt("Log path", sim.log_path.clone());

    let mut locale = InfoSection::new("Locale");
    locale.push_opt("Region", extras.locale.clone());
    locale.push_opt("Languages", extras.languages.as_deref().map(parse_languages));

    let mut host = InfoSection::new("Host");
    host.push_opt("macOS", extras.macos.clone());
    host.push_opt("Xcode", extras.xcode.as_deref().map(|x| x.trim_start_matches("Xcode ").to_string()));
    host.push("Architecture", std::env::consts::ARCH.replace("aarch64", "arm64"));

    [device, software, storage, locale, host].into_iter().filter(|s| !s.items.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::Runtime;
    use serde_json::json;

    #[test]
    fn finds_the_runtime_build() {
        let r = json!({"runtimes": [{"version": "26.2", "buildversion": "23C54"}, {"version": "26.4", "buildversion": "23E1"}]});
        assert_eq!(runtime_build(&r, "26.2").as_deref(), Some("23C54"));
        assert_eq!(runtime_build(&r, "18.0"), None);
    }

    #[test]
    fn formats_languages_and_sizes() {
        assert_eq!(parse_languages("(\n    \"pt-BR\",\n    \"en-BR\"\n)\n"), "pt-BR, en-BR");
        assert_eq!(format_bytes(19_926_409_216), "18.6 GB");
        assert_eq!(format_bytes(5_242_880), "5 MB");
    }

    #[test]
    fn builds_sections() {
        let sim = SimDevice {
            udid: "F8CE84C1".into(),
            name: "iPhone 17".into(),
            state: "Booted".into(),
            is_available: true,
            device_type_identifier: Some("com.apple.CoreSimulator.SimDeviceType.iPhone-17".into()),
            data_path: Some("/tmp/data".into()),
            data_path_size: Some(1 << 30),
            log_path: None,
            last_booted_at: Some("2026-10-23T10:45:08Z".into()),
            runtime: Some(Runtime { os: "iOS".into(), version: "26.2".into() }),
        };
        let sections = build_sections(&sim, &Extras { locale: Some("pt_BR".into()), ..Default::default() });
        let titles: Vec<_> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Device", "Software", "Storage", "Locale", "Host"]);
        assert!(sections[0].items.contains(&("Last booted".into(), "2026-10-23 10:45:08 UTC".into())));
        assert!(sections[1].items.contains(&("iOS".into(), "26.2".into())));
    }
}
