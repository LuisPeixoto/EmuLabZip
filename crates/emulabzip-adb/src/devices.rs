//! Parsing the device list returned by `host:devices-l` and `host:track-devices-l`.

/// One line of `adb devices -l`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    pub serial: String,
    /// `device`, `offline`, `unauthorized`, `authorizing`, `recovery`, …
    pub state: String,
    pub product: Option<String>,
    pub model: Option<String>,
    pub device: Option<String>,
    pub transport_id: Option<u32>,
}

impl DeviceEntry {
    pub fn is_online(&self) -> bool {
        self.state == "device"
    }

    /// Emulators always register as `emulator-<console port>`.
    pub fn is_emulator(&self) -> bool {
        self.serial.starts_with("emulator-")
    }
}

/// Parses the long device list. Unknown attributes are ignored.
pub fn parse_devices(text: &str) -> Vec<DeviceEntry> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<DeviceEntry> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
        return None;
    }
    let mut parts = line.split_whitespace();
    let serial = parts.next()?.to_string();
    let rest: Vec<&str> = parts.collect();

    // The state is everything before the first `key:value` attribute. It is usually one word, but
    // `no permissions (…)` spans several.
    let first_attr = rest.iter().position(|p| is_attribute(p)).unwrap_or(rest.len());
    let state = rest[..first_attr].join(" ");
    if state.is_empty() {
        return None;
    }

    let mut entry = DeviceEntry { serial, state, product: None, model: None, device: None, transport_id: None };
    for attr in &rest[first_attr..] {
        let Some((key, value)) = attr.split_once(':') else { continue };
        match key {
            "product" => entry.product = Some(value.to_string()),
            "model" => entry.model = Some(value.replace('_', " ")),
            "device" => entry.device = Some(value.to_string()),
            "transport_id" => entry.transport_id = value.parse().ok(),
            _ => {}
        }
    }
    Some(entry)
}

fn is_attribute(part: &str) -> bool {
    matches!(part.split_once(':'), Some((k, _)) if matches!(k, "usb" | "product" | "model" | "device" | "transport_id"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "List of devices attached\n\
        emulator-5554          device product:sdk_gphone64_arm64 model:sdk_gphone64_arm64 device:emu64a transport_id:1\n\
        R58M12ABCDE            unauthorized usb:1-1 transport_id:3\n\
        0123456789ABCDEF       no permissions (missing udev rules? user is in the plugdev group); see [http://developer.android.com/tools/device.html] usb:1-2 transport_id:4\n\
        emulator-5556          offline transport_id:2\n";

    #[test]
    fn parses_every_state() {
        let devices = parse_devices(LIST);
        assert_eq!(devices.len(), 4);

        let emu = &devices[0];
        assert_eq!(emu.serial, "emulator-5554");
        assert!(emu.is_online());
        assert!(emu.is_emulator());
        assert_eq!(emu.model.as_deref(), Some("sdk gphone64 arm64"));
        assert_eq!(emu.transport_id, Some(1));

        assert_eq!(devices[1].state, "unauthorized");
        assert!(!devices[1].is_emulator());
        assert!(devices[2].state.starts_with("no permissions"));
        assert_eq!(devices[2].transport_id, Some(4));
        assert_eq!(devices[3].state, "offline");
    }

    #[test]
    fn ignores_headers_and_daemon_messages() {
        assert!(
            parse_devices("* daemon not running; starting now at tcp:5037\nList of devices attached\n\n").is_empty()
        );
    }

    #[test]
    fn accepts_the_short_format() {
        let d = parse_devices("emulator-5554\tdevice\n");
        assert_eq!(d[0].state, "device");
        assert_eq!(d[0].model, None);
    }
}
