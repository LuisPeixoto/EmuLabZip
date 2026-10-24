//! Crashes, native crashes and ANRs, read from Android's DropBox.
//!
//! The system keeps a copy of every crash, ANR and tombstone in DropBox, and `dumpsys dropbox
//! --print <tag>` prints them. The shell user may run it, so this works on any device or emulator,
//! with no root and nothing installed.

use std::collections::HashMap;

use emulabzip_adb::{AdbClient, is_package_name};
use emulabzip_core::{CrashKind, CrashReport, CrashSummary, Error, Frame, FrameOrigin, Hint, Result, ThreadTrace};

/// DropBox tags that hold what we show, with the kind of each.
const TAGS: &[(&str, CrashKind)] = &[
    ("data_app_crash", CrashKind::Crash),
    ("system_app_crash", CrashKind::Crash),
    ("data_app_native_crash", CrashKind::NativeCrash),
    ("SYSTEM_TOMBSTONE", CrashKind::NativeCrash),
    ("data_app_anr", CrashKind::Anr),
    ("system_app_anr", CrashKind::Anr),
];

const SEPARATOR: &str = "========================================";

/// One entry of `dumpsys dropbox --print`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub tag: String,
    /// `2026-10-23 20:59:28`, in the device's time zone.
    pub date: String,
    pub body: String,
}

/// Splits the output of `dumpsys dropbox --print <tag>` (one or several tags in a row) into its
/// entries.
pub fn parse_entries(text: &str) -> Vec<Entry> {
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut open = false;
    for line in text.lines() {
        if line == SEPARATOR {
            blocks.push(Vec::new());
            open = true;
        } else if line.starts_with("Drop box contents:") {
            // The next tag's preamble: the entry before it is over.
            open = false;
        } else if open && let Some(block) = blocks.last_mut() {
            block.push(line);
        }
    }
    blocks
        .into_iter()
        .filter_map(|block| {
            // `2026-10-23 20:59:28 data_app_crash (text, 2291 bytes)`
            let (head, body) = block.split_first()?;
            let mut parts = head.splitn(3, ' ');
            let (day, clock, rest) = (parts.next()?, parts.next()?, parts.next()?);
            if day.len() != 10 || clock.len() != 8 {
                return None;
            }
            let tag = rest.split(' ').next()?;
            Some(Entry {
                tag: tag.to_string(),
                date: format!("{day} {clock}"),
                body: body.join("\n").trim_end().to_string(),
            })
        })
        .collect()
}

fn compact(date: &str) -> String {
    date.chars().filter(char::is_ascii_digit).collect()
}

fn id_of(entry: &Entry) -> String {
    format!("dropbox:{}:{}", entry.tag, compact(&entry.date))
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds since the epoch of `2026-10-23 20:59:28` read as if it were UTC.
fn naive_seconds(date: &str) -> Option<i64> {
    let (day, clock) = date.split_once(' ')?;
    let mut d = day.split('-').map(|p| p.parse::<i64>().ok());
    let mut c = clock.split(':').map(|p| p.parse::<i64>().ok());
    let (y, m, dd) = (d.next()??, d.next()??, d.next()??);
    let (h, mi, s) = (c.next()??, c.next()??, c.next()??);
    Some(days_from_civil(y, m, dd) * 86_400 + h * 3600 + mi * 60 + s)
}

/// The device's offset from UTC in seconds, from its clock read two ways at once.
pub fn utc_offset(epoch: &str, local: &str) -> Option<i64> {
    Some(naive_seconds(local.trim())? - epoch.trim().parse::<i64>().ok()?)
}

fn header<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    // Headers come before the first blank line.
    body.lines()
        .take_while(|l| !l.trim().is_empty())
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(": "))
        .map(str::trim)
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    line.chars().take(240).collect()
}

fn kind_of(tag: &str) -> Option<CrashKind> {
    TAGS.iter().find(|(t, _)| *t == tag).map(|(_, k)| *k)
}

/// The exception, signal or ANR subject in one line.
fn title_of(kind: CrashKind, body: &str) -> String {
    match kind {
        CrashKind::Crash => {
            // The root cause is the last `Caused by:` of the chain, or the exception itself.
            let text = body.split_once("\n\n").map_or(body, |(_, rest)| rest);
            let cause = text.lines().filter_map(|l| l.strip_prefix("Caused by: ")).next_back();
            first_line(cause.unwrap_or(text))
        }
        CrashKind::NativeCrash => {
            let signal = body.lines().find(|l| l.starts_with("signal ")).unwrap_or("native crash");
            match body.lines().find_map(|l| l.strip_prefix("Abort message: ")) {
                Some(m) => format!("{signal}: {}", m.trim_matches('\'')),
                None => signal.to_string(),
            }
        }
        CrashKind::Anr => {
            let subject = header(body, "Subject")
                .or_else(|| body.lines().find_map(|l| l.strip_prefix("Subject: ")))
                .unwrap_or("ANR");
            // The subject goes on with the input event; the first sentence is enough.
            first_line(subject.split(" Waited ").next().unwrap_or(subject))
        }
    }
}

fn process_of(kind: CrashKind, body: &str) -> (String, Option<u32>, Option<String>) {
    let mut process = header(body, "Process").map(str::to_string);
    let mut pid = header(body, "PID").and_then(|p| p.parse().ok());
    let mut app = header(body, "Package").and_then(|p| p.split(' ').next()).map(str::to_string);
    if kind == CrashKind::NativeCrash {
        // `pid: 3832, tid: 3832, name: mulabzip.crashy  >>> dev.emulabzip.crashy <<<`
        if let Some(line) = body.lines().find(|l| l.starts_with("pid: ")) {
            if pid.is_none() {
                pid = line.trim_start_matches("pid: ").split(',').next().and_then(|p| p.parse().ok());
            }
            if let Some((_, rest)) = line.split_once(">>> ") {
                let name = rest.split(" <<<").next().unwrap_or(rest).trim().to_string();
                process.get_or_insert_with(|| name.clone());
                if app.is_none() && is_package_name(&name) {
                    app = Some(name);
                }
            }
        }
    }
    let process = process.or_else(|| app.clone()).unwrap_or_else(|| "unknown".into());
    if app.is_none() && is_package_name(&process) {
        app = Some(process.clone());
    }
    (process, pid, app)
}

/// Turns entries into summaries, newest first. `offset` is the device's UTC offset in seconds.
pub fn summaries(entries: &[Entry], offset: Option<i64>, app: Option<&str>) -> Vec<CrashSummary> {
    let mut all: Vec<CrashSummary> = entries
        .iter()
        .filter_map(|e| {
            let kind = kind_of(&e.tag)?;
            let (process, pid, package) = process_of(kind, &e.body);
            if app.is_some_and(|a| package.as_deref() != Some(a) && process != a) {
                return None;
            }
            let time_ms = naive_seconds(&e.date).zip(offset).map(|(n, o)| ((n - o).max(0) as u64) * 1000);
            Some(CrashSummary {
                id: id_of(e),
                kind,
                app: package,
                process,
                pid,
                time_ms,
                title: title_of(kind, &e.body),
            })
        })
        .collect();
    all.sort_by(|a, b| b.time_ms.cmp(&a.time_ms).then_with(|| b.id.cmp(&a.id)));
    all.dedup_by(|a, b| a.id == b.id);
    all
}

/// Class-name prefixes that belong to the platform and not to the app.
const PLATFORM: &[&str] = &[
    "android.",
    "androidx.",
    "java.",
    "javax.",
    "dalvik.",
    "libcore.",
    "com.android.",
    "com.google.android.",
    "kotlin.",
    "kotlinx.",
    "sun.",
    "jdk.",
    "org.apache.",
    "org.json.",
    "org.xml.",
];

fn java_origin(symbol: &str, package: Option<&str>) -> FrameOrigin {
    if package.is_some_and(|p| symbol.starts_with(p)) {
        FrameOrigin::App
    } else if PLATFORM.iter().any(|p| symbol.starts_with(p)) {
        FrameOrigin::Framework
    } else {
        FrameOrigin::App
    }
}

/// `at pkg.Class.method(File.java:12)`, `at pkg.Class.method(Native Method)`.
fn parse_java_frame(line: &str, index: u32, package: Option<&str>) -> Option<Frame> {
    let rest = line.trim().strip_prefix("at ")?;
    let (symbol, place) = rest.split_once('(')?;
    let place = place.trim_end_matches(')');
    let (image, location) = match place.rsplit_once(':') {
        Some((file, line)) => (Some(file.to_string()), Some(line.to_string())),
        None if place == "Native Method" || place == "Native method" => (None, Some("native".to_string())),
        None => (Some(place.to_string()).filter(|p| p != "Unknown Source"), None),
    };
    Some(Frame { index, symbol: symbol.to_string(), image, location, origin: java_origin(symbol, package) })
}

/// `#00 pc 00000000000b7f57  /apex/.../libc.so (kill+7) (BuildId: fa33…)`
fn parse_native_frame(line: &str, package: Option<&str>) -> Option<Frame> {
    let rest = line.trim().strip_prefix('#')?;
    let (index, rest) = rest.split_once(' ')?;
    let rest = rest.trim_start().strip_prefix("pc ")?;
    let (pc, rest) = rest.split_once(' ')?;
    let rest = rest.trim_start();
    // The symbol is in the first parenthesis that is not the BuildId, but the path may contain
    // none, so cut the BuildId first.
    let rest = rest.split(" (BuildId:").next().unwrap_or(rest);
    let (image, symbol) = match rest.find(" (") {
        Some(i) => (&rest[..i], rest[i + 2..].trim_end_matches(')')),
        None => (rest, ""),
    };
    let symbol = symbol.split_once('+').map_or(symbol, |(s, _)| s);
    let (image, anonymous) = (image.trim(), image.trim().starts_with("<anonymous"));
    let hex = pc.trim_start_matches('0');
    let origin = if anonymous
        || image.ends_with(".jar")
        || image.ends_with(".odex")
        || image.ends_with(".oat")
        || image.ends_with(".vdex")
        || image.ends_with(".art")
    {
        java_origin(symbol, package)
    } else if image.starts_with("/data/app") || image.starts_with("/data/data") {
        FrameOrigin::App
    } else {
        FrameOrigin::Native
    };
    Some(Frame {
        index: index.parse().ok()?,
        symbol: symbol.to_string(),
        image: Some(image.to_string()).filter(|i| !anonymous && !i.is_empty()),
        location: Some(format!("0x{}", if hex.is_empty() { "0" } else { hex })),
        origin,
    })
}

fn java_threads(body: &str, package: Option<&str>) -> Vec<ThreadTrace> {
    // One section per exception of the chain; the root cause is the one that crashed.
    let text = body.split_once("\n\n").map_or(body, |(_, rest)| rest);
    let mut threads: Vec<ThreadTrace> = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(cause) = t.strip_prefix("Caused by: ") {
            threads.push(ThreadTrace { name: first_line(cause), crashed: false, frames: Vec::new() });
        } else if t.starts_with("at ") {
            if threads.is_empty() {
                threads.push(ThreadTrace { name: "exception".into(), crashed: false, frames: Vec::new() });
            }
            let thread = threads.last_mut().expect("just pushed");
            let index = thread.frames.len() as u32;
            thread.frames.extend(parse_java_frame(t, index, package));
        } else if !t.starts_with("...") && !t.is_empty() && threads.is_empty() {
            threads.push(ThreadTrace { name: first_line(t), crashed: false, frames: Vec::new() });
        }
    }
    if threads.is_empty() {
        return threads;
    }
    // The first line is the outermost exception: name it by its message.
    if threads[0].name == "exception" {
        threads[0].name = first_line(text);
    }
    if let Some(root) = threads.last_mut() {
        root.crashed = true;
    }
    threads
}

fn native_threads(body: &str, package: Option<&str>) -> Vec<ThreadTrace> {
    let name = body
        .lines()
        .find(|l| l.starts_with("pid: "))
        .and_then(|l| l.split("name: ").nth(1))
        .map(|n| n.split("  >>>").next().unwrap_or(n).trim().to_string())
        .unwrap_or_else(|| "crashed thread".into());
    let mut frames = Vec::new();
    let mut in_backtrace = false;
    for line in body.lines() {
        let t = line.trim();
        if t == "backtrace:" {
            in_backtrace = true;
        } else if in_backtrace {
            match parse_native_frame(t, package) {
                Some(f) => frames.push(f),
                // The backtrace ends at the first line that is not a frame.
                None => break,
            }
        }
    }
    if frames.is_empty() { Vec::new() } else { vec![ThreadTrace { name, crashed: true, frames }] }
}

/// The threads of the ANR dump that belongs to the app's own process.
fn anr_threads(body: &str, pid: Option<u32>, package: Option<&str>) -> Vec<ThreadTrace> {
    let marker = pid.map(|p| format!("----- pid {p} at "));
    let mut in_section = false;
    let mut threads: Vec<ThreadTrace> = Vec::new();
    for line in body.lines() {
        if line.starts_with("----- pid ") {
            in_section = marker.as_deref().is_some_and(|m| line.starts_with(m));
            continue;
        }
        if line.starts_with("----- end ") {
            in_section = false;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix('"') {
            let name = rest.split('"').next().unwrap_or(rest).to_string();
            threads.push(ThreadTrace { crashed: name == "main", name, frames: Vec::new() });
        } else if let Some(thread) = threads.last_mut() {
            let t = line.trim_start();
            let index = thread.frames.len() as u32;
            if t.starts_with("at ") {
                thread.frames.extend(parse_java_frame(t, index, package));
            } else if let Some(native) = t.strip_prefix("native: ") {
                thread.frames.extend(parse_native_frame(native, package).map(|mut f| {
                    f.index = index;
                    f
                }));
            }
        }
    }
    // The blocked main thread comes first, then the rest as the device listed them.
    threads.sort_by_key(|t| !t.crashed);
    threads
}

fn details_of(kind: CrashKind, body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut add = |k: &str, v: Option<&str>| {
        if let Some(v) = v.map(str::trim).filter(|v| !v.is_empty()) {
            out.push((k.to_string(), v.to_string()));
        }
    };
    add("Package", header(body, "Package"));
    add("Build", header(body, "Build"));
    if kind == CrashKind::NativeCrash {
        let signal = body.lines().find(|l| l.starts_with("signal "));
        add("Signal", signal.and_then(|s| s.strip_prefix("signal ")));
        add("ABI", body.lines().find_map(|l| l.strip_prefix("ABI: ")).map(|v| v.trim_matches('\'')));
        add("Process uptime", body.lines().find_map(|l| l.strip_prefix("Process uptime: ")));
        add(
            "Abort message",
            body.lines().find_map(|l| l.strip_prefix("Abort message: ")).map(|v| v.trim_matches('\'')),
        );
    }
    if kind == CrashKind::Anr {
        add("Activity", header(body, "Activity"));
        add("CPU pressure", body.lines().skip_while(|l| !l.contains("/proc/pressure/cpu")).nth(1));
    }
    out
}

fn hints_of(kind: CrashKind, threads: &[ThreadTrace], body: &str, package: Option<&str>) -> Vec<Hint> {
    let mut hints = Vec::new();
    let crashed = threads.iter().find(|t| t.crashed);
    let obfuscated = crashed.is_some_and(|t| {
        t.frames.iter().any(|f| {
            f.origin == FrameOrigin::App
                && f.symbol.split('.').next().is_some_and(|c| c.len() <= 2 && !c.is_empty())
                && f.symbol.contains('.')
        })
    });
    if obfuscated {
        hints.push(Hint {
            text: "The class names look obfuscated. Map them back with the mapping.txt of this build.".into(),
            command: Some("retrace mapping.txt stacktrace.txt".into()),
        });
    }
    match kind {
        CrashKind::NativeCrash => {
            let unsymbolicated =
                crashed.is_some_and(|t| t.frames.iter().any(|f| f.origin == FrameOrigin::App && f.symbol.is_empty()));
            if unsymbolicated {
                hints.push(Hint {
                    text: "Some frames of the app's native code have no symbols. Symbolicate them with the unstripped library from the same build (the BuildId must match).".into(),
                    command: Some("ndk-stack -sym <folder with unstripped .so files> -dump tombstone.txt".into()),
                });
                hints.push(Hint {
                    text: "Or resolve one address at a time.".into(),
                    command: Some("llvm-symbolizer --obj=<lib.so> <offset>".into()),
                });
            }
            if body.contains("SI_USER") {
                hints.push(Hint {
                    text: "The signal was sent by a process with kill(), not raised by a fault in the code.".into(),
                    command: None,
                });
            }
        }
        CrashKind::Anr => {
            let main = crashed.and_then(|t| t.frames.first());
            if main.is_some_and(|f| {
                let s = f.symbol.as_str();
                s.contains("Thread.sleep")
                    || s.contains("Object.wait")
                    || s.contains("LockSupport.park")
                    || s.contains("Unsafe.park")
            }) {
                hints.push(Hint {
                    text: "The main thread was blocked (sleeping or waiting) when the system gave up on it. Move that work off the main thread, or find who holds the lock.".into(),
                    command: None,
                });
            }
            if body.lines().any(|l| {
                l.starts_with("some avg10=")
                    && l.split("avg10=")
                        .nth(1)
                        .and_then(|v| v.split(' ').next())
                        .and_then(|v| v.parse::<f64>().ok())
                        .is_some_and(|v| v > 60.0)
            }) {
                hints.push(Hint {
                    text: "The device was short of CPU or memory at the time, which can cause an ANR in a healthy app."
                        .into(),
                    command: None,
                });
            }
        }
        CrashKind::Crash => {}
    }
    if let Some(pkg) = package {
        hints.push(Hint {
            text: "See what the app logged around the crash.".into(),
            command: Some(format!("emulabzip logs --app {pkg}")),
        });
    }
    hints
}

/// Builds the report for one entry.
pub fn report(entry: &Entry, offset: Option<i64>) -> Result<CrashReport> {
    let kind = kind_of(&entry.tag).ok_or_else(|| Error::invalid("crash", entry.tag.clone()))?;
    let summary =
        summaries(std::slice::from_ref(entry), offset, None).pop().ok_or_else(|| Error::failed("empty crash entry"))?;
    let package = summary.app.as_deref();
    let threads = match kind {
        CrashKind::Crash => java_threads(&entry.body, package),
        CrashKind::NativeCrash => native_threads(&entry.body, package),
        CrashKind::Anr => anr_threads(&entry.body, summary.pid, package),
    };
    let hints = hints_of(kind, &threads, &entry.body, package);
    Ok(CrashReport { details: details_of(kind, &entry.body), hints, threads, raw: entry.body.clone(), summary })
}

/// `dumpsys dropbox --print` for every tag, then the device's clock twice.
fn script() -> String {
    let tags: Vec<&str> = TAGS.iter().map(|(t, _)| *t).collect();
    format!(
        "for t in {}; do dumpsys dropbox --print $t; done; echo '{CLOCK}'; date +%s; date '+%Y-%m-%d %H:%M:%S'",
        tags.join(" ")
    )
}

const CLOCK: &str = "@@clock@@";

async fn fetch(adb: &AdbClient, serial: &str) -> Result<(Vec<Entry>, Option<i64>)> {
    let out = adb.shell(serial, &script()).await?;
    let (dump, clock) = out.split_once(&format!("{CLOCK}\n")).unwrap_or((&out, ""));
    let mut lines = clock.lines();
    let offset = lines.next().zip(lines.next()).and_then(|(e, l)| utc_offset(e, l));
    Ok((parse_entries(dump), offset))
}

pub async fn list(adb: &AdbClient, serial: &str, app: Option<&str>) -> Result<Vec<CrashSummary>> {
    let (entries, offset) = fetch(adb, serial).await?;
    Ok(summaries(&entries, offset, app))
}

pub async fn get(adb: &AdbClient, serial: &str, id: &str) -> Result<CrashReport> {
    let (entries, offset) = fetch(adb, serial).await?;
    let entry = entries.iter().find(|e| id_of(e) == id).ok_or_else(|| Error::NotFound(format!("crash {id}")))?;
    report(entry, offset)
}

/// Entries by id, for tests and callers that already have the text.
pub fn index(entries: &[Entry]) -> HashMap<String, &Entry> {
    entries.iter().map(|e| (id_of(e), e)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRASH: &str = include_str!("../tests/fixtures/crash/data_app_crash.txt");
    const NATIVE: &str = include_str!("../tests/fixtures/crash/data_app_native_crash.txt");
    const ANR: &str = include_str!("../tests/fixtures/crash/data_app_anr.txt");
    /// Three hours behind UTC, like the emulator the fixtures came from.
    const OFFSET: Option<i64> = Some(-3 * 3600);

    fn one(text: &str) -> Entry {
        let mut entries = parse_entries(text);
        assert_eq!(entries.len(), 1);
        entries.remove(0)
    }

    #[test]
    fn finds_the_entries_in_a_dump_of_several_tags() {
        let both = format!("{CRASH}\n{NATIVE}");
        let entries = parse_entries(&both);
        assert_eq!(
            entries.iter().map(|e| e.tag.as_str()).collect::<Vec<_>>(),
            ["data_app_crash", "data_app_native_crash"]
        );
        assert!(!entries[0].body.contains("Drop box contents"), "the next tag's preamble is not part of the entry");
        assert!(entries[0].body.starts_with("Process: dev.emulabzip.crashy"));
    }

    #[test]
    fn works_out_the_time_zone_from_two_readings_of_the_clock() {
        assert_eq!(utc_offset("1792799968", "2026-10-23 20:59:28"), Some(-10_800));
        assert_eq!(utc_offset("0", "1970-01-01 05:30:00"), Some(19_800));
        assert_eq!(utc_offset("x", "y"), None);
    }

    #[test]
    fn summarizes_a_java_crash_by_its_root_cause() {
        let s = &summaries(&[one(CRASH)], OFFSET, None)[0];
        assert_eq!(s.kind, CrashKind::Crash);
        assert_eq!((s.app.as_deref(), s.pid), (Some("dev.emulabzip.crashy"), Some(3642)));
        assert_eq!(s.title, "java.lang.NumberFormatException: For input string: \"not a number\"");
        assert_eq!(s.time_ms, Some(1_792_799_968_000));
        assert_eq!(s.id, "dropbox:data_app_crash:20261023205928");
    }

    #[test]
    fn reads_the_exception_chain_of_a_java_crash() {
        let r = report(&one(CRASH), OFFSET).unwrap();
        assert_eq!(r.threads.len(), 3, "the exception and its two causes");
        assert!(r.threads[0].name.starts_with("java.lang.RuntimeException: Unable to start activity"));
        assert_eq!(r.threads.iter().map(|t| t.crashed).collect::<Vec<_>>(), [false, false, true]);
        let root = &r.threads[2];
        assert_eq!(
            (root.frames[0].symbol.as_str(), root.frames[0].origin),
            ("java.lang.Integer.parseInt", FrameOrigin::Framework)
        );
        assert_eq!(
            (root.frames[0].image.as_deref(), root.frames[0].location.as_deref()),
            (Some("Integer.java"), Some("797"))
        );
        let mine = root.frames.iter().find(|f| f.symbol.ends_with("MainActivity.crash")).unwrap();
        assert_eq!((mine.origin, mine.location.as_deref()), (FrameOrigin::App, Some("53")));
        assert!(r.hints.iter().any(|h| h.command.as_deref() == Some("emulabzip logs --app dev.emulabzip.crashy")));
    }

    #[test]
    fn reads_a_native_crash_from_a_tombstone() {
        let e = one(NATIVE);
        let s = &summaries(std::slice::from_ref(&e), OFFSET, None)[0];
        assert_eq!(s.kind, CrashKind::NativeCrash);
        assert_eq!(s.title, "signal 11 (SIGSEGV), code 0 (SI_USER), fault addr --------");
        assert_eq!((s.app.as_deref(), s.pid), (Some("dev.emulabzip.crashy"), Some(3832)));

        let r = report(&e, OFFSET).unwrap();
        let t = &r.threads[0];
        assert!(t.crashed && t.name.contains("crashy"));
        assert!(t.frames.len() > 80, "{} frames", t.frames.len());
        let top = &t.frames[0];
        assert_eq!(
            (top.symbol.as_str(), top.origin, top.location.as_deref()),
            ("kill", FrameOrigin::Native, Some("0xb7f57"))
        );
        assert!(top.image.as_deref().is_some_and(|i| i.ends_with("libc.so")));
        let app = t.frames.iter().find(|f| f.symbol == "dev.emulabzip.crashy.MainActivity.run").unwrap();
        assert_eq!((app.index, app.origin, app.image.as_deref()), (14, FrameOrigin::App, None));
        let framework = t.frames.iter().find(|f| f.symbol == "android.app.Activity.performCreate").unwrap();
        assert_eq!(framework.origin, FrameOrigin::Framework);
        assert!(r.hints.iter().any(|h| h.text.contains("kill()")));
        assert!(r.details.iter().any(|(k, v)| k == "Signal" && v.starts_with("11 (SIGSEGV)")));
    }

    #[test]
    fn reads_an_anr_with_the_blocked_main_thread_first() {
        let e = one(ANR);
        let s = &summaries(std::slice::from_ref(&e), OFFSET, None)[0];
        assert_eq!(s.kind, CrashKind::Anr);
        assert!(s.title.starts_with("Input dispatching timed out"), "{}", s.title);
        assert!(!s.title.contains("Waited"), "the event is left out of the title");
        assert_eq!((s.app.as_deref(), s.pid), (Some("dev.emulabzip.crashy"), Some(5665)));

        let r = report(&e, OFFSET).unwrap();
        let main = &r.threads[0];
        assert!(main.crashed && main.name == "main");
        assert_eq!(main.frames[0].symbol, "java.lang.Thread.sleep");
        let mine = main.frames.iter().find(|f| f.symbol.contains("MainActivity.lambda$run$0")).unwrap();
        assert_eq!((mine.origin, mine.location.as_deref()), (FrameOrigin::App, Some("46")));
        assert!(r.threads.len() > 5, "the other threads of the process are there too");
        assert!(r.hints.iter().any(|h| h.text.contains("main thread was blocked")));
    }

    #[test]
    fn filters_by_app_and_sorts_newest_first() {
        let mut older = one(CRASH);
        older.date = "2026-10-22 10:00:00".into();
        let entries = [older, one(NATIVE), one(ANR)];
        let all = summaries(&entries, OFFSET, None);
        assert_eq!(
            all.iter().map(|s| s.kind).collect::<Vec<_>>(),
            [CrashKind::Anr, CrashKind::NativeCrash, CrashKind::Crash]
        );
        assert!(summaries(&entries, OFFSET, Some("com.other.app")).is_empty());
        assert_eq!(summaries(&entries, OFFSET, Some("dev.emulabzip.crashy")).len(), 3);
    }

    #[test]
    fn ignores_tags_that_are_not_crashes() {
        let mut e = one(CRASH);
        e.tag = "system_server_strictmode".into();
        assert!(summaries(&[e.clone()], OFFSET, None).is_empty());
        assert!(report(&e, OFFSET).is_err());
    }

    #[test]
    fn indexes_entries_by_id() {
        let entries = [one(CRASH)];
        assert!(index(&entries).contains_key("dropbox:data_app_crash:20261023205928"));
    }
}
