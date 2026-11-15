//! Phase 14 against a real, disposable emulator: the proxy and its cleanup, a watch, a session, and a confirmed wipe.
//! `EMULABZIP_DEVICE=emulator-5560 cargo test -p emulabzip-cli --test mcp_live14 -- --ignored --test-threads=1`.
//! Only ever run this on an emulator made for it: the last test wipes it.

mod common;

use common::Server;
use serde_json::{Value, json};

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("EMULABZIP_DEVICE")
}

fn call(s: &mut Server, id: u32, name: &str, mut args: Value) -> Value {
    if args.get("device").is_none() {
        args["device"] = json!(device());
    }
    s.call(id, "tools/call", json!({ "name": name, "arguments": args }))
}

fn ok(v: &Value) -> String {
    assert_eq!(v["result"]["isError"], false, "{v}");
    v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn ok_text(v: &Value) -> String {
    v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn adb(args: &[&str]) -> String {
    let out = std::process::Command::new("adb").args(["-s", &device()]).args(args).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[ignore = "needs a disposable emulator"]
fn the_proxy_is_started_connected_and_always_put_back() {
    // Android clears a proxy by writing `:0`; an untouched device says `null`.
    let cleared = |v: &str| v.is_empty() || v == "null" || v == ":0";
    let before = adb(&["shell", "settings", "get", "global", "http_proxy"]);
    assert!(cleared(&before), "start from a device without a proxy: {before}");
    {
        let mut s = Server::start_session();
        let started: Value =
            serde_json::from_str(&ok(&call(&mut s, 2, "net_start", json!({ "port": 18088 })))).unwrap();
        assert!(started.to_string().contains("18088"), "{started}");
        ok(&call(&mut s, 3, "net_connect_device", json!({})));
        let during = adb(&["shell", "settings", "get", "global", "http_proxy"]);
        assert!(during.contains("18088"), "the device uses the proxy: {during}");
        let status = ok(&call(&mut s, 4, "net_status", json!({})));
        assert!(status.contains("18088"), "{status}");
        // The assistant disconnects without cleaning up: the server must.
        s.finish();
    }
    let after = adb(&["shell", "settings", "get", "global", "http_proxy"]);
    assert!(cleared(&after), "the device's proxy setting was put back, not left at {after}");
}

#[test]
#[ignore = "needs a disposable emulator"]
fn a_watch_follows_the_logs_and_a_session_is_recorded() {
    let mut s = Server::start_session();
    let started: Value = serde_json::from_str(&ok(&call(&mut s, 2, "watch_start", json!({ "kind": "logs" })))).unwrap();
    let id = started["watch"].clone();
    adb(&["shell", "log", "-t", "EmuLabLive14", "hello from the test"]);
    std::thread::sleep(std::time::Duration::from_secs(2));
    let read: Value =
        serde_json::from_str(&ok(&call(&mut s, 3, "watch_read", json!({ "id": id, "limit": 500 })))).unwrap();
    assert!(read.to_string().contains("hello from the test"), "{read}");
    ok(&call(&mut s, 4, "watch_stop", json!({ "id": id })));

    let name = format!("live14-{}", std::process::id());
    ok(&call(&mut s, 5, "session_start", json!({ "name": name, "video_seconds": 5 })));
    adb(&["shell", "input", "tap", "300", "600"]);
    std::thread::sleep(std::time::Duration::from_secs(3));
    let stopped = ok(&call(&mut s, 6, "session_stop", json!({})));
    assert!(stopped.contains(&name), "{stopped}");
    let summary = ok(&call(&mut s, 7, "session_summary", json!({ "id": name })));
    assert!(summary.contains("duration") || summary.contains("Duration"), "{summary}");
    let timeline = ok(&call(&mut s, 8, "session_timeline", json!({ "id": name })));
    assert!(!timeline.is_empty());
}

#[test]
#[ignore = "needs a disposable emulator"]
fn the_screen_and_the_facts_are_read() {
    let mut s = Server::start_session();
    let shot = call(&mut s, 2, "screenshot", json!({}));
    assert_eq!(
        shot["result"]["content"][0]["type"],
        "image",
        "{}",
        &shot.to_string()[..200.min(shot.to_string().len())]
    );
    let same = ok(&call(&mut s, 3, "compare_devices", json!({ "a": device(), "b": device() })));
    assert!(same.contains("\"differing\": 0"), "{same}");
    let options = ok(&call(&mut s, 4, "device_options", json!({ "platform": "android" })));
    assert!(options.contains("android"), "{options}");
}

#[test]
#[ignore = "needs a disposable emulator: it is wiped"]
fn a_wipe_is_declined_and_then_accepted_only_when_the_person_says_so() {
    let mut s = Server::start_session_with(&["--allow-writes"], Some("decline"));
    // A running device is refused before the person is asked.
    let r = call(&mut s, 2, "wipe_device", json!({ "device": "emulabzip-live14", "platform": "android" }));
    assert_eq!(r["result"]["isError"], true, "{r}");
    assert!(ok_text(&r).contains("running"), "{r}");
    assert!(s.asked.is_empty(), "{:?}", s.asked);
    ok(&call(&mut s, 3, "shutdown_device", json!({})));
    // Declined: the AVD still has its data.
    let r = call(&mut s, 4, "wipe_device", json!({ "device": "emulabzip-live14", "platform": "android" }));
    assert_eq!(r["result"]["isError"], true, "{r}");
    assert!(s.asked[0].contains("Wipe"), "{:?}", s.asked);
    s.finish();

    let mut s = Server::start_session_with(&["--allow-writes"], Some("accept"));
    let r = call(&mut s, 2, "wipe_device", json!({ "device": "emulabzip-live14", "platform": "android" }));
    assert_eq!(r["result"]["isError"], false, "{r}");
}
