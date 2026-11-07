//! A report made from a session that has screen recording: the clip around the crash, and the last minute as a strip.

use std::path::{Path, PathBuf};

use emulabzip_core::{CrashKind, CrashReport, CrashSummary};
use emulabzip_report::html::html;
use emulabzip_report::{BugReport, Options};
use emulabzip_session::Session;
use emulabzip_symbols::store::Store;

const START: u64 = 1_794_052_800_000;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-report-video-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("crashes")).unwrap();
    std::fs::create_dir_all(dir.join("video")).unwrap();
    dir
}

fn crash(at_ms: u64) -> CrashReport {
    CrashReport {
        summary: CrashSummary {
            id: "c1".into(),
            kind: CrashKind::Crash,
            app: Some("com.example".into()),
            process: "com.example".into(),
            pid: Some(1),
            time_ms: Some(at_ms),
            title: "java.lang.IllegalStateException: boom".into(),
        },
        details: vec![],
        threads: vec![],
        hints: vec![],
        app_version: None,
        raw: String::new(),
    }
}

/// A 3-piece recording of 90 s. The crash, when there is one, happens 50 s in (the second piece).
fn write(dir: &Path, with_crash: bool) -> Session {
    std::fs::write(
        dir.join("session.json"),
        format!(
            r#"{{"version":2,"tool":"t","device":{{"id":"x","name":"Pixel","platform":"android","osVersion":"14","model":null}},
            "app":"com.example","startedMs":{START},"endedMs":{},"network":false,"screensEverySecs":null,"logsDropped":0,
            "clock":{{"offsetMs":0,"tzMinutes":0,"bootEpochMs":0}}}}"#,
            START + 90_000
        ),
    )
    .unwrap();
    let mut index = String::new();
    for (i, from) in [0u64, 30, 60].into_iter().enumerate() {
        let file = format!("{:04}.mp4", i + 1);
        std::fs::write(dir.join("video").join(&file), format!("clip {}", i + 1)).unwrap();
        index += &format!(
            "{{\"file\":\"{file}\",\"startMs\":{},\"endMs\":{}}}\n",
            START + from * 1000,
            START + (from + 30) * 1000
        );
    }
    std::fs::write(dir.join("video/index.jsonl"), index).unwrap();
    std::fs::write(
        dir.join("input.jsonl"),
        format!(r#"{{"tMs":{},"kind":"tap","x":5,"y":6,"x2":5,"y2":6,"durationMs":70}}"#, START + 48_000),
    )
    .unwrap();
    if with_crash {
        std::fs::write(dir.join("crashes/c1.json"), serde_json::to_vec(&crash(START + 50_000)).unwrap()).unwrap();
    }
    Session::open(dir).unwrap()
}

fn store(dir: &Path) -> Store {
    Store::at(dir.join("mappings"))
}

#[test]
fn the_clip_with_the_crash_and_the_minute_before_it() {
    let dir = scratch("crash");
    let session = write(&dir, true);
    let report = BugReport::from_session(&session, &Options::everything(), &store(&dir));

    let clip = report.video.as_ref().expect("a clip");
    assert_eq!(clip.bytes, b"clip 2");
    assert!(clip.label.contains("around the crash") && clip.label.contains("piece 2 of 3"), "{}", clip.label);

    let strip = report.strip.as_ref().expect("a strip");
    assert!(strip.ends_at_crash);
    // The crash is 50 s in, so the window starts with the recording and is 50 s long.
    assert_eq!(strip.window_ms, 50_000);
    let lanes: Vec<&str> = strip.marks.iter().map(|m| m.lane.as_str()).collect();
    assert!(lanes.contains(&"taps") && lanes.contains(&"crashes"), "{lanes:?}");
    let tap = strip.marks.iter().find(|m| m.lane == "taps").unwrap();
    assert_eq!(tap.t_ms, 48_000);

    let page = html(&report);
    assert!(page.contains("Screen recording") && page.contains("Timeline, the last minute"));
    assert!(report.video_sidecar().is_none(), "a small clip is part of the file");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn without_a_crash_the_last_piece_and_the_end_of_the_run() {
    let dir = scratch("nocrash");
    let session = write(&dir, false);
    let report = BugReport::from_session(&session, &Options::everything(), &store(&dir));
    let clip = report.video.as_ref().unwrap();
    assert_eq!(clip.bytes, b"clip 3");
    assert!(clip.label.contains("the last piece of the run"), "{}", clip.label);
    let strip = report.strip.as_ref().unwrap();
    assert!(!strip.ends_at_crash);
    assert_eq!(strip.window_ms, 60_000);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn video_can_be_left_out() {
    let dir = scratch("off");
    let session = write(&dir, true);
    let options = Options { include_video: false, ..Options::everything() };
    let report = BugReport::from_session(&session, &options, &store(&dir));
    assert!(report.video.is_none());
    assert!(!html(&report).contains("<video"));
    std::fs::remove_dir_all(dir).ok();
}
