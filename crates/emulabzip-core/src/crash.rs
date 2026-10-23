//! Crashes, native crashes and ANRs collected from a device, with the stack of each.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CrashKind {
    /// An uncaught exception (Java, Kotlin, Swift, Objective-C).
    Crash,
    /// A fatal signal in native code, with a tombstone or a crash report.
    NativeCrash,
    /// Application Not Responding.
    Anr,
}

impl CrashKind {
    pub fn label(self) -> &'static str {
        match self {
            CrashKind::Crash => "Crash",
            CrashKind::NativeCrash => "Native crash",
            CrashKind::Anr => "ANR",
        }
    }
}

/// One line of the list of crashes, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashSummary {
    /// Stable for one crash, usable with [`crate::Backend::crash`].
    pub id: String,
    pub kind: CrashKind,
    /// Package name or bundle id, when known.
    pub app: Option<String>,
    pub process: String,
    pub pid: Option<u32>,
    /// Milliseconds since the Unix epoch, when the device recorded it.
    pub time_ms: Option<u64>,
    /// The exception, signal or ANR subject in one line.
    pub title: String,
}

/// Whose code a frame is, the same four groups the flame graph colours by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameOrigin {
    App,
    Framework,
    Native,
    Kernel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub index: u32,
    /// The function, with its class or file when the device gave one. Empty when unsymbolicated.
    pub symbol: String,
    /// The library or binary the frame is in, or the source file for managed code.
    pub image: Option<String>,
    /// The offset into the image, as the device printed it (`0x1a2b`) or the line number.
    pub location: Option<String>,
    pub origin: FrameOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTrace {
    pub name: String,
    /// The thread that crashed, or the main thread of an ANR.
    pub crashed: bool,
    pub frames: Vec<Frame>,
}

/// Something to try next, such as the command that symbolicates a frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hint {
    pub text: String,
    /// A command to copy, when there is one.
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub summary: CrashSummary,
    /// Extra facts the report states: the signal, the fault address, the OS version.
    pub details: Vec<(String, String)>,
    pub threads: Vec<ThreadTrace>,
    pub hints: Vec<Hint>,
    /// The report as the device wrote it.
    pub raw: String,
}

/// Longest crash id accepted.
const MAX_ID: usize = 200;

/// Crash ids come from backends, and travel through the UI and MCP tools back to them. Only plain
/// characters are accepted, so an id can never name a path or a shell word.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && !id.contains("..")
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_ids_only() {
        assert!(is_valid_id("dropbox:data_app_crash:1792796123000"));
        assert!(is_valid_id("ips:crashy-2026-10-23-185501.ips"));
        for bad in ["", "..", "a/b", "a b", "a;b", "a\\b", "$(x)", &"x".repeat(MAX_ID + 1)] {
            assert!(!is_valid_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn serializes_in_camel_case() {
        let s = CrashSummary {
            id: "x".into(),
            kind: CrashKind::NativeCrash,
            app: None,
            process: "p".into(),
            pid: Some(1),
            time_ms: Some(2),
            title: "t".into(),
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!((json["kind"].as_str(), json["timeMs"].as_u64()), (Some("nativeCrash"), Some(2)));
    }
}
