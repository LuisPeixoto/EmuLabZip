//! Making a crash report readable with the files that belong to it.
//!
//! Three kinds of frame, three kinds of file:
//!
//! - **Java and Kotlin** frames, with a `mapping.txt`: names and lines before shrinking, with inlined methods shown as
//!   the frames they were.
//! - **JavaScript** frames of a React Native app, with a source map: the file, line and function of the source.
//! - **Native iOS** frames, with the `.dSYM` of the build (checked by UUID), through `atos`.
//!
//! What was used, and what was needed but not there, is told, so a screen can say "readable with mapping.txt of 1.2.0"
//! or "add the dSYM of this build". The report that comes in is not changed: the caller keeps it, to show the original.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use emulabzip_core::{CrashReport, Frame, FrameOrigin, Language, ThreadTrace};

use crate::dsym::Dsym;
use crate::retrace::Mapping;
use crate::sourcemap::SourceMap;
use crate::store::{Kind, Stored};

/// A file to read frames with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: Kind,
    pub path: PathBuf,
}

impl From<&Stored> for Source {
    fn from(s: &Stored) -> Self {
        Source { kind: s.kind, path: s.path.clone() }
    }
}

impl Source {
    /// A file the user named: what it is is worked out from it.
    pub fn detect(path: &Path) -> emulabzip_core::Result<Source> {
        Ok(Source { kind: crate::store::detect(path)?, path: path.to_path_buf() })
    }

    fn name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    }
}

/// What a file did for a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub kind: Kind,
    pub source: String,
    /// How many frames it made readable.
    pub frames: usize,
}

#[derive(Debug, Clone)]
pub struct Symbolicated {
    pub report: CrashReport,
    pub applied: Vec<Applied>,
    /// What would make more of it readable, and was not there or did not fit.
    pub missing: Vec<String>,
}

/// A Java frame: `a.b.c.method`, in a file.
fn is_java(f: &Frame) -> bool {
    match f.language {
        Some(Language::Java) => true,
        Some(_) => false,
        // Reports made before frames said what they were.
        None => {
            f.image.as_deref().is_some_and(|i| i.ends_with(".java") || i.ends_with(".kt") || i == "SourceFile")
                && f.symbol.contains('.')
        }
    }
}

/// A class that looks shrunk: its first package part is one or two letters (`a.b.c`).
fn looks_shrunk(symbol: &str) -> bool {
    symbol.split('.').next().is_some_and(|first| first.len() <= 2) && symbol.matches('.').count() >= 1
}

fn split_symbol(symbol: &str) -> Option<(&str, &str)> {
    symbol.rsplit_once('.')
}

fn line_of(f: &Frame) -> Option<u32> {
    f.location.as_deref().and_then(|l| l.parse().ok())
}

/// Reads a mapping, keeping the members of the classes the report's frames are in.
fn read_mapping(path: &Path, classes: &HashSet<String>) -> Option<Mapping> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(Mapping::parse_with(&text, |obf| classes.contains(obf)))
}

fn renumber(frames: &mut [Frame]) {
    for (i, f) in frames.iter_mut().enumerate() {
        f.index = i as u32;
    }
}

fn retrace_threads(threads: &mut [ThreadTrace], mapping: &Mapping) -> usize {
    let mut changed = 0;
    for thread in threads.iter_mut() {
        let mut out: Vec<Frame> = Vec::with_capacity(thread.frames.len());
        for frame in thread.frames.drain(..) {
            let split = if is_java(&frame) && frame.location.as_deref() != Some("native") {
                split_symbol(&frame.symbol).map(|(c, m)| (c.to_string(), m.to_string()))
            } else {
                None
            };
            let Some((class, method)) = split.filter(|(c, _)| mapping.knows(c)) else {
                out.push(frame);
                continue;
            };
            changed += 1;
            for r in mapping.frame(&class, &method, line_of(&frame)) {
                let mut f = frame.clone();
                f.symbol = format!("{}.{}", r.class, r.method);
                f.image = r.file.or_else(|| frame.image.clone().filter(|i| i != "SourceFile"));
                f.location = r.line.map(|l| l.to_string()).or_else(|| frame.location.clone());
                // A restored class name can show that a frame is the app's own.
                if f.origin == FrameOrigin::Framework
                    && !r.class.starts_with("android.")
                    && !r.class.starts_with("java.")
                {
                    f.origin = FrameOrigin::App;
                }
                out.push(f);
            }
        }
        renumber(&mut out);
        thread.frames = out;
    }
    changed
}

fn apply_source_map(threads: &mut [ThreadTrace], map: &SourceMap) -> usize {
    let mut changed = 0;
    for frame in threads.iter_mut().flat_map(|t| t.frames.iter_mut()).filter(|f| f.language == Some(Language::Js)) {
        let Some((line, column)) = frame
            .location
            .as_deref()
            .and_then(|l| l.split_once(':'))
            .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        else {
            continue;
        };
        let Some(original) = map.lookup(line, column) else { continue };
        if let Some(name) = original.name.clone().or_else(|| Some(frame.symbol.clone()).filter(|s| !s.is_empty())) {
            frame.symbol = name;
        }
        frame.origin = if original.source.contains("node_modules") { FrameOrigin::Framework } else { FrameOrigin::App };
        frame.image = Some(original.source);
        frame.location = Some(format!("{}:{}", original.line, original.column));
        changed += 1;
    }
    changed
}

/// The frames atos can name: native, with an address and the image's load address and UUID, and no name yet.
fn needs_atos(f: &Frame) -> bool {
    f.symbol.is_empty()
        && f.address.is_some()
        && f.image_base.is_some()
        && f.build_id.is_some()
        && f.language == Some(Language::Native)
}

fn apply_dsyms(report: &mut CrashReport, dsyms: &[(String, Dsym)], arch: &str, missing: &mut Vec<String>) -> usize {
    // One lookup per image, with all its addresses.
    let mut groups: BTreeMap<(String, u64), Vec<(usize, usize)>> = BTreeMap::new();
    for (ti, thread) in report.threads.iter().enumerate() {
        for (fi, frame) in thread.frames.iter().enumerate().filter(|(_, f)| needs_atos(f)) {
            groups
                .entry((frame.build_id.clone().unwrap_or_default(), frame.image_base.unwrap_or(0)))
                .or_default()
                .push((ti, fi));
        }
    }
    let mut changed = 0;
    for ((uuid, base), at) in groups {
        let dsym = dsyms.iter().find(|(_, d)| d.is_for(&uuid).unwrap_or(false));
        let Some((_, dsym)) = dsym else {
            let image = report.threads[at[0].0].frames[at[0].1].image.clone().unwrap_or_else(|| "an image".into());
            missing.push(format!("The dSYM of {image} (UUID {uuid}) is not among the files given."));
            continue;
        };
        let addresses: Vec<u64> = at.iter().map(|(t, f)| report.threads[*t].frames[*f].address.unwrap_or(0)).collect();
        match dsym.symbolicate(arch, base, &addresses) {
            Ok(found) => {
                for ((t, f), symbol) in at.into_iter().zip(found) {
                    let Some(symbol) = symbol else { continue };
                    let frame = &mut report.threads[t].frames[f];
                    frame.symbol = symbol.function;
                    if let Some(file) = symbol.file {
                        let short = file.rsplit('/').next().unwrap_or(&file).to_string();
                        frame.location = Some(match symbol.line {
                            Some(l) => format!("{short}:{l}"),
                            None => short,
                        });
                    }
                    changed += 1;
                }
            }
            Err(e) => missing.push(e.to_string()),
        }
    }
    changed
}

fn architecture_of(report: &CrashReport) -> String {
    report.details.iter().find(|(k, _)| k == "Architecture").map(|(_, v)| v.clone()).unwrap_or_else(|| "arm64".into())
}

/// Reads `report` with `sources`: whatever fits is used, and the rest is told in `missing`.
pub fn symbolicate(report: &CrashReport, sources: &[Source]) -> Symbolicated {
    let mut out = report.clone();
    let mut applied = Vec::new();
    let mut missing = Vec::new();

    let java: HashSet<String> = report
        .threads
        .iter()
        .flat_map(|t| &t.frames)
        .filter(|f| is_java(f))
        .filter_map(|f| split_symbol(&f.symbol).map(|(c, _)| c.to_string()))
        .collect();
    let shrunk = report.threads.iter().flat_map(|t| &t.frames).any(|f| is_java(f) && looks_shrunk(&f.symbol));
    let has_js = report.threads.iter().flat_map(|t| &t.frames).any(|f| f.language == Some(Language::Js));
    let wants_dsym = report.threads.iter().flat_map(|t| &t.frames).any(needs_atos);

    let mut got_mapping = false;
    let mut got_map = false;
    let mut dsyms: Vec<(String, Dsym)> = Vec::new();

    for source in sources.iter().filter(|s| s.kind == Kind::Mapping) {
        match read_mapping(&source.path, &java) {
            Some(mapping) if java.iter().any(|c| mapping.knows(c)) => {
                got_mapping = true;
                let frames = retrace_threads(&mut out.threads, &mapping);
                out.summary.title = mapping.text(&out.summary.title);
                applied.push(Applied { kind: Kind::Mapping, source: source.name(), frames });
            }
            Some(_) if !java.is_empty() => {
                missing.push(format!(
                    "{} does not know the classes of this crash: it is probably from another build.",
                    source.name()
                ));
            }
            Some(_) => {}
            None => missing.push(format!("{} could not be read.", source.name())),
        }
    }
    for source in sources.iter().filter(|s| s.kind == Kind::SourceMap) {
        let Ok(text) = std::fs::read_to_string(&source.path) else {
            missing.push(format!("{} could not be read.", source.name()));
            continue;
        };
        match SourceMap::parse(&text) {
            Ok(map) if has_js => {
                let frames = apply_source_map(&mut out.threads, &map);
                if frames > 0 {
                    got_map = true;
                    applied.push(Applied { kind: Kind::SourceMap, source: source.name(), frames });
                }
            }
            Ok(_) => {}
            Err(e) => missing.push(format!("{}: {e}", source.name())),
        }
    }
    for source in sources.iter().filter(|s| s.kind == Kind::Dsym) {
        match Dsym::open(&source.path) {
            Ok(d) => dsyms.push((source.name(), d)),
            Err(e) => missing.push(e.to_string()),
        }
    }
    if wants_dsym && !dsyms.is_empty() {
        let arch = architecture_of(report);
        let frames = apply_dsyms(&mut out, &dsyms, &arch, &mut missing);
        if frames > 0 {
            applied.push(Applied {
                kind: Kind::Dsym,
                source: dsyms.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "),
                frames,
            });
        }
    }

    if shrunk && !got_mapping && !missing.iter().any(|m| m.contains("mapping.txt")) {
        missing.push(format!("Its Java names look shrunk: add the mapping.txt of {}.", describe(report)));
    }
    if has_js && !got_map && !missing.iter().any(|m| m.contains("source map")) {
        missing.push(format!("Its JavaScript points into a bundle: add the source map of {}.", describe(report)));
    }
    if wants_dsym && dsyms.is_empty() {
        missing.push(format!("Its native frames are bare addresses: add the .dSYM of {}.", describe(report)));
    }
    Symbolicated { report: out, applied, missing }
}

/// Reads a stack pasted as text (from a log, a bug report, a CI run): Java frames with a `mapping.txt`, JavaScript
/// frames with a source map. Dsyms are not used here, since a pasted stack has no image addresses to give `atos`.
pub fn symbolicate_text(stack: &str, sources: &[Source]) -> Result<String, String> {
    let mut text = stack.to_string();
    for source in sources.iter().filter(|s| s.kind == Kind::Mapping) {
        let raw = std::fs::read_to_string(&source.path)
            .map_err(|e| format!("Could not read {}: {e}", source.path.display()))?;
        text = Mapping::parse(&raw).stack(&text);
    }
    for source in sources.iter().filter(|s| s.kind == Kind::SourceMap) {
        let raw = std::fs::read_to_string(&source.path)
            .map_err(|e| format!("Could not read {}: {e}", source.path.display()))?;
        let map = SourceMap::parse(&raw).map_err(|e| format!("{}: {e}", source.name()))?;
        text = map.stack(&text);
    }
    if sources.iter().all(|s| s.kind == Kind::Dsym) {
        return Err(
            "A pasted stack is read with a mapping.txt or a source map; a .dSYM needs the addresses of a crash report."
                .into(),
        );
    }
    Ok(text)
}

fn describe(report: &CrashReport) -> String {
    match (&report.summary.app, &report.app_version) {
        (Some(app), Some(v)) => format!("{app} {v}"),
        (Some(app), None) => app.clone(),
        _ => "this build".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emulabzip_core::{CrashKind, CrashSummary};

    fn java(index: u32, symbol: &str, file: &str, line: &str) -> Frame {
        let mut f = Frame::new(index, symbol, Some(file.into()), Some(line.into()), FrameOrigin::App);
        f.language = Some(Language::Java);
        f
    }

    fn report(frames: Vec<Frame>) -> CrashReport {
        CrashReport {
            summary: CrashSummary {
                id: "x".into(),
                kind: CrashKind::Crash,
                app: Some("com.example.app".into()),
                process: "com.example.app".into(),
                pid: Some(1),
                time_ms: None,
                title: "java.lang.IllegalStateException: a.b.c is gone".into(),
            },
            details: vec![],
            threads: vec![ThreadTrace { name: "main".into(), crashed: true, frames }],
            hints: vec![],
            app_version: Some("1.2.0 (42)".into()),
            raw: "raw".into(),
        }
    }

    fn files(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("emulabzip-sym-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const MAPPING: &str = "com.example.app.MainActivity -> a.b.c:\n# {\"id\":\"sourceFile\",\"fileName\":\"MainActivity.kt\"}\n    1:3:void crashDeep(int):40:42 -> a\n    4:4:void helper():10:10 -> a\n    4:4:void crashDeep(int):43:43 -> a\n";

    #[test]
    fn makes_a_java_stack_readable_and_expands_what_was_inlined() {
        let dir = files("java");
        std::fs::write(dir.join("mapping.txt"), MAPPING).unwrap();
        let r = report(vec![
            java(0, "a.b.c.a", "SourceFile", "4"),
            java(1, "android.app.Activity.performCreate", "Activity.java", "8000"),
        ]);
        let source = Source::detect(&dir.join("mapping.txt")).unwrap();
        let s = symbolicate(&r, &[source]);
        let frames = &s.report.threads[0].frames;
        assert_eq!(frames.len(), 3);
        assert_eq!(
            (frames[0].symbol.as_str(), frames[0].image.as_deref(), frames[0].location.as_deref(), frames[0].index),
            ("com.example.app.MainActivity.helper", Some("MainActivity.kt"), Some("10"), 0)
        );
        assert_eq!(
            (frames[1].symbol.as_str(), frames[1].location.as_deref()),
            ("com.example.app.MainActivity.crashDeep", Some("43"))
        );
        assert_eq!((frames[2].symbol.as_str(), frames[2].index), ("android.app.Activity.performCreate", 2));
        assert_eq!(s.report.summary.title, "java.lang.IllegalStateException: com.example.app.MainActivity is gone");
        assert_eq!(s.applied, [Applied { kind: Kind::Mapping, source: "mapping.txt".into(), frames: 1 }]);
        assert!(s.missing.is_empty(), "{:?}", s.missing);
        // The report that came in is as it was.
        assert_eq!(r.threads[0].frames[0].symbol, "a.b.c.a");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn says_what_is_missing_for_a_shrunk_stack() {
        let r = report(vec![java(0, "a.b.c.a", "SourceFile", "4")]);
        let s = symbolicate(&r, &[]);
        assert!(s.applied.is_empty());
        assert_eq!(s.missing, ["Its Java names look shrunk: add the mapping.txt of com.example.app 1.2.0 (42)."]);
        // A stack that is not shrunk asks for nothing.
        let plain = report(vec![java(0, "com.example.app.MainActivity.run", "MainActivity.java", "9")]);
        assert!(symbolicate(&plain, &[]).missing.is_empty());
    }

    #[test]
    fn a_mapping_of_another_build_is_not_used() {
        let dir = files("other");
        std::fs::write(dir.join("mapping.txt"), "com.other.Thing -> x.y:\n    void run() -> a\n").unwrap();
        let r = report(vec![java(0, "a.b.c.a", "SourceFile", "4")]);
        let s = symbolicate(&r, &[Source::detect(&dir.join("mapping.txt")).unwrap()]);
        assert!(s.applied.is_empty());
        assert_eq!(s.report.threads[0].frames[0].symbol, "a.b.c.a");
        assert!(s.missing.iter().any(|m| m.contains("another build")), "{:?}", s.missing);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn makes_javascript_frames_point_at_the_source() {
        let dir = files("js");
        // One segment: generated column 5000 is src 0, line 30, column 8, name 0.
        let segment: String = [5000, 0, 30, 8, 0].iter().map(|n| crate::sourcemap::encode_vlq(*n)).collect();
        let map = format!(
            r#"{{"version":3,"sources":["src/App.tsx","node_modules/react/index.js"],"names":["crashDeep"],"mappings":"{segment}"}}"#
        );
        std::fs::write(dir.join("index.android.bundle.map"), map).unwrap();
        let mut js = Frame::new(0, "a", Some("index.android.bundle".into()), Some("1:5001".into()), FrameOrigin::App);
        js.language = Some(Language::Js);
        let r = report(vec![js]);
        let s = symbolicate(&r, &[Source::detect(&dir.join("index.android.bundle.map")).unwrap()]);
        let f = &s.report.threads[0].frames[0];
        assert_eq!(
            (f.symbol.as_str(), f.image.as_deref(), f.location.as_deref()),
            ("crashDeep", Some("src/App.tsx"), Some("31:9"))
        );
        assert_eq!(s.applied[0].kind, Kind::SourceMap);
        let none = symbolicate(&r, &[]);
        assert!(none.missing[0].contains("source map"), "{:?}", none.missing);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bare_native_frames_ask_for_the_dsym() {
        let mut f = Frame::new(0, "", Some("Crashy".into()), Some("0x100000f64".into()), FrameOrigin::App);
        f.address = Some(0x1_0000_0f64);
        f.image_base = Some(0x1_0000_0000);
        f.build_id = Some("4c4c44b2-5555-3144-a1be-f9c7c8b1b2a0".into());
        f.language = Some(Language::Native);
        let r = report(vec![f]);
        let s = symbolicate(&r, &[]);
        assert_eq!(s.missing, ["Its native frames are bare addresses: add the .dSYM of com.example.app 1.2.0 (42)."]);
        // A frame that already has a name needs nothing.
        let mut named = r.threads[0].frames[0].clone();
        named.symbol = "main".into();
        assert!(symbolicate(&report(vec![named]), &[]).missing.is_empty());
    }

    #[test]
    fn reads_a_pasted_stack() {
        let dir = files("text");
        std::fs::write(dir.join("mapping.txt"), MAPPING).unwrap();
        let source = Source::detect(&dir.join("mapping.txt")).unwrap();
        let out = symbolicate_text("java.lang.Error: a.b.c\n\tat a.b.c.a(SourceFile:4)\n", &[source]).unwrap();
        assert!(out.contains("com.example.app.MainActivity.crashDeep(MainActivity.kt:43)"), "{out}");
        assert!(out.starts_with("java.lang.Error: com.example.app.MainActivity\n"));
        std::fs::create_dir_all(dir.join("X.dSYM/Contents/Resources/DWARF")).unwrap();
        std::fs::write(dir.join("X.dSYM/Contents/Resources/DWARF/X"), b"b").unwrap();
        let only_dsym = Source { kind: Kind::Dsym, path: dir.join("X.dSYM") };
        assert!(symbolicate_text("x", &[only_dsym]).is_err());
        assert!(symbolicate_text("x", &[Source { kind: Kind::Mapping, path: dir.join("missing") }]).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_old_frame_without_a_language_is_recognized_by_its_file() {
        let mut f = Frame::new(0, "a.b.c.a", Some("SourceFile".into()), Some("4".into()), FrameOrigin::App);
        f.language = None;
        assert!(is_java(&f));
        f.image = Some("libc.so".into());
        assert!(!is_java(&f));
    }
}
