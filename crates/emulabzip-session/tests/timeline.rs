//! The timeline of a session written by hand, so every time is known. No device or daemon is involved.

use std::path::PathBuf;

use emulabzip_session::timeline::{self, Lane, Severity};
use emulabzip_session::{ClockSource, Session};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-timeline-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("crashes")).unwrap();
    dir
}

/// 2026-11-07 12:00:00 UTC.
const START: u64 = 1_794_052_800_000;

fn meta(clock: &str) -> String {
    format!(
        r#"{{"version":2,"tool":"t","device":{{"id":"x","name":"X","platform":"android","osVersion":null,"model":null}},
        "app":"com.example","startedMs":{START},"endedMs":{},"network":true,"screensEverySecs":null,"logsDropped":0{clock}}}"#,
        START + 20_000
    )
}

fn log(time: &str, level: &str, tag: &str, message: &str) -> String {
    format!(
        r#"{{"seq":1,"time":"{time}","pid":1,"tid":1,"level":"{level}","tag":"{tag}","message":"{message}","subsystem":null}}"#
    )
}

fn write(dir: &std::path::Path, meta: String, logs: &[String]) -> Session {
    std::fs::write(dir.join("session.json"), meta).unwrap();
    std::fs::write(dir.join("logs.jsonl"), logs.join("\n")).unwrap();
    std::fs::write(
        dir.join("input.jsonl"),
        format!(r#"{{"tMs":{},"kind":"tap","x":10,"y":20,"x2":10,"y2":20,"durationMs":80}}"#, START + 1000),
    )
    .unwrap();
    std::fs::write(dir.join("frames.jsonl"), format!(r#"{{"tMs":{},"durationMs":120.0}}"#, START + 2500)).unwrap();
    std::fs::write(
        dir.join("network.har"),
        r#"{"log":{"entries":[
          {"startedDateTime":"2026-11-07T12:00:03.000Z","time":250,"request":{"method":"GET","url":"https://api.example.com/items?page=1"},"response":{"status":200},"comment":""},
          {"startedDateTime":"2026-11-07T12:00:04.000Z","time":40,"request":{"method":"POST","url":"https://api.example.com/pay"},"response":{"status":500},"comment":""},
          {"startedDateTime":"2026-11-07T12:00:05.000Z","time":0,"request":{"method":"GET","url":"https://down.example.com/"},"response":{"status":0},"comment":"connection refused"}
        ]}}"#,
    )
    .unwrap();
    Session::open(dir).unwrap()
}

#[test]
fn every_stream_lands_on_the_same_axis() {
    let dir = scratch("axis");
    // The device is in UTC-3, and its clock is 2 s ahead of this computer's.
    let clock = r#","clock":{"offsetMs":2000,"tzMinutes":-180,"bootEpochMs":0}"#;
    let logs = [
        log("11-07 09:00:07.000", "error", "Net", "request failed"),
        log("11-07 09:00:08.000", "info", "Net", "fine"),
        log(
            "11-07 09:00:09.000",
            "warn",
            "art",
            "Background concurrent mark compact GC freed 5160KB AllocSpace bytes, 0(0B) LOS objects, 49% free, 6118KB/11MB, paused 2.134ms,4.023ms total 76.451ms",
        ),
    ];
    let session = write(&dir, meta(clock), &logs);
    let t = timeline::build(&session);
    assert_eq!(t.clock, ClockSource::Measured);
    assert_eq!(t.start_ms, START);

    let at = |lane: Lane| t.events.iter().filter(|e| e.lane == lane).collect::<Vec<_>>();
    // 09:00:07 in the device's zone is 12:00:07 UTC on its clock, which runs 2 s ahead: 5 s after the start.
    let errors = at(Lane::Logs);
    assert_eq!(
        errors.iter().map(|e| (e.t_ms, e.severity)).collect::<Vec<_>>(),
        vec![(5000, Severity::Error), (7000, Severity::Warning)]
    );
    assert_eq!(at(Lane::Taps)[0].t_ms, 1000);
    assert_eq!(at(Lane::Frames)[0].t_ms, 2500);
    let requests = at(Lane::Requests);
    assert_eq!(requests.iter().map(|e| e.t_ms).collect::<Vec<_>>(), vec![3000, 4000, 5000]);
    assert_eq!(
        requests.iter().map(|e| e.severity).collect::<Vec<_>>(),
        vec![Severity::Info, Severity::Error, Severity::Error]
    );
    assert_eq!(requests[2].detail.as_deref(), Some("connection refused"));
    assert!(requests[0].label.contains("api.example.com/items") && !requests[0].label.contains("page=1"));
    // The collection ended at 7 s and lasted 76 ms.
    let gc = at(Lane::Gc);
    assert_eq!((gc[0].t_ms, gc[0].dur_ms), (6924, Some(76)));
    // Sorted, and the summary counts every lane that has something.
    assert!(t.events.windows(2).all(|w| w[0].t_ms <= w[1].t_ms));
    assert_eq!(t.lanes.iter().find(|l| l.lane == Lane::Requests).unwrap().total, 3);
    assert_eq!(t.duration_ms, 20_000);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_session_without_a_clock_works_the_zone_out_from_its_first_line() {
    let dir = scratch("estimate");
    // Version 1 style: no clock. The device is in UTC-3 and a line was printed 1 s after the start.
    let logs = [log("11-07 09:00:01.000", "error", "Net", "boom")];
    let session = write(&dir, meta(""), &logs);
    let t = timeline::build(&session);
    assert_eq!(t.clock, ClockSource::Estimated);
    assert_eq!(t.events.iter().find(|e| e.lane == Lane::Logs).unwrap().t_ms, 1000);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_recording_over_new_year_picks_the_right_year() {
    let dir = scratch("newyear");
    // Started at 2026-12-31 23:59:50 UTC; the line was printed 20 s later, in January.
    let start = 1_798_761_590_000u64;
    let m = format!(
        r#"{{"version":2,"tool":"t","device":{{"id":"x","name":"X","platform":"android","osVersion":null,"model":null}},
        "app":null,"startedMs":{start},"endedMs":{},"network":false,"screensEverySecs":null,"logsDropped":0,
        "clock":{{"offsetMs":0,"tzMinutes":0,"bootEpochMs":0}}}}"#,
        start + 60_000
    );
    std::fs::write(dir.join("session.json"), m).unwrap();
    std::fs::write(dir.join("logs.jsonl"), log("01-01 00:00:10.000", "error", "T", "later")).unwrap();
    let t = timeline::build(&Session::open(&dir).unwrap());
    assert_eq!(t.events.iter().find(|e| e.lane == Lane::Logs).unwrap().t_ms, 20_000);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn lanes_can_be_picked() {
    let dir = scratch("pick");
    let session = write(&dir, meta(""), &[]);
    let t = timeline::build(&session);
    let only = t.only(&[Lane::Taps, Lane::Frames]);
    assert_eq!(only.len(), 2);
    assert_eq!(Lane::parse("GC"), Some(Lane::Gc));
    assert_eq!(Lane::parse("nope"), None);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_crowded_lane_is_cut_but_keeps_its_errors() {
    let dir = scratch("cap");
    let clock = r#","clock":{"offsetMs":0,"tzMinutes":-180,"bootEpochMs":0}"#;
    let mut logs: Vec<String> = (0..6000)
        .map(|i| log(&format!("11-07 09:00:{:02}.{:03}", (i / 1000) % 60, i % 1000), "warn", "T", "slow"))
        .collect();
    logs.push(log("11-07 09:00:19.000", "error", "T", "the real problem"));
    let session = write(&dir, meta(clock), &logs);
    let t = timeline::build(&session);
    let lane = t.lanes.iter().find(|l| l.lane == Lane::Logs).unwrap();
    assert_eq!(lane.total, 6001);
    assert!(lane.shown <= 3000 && lane.shown > 2000, "{}", lane.shown);
    assert!(t.events.iter().any(|e| e.severity == Severity::Error && e.label.contains("the real problem")));
    std::fs::remove_dir_all(dir).ok();
}
