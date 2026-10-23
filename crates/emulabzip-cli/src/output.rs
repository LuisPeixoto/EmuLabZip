//! Terminal styling shared by the commands.

use emulabzip_core::{Device, DeviceState, LogEntry, LogLevel, Platform};
use owo_colors::{OwoColorize, Stream::Stdout, Style};

pub fn heading(text: &str) -> String {
    text.if_supports_color(Stdout, |t| t.bold()).to_string()
}

pub fn dim(text: &str) -> String {
    text.if_supports_color(Stdout, |t| t.dimmed()).to_string()
}

fn level_style(level: LogLevel) -> Style {
    match level {
        LogLevel::Verbose => Style::new().dimmed(),
        LogLevel::Debug => Style::new().blue(),
        LogLevel::Info => Style::new().green(),
        LogLevel::Warn => Style::new().yellow(),
        LogLevel::Error => Style::new().red(),
        LogLevel::Fatal => Style::new().bright_white().on_red(),
    }
}

/// One log line in the familiar logcat layout, coloured by level.
pub fn log_line(e: &LogEntry) -> String {
    let style = level_style(e.level);
    let badge = format!(" {} ", e.level.letter());
    format!(
        "{} {} {} {}: {}",
        dim(&e.time),
        dim(&format!("{:>5}", e.pid)),
        badge.if_supports_color(Stdout, |t| t.style(style)),
        e.tag.if_supports_color(Stdout, |t| t.style(style.bold())),
        e.message
    )
}

pub fn platform_badge(p: Platform) -> String {
    match p {
        Platform::Android => p.label().if_supports_color(Stdout, |t| t.green()).to_string(),
        Platform::Ios => p.label().if_supports_color(Stdout, |t| t.cyan()).to_string(),
    }
}

pub fn state(s: DeviceState) -> String {
    let (text, style) = match s {
        DeviceState::Online => ("online", Style::new().green()),
        DeviceState::Booting => ("booting", Style::new().yellow()),
        DeviceState::Offline => ("offline", Style::new().dimmed()),
        DeviceState::Unauthorized => ("unauthorized", Style::new().red()),
    };
    text.if_supports_color(Stdout, |t| t.style(style)).to_string()
}

/// A fixed-width table. Widths are measured on the plain text so colours do not break alignment.
pub fn table(headers: &[&str], rows: &[Vec<(String, String)>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, (plain, _)) in row.iter().enumerate() {
            widths[i] = widths[i].max(plain.chars().count());
        }
    }
    let mut out = String::new();
    let header: Vec<String> = headers.iter().enumerate().map(|(i, h)| format!("{h:<w$}", w = widths[i])).collect();
    out.push_str(&heading(header.join("  ").trim_end()));
    out.push('\n');
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, (plain, styled))| {
                let pad = widths[i].saturating_sub(plain.chars().count());
                format!("{styled}{}", " ".repeat(pad))
            })
            .collect();
        out.push_str(cells.join("  ").trim_end());
        out.push('\n');
    }
    out
}

/// A plain cell.
pub fn cell(text: impl Into<String>) -> (String, String) {
    let t = text.into();
    (t.clone(), t)
}

/// A cell whose terminal rendering differs from its plain text.
pub fn styled(plain: impl Into<String>, styled: String) -> (String, String) {
    (plain.into(), styled)
}

pub fn device_row(d: &Device) -> Vec<(String, String)> {
    let state_plain = format!("{:?}", d.state).to_lowercase();
    vec![
        styled(d.platform.label(), platform_badge(d.platform)),
        cell(d.name.clone()),
        cell(d.os_label()),
        styled(state_plain, state(d.state)),
        cell(d.id.clone()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_columns_on_plain_text() {
        let rows = vec![vec![cell("a"), styled("bb", "\u{1b}[32mbb\u{1b}[0m".into())], vec![cell("ccc"), cell("d")]];
        let t = table(&["X", "Y"], &rows);
        let lines: Vec<&str> = t.lines().collect();
        assert!(lines[1].starts_with("a    "));
        assert!(lines[2].starts_with("ccc  d"));
    }
}
