//! Device facts for the Overview: build, hardware, display, battery and locale.

use emulabzip_core::InfoSection;

use crate::props::Props;

/// Separates the outputs of the commands batched into one shell call.
pub const SEPARATOR: &str = "__EMULABZIP_SEPARATOR__";

/// One shell call that gathers everything [`build_sections`] needs. Batching keeps the Overview
/// fast on slow emulators.
pub fn command() -> String {
    let parts = [
        "getprop",
        "wm size",
        "wm density",
        "dumpsys battery",
        "cat /proc/meminfo",
        "uname -r",
        "cat /sys/devices/system/cpu/possible",
        "df -k /data",
    ];
    parts.join(&format!(" 2>&1; echo {SEPARATOR}; "))
}

/// The raw outputs of [`command`], split per command.
#[derive(Debug, Default)]
pub struct RawInfo<'a> {
    pub props: Props,
    pub wm_size: &'a str,
    pub wm_density: &'a str,
    pub battery: &'a str,
    pub meminfo: &'a str,
    pub kernel: &'a str,
    pub cpus: &'a str,
    pub df: &'a str,
}

impl<'a> RawInfo<'a> {
    pub fn split(output: &'a str) -> Self {
        let mut parts = output.split(SEPARATOR).map(str::trim);
        let mut next = || parts.next().unwrap_or("");
        RawInfo {
            props: Props::parse(next()),
            wm_size: next(),
            wm_density: next(),
            battery: next(),
            meminfo: next(),
            kernel: next(),
            cpus: next(),
            df: next(),
        }
    }
}

/// `key: value` lines, as printed by `dumpsys battery`, `wm size` and `/proc/meminfo`.
pub fn key_values(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim(), v.trim()))
}

fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    key_values(text).find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

/// Battery facts from `dumpsys battery`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Battery {
    pub level: Option<u8>,
    pub status: Option<&'static str>,
    pub health: Option<&'static str>,
    pub plugged: Option<&'static str>,
    /// Tenths of a degree Celsius, as reported by the platform.
    pub temperature: Option<i32>,
    pub voltage_mv: Option<u32>,
}

impl Battery {
    pub fn parse(text: &str) -> Self {
        let num = |k| value(text, k).and_then(|v| v.parse::<i64>().ok());
        let plugged = if value(text, "AC powered") == Some("true") {
            Some("AC")
        } else if value(text, "USB powered") == Some("true") {
            Some("USB")
        } else if value(text, "Wireless powered") == Some("true") {
            Some("Wireless")
        } else if value(text, "level").is_some() {
            Some("Unplugged")
        } else {
            None
        };
        Battery {
            level: num("level").and_then(|v| u8::try_from(v).ok()),
            // BatteryManager.BATTERY_STATUS_* constants.
            status: num("status").and_then(|s| match s {
                2 => Some("Charging"),
                3 => Some("Discharging"),
                4 => Some("Not charging"),
                5 => Some("Full"),
                1 => Some("Unknown"),
                _ => None,
            }),
            // BatteryManager.BATTERY_HEALTH_* constants.
            health: num("health").and_then(|h| match h {
                2 => Some("Good"),
                3 => Some("Overheat"),
                4 => Some("Dead"),
                5 => Some("Over voltage"),
                6 => Some("Failure"),
                7 => Some("Cold"),
                1 => Some("Unknown"),
                _ => None,
            }),
            plugged,
            temperature: num("temperature").and_then(|t| i32::try_from(t).ok()),
            voltage_mv: num("voltage").and_then(|v| u32::try_from(v).ok()),
        }
    }
}

/// `Physical size: 1080x2424` (and `Override size:` when set). The override wins.
pub fn screen_size(text: &str) -> Option<&str> {
    value(text, "Override size").or_else(|| value(text, "Physical size"))
}

pub fn screen_density(text: &str) -> Option<&str> {
    value(text, "Override density").or_else(|| value(text, "Physical density"))
}

/// `MemTotal:        2014312 kB` from `/proc/meminfo`, in kilobytes.
pub fn mem_total_kb(meminfo: &str) -> Option<u64> {
    value(meminfo, "MemTotal")?.split_whitespace().next()?.parse().ok()
}

/// Core count from `/sys/devices/system/cpu/possible`, which looks like `0-7` or `0,2-3`.
pub fn cpu_count(possible: &str) -> Option<u32> {
    let mut total = 0;
    for range in possible.trim().split(',').filter(|r| !r.is_empty()) {
        total += match range.split_once('-') {
            Some((a, b)) => b.trim().parse::<u32>().ok()?.checked_sub(a.trim().parse().ok()?)? + 1,
            None => {
                range.trim().parse::<u32>().ok()?;
                1
            }
        };
    }
    (total > 0).then_some(total)
}

/// Size and free space of `/data` from `df -k`, in kilobytes.
pub fn data_partition(df: &str) -> Option<(u64, u64)> {
    // Newer images report the bind mount (`/data/user/0`) instead of `/data`.
    let line = df.lines().skip(1).find(|l| l.split_whitespace().last().is_some_and(|m| m.starts_with("/data")))?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    // Filesystem 1K-blocks Used Available Use% Mounted-on
    Some((cols.get(1)?.parse().ok()?, cols.get(3)?.parse().ok()?))
}

pub fn format_kb(kb: u64) -> String {
    let mb = kb as f64 / 1024.0;
    if mb >= 1024.0 { format!("{:.1} GB", mb / 1024.0) } else { format!("{mb:.0} MB") }
}

/// Builds the Overview cards.
pub fn build_sections(serial: &str, raw: &RawInfo<'_>) -> Vec<InfoSection> {
    let p = &raw.props;

    let mut device = InfoSection::new("Device");
    device.push_opt("Name", p.avd_name().map(|n| n.replace('_', " ")));
    device.push_opt("Model", p.get("ro.product.model"));
    device.push_opt("Manufacturer", p.first(&["ro.product.manufacturer", "ro.product.brand"]));
    device.push("Type", if p.is_emulator() { "Emulator" } else { "Physical device" });
    device.push("Serial", serial);

    let mut software = InfoSection::new("Software");
    software.push_opt("Android", p.get("ro.build.version.release"));
    software.push_opt("API level", p.get("ro.build.version.sdk"));
    software.push_opt("Security patch", p.get("ro.build.version.security_patch"));
    software.push_opt("Build", p.get("ro.build.display.id"));
    software.push_opt("Build type", p.get("ro.build.type"));
    software.push("Kernel", raw.kernel.lines().next().unwrap_or(""));
    software.push_opt("Fingerprint", p.get("ro.build.fingerprint"));

    let mut hardware = InfoSection::new("Hardware");
    hardware.push_opt("ABI", p.get("ro.product.cpu.abi"));
    hardware.push_opt("ABI list", p.get("ro.product.cpu.abilist"));
    hardware.push_opt("CPU cores", cpu_count(raw.cpus).map(|n| n.to_string()));
    hardware.push_opt("Memory", mem_total_kb(raw.meminfo).map(format_kb));
    if let Some((size, free)) = data_partition(raw.df) {
        hardware.push("Storage", format!("{} free of {}", format_kb(free), format_kb(size)));
    }
    hardware.push_opt("Hardware", p.get("ro.hardware"));

    let mut display = InfoSection::new("Display");
    display.push_opt("Resolution", screen_size(raw.wm_size).map(|s| s.replace('x', " × ")));
    display.push_opt("Density", screen_density(raw.wm_density).map(|d| format!("{d} dpi")));
    display.push_opt("Refresh rate", p.get("ro.boot.qemu.vsync").map(|v| format!("{v} Hz")));

    let mut battery = InfoSection::new("Battery");
    let b = Battery::parse(raw.battery);
    battery.push_opt("Level", b.level.map(|l| format!("{l}%")));
    battery.push_opt("Status", b.status);
    battery.push_opt("Power", b.plugged);
    battery.push_opt("Health", b.health);
    battery.push_opt("Temperature", b.temperature.map(|t| format!("{:.1} °C", t as f64 / 10.0)));
    battery.push_opt("Voltage", b.voltage_mv.map(|v| format!("{v} mV")));

    let mut locale = InfoSection::new("Locale");
    locale.push_opt("Language", p.first(&["persist.sys.locale", "ro.product.locale"]));
    locale.push_opt("Time zone", p.first(&["persist.sys.timezone", "persist.sys.time_zone"]));

    [device, software, hardware, display, battery, locale].into_iter().filter(|s| !s.items.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BATTERY: &str = "Current Battery Service state:\n  AC powered: true\n  USB powered: false\n  \
        Wireless powered: false\n  status: 2\n  health: 2\n  present: true\n  level: 87\n  scale: 100\n  \
        voltage: 5000\n  temperature: 250\n  technology: Li-ion\n";

    #[test]
    fn parses_battery_state() {
        let b = Battery::parse(BATTERY);
        assert_eq!(b.level, Some(87));
        assert_eq!(b.status, Some("Charging"));
        assert_eq!(b.health, Some("Good"));
        assert_eq!(b.plugged, Some("AC"));
        assert_eq!(b.temperature, Some(250));
        assert_eq!(b.voltage_mv, Some(5000));
    }

    #[test]
    fn prefers_override_screen_values() {
        assert_eq!(screen_size("Physical size: 1080x2424\n"), Some("1080x2424"));
        assert_eq!(screen_size("Physical size: 1080x2424\nOverride size: 720x1616\n"), Some("720x1616"));
        assert_eq!(screen_density("Physical density: 420\n"), Some("420"));
    }

    #[test]
    fn counts_cpus() {
        assert_eq!(cpu_count("0-7\n"), Some(8));
        assert_eq!(cpu_count("0,2-3"), Some(3));
        assert_eq!(cpu_count("0"), Some(1));
        assert_eq!(cpu_count(""), None);
    }

    #[test]
    fn reads_memory_and_storage() {
        assert_eq!(mem_total_kb("MemTotal:        2014312 kB\nMemFree: 1 kB"), Some(2014312));
        let df = "Filesystem     1K-blocks    Used Available Use% Mounted on\n/dev/block/dm-5   6082144 2954728   3110632  49% /data\n";
        assert_eq!(data_partition(df), Some((6082144, 3110632)));
        let bind = "Filesystem 1K-blocks Used Available Use% Mounted on\n/dev/block/dm-55 8107484 4353632 3606396 55% /data/user/0\n";
        assert_eq!(data_partition(bind), Some((8107484, 3606396)));
        assert_eq!(format_kb(2014312), "1.9 GB");
        assert_eq!(format_kb(512_000), "500 MB");
    }

    #[test]
    fn builds_sections_from_batched_output() {
        let out = format!(
            "[ro.boot.qemu]: [1]\n[ro.boot.qemu.avd_name]: [Pixel_9]\n[ro.build.version.release]: [15]\n\
             {SEPARATOR}\nPhysical size: 1080x2424\n{SEPARATOR}\nPhysical density: 420\n{SEPARATOR}\n{BATTERY}\
             {SEPARATOR}\nMemTotal: 2014312 kB\n{SEPARATOR}\n6.6.30\n{SEPARATOR}\n0-3\n{SEPARATOR}\n"
        );
        let sections = build_sections("emulator-5554", &RawInfo::split(&out));
        let titles: Vec<_> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Device", "Software", "Hardware", "Display", "Battery"]);
        assert!(sections[0].items.contains(&("Name".into(), "Pixel 9".into())));
        assert!(sections[2].items.contains(&("CPU cores".into(), "4".into())));
        assert!(sections[3].items.contains(&("Resolution".into(), "1080 × 2424".into())));
    }
}
