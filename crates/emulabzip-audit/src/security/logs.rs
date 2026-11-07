//! What the app wrote to the log.

use super::finding;
use crate::finding::{Area, Finding};
use crate::patterns::scan;

/// Stops a noisy log from filling the report: the same value is reported once, and no more than this many findings.
const MAX_FINDINGS: usize = 100;

pub fn analyze(lines: &[String]) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut seen: Vec<(String, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for hit in scan(line) {
            let rule = format!("{}-in-log", hit.kind.name);
            if seen.contains(&(rule.clone(), hit.redacted.clone())) {
                continue;
            }
            seen.push((rule.clone(), hit.redacted.clone()));
            let mut x = finding(
                rule,
                hit.kind.severity,
                Area::Log,
                format!("The log contains {}.", hit.kind.what),
                "Do not log credentials or personal data. Log a short identifier instead, and keep verbose logging out of release builds.",
            );
            x.evidence = Some(hit.redacted);
            x.location = Some(format!("log line {}", i + 1));
            out.push(x);
            if out.len() >= MAX_FINDINGS {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use emulabzip_core::Severity;

    #[test]
    fn secrets_in_the_log_are_found_once_each() {
        let lines: Vec<String> = [
            "D/Net: GET /me",
            "D/Net: Authorization: Bearer abcdef0123456789abcdef",
            "D/Net: Authorization: Bearer abcdef0123456789abcdef",
            "I/User: signed in as ana@example.com",
            "I/User: password=********",
        ]
        .map(String::from)
        .to_vec();
        let found = analyze(&lines);
        let rules: Vec<_> = found.iter().map(|f| (f.rule.as_str(), f.severity)).collect();
        assert_eq!(rules, [("bearer-token-in-log", Severity::Error), ("email-in-log", Severity::Warning)]);
        assert_eq!(found[0].location.as_deref(), Some("log line 2"));
        assert!(!found[0].evidence.as_deref().unwrap().contains("0123456789"));
    }
}
