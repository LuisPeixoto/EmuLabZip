//! Crash reports of apps in the iOS Simulator.
//!
//! macOS writes a `.ips` report to `~/Library/Logs/DiagnosticReports` when a process crashes,
//! simulated apps included. The file is two JSON documents: a one-line header, then the report
//! with the exception, every thread's frames and the images they point into. A report belongs to a
//! simulator when the app's path runs through `CoreSimulator/Devices/<udid>`.

use std::path::{Path, PathBuf};

use emulabzip_core::{CrashKind, CrashReport, CrashSummary, Error, Frame, FrameOrigin, Hint, Result, ThreadTrace};
use serde_json::Value;

/// Reports older than this are not listed, and at most this many files are read.
const MAX_FILES: usize = 200;
const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Folders macOS keeps crash reports in.
pub fn report_dirs() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else { return Vec::new() };
    let base = PathBuf::from(home).join("Library/Logs/DiagnosticReports");
    vec![base.join("Retired"), base]
}

/// An id for a file name: only plain characters are allowed in ids, so the rest become `_`.
pub fn id_for(file_name: &str) -> String {
    let clean: String =
        file_name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') { c } else { '_' }).collect();
    format!("ips:{clean}")
}

/// Splits a report into its header and its body.
fn split(text: &str) -> Result<(Value, Value)> {
    let (head, body) = text.split_once('\n').ok_or_else(|| Error::parse("crash report", "it has no body"))?;
    let head = serde_json::from_str(head).map_err(|e| Error::parse("crash report header", e.to_string()))?;
    let body = serde_json::from_str(body).map_err(|e| Error::parse("crash report", e.to_string()))?;
    Ok((head, body))
}

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

/// `2026-10-23 21:02:46.00 -0300` as milliseconds since the epoch.
pub fn parse_timestamp(stamp: &str) -> Option<u64> {
    let mut parts = stamp.split_whitespace();
    let (day, clock, zone) = (parts.next()?, parts.next()?, parts.next()?);
    let mut d = day.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, dd) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, "0"));
    let mut c = hms.split(':').map(|p| p.parse::<i64>().ok());
    let (h, mi, s) = (c.next()??, c.next()??, c.next()??);
    let zone_secs = {
        let sign = if zone.starts_with('-') { -1 } else { 1 };
        let digits = zone.trim_start_matches(['+', '-']);
        let (zh, zm) = (digits.get(..2)?.parse::<i64>().ok()?, digits.get(2..4)?.parse::<i64>().ok()?);
        sign * (zh * 3600 + zm * 60)
    };
    let secs = days_from_civil(y, m, dd) * 86_400 + h * 3600 + mi * 60 + s - zone_secs;
    let millis: u64 = format!("{frac:0<3}").get(..3)?.parse().ok()?;
    u64::try_from(secs).ok().map(|s| s * 1000 + millis)
}

fn origin_of(image: Option<&Value>) -> FrameOrigin {
    let name = image.and_then(|i| text(i, "name")).unwrap_or_default();
    let path = image.and_then(|i| text(i, "path")).unwrap_or_default();
    if path.contains(".app/") {
        FrameOrigin::App
    } else if path.contains(".framework/") || name.starts_with("libswift") {
        FrameOrigin::Framework
    } else {
        FrameOrigin::Native
    }
}

fn address_of(image: Option<&Value>, offset: u64) -> Option<u64> {
    image.and_then(|i| i.get("base")).and_then(Value::as_u64).map(|b| b + offset)
}

fn frames_of(thread: &Value, images: &[Value]) -> Vec<Frame> {
    thread
        .get("frames")
        .and_then(Value::as_array)
        .map(|frames| {
            frames
                .iter()
                .enumerate()
                .map(|(index, f)| {
                    let image = f.get("imageIndex").and_then(Value::as_u64).and_then(|i| images.get(i as usize));
                    let offset = f.get("imageOffset").and_then(Value::as_u64).unwrap_or(0);
                    let symbol = text(f, "symbol").unwrap_or_default().to_string();
                    let location = match (text(f, "sourceFile"), f.get("sourceLine").and_then(Value::as_u64)) {
                        (Some(file), Some(line)) => Some(format!("{}:{line}", file.rsplit('/').next().unwrap_or(file))),
                        _ if !symbol.is_empty() => {
                            f.get("symbolLocation").and_then(Value::as_u64).map(|n| format!("+{n}"))
                        }
                        _ => address_of(image, offset).map(|a| format!("{a:#x}")),
                    };
                    Frame {
                        index: index as u32,
                        symbol,
                        image: image.and_then(|i| text(i, "name")).map(str::to_string),
                        location,
                        origin: origin_of(image),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn thread_name(thread: &Value, index: usize) -> String {
    text(thread, "name")
        .or_else(|| text(thread, "queue"))
        .map(str::to_string)
        .unwrap_or_else(|| format!("Thread {index}"))
}

fn kind_of(body: &Value, faulting: Option<&Value>) -> CrashKind {
    let uncaught = faulting.and_then(|t| t.get("frames")).and_then(Value::as_array).is_some_and(|frames| {
        frames.iter().any(|f| matches!(text(f, "symbol"), Some("objc_exception_throw" | "__exceptionPreprocess")))
    }) || body.get("lastExceptionBacktrace").is_some();
    let signal = body.get("exception").and_then(|e| text(e, "signal")).unwrap_or_default();
    if uncaught || matches!(signal, "SIGABRT" | "SIGILL" | "SIGTRAP") {
        CrashKind::Crash
    } else {
        CrashKind::NativeCrash
    }
}

fn title_of(body: &Value, faulting: Option<&Value>, images: &[Value], kind: CrashKind) -> String {
    let exception = body.get("exception");
    let kind_text = exception.and_then(|e| text(e, "type")).unwrap_or("crash");
    let signal = exception.and_then(|e| text(e, "signal"));
    let mut title = match (kind, signal) {
        (CrashKind::Crash, _)
            if faulting.is_some_and(|t| frames_of(t, images).iter().any(|f| f.symbol == "objc_exception_throw")) =>
        {
            "Uncaught Objective-C exception".to_string()
        }
        (_, Some(s)) => format!("{kind_text} ({s})"),
        _ => kind_text.to_string(),
    };
    // The app's own function that was running tells more than the signal alone.
    if let Some(f) = faulting
        .map(|t| frames_of(t, images))
        .and_then(|f| f.into_iter().find(|f| f.origin == FrameOrigin::App && !f.symbol.is_empty()))
    {
        title.push_str(&format!(" in {}", f.symbol));
        if let Some(l) = f.location.filter(|l| l.contains(':')) {
            title.push_str(&format!(" ({l})"));
        }
    }
    title
}

/// Reads one report. `file_name` makes the id.
pub fn parse(text_: &str, file_name: &str) -> Result<CrashReport> {
    parse_with_path(text_, file_name).map(|(report, _)| report)
}

/// Like [`parse`], and also returns the path of the process, which says where the app lives.
fn parse_with_path(text_: &str, file_name: &str) -> Result<(CrashReport, String)> {
    let (head, body) = split(text_)?;
    let proc_path = text(&body, "procPath").unwrap_or_default().to_string();
    let images: Vec<Value> = body.get("usedImages").and_then(Value::as_array).cloned().unwrap_or_default();
    let threads = body.get("threads").and_then(Value::as_array).cloned().unwrap_or_default();
    let faulting_index = body.get("faultingThread").and_then(Value::as_u64).map(|i| i as usize);
    let faulting = faulting_index.and_then(|i| threads.get(i));
    let kind = kind_of(&body, faulting);

    let app = body
        .get("bundleInfo")
        .and_then(|b| text(b, "CFBundleIdentifier"))
        .or_else(|| text(&head, "bundleID"))
        .map(str::to_string);
    let process = text(&body, "procName").or_else(|| text(&head, "app_name")).unwrap_or("unknown").to_string();
    let summary = CrashSummary {
        id: id_for(file_name),
        kind,
        app: app.clone(),
        process,
        pid: body.get("pid").and_then(Value::as_u64).map(|p| p as u32),
        time_ms: text(&head, "timestamp").and_then(parse_timestamp),
        title: title_of(&body, faulting, &images, kind),
    };

    let mut traces: Vec<ThreadTrace> = threads
        .iter()
        .enumerate()
        .map(|(i, t)| ThreadTrace {
            name: thread_name(t, i),
            crashed: Some(i) == faulting_index,
            frames: frames_of(t, &images),
        })
        .filter(|t| t.crashed || !t.frames.is_empty())
        .collect();
    traces.sort_by_key(|t| !t.crashed);
    if let Some(last) = body.get("lastExceptionBacktrace").and_then(Value::as_array) {
        let holder = serde_json::json!({ "frames": last });
        traces.insert(
            1.min(traces.len()),
            ThreadTrace {
                name: "Last exception backtrace".into(),
                crashed: false,
                frames: frames_of(&holder, &images),
            },
        );
    }

    let mut details = Vec::new();
    let mut add = |k: &str, v: Option<String>| {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            details.push((k.to_string(), v));
        }
    };
    let exception = body.get("exception");
    add("Exception", exception.and_then(|e| text(e, "type")).map(str::to_string));
    add("Signal", exception.and_then(|e| text(e, "signal")).map(str::to_string));
    add("Subtype", exception.and_then(|e| text(e, "subtype")).map(str::to_string));
    add("Codes", exception.and_then(|e| text(e, "codes")).map(str::to_string));
    add("Termination", body.get("termination").and_then(|t| text(t, "indicator")).map(str::to_string));
    add(
        "App version",
        body.get("bundleInfo").map(|b| {
            format!(
                "{} ({})",
                text(b, "CFBundleShortVersionString").unwrap_or("?"),
                text(b, "CFBundleVersion").unwrap_or("?")
            )
        }),
    );
    add("Architecture", images.iter().find_map(|i| text(i, "arch")).map(str::to_string));

    let hints = hints_of(&traces, &images, app.as_deref());
    Ok((CrashReport { summary, details, threads: traces, hints, raw: text_.to_string() }, proc_path))
}

/// Whether an app at `proc_path` lives in the simulator with this UDID. (The path in the report
/// is the one the process ran from, so it runs through that simulator's folder.)
fn is_in(proc_path: &str, udid: &str) -> bool {
    proc_path.contains(&format!("CoreSimulator/Devices/{udid}/"))
}

fn hints_of(threads: &[ThreadTrace], images: &[Value], app: Option<&str>) -> Vec<Hint> {
    let mut hints = Vec::new();
    // Frames of the app without a symbol need its dSYM.
    let blank = threads
        .iter()
        .find(|t| t.crashed)
        .is_some_and(|t| t.frames.iter().any(|f| f.origin == FrameOrigin::App && f.symbol.is_empty()));
    if blank {
        let (arch, base) = images
            .iter()
            .find(|i| text(i, "path").is_some_and(|p| p.contains(".app/")))
            .map(|i| {
                (text(i, "arch").unwrap_or("arm64").to_string(), i.get("base").and_then(Value::as_u64).unwrap_or(0))
            })
            .unwrap_or_else(|| ("arm64".into(), 0));
        hints.push(Hint {
            text: "Some frames of the app have no symbols. Resolve an address with the dSYM of this exact build (the UUIDs must match).".into(),
            command: Some(format!("atos -o <App.app.dSYM>/Contents/Resources/DWARF/<App> -arch {arch} -l {base:#x} <address>")),
        });
    }
    if let Some(app) = app {
        hints.push(Hint {
            text: "See what the app logged around the crash.".into(),
            command: Some(format!("emulabzip logs --app {app}")),
        });
    }
    hints
}

/// Reads the reports of one simulator, newest first.
pub fn list_in(dirs: &[PathBuf], udid: &str, app: Option<&str>) -> Vec<CrashSummary> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "ips"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            (meta.is_file() && meta.len() <= MAX_BYTES)
                .then(|| (meta.modified().unwrap_or(std::time::UNIX_EPOCH), e.path()))
        })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.truncate(MAX_FILES);
    let mut all: Vec<CrashSummary> = files
        .into_iter()
        .filter_map(|(_, path)| {
            let text = std::fs::read_to_string(&path).ok()?;
            // A cheap check before parsing: the UDID has to appear somewhere.
            if !text.contains(udid) {
                return None;
            }
            let (report, proc_path) = parse_with_path(&text, path.file_name()?.to_str()?).ok()?;
            if !is_in(&proc_path, udid) {
                return None;
            }
            app.is_none_or(|a| report.summary.app.as_deref() == Some(a)).then_some(report.summary)
        })
        .collect();
    all.sort_by(|a, b| b.time_ms.cmp(&a.time_ms).then_with(|| b.id.cmp(&a.id)));
    all
}

pub fn list(udid: &str, app: Option<&str>) -> Vec<CrashSummary> {
    list_in(&report_dirs(), udid, app)
}

pub fn get_in(dirs: &[PathBuf], udid: &str, id: &str) -> Result<CrashReport> {
    for dir in dirs {
        let Ok(read) = std::fs::read_dir(dir) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            if !name.ends_with(".ips") || id_for(name) != id {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            let (report, proc_path) = parse_with_path(&text, name)?;
            if is_in(&proc_path, udid) {
                return Ok(report);
            }
        }
    }
    Err(Error::NotFound(format!("crash {id}")))
}

pub fn get(udid: &str, id: &str) -> Result<CrashReport> {
    get_in(&report_dirs(), udid, id)
}

/// Where a report would be, for messages.
pub fn dir_hint() -> PathBuf {
    report_dirs().pop().unwrap_or_else(|| Path::new("~/Library/Logs/DiagnosticReports").to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FATAL: &str = include_str!("../tests/fixtures/crash-swift-fatal-error.ips");
    const EXCEPTION: &str = include_str!("../tests/fixtures/crash-objc-exception.ips");
    const UDID: &str = "F8CE84C1-04D1-4E33-B52B-02B59E8A09C5";

    #[test]
    fn reads_a_swift_runtime_failure() {
        let r = parse(FATAL, "Crashy-2026-10-23-210246.ips").unwrap();
        let s = &r.summary;
        assert_eq!(
            (s.kind, s.app.as_deref(), s.process.as_str(), s.pid),
            (CrashKind::Crash, Some("dev.emulabzip.crashy"), "Crashy", Some(62022))
        );
        assert_eq!(s.title, "EXC_BAD_INSTRUCTION (SIGILL) in explode(depth:) (main.swift:38)");
        assert_eq!(s.id, "ips:Crashy-2026-10-23-210246.ips");
        // 21:02:46 at UTC-3 is 00:02:46 the next day in UTC.
        assert_eq!(s.time_ms, Some(1_792_800_166_000));

        let main = &r.threads[0];
        assert!(main.crashed && main.name == "com.apple.main-thread");
        assert_eq!(main.frames[0].symbol, "_assertionFailure(_:_:file:line:flags:)");
        assert_eq!(
            (main.frames[0].origin, main.frames[0].image.as_deref()),
            (FrameOrigin::Framework, Some("libswiftCore.dylib"))
        );
        let explode = &main.frames[1];
        assert_eq!(
            (explode.symbol.as_str(), explode.origin, explode.location.as_deref()),
            ("explode(depth:)", FrameOrigin::App, Some("main.swift:38"))
        );
        assert_eq!(
            main.frames.iter().filter(|f| f.symbol == "explode(depth:)").count(),
            4,
            "three recursive calls and the first"
        );
        let dispatch = main.frames.iter().find(|f| f.symbol == "_dispatch_client_callout").unwrap();
        assert_eq!((dispatch.origin, dispatch.location.as_deref()), (FrameOrigin::Native, Some("+6")));
        assert!(r.details.iter().any(|(k, v)| k == "Signal" && v == "SIGILL"));
        assert!(r.details.iter().any(|(k, v)| k == "App version" && v == "1.0 (1)"));
        assert!(r.hints.iter().any(|h| h.command.as_deref() == Some("emulabzip logs --app dev.emulabzip.crashy")));
    }

    #[test]
    fn recognizes_an_uncaught_objective_c_exception() {
        let r = parse(EXCEPTION, "Crashy-2026-10-23-210250.ips").unwrap();
        assert_eq!(r.summary.kind, CrashKind::Crash);
        assert!(
            r.summary.title.starts_with("Uncaught Objective-C exception in closure #1 in AppDelegate"),
            "{}",
            r.summary.title
        );
        let frames = &r.threads[0].frames;
        assert!(frames.iter().any(|f| f.symbol == "objc_exception_throw"));
        assert!(frames.iter().any(|f| f.symbol == "-[NSException raise]" && f.origin == FrameOrigin::Framework));
    }

    #[test]
    fn leaves_out_threads_that_have_no_frames() {
        let r = parse(FATAL, "x.ips").unwrap();
        assert!(r.threads.iter().all(|t| !t.frames.is_empty()));
        assert!(r.threads[0].crashed && r.threads.iter().filter(|t| t.crashed).count() == 1);
    }

    #[test]
    fn rejects_text_that_is_not_a_report() {
        assert!(parse("", "x.ips").is_err());
        assert!(parse("{}", "x.ips").is_err());
        assert!(parse("{}\nnot json", "x.ips").is_err());
    }

    #[test]
    fn parses_the_time_in_any_zone() {
        assert_eq!(parse_timestamp("1970-01-01 00:00:01.50 +0000"), Some(1_500));
        assert_eq!(parse_timestamp("1970-01-01 05:30:00.00 +0530"), Some(0));
        assert_eq!(parse_timestamp("nope"), None);
    }

    #[test]
    fn makes_plain_ids_from_file_names() {
        assert_eq!(id_for("My App-2026-10-23-210246.ips"), "ips:My_App-2026-10-23-210246.ips");
        assert!(emulabzip_core::crash::is_valid_id(&id_for("A (1) – b.ips")));
    }

    #[test]
    fn lists_only_the_reports_of_one_simulator() {
        let dir = std::env::temp_dir().join(format!("emulabzip-ips-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Retired")).unwrap();
        std::fs::write(dir.join("Crashy-1.ips"), FATAL).unwrap();
        std::fs::write(dir.join("Retired").join("Crashy-2.ips"), EXCEPTION).unwrap();
        // Another simulator's report, and a file that is not one.
        std::fs::write(
            dir.join("Other.ips"),
            FATAL.replace(
                &format!("CoreSimulator\\/Devices\\/{UDID}"),
                "CoreSimulator\\/Devices\\/00000000-0000-0000-0000-000000000000",
            ),
        )
        .unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let dirs = [dir.join("Retired"), dir.clone()];

        let all = list_in(&dirs, UDID, None);
        assert_eq!(all.len(), 2);
        assert!(all[0].time_ms >= all[1].time_ms);
        assert_eq!(list_in(&dirs, UDID, Some("dev.emulabzip.crashy")).len(), 2);
        assert!(list_in(&dirs, UDID, Some("com.other")).is_empty());
        assert!(list_in(&dirs, "FFFFFFFF-0000-0000-0000-000000000000", None).is_empty());

        let report = get_in(&dirs, UDID, "ips:Crashy-1.ips").unwrap();
        assert_eq!(report.summary.process, "Crashy");
        assert!(
            matches!(get_in(&dirs, UDID, "ips:Other.ips"), Err(Error::NotFound(_))),
            "another simulator's report is not served"
        );
        assert!(matches!(get_in(&dirs, UDID, "ips:missing.ips"), Err(Error::NotFound(_))));
        std::fs::remove_dir_all(dir).ok();
    }
}
