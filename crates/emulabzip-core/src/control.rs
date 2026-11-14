//! Changing what a device reports or does, so an app can be tried in other conditions: another
//! place, dark mode, a clean status bar, a permission taken away.

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Grant {
    Grant,
    Revoke,
    /// Back to "ask again" (iOS), or revoked (Android, which has no such state).
    Reset,
}

/// What to show in the status bar. Anything left out is not overridden.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBar {
    /// `9:41`.
    pub time: Option<String>,
    /// 0 to 100.
    pub battery: Option<u8>,
    pub wifi: Option<bool>,
    pub cellular: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Control {
    /// Latitude and longitude in degrees, as text so they survive JSON and the command line
    /// without rounding.
    Location {
        lat: String,
        lon: String,
    },
    ClearLocation,
    Appearance {
        appearance: Appearance,
    },
    StatusBar {
        status_bar: StatusBar,
    },
    ClearStatusBar,
    #[serde(rename_all = "camelCase")]
    Permission {
        app: String,
        permission: String,
        grant: Grant,
    },
    /// A local notification (Android) or a simulated push (iOS, with `payload` as the JSON).
    Notification {
        app: String,
        title: String,
        body: String,
        payload: Option<String>,
    },
    OpenUrl {
        url: String,
    },
    Battery {
        level: u8,
        charging: bool,
    },
    ResetBattery,
    /// The language and region an app is shown in, as a BCP 47 tag such as `de-DE` or the pseudo-locales `en-XA` and
    /// `ar-XB`. Android changes it for one app (`app` is required); the iOS Simulator changes it for the whole device,
    /// and apps pick it up the next time they start.
    Locale {
        locale: String,
        app: Option<String>,
    },
    /// Back to the device's own language.
    ResetLocale {
        app: Option<String>,
    },
    /// The text size as a percentage of the normal one: 100 is normal, 130 and 200 are common accessibility settings.
    FontScale {
        percent: u32,
    },
    /// The screen size in pixels and the density in dpi (Android). Give both sides of the size, or neither.
    Display {
        width: Option<u32>,
        height: Option<u32>,
        density: Option<u32>,
    },
    ResetDisplay,
    /// A touch at a point of the screen, in pixels. Android only: iOS has no public way to send touches.
    Tap {
        x: u32,
        y: u32,
    },
    /// A finger dragged from one point to another, in pixels, taking `duration_ms` (300 by default). A swipe from a
    /// point to itself is a long press.
    #[serde(rename_all = "camelCase")]
    Swipe {
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        duration_ms: Option<u32>,
    },
    /// Typed text. Android can only type printable ASCII this way.
    Text {
        text: String,
    },
    /// A key by name: `back`, `home`, `enter`… see [`KEYS`].
    Key {
        key: String,
    },
}

/// What a device supports, and its state right now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlInfo {
    pub appearance: Option<Appearance>,
    /// The controls that make sense here, by the `action` name.
    pub supported: Vec<String>,
    /// Permissions of the app that was asked about, with whether each is granted.
    pub permissions: Vec<PermissionState>,
    /// Why something is missing: an emulator-only control on a physical device, for example.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionState {
    pub name: String,
    pub granted: bool,
}

/// The keys that can be pressed, by the name people use and Android's key code.
pub const KEYS: &[(&str, &str)] = &[
    ("back", "KEYCODE_BACK"),
    ("home", "KEYCODE_HOME"),
    ("app_switch", "KEYCODE_APP_SWITCH"),
    ("menu", "KEYCODE_MENU"),
    ("search", "KEYCODE_SEARCH"),
    ("enter", "KEYCODE_ENTER"),
    ("tab", "KEYCODE_TAB"),
    ("escape", "KEYCODE_ESCAPE"),
    ("delete", "KEYCODE_DEL"),
    ("forward_delete", "KEYCODE_FORWARD_DEL"),
    ("space", "KEYCODE_SPACE"),
    ("volume_up", "KEYCODE_VOLUME_UP"),
    ("volume_down", "KEYCODE_VOLUME_DOWN"),
    ("mute", "KEYCODE_VOLUME_MUTE"),
    ("power", "KEYCODE_POWER"),
    ("wakeup", "KEYCODE_WAKEUP"),
    ("sleep", "KEYCODE_SLEEP"),
    ("dpad_up", "KEYCODE_DPAD_UP"),
    ("dpad_down", "KEYCODE_DPAD_DOWN"),
    ("dpad_left", "KEYCODE_DPAD_LEFT"),
    ("dpad_right", "KEYCODE_DPAD_RIGHT"),
    ("dpad_center", "KEYCODE_DPAD_CENTER"),
    ("page_up", "KEYCODE_PAGE_UP"),
    ("page_down", "KEYCODE_PAGE_DOWN"),
    ("move_home", "KEYCODE_MOVE_HOME"),
    ("move_end", "KEYCODE_MOVE_END"),
];

/// Android's key code for a key name.
pub fn key_code(name: &str) -> Option<&'static str> {
    KEYS.iter().find(|(n, _)| *n == name).map(|(_, code)| *code)
}

/// The largest coordinate, text length and swipe time that are sent to a device.
pub const MAX_COORDINATE: u32 = 20_000;
pub const MAX_TEXT: usize = 1000;
pub const MAX_SWIPE_MS: u32 = 10_000;
/// How long a swipe takes when it does not say.
pub const DEFAULT_SWIPE_MS: u32 = 300;

/// Schemes that read local data or run code, which a "open this link" control must not allow.
const BLOCKED_SCHEMES: &[&str] = &["file", "javascript", "data", "content", "intent", "vbscript", "about"];

fn coordinate(what: &'static str, text: &str, limit: f64) -> Result<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && v.abs() <= limit)
        .ok_or_else(|| Error::invalid(what, text))
}

/// A package name or bundle id.
pub fn is_app_id(text: &str) -> bool {
    is_name(text, &['-'])
}

fn is_name(text: &str, extra: &[char]) -> bool {
    !text.is_empty()
        && text.len() <= 200
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || extra.contains(&c))
}

/// A BCP 47 language tag, short and made only of letters, digits and hyphens: `de`, `pt-BR`, `en-XA`, `zh-Hans-CN`.
pub fn is_locale_tag(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    (2..=24).contains(&text.len())
        && parts.len() <= 4
        && parts.iter().all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
        && parts[0].len() >= 2
        && parts[0].chars().all(|c| c.is_ascii_alphabetic())
}

impl Control {
    /// Rejects values a device should never be sent. Called by [`crate::Lab`] before a backend
    /// sees the control, so every front end (UI, CLI, MCP) gets the same checks.
    pub fn validate(&self) -> Result<()> {
        match self {
            Control::Location { lat, lon } => {
                coordinate("latitude", lat, 90.0)?;
                coordinate("longitude", lon, 180.0)?;
            }
            Control::StatusBar { status_bar } => {
                if let Some(t) = &status_bar.time
                    && !(t.len() <= 5 && t.contains(':') && t.chars().all(|c| c.is_ascii_digit() || c == ':'))
                {
                    return Err(Error::invalid("time", t.clone()));
                }
                if status_bar.battery.is_some_and(|b| b > 100) {
                    return Err(Error::invalid("battery level", status_bar.battery.unwrap_or(0).to_string()));
                }
            }
            Control::Permission { app, permission, .. } => {
                if !is_name(app, &[]) {
                    return Err(Error::invalid("app", app.clone()));
                }
                if !is_name(permission, &['-']) {
                    return Err(Error::invalid("permission", permission.clone()));
                }
            }
            Control::Notification { app, title, body, payload } => {
                if !is_name(app, &[]) {
                    return Err(Error::invalid("app", app.clone()));
                }
                if title.len() > 200 || body.len() > 2000 {
                    return Err(Error::invalid("notification", "the text is too long"));
                }
                if let Some(p) = payload
                    && (p.len() > 4096 || serde_json::from_str::<serde_json::Value>(p).is_err())
                {
                    return Err(Error::invalid("payload", "it must be JSON of at most 4 KB"));
                }
            }
            Control::OpenUrl { url } => {
                let scheme = url.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
                let ok_scheme = scheme.as_deref().is_some_and(|s| {
                    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                        && !BLOCKED_SCHEMES.contains(&s)
                });
                if !ok_scheme || url.len() > 2000 || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
                    return Err(Error::invalid("url", url.clone()));
                }
            }
            Control::Battery { level, .. } if *level > 100 => {
                return Err(Error::invalid("battery level", level.to_string()));
            }
            Control::Locale { locale, app } => {
                if !is_locale_tag(locale) {
                    return Err(Error::invalid("locale (a tag such as de-DE or ar-XB)", locale.clone()));
                }
                if let Some(app) = app
                    && !is_name(app, &[])
                {
                    return Err(Error::invalid("app", app.clone()));
                }
            }
            Control::ResetLocale { app: Some(app) } if !is_name(app, &[]) => {
                return Err(Error::invalid("app", app.clone()));
            }
            Control::FontScale { percent } if !(50..=300).contains(percent) => {
                return Err(Error::invalid("text size (50 to 300 percent)", percent.to_string()));
            }
            Control::Display { width, height, density } => {
                if width.is_some() != height.is_some() {
                    return Err(Error::invalid("screen size", "give both the width and the height"));
                }
                if width.is_none() && density.is_none() {
                    return Err(Error::invalid("display", "give a size, a density, or both"));
                }
                for (what, v) in [("width", *width), ("height", *height)] {
                    if v.is_some_and(|v| !(200..=10_000).contains(&v)) {
                        return Err(Error::invalid(
                            "screen size in pixels (200 to 10000)",
                            format!("{what} {}", v.unwrap_or(0)),
                        ));
                    }
                }
                if density.is_some_and(|d| !(72..=1200).contains(&d)) {
                    return Err(Error::invalid("density (72 to 1200 dpi)", density.unwrap_or(0).to_string()));
                }
            }
            Control::Tap { x, y } => {
                for (what, v) in [("x", *x), ("y", *y)] {
                    if v > MAX_COORDINATE {
                        return Err(Error::invalid("a screen position in pixels", format!("{what} {v}")));
                    }
                }
            }
            Control::Swipe { x1, y1, x2, y2, duration_ms } => {
                for (what, v) in [("x1", *x1), ("y1", *y1), ("x2", *x2), ("y2", *y2)] {
                    if v > MAX_COORDINATE {
                        return Err(Error::invalid("a screen position in pixels", format!("{what} {v}")));
                    }
                }
                if duration_ms.is_some_and(|d| d == 0 || d > MAX_SWIPE_MS) {
                    return Err(Error::invalid(
                        "swipe time in milliseconds (1 to 10000)",
                        duration_ms.unwrap_or(0).to_string(),
                    ));
                }
            }
            Control::Text { text } => {
                if text.is_empty() || text.chars().count() > MAX_TEXT {
                    return Err(Error::invalid("text (1 to 1000 characters)", format!("{} characters", text.len())));
                }
                // `input text` can only type what a keyboard of the US layout has, and reads `%s` as a space.
                if let Some(c) = text.chars().find(|c| !(' '..='~').contains(c)) {
                    return Err(Error::invalid("text (only printable ASCII can be typed this way)", format!("{c:?}")));
                }
                if text.contains("%s") {
                    return Err(Error::invalid("text", "it cannot contain `%s`, which Android reads as a space"));
                }
            }
            Control::Key { key } if key_code(key).is_none() => {
                let names: Vec<&str> = KEYS.iter().map(|(n, _)| *n).collect();
                return Err(Error::invalid("key", format!("{key} (use one of: {})", names.join(", "))));
            }
            _ => {}
        }
        Ok(())
    }

    /// The coordinates of a [`Control::Location`] as numbers.
    pub fn coordinates(&self) -> Option<(f64, f64)> {
        match self {
            Control::Location { lat, lon } => Some((lat.trim().parse().ok()?, lon.trim().parse().ok()?)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(lat: &str, lon: &str) -> Control {
        Control::Location { lat: lat.into(), lon: lon.into() }
    }

    #[test]
    fn checks_coordinates() {
        assert!(loc("-23.5505", "-46.6333").validate().is_ok());
        assert_eq!(loc("-23.5505", "-46.6333").coordinates(), Some((-23.5505, -46.6333)));
        for (lat, lon) in [("91", "0"), ("0", "181"), ("x", "0"), ("NaN", "0"), ("inf", "0"), ("", "")] {
            assert!(loc(lat, lon).validate().is_err(), "{lat} {lon}");
        }
    }

    #[test]
    fn touches_keys_and_text_are_checked() {
        let ok = |c: Control| c.validate().is_ok();
        assert!(ok(Control::Tap { x: 540, y: 1200 }));
        assert!(!ok(Control::Tap { x: MAX_COORDINATE + 1, y: 0 }));
        assert!(ok(Control::Swipe { x1: 1, y1: 2, x2: 3, y2: 4, duration_ms: None }));
        assert!(ok(Control::Swipe { x1: 1, y1: 2, x2: 1, y2: 2, duration_ms: Some(MAX_SWIPE_MS) }));
        assert!(!ok(Control::Swipe { x1: 1, y1: 2, x2: 3, y2: 4, duration_ms: Some(0) }));
        assert!(!ok(Control::Swipe { x1: 1, y1: 2, x2: 3, y2: 4, duration_ms: Some(MAX_SWIPE_MS + 1) }));
        assert!(!ok(Control::Swipe { x1: 0, y1: 0, x2: 0, y2: MAX_COORDINATE + 1, duration_ms: None }));
        assert!(ok(Control::Text { text: "Hello, World! 100% & more".into() }));
        for bad in ["", "caf\u{e9}", "tab\there", "line\nbreak", "50%send", "\u{1b}[0m"] {
            assert!(!ok(Control::Text { text: bad.into() }), "{bad:?}");
        }
        assert!(!ok(Control::Text { text: "x".repeat(MAX_TEXT + 1) }));
        assert!(ok(Control::Text { text: "x".repeat(MAX_TEXT) }));
        assert!(ok(Control::Key { key: "back".into() }));
        assert!(!ok(Control::Key { key: "KEYCODE_BACK; reboot".into() }));
        assert_eq!(key_code("enter"), Some("KEYCODE_ENTER"));
        assert_eq!(key_code("nope"), None);
    }

    #[test]
    fn input_controls_are_named_like_the_others_in_json() {
        let json = serde_json::to_value(Control::Swipe { x1: 1, y1: 2, x2: 3, y2: 4, duration_ms: Some(500) }).unwrap();
        assert_eq!(json["action"], "swipe");
        assert_eq!(json["durationMs"], 500);
        let back: Control = serde_json::from_value(serde_json::json!({ "action": "tap", "x": 5, "y": 6 })).unwrap();
        assert_eq!(back, Control::Tap { x: 5, y: 6 });
    }

    #[test]
    fn only_safe_links_open() {
        let url = |u: &str| Control::OpenUrl { url: u.into() }.validate();
        assert!(url("https://example.com/a?b=1").is_ok());
        assert!(url("myapp://open/item/3").is_ok());
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,x",
            "intent://x",
            "no scheme",
            "https://a b",
            "https://x\n",
            "1http://x",
            "",
        ] {
            assert!(url(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn checks_permissions_and_payloads() {
        let p = |app: &str, perm: &str| {
            Control::Permission { app: app.into(), permission: perm.into(), grant: Grant::Grant }.validate()
        };
        assert!(p("com.example.app", "android.permission.CAMERA").is_ok());
        assert!(p("photos", "photos").is_ok());
        assert!(p("com.example; rm -rf /", "x").is_err());
        assert!(p("com.example", "a b").is_err());
        let n = |payload: Option<&str>| {
            Control::Notification {
                app: "a.b".into(),
                title: "t".into(),
                body: "b".into(),
                payload: payload.map(String::from),
            }
            .validate()
        };
        assert!(n(None).is_ok() && n(Some("{\"aps\":{\"alert\":\"hi\"}}")).is_ok());
        assert!(n(Some("not json")).is_err());
    }

    #[test]
    fn checks_locale_text_size_and_display() {
        let locale =
            |l: &str, app: Option<&str>| Control::Locale { locale: l.into(), app: app.map(String::from) }.validate();
        for ok in ["de", "pt-BR", "en-XA", "ar-XB", "zh-Hans-CN"] {
            assert!(locale(ok, Some("com.example")).is_ok(), "{ok}");
        }
        for bad in ["", "d", "de_DE", "de DE", "de;reboot", "1e", "a-b-c-d-e", "toolonglanguagetagthatgoeson"] {
            assert!(locale(bad, None).is_err(), "{bad:?}");
        }
        assert!(locale("de", Some("com.example; rm")).is_err());
        assert!(Control::ResetLocale { app: Some("a b".into()) }.validate().is_err());
        assert!(Control::ResetLocale { app: None }.validate().is_ok());
        assert!(Control::FontScale { percent: 130 }.validate().is_ok());
        assert!(
            Control::FontScale { percent: 20 }.validate().is_err()
                && Control::FontScale { percent: 400 }.validate().is_err()
        );
        let display = |w, h, d| Control::Display { width: w, height: h, density: d }.validate();
        assert!(display(Some(1080), Some(2400), Some(420)).is_ok() && display(None, None, Some(320)).is_ok());
        assert!(display(Some(1080), None, None).is_err() && display(None, None, None).is_err());
        assert!(display(Some(100), Some(2400), None).is_err() && display(None, None, Some(5)).is_err());
        assert!(Control::ResetDisplay.validate().is_ok());
    }

    #[test]
    fn checks_status_bar_and_battery() {
        let sb = |time: Option<&str>, battery: Option<u8>| {
            Control::StatusBar { status_bar: StatusBar { time: time.map(String::from), battery, ..Default::default() } }
                .validate()
        };
        assert!(sb(Some("9:41"), Some(100)).is_ok());
        assert!(sb(Some("9:4x"), None).is_err() && sb(None, Some(101)).is_err());
        assert!(Control::Battery { level: 101, charging: false }.validate().is_err());
    }

    #[test]
    fn controls_round_trip_through_json() {
        let c = Control::Permission { app: "a.b".into(), permission: "x".into(), grant: Grant::Revoke };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["action"], "permission");
        assert_eq!(serde_json::from_value::<Control>(json).unwrap(), c);
    }
}
