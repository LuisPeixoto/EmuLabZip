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

/// The kind of code a frame is: what decides how it can be made readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    /// Java or Kotlin: obfuscated names are restored with a ProGuard or R8 `mapping.txt`.
    Java,
    /// Machine code in a library or binary: symbols come from the debug info (a `.dSYM`, or an unstripped `.so`).
    Native,
    /// JavaScript of a React Native app: positions in a bundle are looked up in a source map.
    Js,
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
    /// For native frames: the address in the image on Android, the address in the process on iOS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<u64>,
    /// Where the image was loaded, when the report says it (iOS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_base: Option<u64>,
    /// Tells which build of the image this is: the ELF build id on Android, the Mach-O UUID on iOS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
}

impl Frame {
    /// A frame with only what every report has; the rest can be set after.
    pub fn new(
        index: u32,
        symbol: impl Into<String>,
        image: Option<String>,
        location: Option<String>,
        origin: FrameOrigin,
    ) -> Self {
        Frame {
            index,
            symbol: symbol.into(),
            image,
            location,
            origin,
            address: None,
            image_base: None,
            build_id: None,
            language: None,
        }
    }
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
    /// The version of the app, `1.2.0 (42)`, which is what tells which mapping file belongs to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    /// The report as the device wrote it.
    pub raw: String,
}

/// A frame of a JavaScript stack, as the engine printed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsFrame {
    pub function: Option<String>,
    /// The file as printed, usually the bundle. Empty when the engine printed only the position, as Hermes does for
    /// bytecode: `name@1:12345`.
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Reads one line of a Hermes, V8 or JavaScriptCore stack:
///
/// ```text
///     at onPress (address at index.android.bundle:1:123456)
///     at foo (index.android.bundle:10:20)
///     foo@index.ios.bundle:10:20
///     foo@1:12345
///     @index.ios.bundle:10:20
/// ```
pub fn parse_js_frame(line: &str) -> Option<JsFrame> {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix("at ") {
        let (function, place) = match rest.rsplit_once(" (") {
            Some((f, p)) => (Some(f.trim()), p.strip_suffix(')')?),
            None => (None, rest),
        };
        let place = place.strip_prefix("address at ").unwrap_or(place);
        let (file, line_no, column) = position(place)?;
        let function = function.filter(|f| !f.is_empty() && *f != "anonymous" && *f != "?");
        return Some(JsFrame { function: function.map(str::to_string), file, line: line_no, column });
    }
    // JavaScriptCore and React Native's own format: `name@file:line:column` or `name@line:column`.
    let (function, place) = t.split_once('@')?;
    if function.contains(' ') {
        return None;
    }
    let (file, line_no, column) = position(place)?;
    Some(JsFrame { function: Some(function.to_string()).filter(|f| !f.is_empty()), file, line: line_no, column })
}

/// `file:line:column`, `line:column`, where the file may itself hold colons (`http://host/index.bundle`).
fn position(place: &str) -> Option<(String, u32, u32)> {
    let (rest, column) = place.rsplit_once(':')?;
    let column = column.parse().ok()?;
    match rest.rsplit_once(':') {
        Some((file, line)) => match line.parse() {
            Ok(line) => Some((file.to_string(), line, column)),
            Err(_) => None,
        },
        None => Some((String::new(), rest.parse().ok()?, column)),
    }
}

/// The JavaScript stack in a crash report, as a thread, when the report holds one. React Native writes it into the
/// message of the exception it throws (`JavascriptException`, `RCTFatalException`), so it is found by its shape.
pub fn js_thread(text: &str) -> Option<ThreadTrace> {
    let mut frames: Vec<Frame> = Vec::new();
    let mut seen: Vec<JsFrame> = Vec::new();
    for line in text.lines() {
        let Some(js) = parse_js_frame(line) else { continue };
        // The same stack is often printed twice, in the message and in the cause: one copy is enough.
        if seen.first() == Some(&js) && !seen.is_empty() {
            break;
        }
        let index = frames.len() as u32;
        let image = Some(js.file.clone()).filter(|f| !f.is_empty());
        let location = Some(format!("{}:{}", js.line, js.column));
        let mut frame = Frame::new(index, js.function.clone().unwrap_or_default(), image, location, FrameOrigin::App);
        frame.language = Some(Language::Js);
        frames.push(frame);
        seen.push(js);
    }
    (!frames.is_empty()).then(|| ThreadTrace { name: "JavaScript".into(), crashed: true, frames })
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

    #[test]
    fn an_old_frame_without_the_new_fields_still_reads() {
        let old = r#"{"index":0,"symbol":"a.B.c","image":"B.java","location":"12","origin":"app"}"#;
        let f: Frame = serde_json::from_str(old).unwrap();
        assert_eq!((f.address, f.image_base, f.build_id, f.language), (None, None, None, None));
        let json = serde_json::to_string(&Frame::new(1, "x", None, None, FrameOrigin::Native)).unwrap();
        assert!(!json.contains("address") && !json.contains("buildId"), "{json}");
        let mut with = Frame::new(2, "y", None, None, FrameOrigin::Native);
        with.build_id = Some("ab12".into());
        with.language = Some(Language::Native);
        let json = serde_json::to_value(&with).unwrap();
        assert_eq!((json["buildId"].as_str(), json["language"].as_str()), (Some("ab12"), Some("native")));
    }

    #[test]
    fn reads_the_frames_each_engine_prints() {
        let f = parse_js_frame("    at onPress (address at index.android.bundle:1:123456)").unwrap();
        assert_eq!(
            (f.function.as_deref(), f.file.as_str(), f.line, f.column),
            (Some("onPress"), "index.android.bundle", 1, 123456)
        );
        let f = parse_js_frame("at foo (http://localhost:8081/index.bundle:10:20)").unwrap();
        assert_eq!((f.file.as_str(), f.line, f.column), ("http://localhost:8081/index.bundle", 10, 20));
        let f = parse_js_frame("    at index.android.bundle:3:4").unwrap();
        assert_eq!((f.function, f.file.as_str()), (None, "index.android.bundle"));
        let f = parse_js_frame("anonymous@index.ios.bundle:10:20").unwrap();
        assert_eq!((f.function.as_deref(), f.line), (Some("anonymous"), 10));
        let f = parse_js_frame("@index.ios.bundle:10:20").unwrap();
        assert_eq!(f.function, None);
        let f = parse_js_frame("crashDeep@1:12345").unwrap();
        assert_eq!((f.function.as_deref(), f.file.as_str(), f.line, f.column), (Some("crashDeep"), "", 1, 12345));
        for not in [
            "",
            "Error: boom",
            "    at native",
            "    at foo (native)",
            "at foo (a:b:c)",
            "x@y",
            "mail me at a b@c.d:1:2",
            "\tat com.x.Y.m(Y.java:12)",
        ] {
            assert_eq!(parse_js_frame(not), None, "{not:?}");
        }
    }

    #[test]
    fn finds_the_javascript_stack_in_a_report_and_keeps_one_copy() {
        let text = "Process: dev.rn\n\ncom.facebook.react.common.JavascriptException: Error: boom, js engine: hermes, stack:\ncrashDeep@1:12345\nonPress@1:23456\n\n\tat com.facebook.react.X.run(X.java:10)\nCaused by: com.facebook.react.common.JavascriptException: Error: boom, js engine: hermes, stack:\ncrashDeep@1:12345\nonPress@1:23456\n";
        let t = js_thread(text).unwrap();
        assert_eq!((t.name.as_str(), t.crashed, t.frames.len()), ("JavaScript", true, 2));
        assert_eq!(t.frames[0].symbol, "crashDeep");
        assert_eq!((t.frames[0].location.as_deref(), t.frames[0].language), (Some("1:12345"), Some(Language::Js)));
        assert_eq!(t.frames[1].index, 1);
        assert!(js_thread("java.lang.RuntimeException: boom\n\tat a.B.c(B.java:1)").is_none());
    }
}
