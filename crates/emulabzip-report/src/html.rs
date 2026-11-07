//! The report as one HTML file: inline style, the screenshot as a data URI, nothing loaded from anywhere.

use base64::Engine;

use crate::model::BugReport;

/// Escapes text for HTML, in content and in attributes.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

const STYLE: &str = "\
:root{color-scheme:light dark;--bg:#fff;--fg:#1c1c1e;--mut:#6b6b70;--line:#e2e2e6;--card:#f6f6f8;--bad:#c62828}\
@media(prefers-color-scheme:dark){:root{--bg:#161618;--fg:#ececf0;--mut:#9a9aa2;--line:#2c2c30;--card:#202024;--bad:#ef6b6b}}\
body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.5 -apple-system,system-ui,sans-serif}\
main{max-width:960px;margin:0 auto;padding:24px 16px 64px}\
h1{font-size:22px;margin:0 0 4px}h2{font-size:15px;margin:0}\
.mut{color:var(--mut)}.bad{color:var(--bad)}\
details{border:1px solid var(--line);border-radius:8px;margin:12px 0;background:var(--card)}\
summary{cursor:pointer;padding:10px 14px;font-weight:600}\
.body{padding:0 14px 14px;overflow-x:auto}\
table{border-collapse:collapse;width:100%}td,th{text-align:left;padding:3px 10px 3px 0;vertical-align:top}\
pre{margin:0;font:12px/1.45 ui-monospace,Menlo,monospace;white-space:pre-wrap;word-break:break-word}\
img{max-width:100%;max-height:640px;border:1px solid var(--line);border-radius:6px}\
.desc{white-space:pre-wrap}";

fn section(out: &mut String, title: &str, count: Option<usize>, open: bool, body: &str) {
    let n = count.map(|c| format!(" <span class=\"mut\">({c})</span>")).unwrap_or_default();
    out.push_str(&format!(
        "<details{}><summary>{}{n}</summary><div class=\"body\">{body}</div></details>\n",
        if open { " open" } else { "" },
        escape(title)
    ));
}

fn lines(items: &[crate::model::LogLine]) -> String {
    let text: Vec<String> = items.iter().map(|l| format!("{} {} {}: {}", l.time, l.level, l.tag, l.message)).collect();
    format!("<pre>{}</pre>", escape(&text.join("\n")))
}

/// The whole report as an HTML document.
pub fn html(r: &BugReport) -> String {
    let title = if r.title.trim().is_empty() { "Bug report" } else { r.title.trim() };
    let mut out = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>{STYLE}</style></head><body><main>\n<h1>{}</h1>\n",
        escape(title),
        escape(title)
    );
    let app = match (&r.app, &r.app_version) {
        (Some(a), Some(v)) => format!("{a} {v}"),
        (Some(a), None) => a.clone(),
        _ => String::new(),
    };
    out.push_str(&format!(
        "<p class=\"mut\">{}{}</p>\n",
        escape(&app),
        if app.is_empty() { String::new() } else { " · ".into() } + &escape(&r.tool)
    ));
    if !r.description.trim().is_empty() {
        out.push_str(&format!("<p class=\"desc\">{}</p>\n", escape(r.description.trim())));
    }
    if let Some(n) = &r.session_note {
        out.push_str(&format!("<p class=\"mut\">{}</p>\n", escape(n)));
    }
    for n in &r.notes {
        out.push_str(&format!("<p class=\"mut\">• {}</p>\n", escape(n)));
    }
    if let Some(png) = &r.screenshot {
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        section(
            &mut out,
            "Screenshot",
            None,
            true,
            &format!("<img alt=\"Screenshot\" src=\"data:image/png;base64,{b64}\">"),
        );
    }
    if !r.device.is_empty() {
        let rows: String =
            r.device.iter().map(|(k, v)| format!("<tr><th>{}</th><td>{}</td></tr>", escape(k), escape(v))).collect();
        section(&mut out, "Device", None, true, &format!("<table>{rows}</table>"));
    }
    for c in &r.crashes {
        let mut body = format!(
            "<p><b class=\"bad\">{}</b></p><p class=\"mut\">{}{}</p>",
            escape(&c.title),
            escape(&c.kind),
            if c.process.is_empty() { String::new() } else { format!(" · {}", escape(&c.process)) }
        );
        if c.readable {
            body.push_str(&format!("<p class=\"mut\">Made readable with: {}</p>", escape(&c.applied.join(", "))));
        }
        for m in &c.missing {
            body.push_str(&format!("<p class=\"mut\">Missing: {}</p>", escape(m)));
        }
        for (name, frames) in &c.threads {
            body.push_str(&format!("<p><b>{}</b></p><pre>{}</pre>", escape(name), escape(&frames.join("\n"))));
        }
        section(&mut out, "Crash", None, true, &body);
    }
    if r.requests_total > 0 || !r.requests.is_empty() {
        let rows: String = r
            .requests
            .iter()
            .map(|q| {
                let status = if q.status == 0 { "—".to_string() } else { q.status.to_string() };
                let extra = q.error.clone().or_else(|| q.snippet.clone()).unwrap_or_default();
                format!(
                    "<tr><td>{}</td><td class=\"bad\">{}</td><td>{}</td><td class=\"mut\">{}</td></tr>",
                    escape(&q.method),
                    escape(&status),
                    escape(&q.url),
                    escape(&extra)
                )
            })
            .collect();
        let body = if rows.is_empty() {
            "<p class=\"mut\">None failed.</p>".to_string()
        } else {
            format!("<table>{rows}</table>")
        };
        section(
            &mut out,
            &format!("Failed requests of {}", r.requests_total),
            Some(r.requests.len()),
            !rows.is_empty(),
            &body,
        );
    }
    if !r.logs.problems.is_empty() {
        section(&mut out, "Warnings and errors", Some(r.logs.problems_total), true, &lines(&r.logs.problems));
    }
    if !r.logs.tail.is_empty() {
        section(&mut out, "Last log lines", Some(r.logs.tail.len()), false, &lines(&r.logs.tail));
    }
    out.push_str("</main></body></html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FailedRequest, LogLine};

    #[test]
    fn escapes_everything_that_could_become_markup() {
        assert_eq!(
            escape("<script>alert(\"x\" & 'y')</script>"),
            "&lt;script&gt;alert(&quot;x&quot; &amp; &#39;y&#39;)&lt;/script&gt;"
        );
        assert_eq!(escape("a\u{0}b\u{7}c\nd"), "abc\nd");
    }

    #[test]
    fn a_report_is_one_file_and_nothing_in_it_runs() {
        let r = BugReport {
            title: "Crash on <login>".into(),
            description: "it \"dies\"".into(),
            screenshot: Some(vec![137, 80, 78, 71]),
            requests: vec![FailedRequest {
                method: "GET".into(),
                url: "https://a/<x>".into(),
                status: 500,
                error: None,
                duration_ms: None,
                snippet: Some("<b>boom</b>".into()),
            }],
            requests_total: 3,
            logs: crate::model::LogSection {
                total: 1,
                problems: vec![LogLine {
                    time: "t".into(),
                    level: "E".into(),
                    tag: "A".into(),
                    message: "<img src=x onerror=1>".into(),
                }],
                problems_total: 1,
                tail: vec![],
            },
            ..Default::default()
        };
        let h = html(&r);
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("data:image/png;base64,iVBORw=="));
        assert!(h.contains("Crash on &lt;login&gt;"));
        assert!(!h.contains("<img src=x"), "log text is escaped");
        assert!(!h.contains("<b>boom"), "bodies are escaped");
        assert!(!h.contains("<script"), "no script at all");
        assert!(!h.contains("http://") || !h.contains("src=\"http"), "nothing is loaded from a server");
    }

    #[test]
    fn an_empty_title_gets_a_default() {
        assert!(html(&BugReport::default()).contains("<h1>Bug report</h1>"));
    }
}
