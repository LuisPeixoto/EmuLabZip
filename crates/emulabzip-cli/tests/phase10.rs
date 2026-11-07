//! The timeline, screen video and replay without a device: the arguments are checked first, and a session on disk needs
//! nothing else.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-phase10-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_emulabzip")).env("EMULABZIP_HOME", home).args(args).output().unwrap()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

const START: u64 = 1_794_052_800_000;

/// A recording of 90 s with a tap, a failed request, a slow frame, a crash and three pieces of video, as a folder.
fn session(dir: &Path) -> PathBuf {
    let folder = dir.join("run.ezsession.partial");
    std::fs::create_dir_all(folder.join("crashes")).unwrap();
    std::fs::create_dir_all(folder.join("video")).unwrap();
    std::fs::write(
        folder.join("session.json"),
        format!(
            r#"{{"version":2,"tool":"t","device":{{"id":"x","name":"Pixel","platform":"android","osVersion":"14","model":null}},
            "app":"com.example","startedMs":{START},"endedMs":{},"network":true,"screensEverySecs":null,"logsDropped":0,
            "clock":{{"offsetMs":0,"tzMinutes":0,"bootEpochMs":0}}}}"#,
            START + 90_000
        ),
    )
    .unwrap();
    std::fs::write(
        folder.join("input.jsonl"),
        format!(r#"{{"tMs":{},"kind":"tap","x":5,"y":6,"x2":5,"y2":6,"durationMs":70}}"#, START + 48_000),
    )
    .unwrap();
    std::fs::write(folder.join("frames.jsonl"), format!(r#"{{"tMs":{},"durationMs":96.0}}"#, START + 49_000)).unwrap();
    std::fs::write(
        folder.join("network.har"),
        r#"{"log":{"entries":[
          {"startedDateTime":"2026-11-07T12:00:49.500Z","time":300,"request":{"method":"GET","url":"https://api.example.com/items"},
           "response":{"status":200,"headers":[{"name":"content-type","value":"application/json"}],"content":{"mimeType":"application/json","text":"{\"items\":[1,2]}"}},"timings":{"wait":100}},
          {"startedDateTime":"2026-11-07T12:00:49.800Z","time":40,"request":{"method":"POST","url":"https://api.example.com/pay"},
           "response":{"status":500,"headers":[],"content":{"text":"fail"}},"comment":""}
        ]}}"#,
    )
    .unwrap();
    std::fs::write(
        folder.join("crashes/c1.json"),
        format!(
            r#"{{"summary":{{"id":"c1","kind":"crash","app":"com.example","process":"com.example","pid":1,"timeMs":{},"title":"boom"}},
            "details":[],"threads":[],"hints":[],"raw":""}}"#,
            START + 50_000
        ),
    )
    .unwrap();
    let mut index = String::new();
    for (i, from) in [0u64, 30, 60].into_iter().enumerate() {
        let file = format!("{:04}.mp4", i + 1);
        std::fs::write(folder.join("video").join(&file), format!("clip {}", i + 1)).unwrap();
        index += &format!(
            "{{\"file\":\"{file}\",\"startMs\":{},\"endMs\":{}}}\n",
            START + from * 1000,
            START + (from + 30) * 1000
        );
    }
    std::fs::write(folder.join("video/index.jsonl"), index).unwrap();
    folder
}

#[test]
fn the_timeline_lists_everything_in_time_order() {
    let dir = scratch("timeline");
    let folder = session(&dir);
    let o = run(&dir, &["session", "timeline", folder.to_str().unwrap()]);
    assert!(o.status.success(), "{}", err(&o));
    let text = out(&o);
    let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle} is not in\n{text}"));
    assert!(at("+00:48.000  taps") < at("+00:49.000  frames"), "{text}");
    assert!(at("+00:49.000  frames") < at("requests"), "{text}");
    assert!(
        text.contains("Tap (5, 6)") && text.contains("Slow frame, 96 ms") && text.contains("api.example.com/items")
    );
    assert!(text.contains("+00:50.000  crashes   boom"), "{text}");
    assert!(err(&o).contains("1 taps") && err(&o).contains("2 requests"), "{}", err(&o));
}

#[test]
fn the_timeline_can_be_limited_to_lanes_and_printed_as_json() {
    let dir = scratch("lanes");
    let folder = session(&dir);
    let o = run(&dir, &["session", "timeline", folder.to_str().unwrap(), "--lane", "requests", "--lane", "crashes"]);
    let text = out(&o);
    assert!(text.contains("requests") && text.contains("crashes") && !text.contains("taps"), "{text}");

    let o = run(&dir, &["session", "timeline", folder.to_str().unwrap(), "--json", "--lane", "taps"]);
    let json: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(json["events"].as_array().unwrap().len(), 1);
    assert_eq!(json["events"][0]["lane"], "taps");
    assert_eq!(json["events"][0]["tMs"], 48_000);
    assert_eq!(json["clock"], "measured");

    let o = run(&dir, &["session", "timeline", folder.to_str().unwrap(), "--lane", "nope"]);
    assert!(!o.status.success() && err(&o).contains("taps, logs, requests"), "{}", err(&o));
}

#[test]
fn the_pieces_of_video_are_written_to_a_folder() {
    let dir = scratch("video");
    let folder = session(&dir);
    let target = dir.join("out");
    let o = run(&dir, &["session", "video", folder.to_str().unwrap(), "-o", target.to_str().unwrap()]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(std::fs::read(target.join("0002.mp4")).unwrap(), b"clip 2");
    assert!(out(&o).contains("+00:30.000 to +01:00.000"), "{}", out(&o));

    // A session with no video says how to record one.
    std::fs::remove_dir_all(folder.join("video")).unwrap();
    let o = run(&dir, &["session", "video", folder.to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("--video"), "{}", err(&o));
}

#[test]
fn the_report_of_a_session_has_the_clip_around_the_crash_and_can_leave_it_out() {
    let dir = scratch("report");
    let folder = session(&dir);
    let html = dir.join("report.html");
    let o = run(&dir, &["report", "--session", folder.to_str().unwrap(), "-o", html.to_str().unwrap()]);
    assert!(o.status.success(), "{}", err(&o));
    let page = std::fs::read_to_string(&html).unwrap();
    assert!(page.contains("<video controls") && page.contains("Timeline, the last minute"), "no video in the report");
    assert!(page.contains("piece 2 of 3"), "the clip with the crash is picked");
    assert!(!dir.join("emulabzip-report.mp4").exists(), "a small clip is part of the file");

    let o = run(
        &dir,
        &["report", "--session", folder.to_str().unwrap(), "-o", html.to_str().unwrap(), "--without", "video"],
    );
    assert!(o.status.success(), "{}", err(&o));
    assert!(!std::fs::read_to_string(&html).unwrap().contains("<video"));
}

#[test]
fn replay_checks_its_arguments_before_any_device_is_looked_for() {
    let dir = scratch("args");
    // A file that is not there.
    let o = run(&dir, &["net", "--replay", dir.join("missing.har").to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("could not open"), "{}", err(&o));
    // Not a recording.
    let junk = dir.join("junk.har");
    std::fs::write(&junk, "nope").unwrap();
    let o = run(&dir, &["net", "--replay", junk.to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("not a JSON"), "{}", err(&o));
    // The flags that only make sense with a recording.
    let o = run(&dir, &["net", "--passthrough"]);
    assert!(!o.status.success() && err(&o).contains("--replay"), "{}", err(&o));
    // A session with no network cannot be replayed, and says how to record one.
    let folder = session(&dir);
    std::fs::remove_file(folder.join("network.har")).unwrap();
    let o = run(&dir, &["net", "--replay", folder.to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("--network"), "{}", err(&o));
}

#[test]
fn recording_and_ci_check_replay_and_video_before_any_device_is_looked_for() {
    let dir = scratch("record");
    let har = dir.join("capture.har");
    std::fs::write(&har, r#"{"log":{"entries":[]}}"#).unwrap();
    // --replay needs --network.
    let out_file = dir.join("run.ezsession");
    let o = run(&dir, &["session", "record", "-o", out_file.to_str().unwrap(), "--replay", har.to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("--network"), "{}", err(&o));
    // An empty recording cannot answer anything.
    let o = run(
        &dir,
        &["session", "record", "-o", out_file.to_str().unwrap(), "--network", "--replay", har.to_str().unwrap()],
    );
    assert!(!o.status.success() && err(&o).contains("no request that can be replayed"), "{}", err(&o));
    // Video pieces are 1 to 180 seconds.
    let o = run(&dir, &["session", "record", "-o", out_file.to_str().unwrap(), "--video", "0"]);
    assert!(!o.status.success() && err(&o).contains("1 to 180"), "{}", err(&o));
    let o = run(&dir, &["session", "record", "-o", out_file.to_str().unwrap(), "--video", "999"]);
    assert!(!o.status.success() && err(&o).contains("1 to 180"), "{}", err(&o));
    // ci: --replay needs --network.
    let o = run(&dir, &["ci", "-o", out_file.to_str().unwrap(), "--replay", har.to_str().unwrap(), "--", "true"]);
    assert!(!o.status.success() && err(&o).contains("--network"), "{}", err(&o));
}
