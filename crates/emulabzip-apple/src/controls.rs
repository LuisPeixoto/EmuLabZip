//! Changing what an iOS Simulator reports or does, with `xcrun simctl`: location, appearance,
//! the status bar, privacy permissions, a push notification and a link.

use emulabzip_core::{Appearance, Control, ControlInfo, Error, Grant, Result, StatusBar};

use crate::simctl;

/// The privacy services `simctl privacy` knows.
pub const SERVICES: &[&str] = &[
    "all",
    "calendar",
    "contacts-limited",
    "contacts",
    "location",
    "location-always",
    "photos-add",
    "photos",
    "media-library",
    "microphone",
    "motion",
    "reminders",
    "siri",
];

/// The simctl arguments (after the device) that apply a control. Several when it takes more than
/// one call. A push is not among them: it sends its payload on standard input.
pub fn args_for(control: &Control) -> Result<Vec<Vec<String>>> {
    let v = |parts: &[&str]| parts.iter().map(|p| p.to_string()).collect::<Vec<String>>();
    Ok(match control {
        Control::Location { lat, lon } => vec![v(&["location", "set", &format!("{},{}", lat.trim(), lon.trim())])],
        Control::ClearLocation => vec![v(&["location", "clear"])],
        Control::Appearance { appearance } => {
            vec![v(&["ui", "appearance", if *appearance == Appearance::Dark { "dark" } else { "light" }])]
        }
        Control::StatusBar { status_bar } => vec![status_bar_args(status_bar)?],
        Control::ClearStatusBar => vec![v(&["status_bar", "clear"])],
        Control::Permission { app, permission, grant } => {
            if !SERVICES.contains(&permission.as_str()) {
                return Err(Error::failed(format!(
                    "`{permission}` is not a service of the simulator. Use one of: {}.",
                    SERVICES.join(", ")
                )));
            }
            let action = match grant {
                Grant::Grant => "grant",
                Grant::Revoke => "revoke",
                Grant::Reset => "reset",
            };
            vec![v(&["privacy", action, permission, app])]
        }
        Control::OpenUrl { url } => vec![v(&["openurl", url])],
        Control::Notification { .. } => Vec::new(),
        Control::Battery { .. } | Control::ResetBattery => {
            return Err(Error::Unsupported(
                "Changing the battery of the simulator (use the status bar to show another level)",
            ));
        }
    })
}

fn status_bar_args(sb: &StatusBar) -> Result<Vec<String>> {
    let mut a: Vec<String> = vec!["status_bar".into(), "override".into()];
    let nothing = sb.time.is_none() && sb.battery.is_none() && sb.wifi.is_none() && sb.cellular.is_none();
    let mut add = |flag: &str, value: &str| {
        a.push(flag.to_string());
        a.push(value.to_string());
    };
    if let Some(time) = &sb.time {
        add("--time", time);
    }
    if let Some(level) = sb.battery {
        add("--batteryState", "charged");
        add("--batteryLevel", &level.to_string());
    }
    match sb.wifi {
        Some(true) => {
            add("--wifiMode", "active");
            add("--wifiBars", "3");
        }
        Some(false) => add("--wifiMode", "failed"),
        None => {}
    }
    match sb.cellular {
        Some(true) => {
            add("--cellularMode", "active");
            add("--cellularBars", "4");
            add("--operatorName", "");
        }
        Some(false) => add("--cellularMode", "notSupported"),
        None => {}
    }
    if nothing {
        // simctl wants at least one flag.
        add("--time", "9:41");
    }
    Ok(a)
}

/// The payload of a push: the app's own JSON, or a plain alert with a title and a body.
pub fn push_payload(title: &str, body: &str, payload: Option<&str>) -> String {
    match payload {
        Some(p) => p.to_string(),
        None => {
            serde_json::json!({ "aps": { "alert": { "title": title, "body": body }, "sound": "default" } }).to_string()
        }
    }
}

pub async fn info(udid: &str) -> Result<ControlInfo> {
    let appearance = simctl::run_text(["ui", udid, "appearance"]).await.ok().and_then(|t| match t.trim() {
        "dark" => Some(Appearance::Dark),
        "light" => Some(Appearance::Light),
        _ => None,
    });
    Ok(ControlInfo {
        appearance,
        supported: [
            "location",
            "clearLocation",
            "appearance",
            "statusBar",
            "clearStatusBar",
            "permission",
            "notification",
            "openUrl",
        ]
        .map(String::from)
        .to_vec(),
        permissions: Vec::new(),
        notes: vec![
            format!("iOS does not report which permissions an app has. Services: {}.", SERVICES.join(", ")),
            "The battery of the simulator cannot be changed; the status bar can show any level.".into(),
        ],
    })
}

pub async fn apply(udid: &str, control: &Control) -> Result<()> {
    if let Control::Notification { app, title, body, payload } = control {
        let json = push_payload(title, body, payload.as_deref());
        simctl::run_with_stdin(["push", udid, app, "-"], json.as_bytes()).await?;
        return Ok(());
    }
    for args in args_for(control)? {
        // simctl takes the device right after the verb: `ui <device> appearance dark`.
        let (verb, rest) = args.split_first().expect("a command has a verb");
        simctl::run(std::iter::once(verb.as_str()).chain(std::iter::once(udid)).chain(rest.iter().map(String::as_str)))
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(control: Control) -> Vec<String> {
        let mut all = args_for(&control).unwrap();
        assert_eq!(all.len(), 1);
        all.remove(0)
    }

    #[test]
    fn builds_the_simctl_arguments() {
        assert_eq!(
            one(Control::Location { lat: "-23.55".into(), lon: " -46.63 ".into() }),
            ["location", "set", "-23.55,-46.63"]
        );
        assert_eq!(one(Control::ClearLocation), ["location", "clear"]);
        assert_eq!(one(Control::Appearance { appearance: Appearance::Dark }), ["ui", "appearance", "dark"]);
        assert_eq!(one(Control::ClearStatusBar), ["status_bar", "clear"]);
        assert_eq!(one(Control::OpenUrl { url: "https://example.com".into() }), ["openurl", "https://example.com"]);
        assert_eq!(
            one(Control::Permission {
                app: "com.example.app".into(),
                permission: "photos".into(),
                grant: Grant::Reset
            }),
            ["privacy", "reset", "photos", "com.example.app"]
        );
    }

    #[test]
    fn the_status_bar_sets_only_what_was_asked() {
        let args = one(Control::StatusBar {
            status_bar: StatusBar {
                time: Some("9:41".into()),
                battery: Some(100),
                wifi: Some(true),
                cellular: Some(false),
            },
        });
        assert_eq!(
            args,
            [
                "status_bar",
                "override",
                "--time",
                "9:41",
                "--batteryState",
                "charged",
                "--batteryLevel",
                "100",
                "--wifiMode",
                "active",
                "--wifiBars",
                "3",
                "--cellularMode",
                "notSupported"
            ]
        );
        let bare = one(Control::StatusBar { status_bar: StatusBar::default() });
        assert_eq!(bare, ["status_bar", "override", "--time", "9:41"], "simctl needs at least one flag");
    }

    #[test]
    fn only_services_simctl_knows_are_accepted() {
        let bad = Control::Permission { app: "a.b".into(), permission: "camera".into(), grant: Grant::Grant };
        assert!(args_for(&bad).unwrap_err().to_string().contains("photos"), "the message lists the services");
        assert!(SERVICES.iter().all(|s| {
            args_for(&Control::Permission { app: "a.b".into(), permission: s.to_string(), grant: Grant::Revoke })
                .is_ok()
        }));
    }

    #[test]
    fn the_battery_is_not_something_the_simulator_has() {
        assert!(matches!(args_for(&Control::Battery { level: 5, charging: false }), Err(Error::Unsupported(_))));
        assert!(matches!(args_for(&Control::ResetBattery), Err(Error::Unsupported(_))));
    }

    #[test]
    fn builds_the_push_payload() {
        let p: serde_json::Value = serde_json::from_str(&push_payload("Hi", "there", None)).unwrap();
        assert_eq!(
            (p["aps"]["alert"]["title"].as_str(), p["aps"]["alert"]["body"].as_str()),
            (Some("Hi"), Some("there"))
        );
        assert_eq!(push_payload("x", "y", Some("{\"aps\":{}}")), "{\"aps\":{}}");
        let tricky: serde_json::Value =
            serde_json::from_str(&push_payload("a \"quoted\" title", "line\nbreak", None)).unwrap();
        assert_eq!(tricky["aps"]["alert"]["title"], "a \"quoted\" title");
    }
}
