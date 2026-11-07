//! The report as one HTML file: inline style, the screenshot as a data URI, nothing loaded from anywhere.

use base64::Engine;

use crate::model::{BugReport, MAX_INLINE_VIDEO, Strip, VIDEO_FILE};

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
.desc{white-space:pre-wrap}\
video{max-width:100%;max-height:640px;border:1px solid var(--line);border-radius:6px;background:#000}\
.strip{width:100%;height:auto}.strip text{font:11px -apple-system,system-ui,sans-serif;fill:var(--mut)}\
.strip line{stroke:var(--line)}.strip .m-info{fill:#4a90d9}.strip .m-warning{fill:#d9a23a}.strip .m-error{fill:var(--bad)}";

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

/// The lanes of the strip, in the order they are drawn, with the name shown for each.
const STRIP_LANES: [(&str, &str); 6] = [
    ("taps", "Touches"),
    ("requests", "Requests"),
    ("logs", "Log"),
    ("frames", "Slow frames"),
    ("gc", "GC"),
    ("crashes", "Crash"),
];

/// The last minute as an SVG: one row per kind of thing, one dot for each, time running to the right. It is drawn
/// from numbers and escaped text only, and has no script: the hover text is plain `<title>` elements.
fn strip_svg(strip: &Strip) -> String {
    const LEFT: f64 = 84.0;
    const RIGHT: f64 = 990.0;
    const ROW: f64 = 22.0;
    let rows: Vec<&(&str, &str)> =
        STRIP_LANES.iter().filter(|(lane, _)| strip.marks.iter().any(|m| m.lane == *lane)).collect();
    let height = ROW * rows.len() as f64 + 26.0;
    let window = strip.window_ms.max(1) as f64;
    let x = |t_ms: u64| LEFT + (RIGHT - LEFT) * (t_ms as f64 / window);
    let mut svg = format!(
        "<svg class=\"strip\" viewBox=\"0 0 1000 {height}\" role=\"img\" aria-label=\"Timeline of the last minute\">"
    );
    for (i, (lane, name)) in rows.iter().enumerate() {
        let y = ROW * i as f64 + ROW / 2.0 + 2.0;
        svg.push_str(&format!(
            "<text x=\"4\" y=\"{:.1}\">{}</text><line x1=\"{LEFT}\" y1=\"{y:.1}\" x2=\"{RIGHT}\" y2=\"{y:.1}\"/>",
            y + 4.0,
            escape(name)
        ));
        for m in strip.marks.iter().filter(|m| m.lane == *lane) {
            let class = match m.severity.as_str() {
                "error" => "m-error",
                "warning" => "m-warning",
                _ => "m-info",
            };
            svg.push_str(&format!(
                "<circle class=\"{class}\" cx=\"{:.1}\" cy=\"{y:.1}\" r=\"4\"><title>{}</title></circle>",
                x(m.t_ms),
                escape(&format!("{:.1} s: {}", m.t_ms as f64 / 1000.0, m.label))
            ));
        }
    }
    let base = ROW * rows.len() as f64 + 16.0;
    let seconds = strip.window_ms / 1000;
    let end = if strip.ends_at_crash { "crash" } else { "end" };
    svg.push_str(&format!(
        "<text x=\"{LEFT}\" y=\"{base}\">−{seconds} s</text><text x=\"{RIGHT}\" y=\"{base}\" text-anchor=\"end\">{end}</text></svg>"
    ));
    svg
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
    if let Some(clip) = &r.video {
        let (src, extra) = if clip.bytes.len() <= MAX_INLINE_VIDEO {
            (
                format!("data:video/mp4;base64,{}", base64::engine::general_purpose::STANDARD.encode(&clip.bytes)),
                String::new(),
            )
        } else {
            (
                VIDEO_FILE.to_string(),
                format!(" The video is the file {VIDEO_FILE}, next to this one: keep the two together."),
            )
        };
        section(
            &mut out,
            "Screen recording",
            None,
            true,
            &format!(
                "<video controls preload=\"metadata\" src=\"{}\"></video><p class=\"mut\">{}{}</p>",
                escape(&src),
                escape(&clip.label),
                escape(&extra)
            ),
        );
    }
    if let Some(strip) = &r.strip {
        section(&mut out, "Timeline, the last minute", Some(strip.marks.len()), true, &strip_svg(strip));
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
    fn a_short_clip_is_part_of_the_file_and_a_long_one_is_next_to_it() {
        use crate::model::VideoClip;
        let small = BugReport {
            video: Some(VideoClip { bytes: b"\0\0\0 ftypmp42".to_vec(), label: "Screen recording, 30 s".into() }),
            ..Default::default()
        };
        let h = html(&small);
        assert!(h.contains("<video controls") && h.contains("src=\"data:video/mp4;base64,"));
        assert!(small.video_sidecar().is_none());

        let big = BugReport {
            video: Some(VideoClip { bytes: vec![0; MAX_INLINE_VIDEO + 1], label: "Screen recording".into() }),
            ..Default::default()
        };
        let h = html(&big);
        assert!(h.contains(&format!("src=\"{VIDEO_FILE}\"")) && !h.contains("base64,AAAA"));
        assert!(h.contains("keep the two together"));
        assert_eq!(big.video_sidecar().map(<[u8]>::len), Some(MAX_INLINE_VIDEO + 1));
    }

    #[test]
    fn the_strip_draws_one_row_per_lane_with_escaped_hover_text_and_no_script() {
        use crate::model::StripMark;
        let mark = |t_ms, lane: &str, severity: &str, label: &str| StripMark {
            t_ms,
            lane: lane.into(),
            severity: severity.into(),
            label: label.into(),
        };
        let r = BugReport {
            strip: Some(Strip {
                window_ms: 60_000,
                ends_at_crash: true,
                marks: vec![
                    mark(1000, "taps", "info", "Tap (1, 2)"),
                    mark(59_000, "crashes", "error", "<script>alert(1)</script>"),
                ],
            }),
            ..Default::default()
        };
        let h = html(&r);
        assert!(
            h.contains("<svg class=\"strip\"") && h.contains("m-error") && h.contains("Touches") && h.contains("Crash")
        );
        assert!(!h.contains("Requests</text>"), "a lane with nothing on it is not drawn");
        assert!(h.contains("&lt;script&gt;") && !h.contains("<script"));
        assert!(h.contains("−60 s") && h.contains(">crash<"));
    }

    #[test]
    fn an_empty_title_gets_a_default() {
        assert!(html(&BugReport::default()).contains("<h1>Bug report</h1>"));
    }
}
