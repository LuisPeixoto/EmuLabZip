//! What an audit reports.

use emulabzip_core::Severity;
use serde::{Deserialize, Serialize};

/// Where a problem was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Area {
    /// The app's declared configuration: flags, exported components, App Transport Security.
    Manifest,
    /// Requests the app made.
    Traffic,
    /// The device log.
    Log,
    /// Files the app keeps.
    Storage,
    /// Privacy declarations of an iOS app.
    Privacy,
}

impl Area {
    pub fn label(self) -> &'static str {
        match self {
            Area::Manifest => "manifest",
            Area::Traffic => "traffic",
            Area::Log => "log",
            Area::Storage => "storage",
            Area::Privacy => "privacy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// A short, stable name such as `cleartext-traffic` or `token-in-log`.
    pub rule: String,
    pub severity: Severity,
    pub area: Area,
    pub message: String,
    /// What was seen, with secrets cut down to their first and last characters.
    pub evidence: Option<String>,
    /// Where: a file, a log line, a URL without its query.
    pub location: Option<String>,
    pub fix: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub findings: Vec<Finding>,
    /// The areas that were looked at.
    pub checked: Vec<Area>,
    /// The areas that could not be read, and why.
    pub skipped: Vec<(Area, String)>,
}

impl Audit {
    pub fn count(&self, severity: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == severity).count()
    }

    /// Whether any finding is at least this serious.
    pub fn fails(&self, at_least: Severity) -> bool {
        self.findings.iter().any(|f| f.severity >= at_least)
    }

    /// Puts the most serious first, then by area and rule, so two runs read the same.
    pub fn sort(&mut self) {
        self.findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then((a.area as u8).cmp(&(b.area as u8)))
                .then_with(|| a.rule.cmp(&b.rule))
                .then_with(|| a.location.cmp(&b.location))
        });
    }

    pub fn looked_at(&mut self, area: Area) {
        if !self.checked.contains(&area) {
            self.checked.push(area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(rule: &str, severity: Severity, area: Area) -> Finding {
        Finding {
            rule: rule.into(),
            severity,
            area,
            message: String::new(),
            evidence: None,
            location: None,
            fix: String::new(),
        }
    }

    #[test]
    fn the_worst_comes_first_and_the_order_is_stable() {
        let mut audit = Audit::default();
        audit.findings.push(finding("b", Severity::Info, Area::Log));
        audit.findings.push(finding("z", Severity::Error, Area::Storage));
        audit.findings.push(finding("a", Severity::Error, Area::Manifest));
        audit.findings.push(finding("c", Severity::Warning, Area::Log));
        audit.sort();
        let rules: Vec<_> = audit.findings.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(rules, ["a", "z", "c", "b"]);
        assert_eq!(audit.count(Severity::Error), 2);
        assert!(audit.fails(Severity::Warning) && !Audit::default().fails(Severity::Info));
    }
}
