//! What an iOS app declares: App Transport Security in `Info.plist`, and its privacy manifest.

use emulabzip_core::Severity;
use serde_json::Value;

use super::finding;
use crate::finding::{Area, Finding};

/// Hosts of advertising and analytics services. An app that talks to one tracks its users, and Apple asks it to say so.
const TRACKERS: &[&str] = &[
    "google-analytics.com",
    "app-measurement.com",
    "firebaselogging-pa.googleapis.com",
    "doubleclick.net",
    "segment.io",
    "segment.com",
    "amplitude.com",
    "mixpanel.com",
    "facebook.com",
    "appsflyer.com",
    "adjust.com",
    "branch.io",
    "braze.com",
    "singular.net",
];

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn covers(domain: &str, host: &str) -> bool {
    let d = domain.trim_start_matches('.').to_ascii_lowercase();
    let h = host.to_ascii_lowercase();
    h == d || h.ends_with(&format!(".{d}"))
}

/// The tracker a host belongs to, if any.
pub fn tracker(host: &str) -> Option<&'static str> {
    TRACKERS.iter().copied().find(|t| covers(t, host))
}

pub fn ats(info: &Value) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(ats) = info.get("NSAppTransportSecurity") else { return out };
    let fix = "Remove the exception and serve the content over HTTPS, or limit it to the one domain that needs it with `NSExceptionDomains`.";
    if flag(ats, "NSAllowsArbitraryLoads") {
        out.push(finding(
            "ats-arbitrary-loads",
            Severity::Error,
            Area::Manifest,
            "`NSAllowsArbitraryLoads` is on: App Transport Security is switched off for every connection the app makes.",
            fix,
        ));
    }
    for (key, what) in [
        ("NSAllowsArbitraryLoadsInWebContent", "web views"),
        ("NSAllowsArbitraryLoadsForMedia", "media loaded with AVFoundation"),
    ] {
        if flag(ats, key) {
            out.push(finding(
                "ats-arbitrary-loads-scoped",
                Severity::Warning,
                Area::Manifest,
                format!("`{key}` allows insecure connections from {what}."),
                fix,
            ));
        }
    }
    if let Some(domains) = ats.get("NSExceptionDomains").and_then(Value::as_object) {
        for (domain, rules) in domains {
            if flag(rules, "NSExceptionAllowsInsecureHTTPLoads") {
                let mut x = finding(
                    "ats-insecure-domain",
                    Severity::Warning,
                    Area::Manifest,
                    format!("Plain HTTP is allowed for {domain}."),
                    fix,
                );
                x.location = Some(domain.clone());
                out.push(x);
            }
            if matches!(rules.get("NSExceptionMinimumTLSVersion").and_then(Value::as_str), Some("TLSv1.0" | "TLSv1.1"))
            {
                let mut x = finding(
                    "ats-old-tls",
                    Severity::Warning,
                    Area::Manifest,
                    format!("{domain} may be reached with TLS 1.0 or 1.1, which are no longer considered safe."),
                    "Require TLS 1.2 or later for the domain.",
                );
                x.location = Some(domain.clone());
                out.push(x);
            }
        }
    }
    out
}

/// Compares the tracking domains the app was seen using with what its privacy manifest declares.
pub fn privacy(manifest: Option<&Value>, hosts: &[&str]) -> Vec<Finding> {
    let mut seen: Vec<&str> = Vec::new();
    for h in hosts {
        if tracker(h).is_some() && !seen.contains(h) {
            seen.push(h);
        }
    }
    if seen.is_empty() {
        return Vec::new();
    }
    let Some(manifest) = manifest else {
        let mut x = finding(
            "no-privacy-manifest",
            Severity::Warning,
            Area::Privacy,
            "The app contacted advertising or analytics domains but has no `PrivacyInfo.xcprivacy`.",
            "Add a privacy manifest that lists the data the app collects and the domains it uses for tracking.",
        );
        x.evidence = Some(seen.join(", "));
        return vec![x];
    };
    let declared: Vec<String> = manifest
        .get("NSPrivacyTrackingDomains")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    if !flag(manifest, "NSPrivacyTracking") {
        let mut x = finding(
            "tracking-not-declared",
            Severity::Warning,
            Area::Privacy,
            "`NSPrivacyTracking` is false, but the app contacted advertising or analytics domains.",
            "Set `NSPrivacyTracking` to true if these calls track users across apps and sites, and list the domains.",
        );
        x.evidence = Some(seen.join(", "));
        out.push(x);
    }
    for host in seen {
        if !declared.iter().any(|d| covers(d, host)) {
            let mut x = finding(
                "undeclared-tracking-domain",
                Severity::Warning,
                Area::Privacy,
                format!("{host} is not listed in `NSPrivacyTrackingDomains`."),
                "Add the domain to `NSPrivacyTrackingDomains` in `PrivacyInfo.xcprivacy`.",
            );
            x.location = Some(host.to_string());
            out.push(x);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn app_transport_security_exceptions_are_found() {
        let plist = json!({ "NSAppTransportSecurity": {
            "NSAllowsArbitraryLoads": true,
            "NSAllowsArbitraryLoadsInWebContent": true,
            "NSExceptionDomains": {
                "legacy.example.com": { "NSExceptionAllowsInsecureHTTPLoads": true, "NSExceptionMinimumTLSVersion": "TLSv1.0" },
                "fine.example.com": { "NSIncludesSubdomains": true }
            } } });
        let rules: Vec<_> = ats(&plist).into_iter().map(|f| (f.rule, f.severity)).collect();
        assert!(rules.contains(&("ats-arbitrary-loads".into(), Severity::Error)));
        assert!(rules.contains(&("ats-arbitrary-loads-scoped".into(), Severity::Warning)));
        assert_eq!(rules.iter().filter(|(r, _)| r == "ats-insecure-domain").count(), 1);
        assert_eq!(rules.iter().filter(|(r, _)| r == "ats-old-tls").count(), 1);
        assert!(ats(&json!({ "CFBundleName": "Shop" })).is_empty());
        assert!(ats(&json!({ "NSAppTransportSecurity": {} })).is_empty());
    }

    #[test]
    fn tracking_domains_are_compared_with_the_privacy_manifest() {
        let hosts = ["api.shop.com", "api.segment.io", "cdn.segment.io", "app-measurement.com"];
        let none = privacy(None, &hosts);
        assert_eq!(none.len(), 1);
        assert_eq!(none[0].rule, "no-privacy-manifest");
        assert!(none[0].evidence.as_deref().unwrap().contains("segment.io"));

        let manifest = json!({ "NSPrivacyTracking": true, "NSPrivacyTrackingDomains": ["segment.io"] });
        let found = privacy(Some(&manifest), &hosts);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, "undeclared-tracking-domain");
        assert_eq!(found[0].location.as_deref(), Some("app-measurement.com"));

        let off = json!({ "NSPrivacyTracking": false, "NSPrivacyTrackingDomains": [] });
        assert!(privacy(Some(&off), &hosts).iter().any(|f| f.rule == "tracking-not-declared"));
        assert!(privacy(None, &["api.shop.com"]).is_empty(), "no tracker, nothing to declare");
        assert!(tracker("notsegment.io").is_none() && tracker("x.mixpanel.com").is_some());
    }
}
