//! The security and privacy audit: rules that read what an app declares, sends, logs and stores.
//!
//! Each module takes plain values (the text of `dumpsys package`, a list of requests, log lines, stored keys) and returns
//! findings, so every rule is tested without a device. [`crate::collect`] gathers the inputs from one.

pub mod android;
pub mod ios;
pub mod logs;
pub mod storage;
pub mod traffic;

use crate::finding::{Area, Audit, Finding};
use emulabzip_core::Severity;

pub(crate) fn finding(
    rule: impl Into<String>,
    severity: Severity,
    area: Area,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> Finding {
    Finding {
        rule: rule.into(),
        severity,
        area,
        message: message.into(),
        evidence: None,
        location: None,
        fix: fix.into(),
    }
}

/// Everything the rules look at, any part of which may be missing.
#[derive(Debug, Default)]
pub struct Input {
    /// `dumpsys package <app>` on Android.
    pub dumpsys: Option<String>,
    /// `Info.plist` as JSON on iOS.
    pub info_plist: Option<serde_json::Value>,
    /// `PrivacyInfo.xcprivacy` as JSON on iOS, `None` when the app has none.
    pub privacy_manifest: Option<serde_json::Value>,
    pub requests: Option<Vec<traffic::Request>>,
    pub log_lines: Option<Vec<String>>,
    pub stored: Option<Vec<storage::Stored>>,
    pub columns: Option<Vec<storage::Column>>,
    /// Why an input could not be read, by area, so the report says so instead of looking clean.
    pub unreadable: Vec<(Area, String)>,
}

/// Runs every rule over what was given.
pub fn audit(input: &Input) -> Audit {
    let mut audit = Audit::default();
    if let Some(text) = &input.dumpsys {
        audit.looked_at(Area::Manifest);
        audit.findings.extend(android::analyze(&android::parse(text)));
    }
    if let Some(plist) = &input.info_plist {
        audit.looked_at(Area::Manifest);
        audit.findings.extend(ios::ats(plist));
        audit.looked_at(Area::Privacy);
        let hosts: Vec<&str> = input.requests.iter().flatten().filter_map(|r| traffic::host(&r.url)).collect();
        audit.findings.extend(ios::privacy(input.privacy_manifest.as_ref(), &hosts));
    }
    if let Some(requests) = &input.requests {
        audit.looked_at(Area::Traffic);
        audit.findings.extend(traffic::analyze(requests));
    }
    if let Some(lines) = &input.log_lines {
        audit.looked_at(Area::Log);
        audit.findings.extend(logs::analyze(lines));
    }
    if input.stored.is_some() || input.columns.is_some() {
        audit.looked_at(Area::Storage);
        audit.findings.extend(storage::analyze(
            input.stored.as_deref().unwrap_or_default(),
            input.columns.as_deref().unwrap_or_default(),
        ));
    }
    audit.skipped.extend(input.unreadable.iter().cloned());
    audit.sort();
    audit
}
