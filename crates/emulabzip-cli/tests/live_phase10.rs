//! Video, touches, the timeline and replay on a real device. Opt in with `--ignored`:
//!
//! ```bash
//! tools/compose-app/install.sh                 # the demo with the two fetch buttons
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_phase10 -- --ignored --nocapture --test-threads=1
//! ```
//!
//! They need an Android emulator (touches are sent through its console) and the internet for the first fetch.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const APP: &str = "dev.emulabzip.composedemo";

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-live10-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn adb(args: &[&str]) -> String {
    let out = Command::new("adb").args(["-s", &device()]).args(args).output().unwrap();
    assert!(out.status.success(), "adb {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn emulabzip_command(home: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_emulabzip"));
    cmd.env("EMULABZIP_HOME", home).args(["-d", &device()]).args(args);
    cmd
}

fn emulabzip(home: &Path, args: &[&str]) -> (bool, String, String) {
    let out = emulabzip_command(home, args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn pause(secs: f64) {
    std::thread::sleep(Duration::from_secs_f64(secs));
}

/// What the "Server said" line of the demo shows right now.
fn said() -> String {
    adb(&["shell", "uiautomator dump /sdcard/ui.xml >/dev/null; cat /sdcard/ui.xml"])
        .split("Server said: ")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn start_the_demo() {
    adb(&["shell", "am", "force-stop", APP]);
    adb(&["shell", &format!("am start -n {APP}/.MainActivity >/dev/null")]);
    pause(3.0);
}

/// A finger on the screen, through the emulator's console, which reaches the touch device like a real one.
fn touch(x: u32, y: u32) {
    let send = |args: &[&str]| {
        let out = Command::new("adb").args(["-s", &device(), "emu", "event", "send"]).args(args).output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("OK"), "{}", String::from_utf8_lossy(&out.stdout));
    };
    send(&[
        "EV_ABS:ABS_MT_TRACKING_ID:9",
        &format!("EV_ABS:ABS_MT_POSITION_X:{}", x * 32767 / 1080),
        &format!("EV_ABS:ABS_MT_POSITION_Y:{}", y * 32767 / 2400),
        "EV_KEY:BTN_TOUCH:1",
        "EV_SYN:0:0",
    ]);
    pause(0.12);
    send(&["EV_ABS:ABS_MT_TRACKING_ID:-1", "EV_KEY:BTN_TOUCH:0", "EV_SYN:0:0"]);
}

#[test]
#[ignore = "needs a running Android emulator with the Compose demo (tools/compose-app/install.sh)"]
fn video_touches_and_requests_land_on_one_timeline() {
    let home = home("timeline");
    start_the_demo();
    let out = home.join("run.ezsession");
    let mut recording = emulabzip_command(
        &home,
        &["session", "record", "-o", out.to_str().unwrap(), "--app", APP, "--video", "8", "--taps", "--duration", "22"],
    )
    .spawn()
    .unwrap();
    pause(5.0);
    touch(222, 274); // the "Fetch the time" button, by a real touch
    pause(3.0);
    touch(600, 1500);
    assert!(recording.wait().unwrap().success());

    let (ok, json, err) = emulabzip(&home, &["session", "timeline", out.to_str().unwrap(), "--json"]);
    assert!(ok, "{err}");
    let t: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(t["clock"], "measured");
    let lane = |name: &str| -> Vec<&serde_json::Value> {
        t["events"].as_array().unwrap().iter().filter(|e| e["lane"] == name).collect()
    };
    let taps = lane("taps");
    assert!(taps.len() >= 2, "both touches are on the timeline: {taps:?}");
    // The first touch was five seconds in, give or take the time the recorder took to start.
    let first = taps[0]["tMs"].as_u64().unwrap();
    assert!((3_000..9_000).contains(&first), "{first} ms");
    let video = lane("video");
    assert!(!video.is_empty(), "the screen was recorded");
    // The pieces run one after the other, and the first starts near the beginning.
    assert!(video[0]["tMs"].as_u64().unwrap() < 3_000, "{video:?}");

    // Every piece is a movie file.
    let folder = home.join("pieces");
    let (ok, _, err) = emulabzip(&home, &["session", "video", out.to_str().unwrap(), "-o", folder.to_str().unwrap()]);
    assert!(ok, "{err}");
    let first_piece = std::fs::read(folder.join("0001.mp4")).unwrap();
    assert_eq!(&first_piece[4..8], b"ftyp", "an mp4 starts with its file type");
    assert!(first_piece.len() > 10_000);
}

#[test]
#[ignore = "needs a running Android emulator with the Compose demo, and the internet for the recording"]
fn a_recorded_session_is_replayed_without_the_server() {
    let home = home("replay");
    start_the_demo();
    let out = home.join("run.ezsession");
    let mut recording = emulabzip_command(
        &home,
        &[
            "session",
            "record",
            "-o",
            out.to_str().unwrap(),
            "--app",
            APP,
            "--network",
            "--port",
            "9292",
            "--duration",
            "16",
        ],
    )
    .spawn()
    .unwrap();
    pause(5.0);
    adb(&["shell", "input", "tap", "222", "274"]);
    pause(2.5);
    let recorded = said();
    assert!(recorded.parse::<u64>().is_ok(), "the first answer is the time in seconds: {recorded:?}");
    assert!(recording.wait().unwrap().success());

    // Time moves on, so a live answer would differ. Replayed, the app gets what was recorded.
    pause(2.0);
    let mut replay =
        emulabzip_command(&home, &["net", "--replay", out.to_str().unwrap(), "--port", "9292"]).spawn().unwrap();
    pause(4.0);
    start_the_demo();
    adb(&["shell", "input", "tap", "222", "274"]);
    pause(2.5);
    assert_eq!(said(), recorded, "the app got the recorded answer");
    // A page the recording does not hold fails, and does not reach the internet.
    adb(&["shell", "input", "tap", "643", "274"]);
    pause(2.5);
    assert_eq!(said(), "HTTP 504");

    let _ = Command::new("kill").args(["-INT", &replay.id().to_string()]).status();
    assert!(replay.wait().unwrap().success());
    // The device is put back.
    assert_eq!(adb(&["shell", "settings", "get", "global", "http_proxy"]).trim(), ":0");
}

#[test]
#[ignore = "needs a running Android emulator with the Compose demo (tools/compose-app/install.sh)"]
fn the_report_of_a_recording_has_a_clip_of_the_screen() {
    let home = home("report");
    start_the_demo();
    let out = home.join("run.ezsession");
    let mut recording = emulabzip_command(
        &home,
        &["session", "record", "-o", out.to_str().unwrap(), "--app", APP, "--video", "6", "--duration", "10"],
    )
    .spawn()
    .unwrap();
    pause(2.0);
    adb(&["shell", "input", "tap", "222", "122"]);
    assert!(recording.wait().unwrap().success());

    let html = home.join("report.html");
    let (ok, _, err) = emulabzip(&home, &["report", "--session", out.to_str().unwrap(), "-o", html.to_str().unwrap()]);
    assert!(ok, "{err}");
    let page = std::fs::read_to_string(&html).unwrap();
    assert!(
        page.contains("<video controls") && page.contains("data:video/mp4;base64,"),
        "the clip is part of the file"
    );
    assert!(!page.contains("<script"));
}
