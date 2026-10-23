//! Log entries from logcat and the unified logging system, and the filter used by the CLI and the UI.

use std::fmt;
use std::str::FromStr;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

/// Severity, ordered from the most to the least verbose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    pub const ALL: [LogLevel; 6] =
        [LogLevel::Verbose, LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error, LogLevel::Fatal];

    /// The single letter used by logcat (`V`, `D`, `I`, `W`, `E`, `F`).
    pub fn letter(self) -> char {
        match self {
            LogLevel::Verbose => 'V',
            LogLevel::Debug => 'D',
            LogLevel::Info => 'I',
            LogLevel::Warn => 'W',
            LogLevel::Error => 'E',
            LogLevel::Fatal => 'F',
        }
    }

    pub fn from_letter(c: char) -> Option<Self> {
        Some(match c.to_ascii_uppercase() {
            'V' => LogLevel::Verbose,
            'D' => LogLevel::Debug,
            'I' => LogLevel::Info,
            'W' => LogLevel::Warn,
            'E' => LogLevel::Error,
            // `A` is "assert", which logcat prints for wtf() and native aborts.
            'F' | 'A' => LogLevel::Fatal,
            _ => return None,
        })
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.letter())
    }
}

impl FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.trim().to_ascii_lowercase();
        let level = match lower.as_str() {
            "verbose" => Some(LogLevel::Verbose),
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warn" | "warning" => Some(LogLevel::Warn),
            "error" => Some(LogLevel::Error),
            "fatal" | "assert" => Some(LogLevel::Fatal),
            _ if lower.len() == 1 => lower.chars().next().and_then(LogLevel::from_letter),
            _ => None,
        };
        level.ok_or_else(|| format!("unknown log level `{s}`"))
    }
}

/// One log line, already parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Monotonic sequence number assigned by the stream, used by the UI as a stable key.
    pub seq: u64,
    /// Time as printed by the device, for example `10-23 14:02:11.123`.
    pub time: String,
    pub pid: u32,
    pub tid: u32,
    pub level: LogLevel,
    /// logcat tag, or the process name on iOS.
    pub tag: String,
    pub message: String,
    /// Unified logging subsystem and category (`com.apple.network:connection`). iOS only.
    pub subsystem: Option<String>,
}

impl LogEntry {
    /// The line in a logcat `threadtime`-like layout, used for plain-text export.
    pub fn to_line(&self) -> String {
        format!("{} {:>5} {:>5} {} {}: {}", self.time, self.pid, self.tid, self.level, self.tag, self.message)
    }
}

/// What to keep from a log stream. Every field is optional; an empty filter keeps everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilter {
    pub min_level: Option<LogLevel>,
    /// Only lines from these process ids. Empty means every process.
    #[serde(default)]
    pub pids: Vec<u32>,
    /// Case-insensitive substring of the tag.
    pub tag: Option<String>,
    /// Text searched in the tag and the message.
    pub text: Option<String>,
    /// Treat `text` as a regular expression.
    #[serde(default)]
    pub regex: bool,
}

impl LogFilter {
    /// Compiles the filter once so it can be applied to many lines.
    pub fn compile(&self) -> Result<LogMatcher, regex::Error> {
        let text = match self.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            None => TextMatch::Any,
            Some(t) if self.regex => TextMatch::Regex(RegexBuilder::new(t).case_insensitive(true).build()?),
            Some(t) => TextMatch::Substring(t.to_lowercase()),
        };
        Ok(LogMatcher {
            min_level: self.min_level,
            pids: self.pids.clone(),
            tag: self.tag.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_lowercase),
            text,
        })
    }
}

#[derive(Debug, Clone)]
enum TextMatch {
    Any,
    Substring(String),
    Regex(Regex),
}

/// A compiled [`LogFilter`].
#[derive(Debug, Clone)]
pub struct LogMatcher {
    min_level: Option<LogLevel>,
    pids: Vec<u32>,
    tag: Option<String>,
    text: TextMatch,
}

impl LogMatcher {
    pub fn matches(&self, entry: &LogEntry) -> bool {
        if self.min_level.is_some_and(|min| entry.level < min) {
            return false;
        }
        if !self.pids.is_empty() && !self.pids.contains(&entry.pid) {
            return false;
        }
        if let Some(tag) = &self.tag
            && !entry.tag.to_lowercase().contains(tag)
        {
            return false;
        }
        match &self.text {
            TextMatch::Any => true,
            TextMatch::Substring(t) => entry.message.to_lowercase().contains(t) || entry.tag.to_lowercase().contains(t),
            TextMatch::Regex(re) => re.is_match(&entry.message) || re.is_match(&entry.tag),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(level: LogLevel, tag: &str, message: &str, pid: u32) -> LogEntry {
        LogEntry {
            seq: 0,
            time: "10-23 14:02:11.123".into(),
            pid,
            tid: pid,
            level,
            tag: tag.into(),
            message: message.into(),
            subsystem: None,
        }
    }

    #[test]
    fn levels_are_ordered_by_severity() {
        assert!(LogLevel::Verbose < LogLevel::Debug);
        assert!(LogLevel::Error < LogLevel::Fatal);
    }

    #[test]
    fn parses_names_and_letters() {
        assert_eq!("warning".parse::<LogLevel>(), Ok(LogLevel::Warn));
        assert_eq!("E".parse::<LogLevel>(), Ok(LogLevel::Error));
        assert_eq!("a".parse::<LogLevel>(), Ok(LogLevel::Fatal));
        assert!("loud".parse::<LogLevel>().is_err());
    }

    #[test]
    fn empty_filter_keeps_everything() {
        let m = LogFilter::default().compile().unwrap();
        assert!(m.matches(&entry(LogLevel::Verbose, "x", "y", 1)));
    }

    #[test]
    fn filters_by_level_pid_and_tag() {
        let m = LogFilter {
            min_level: Some(LogLevel::Warn),
            pids: vec![42],
            tag: Some("okhttp".into()),
            ..Default::default()
        }
        .compile()
        .unwrap();
        assert!(m.matches(&entry(LogLevel::Error, "OkHttp", "boom", 42)));
        assert!(!m.matches(&entry(LogLevel::Info, "OkHttp", "boom", 42)));
        assert!(!m.matches(&entry(LogLevel::Error, "OkHttp", "boom", 7)));
        assert!(!m.matches(&entry(LogLevel::Error, "Choreographer", "boom", 42)));
    }

    #[test]
    fn text_search_is_case_insensitive_and_supports_regex() {
        let plain = LogFilter { text: Some("SKIPPED".into()), ..Default::default() }.compile().unwrap();
        assert!(plain.matches(&entry(LogLevel::Info, "Choreographer", "Skipped 31 frames!", 1)));

        let re = LogFilter { text: Some(r"skipped \d+ frames".into()), regex: true, ..Default::default() }
            .compile()
            .unwrap();
        assert!(re.matches(&entry(LogLevel::Info, "Choreographer", "Skipped 31 frames!", 1)));
        assert!(!re.matches(&entry(LogLevel::Info, "Choreographer", "Skipped many frames", 1)));
    }

    #[test]
    fn invalid_regex_is_an_error() {
        assert!(LogFilter { text: Some("(".into()), regex: true, ..Default::default() }.compile().is_err());
    }
}
