//! A short summary for the body of an issue.

use crate::model::BugReport;

/// What goes in an issue: the description, the device, the crash and the first problems. The full report is the HTML
/// file, which the person attaches.
pub fn markdown(r: &BugReport) -> String {
    let mut out = String::new();
    if !r.description.trim().is_empty() {
        out.push_str(r.description.trim());
        out.push_str("\n\n");
    }
    let app = match (&r.app, &r.app_version) {
        (Some(a), Some(v)) => Some(format!("{a} {v}")),
        (Some(a), None) => Some(a.clone()),
        _ => None,
    };
    if let Some(a) = app {
        out.push_str(&format!("**App:** {a}\n"));
    }
    if !r.device.is_empty() {
        let facts: Vec<String> = r
            .device
            .iter()
            .filter(|(k, _)| matches!(k.as_str(), "Device" | "OS version" | "Model" | "API level"))
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        if !facts.is_empty() {
            out.push_str(&format!("**Device:** {}\n", facts.join(" · ")));
        }
    }
    out.push('\n');
    for c in &r.crashes {
        out.push_str(&format!("### Crash\n`{}`\n\n", c.title.replace('`', "'")));
        if let Some((name, frames)) = c.threads.first() {
            out.push_str(&format!(
                "```\n{name}\n{}\n```\n\n",
                frames.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
            ));
        }
    }
    if !r.requests.is_empty() {
        out.push_str("### Failed requests\n");
        for q in r.requests.iter().take(8) {
            let status = if q.status == 0 { "no answer".to_string() } else { q.status.to_string() };
            out.push_str(&format!("- `{} {}` → {status}\n", q.method, q.url));
        }
        out.push('\n');
    }
    if !r.logs.problems.is_empty() {
        out.push_str("### Errors in the log\n```\n");
        for l in r.logs.problems.iter().take(10) {
            out.push_str(&format!("{} {}: {}\n", l.level, l.tag, l.message));
        }
        out.push_str("```\n\n");
    }
    let files = if r.video_sidecar().is_some() {
        format!("emulabzip-report.html and {}", crate::model::VIDEO_FILE)
    } else {
        "emulabzip-report.html".to_string()
    };
    out.push_str(&format!("_Made with {}. The full report is attached: {files}_\n", r.tool));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarises_what_matters() {
        let r = BugReport {
            description: "Taps do nothing".into(),
            app: Some("dev.x".into()),
            app_version: Some("1.2 (3)".into()),
            device: vec![("Device".into(), "Pixel".into()), ("Memory".into(), "3 GB".into())],
            tool: "EmuLabZip 0.9.0".into(),
            ..Default::default()
        };
        let m = markdown(&r);
        assert!(m.starts_with("Taps do nothing"));
        assert!(m.contains("**App:** dev.x 1.2 (3)"));
        assert!(m.contains("Device: Pixel") && !m.contains("Memory"));
        assert!(m.contains("emulabzip-report.html"));
    }
}
