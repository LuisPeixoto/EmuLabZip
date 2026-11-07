//! What an Android app declares, read from the text of `dumpsys package <app>`.
//!
//! `dumpsys` does not print the `exported` attribute. It does list every component that has an intent filter, and those
//! are the ones other apps can reach, so that is what is reported.

use emulabzip_core::Severity;

use super::finding;
use crate::finding::{Area, Finding};

/// Google Play asks new and updated apps to target at least this level.
pub const MIN_TARGET_SDK: u32 = 34;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Activity,
    Receiver,
    Service,
    Provider,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Activity => "activity",
            Kind::Receiver => "receiver",
            Kind::Service => "service",
            Kind::Provider => "content provider",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub kind: Kind,
    pub name: String,
    /// The actions of its intent filters (empty for a provider, whose filters `dumpsys` does not list).
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub package: String,
    pub debuggable: bool,
    pub allow_backup: bool,
    pub target_sdk: Option<u32>,
    pub version: Option<String>,
    pub components: Vec<Component>,
    pub permissions: Vec<String>,
}

/// The package's own name: the first `Package [name]` line.
fn package_name(text: &str) -> String {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("Package [")?.split(']').next().map(str::to_string))
        .unwrap_or_default()
}

pub fn parse(text: &str) -> Facts {
    let mut facts = Facts { package: package_name(text), ..Facts::default() };
    let own = format!("{}/", facts.package);

    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("pkgFlags=[").or_else(|| t.strip_prefix("flags=[")) {
            let flags = rest.trim_end_matches(']');
            if flags.split_whitespace().any(|f| f == "DEBUGGABLE") {
                facts.debuggable = true;
            }
            if flags.split_whitespace().any(|f| f == "ALLOW_BACKUP") {
                facts.allow_backup = true;
            }
        }
        if facts.version.is_none()
            && let Some(v) = t.strip_prefix("versionName=")
        {
            facts.version = Some(v.to_string());
        }
        if facts.target_sdk.is_none()
            && let Some(i) = t.find("targetSdk=")
        {
            facts.target_sdk = t[i + "targetSdk=".len()..].split_whitespace().next().and_then(|n| n.parse().ok());
        }
    }

    // Resolver tables: `<hash> <package>/<class> filter <hash>`, followed by the filter's actions.
    let mut kind = None;
    let mut current: Option<usize> = None;
    let mut in_providers = false;
    let mut in_permissions = false;
    for line in text.lines() {
        let t = line.trim();
        if !line.starts_with(' ') && !t.is_empty() {
            kind = match t {
                "Activity Resolver Table:" => Some(Kind::Activity),
                "Receiver Resolver Table:" => Some(Kind::Receiver),
                "Service Resolver Table:" => Some(Kind::Service),
                _ => None,
            };
            in_providers = t == "Registered ContentProviders:";
            current = None;
        }
        if t == "requested permissions:" {
            in_permissions = true;
            continue;
        }
        if in_permissions {
            if t.ends_with(':') || t.contains(": ") || t.is_empty() {
                in_permissions = false;
            } else {
                facts.permissions.push(t.to_string());
                continue;
            }
        }
        if in_providers {
            if let Some(name) = t.strip_suffix(':').and_then(|n| n.strip_prefix(&own)) {
                facts.components.push(Component { kind: Kind::Provider, name: name.to_string(), actions: Vec::new() });
            }
            continue;
        }
        let Some(k) = kind else { continue };
        if let Some(i) = t.find(&own) {
            let name = t[i + own.len()..].split_whitespace().next().unwrap_or("").to_string();
            if !name.is_empty() && t.contains(" filter ") {
                let existing = facts.components.iter().position(|c| c.kind == k && c.name == name);
                current = Some(existing.unwrap_or_else(|| {
                    facts.components.push(Component { kind: k, name, actions: Vec::new() });
                    facts.components.len() - 1
                }));
            }
        } else if let (Some(at), Some(rest)) = (current, t.strip_prefix("Action: \"")) {
            let action = rest.trim_end_matches('"').to_string();
            if !facts.components[at].actions.contains(&action) {
                facts.components[at].actions.push(action);
            }
        } else if t.starts_with("Category: \"android.intent.category.LAUNCHER\"")
            && let Some(at) = current
        {
            // The launcher entry is meant to be reached by the system.
            facts.components[at].actions.push("(launcher)".into());
        }
    }
    facts
}

pub fn analyze(f: &Facts) -> Vec<Finding> {
    let mut out = Vec::new();
    if f.debuggable {
        let mut x = finding(
            "debuggable",
            Severity::Warning,
            Area::Manifest,
            "The app is debuggable: anyone with a USB cable can read its private data and attach a debugger.",
            "Make sure `android:debuggable` is not set in release builds (Gradle's release build type does this by default).",
        );
        x.location = Some(f.package.clone());
        out.push(x);
    }
    if f.allow_backup {
        let mut x = finding(
            "allow-backup",
            Severity::Warning,
            Area::Manifest,
            "`allowBackup` is on: the app's data can be copied off the device with `adb backup` or restored from a cloud backup.",
            "Set `android:allowBackup=\"false\"`, or declare what to leave out with `android:dataExtractionRules` and `android:fullBackupContent`.",
        );
        x.location = Some(f.package.clone());
        out.push(x);
    }
    if let Some(sdk) = f.target_sdk.filter(|s| *s < MIN_TARGET_SDK) {
        let mut x = finding(
            "old-target-sdk",
            Severity::Warning,
            Area::Manifest,
            format!("The app targets API {sdk}, so newer security and privacy behaviour changes do not apply to it."),
            format!("Raise `targetSdk` to {MIN_TARGET_SDK} or higher."),
        );
        x.location = Some(f.package.clone());
        out.push(x);
    }
    for c in &f.components {
        let launcher = c.actions.iter().any(|a| a == "(launcher)");
        if launcher
            && c.actions.iter().filter(|a| *a != "(launcher)" && *a != "android.intent.action.MAIN").count() == 0
        {
            continue;
        }
        let (severity, rule, message) = match c.kind {
            Kind::Provider => (
                Severity::Info,
                "content-provider",
                format!("{} is a content provider; `dumpsys` does not say whether it is exported.", c.name),
            ),
            _ => (
                Severity::Warning,
                "exposed-component",
                format!(
                    "The {} {} has an intent filter, so other apps can start it unless it asks for a permission.",
                    c.kind.label(),
                    c.name
                ),
            ),
        };
        let mut x = finding(
            rule,
            severity,
            Area::Manifest,
            message,
            "Set `android:exported=\"false\"` if only this app needs it, or protect it with `android:permission`, and validate the intent it receives.",
        );
        x.location = Some(format!("{}/{}", f.package, c.name));
        if !c.actions.is_empty() {
            x.evidence = Some(c.actions.iter().filter(|a| *a != "(launcher)").cloned().collect::<Vec<_>>().join(", "));
        }
        out.push(x);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../tests/fixtures/dumpsys-package.txt");

    #[test]
    fn a_real_dumpsys_is_read() {
        let f = parse(REAL);
        assert_eq!(f.package, "dev.emulabzip.composedemo");
        assert!(f.debuggable && f.allow_backup);
        assert_eq!(f.target_sdk, Some(36));
        assert_eq!(f.version.as_deref(), Some("1.0"));
        assert!(f.permissions.contains(&"android.permission.INTERNET".to_string()), "{:?}", f.permissions);
        let names: Vec<_> = f.components.iter().map(|c| (c.kind, c.name.as_str())).collect();
        assert!(names.contains(&(Kind::Activity, ".MainActivity")), "{names:?}");
        assert!(names.contains(&(Kind::Receiver, "androidx.profileinstaller.ProfileInstallReceiver")), "{names:?}");
        assert!(names.contains(&(Kind::Provider, "androidx.startup.InitializationProvider")), "{names:?}");
        let receiver = f.components.iter().find(|c| c.kind == Kind::Receiver).unwrap();
        assert!(receiver.actions.iter().any(|a| a == "androidx.profileinstaller.action.SAVE_PROFILE"));
    }

    #[test]
    fn the_findings_say_what_to_change() {
        let rules: Vec<_> = analyze(&parse(REAL)).iter().map(|x| x.rule.clone()).collect();
        assert!(rules.contains(&"debuggable".to_string()) && rules.contains(&"allow-backup".to_string()));
        assert!(rules.contains(&"exposed-component".to_string()), "the profile installer receiver is reachable");
        assert!(rules.contains(&"content-provider".to_string()));
        assert!(!rules.contains(&"old-target-sdk".to_string()));
        // The launcher activity is meant to be reached, so it is not a finding.
        let exposed: Vec<_> = analyze(&parse(REAL))
            .into_iter()
            .filter(|x| x.rule == "exposed-component")
            .filter_map(|x| x.location)
            .collect();
        assert!(!exposed.iter().any(|l| l.ends_with(".MainActivity")), "{exposed:?}");
    }

    #[test]
    fn a_clean_release_app_has_nothing_to_report() {
        let text = "Packages:\n  Package [com.shop] (abc):\n    targetSdk=35\n    pkgFlags=[ HAS_CODE ]\n";
        assert!(analyze(&parse(text)).is_empty());
        let old = "  Package [com.shop] (abc):\n    versionCode=1 minSdk=21 targetSdk=28\n";
        let found = analyze(&parse(old));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, "old-target-sdk");
    }
}
