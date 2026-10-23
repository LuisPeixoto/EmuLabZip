//! Installed apps (`simctl listapps`) and running apps (`launchctl list` inside the simulator).

use std::collections::HashMap;

use emulabzip_core::App;
use serde_json::Value;

/// Running UIKit apps from `launchctl list`, keyed by bundle id. Lines look like
/// `18378<TAB>0<TAB>UIKitApplication:com.example.app[f12f][rb-legacy]`.
pub fn parse_running(launchctl: &str) -> HashMap<String, u32> {
    launchctl
        .lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let pid = cols.next()?.trim().parse().ok()?;
            let label = cols.nth(1)?.trim();
            let bundle = label.strip_prefix("UIKitApplication:")?;
            let bundle = bundle.split('[').next()?;
            Some((bundle.to_string(), pid))
        })
        .collect()
}

/// Builds the app list from `listapps` (converted to JSON) and the running processes.
pub fn build_apps(listapps: &Value, running: &HashMap<String, u32>) -> Vec<App> {
    let Some(map) = listapps.as_object() else { return Vec::new() };
    let mut apps: Vec<App> = map
        .iter()
        .map(|(id, info)| {
            let s = |k: &str| info.get(k).and_then(Value::as_str).map(str::to_string);
            App {
                id: id.clone(),
                name: s("CFBundleDisplayName").or_else(|| s("CFBundleName")).unwrap_or_else(|| id.clone()),
                version: match (s("CFBundleShortVersionString"), s("CFBundleVersion")) {
                    (Some(short), Some(build)) if short != build => Some(format!("{short} ({build})")),
                    (short, build) => short.or(build),
                },
                pid: running.get(id).copied(),
                user: s("ApplicationType").as_deref() == Some("User"),
            }
        })
        .collect();
    apps.sort_by(|a, b| b.user.cmp(&a.user).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    apps
}

/// The data container of an app, from `listapps`, as a filesystem path.
pub fn data_container(listapps: &Value, bundle_id: &str) -> Option<String> {
    let url = listapps.get(bundle_id)?.get("DataContainer")?.as_str()?;
    Some(file_url_to_path(url))
}

fn file_url_to_path(url: &str) -> String {
    let path = url.strip_prefix("file://").unwrap_or(url);
    let path = path.strip_suffix('/').unwrap_or(path);
    percent_decode(path)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_running_apps() {
        let out = "PID\tStatus\tLabel\n18378\t0\tUIKitApplication:com.example.app[f12f][rb-legacy]\n-\t0\tcom.apple.thing\n5895\t0\tUIKitApplication:com.apple.mobilecal[5865][rb-legacy]\n";
        let running = parse_running(out);
        assert_eq!(running.len(), 2);
        assert_eq!(running["com.example.app"], 18378);
    }

    #[test]
    fn builds_apps_user_first() {
        let list = json!({
            "com.apple.Passbook": {"CFBundleDisplayName": "Wallet", "ApplicationType": "System", "CFBundleVersion": "1642.3.5"},
            "com.example.app": {"CFBundleName": "Example", "ApplicationType": "User",
                                "CFBundleShortVersionString": "1.2.0", "CFBundleVersion": "42",
                                "DataContainer": "file:///Users/me/Data/Application/ABC%20D/"}
        });
        let running = HashMap::from([("com.example.app".to_string(), 77)]);
        let apps = build_apps(&list, &running);
        assert_eq!(apps[0].id, "com.example.app");
        assert_eq!(apps[0].name, "Example");
        assert_eq!(apps[0].version.as_deref(), Some("1.2.0 (42)"));
        assert_eq!(apps[0].pid, Some(77));
        assert!(apps[0].user);
        assert_eq!(apps[1].name, "Wallet");
        assert_eq!(apps[1].version.as_deref(), Some("1642.3.5"));
        assert_eq!(data_container(&list, "com.example.app").as_deref(), Some("/Users/me/Data/Application/ABC D"));
    }
}
