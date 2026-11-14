//! Phase 13 on a real Android emulator: WebViews, input, flows, deep links and network rules. Opt in with `--ignored`:
//!
//! ```bash
//! tools/compose-app/install.sh                 # the demo, with its WebView screen and app links
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_phase13 -- --ignored --nocapture --test-threads=1
//! ```
//!
//! They tap, type and press keys on the device, set its proxy, and need the internet (the site's `assetlinks.json`,
//! `time.akamai.com`, `example.com`). Use a throwaway emulator.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const APP: &str = "dev.emulabzip.composedemo";

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-live13-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn adb(args: &[&str]) -> String {
    let out = Command::new("adb").args(["-s", &device()]).args(args).output().unwrap();
    assert!(out.status.success(), "adb {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn command(home: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_emulabzip"));
    cmd.env("EMULABZIP_HOME", home).args(["-d", &device()]).args(args);
    cmd
}

fn emulabzip(home: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = command(home, args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ok(home: &Path, args: &[&str]) -> String {
    let (code, out, err) = emulabzip(home, args);
    assert_eq!(code, Some(0), "emulabzip {args:?} failed: {err}");
    out
}

fn pause(secs: f64) {
    std::thread::sleep(Duration::from_secs_f64(secs));
}

fn start(activity: &str) {
    adb(&["shell", "am", "force-stop", APP]);
    adb(&["shell", &format!("am start -n {APP}/{activity} >/dev/null")]);
    pause(4.0);
}

/// Every view of the screen with its text and the middle of its bounds.
fn views(home: &Path) -> Vec<(String, u32, u32)> {
    let json = ok(home, &["layout", "--json"]);
    let layout: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut out = Vec::new();
    fn walk(node: &serde_json::Value, out: &mut Vec<(String, u32, u32)>) {
        let b = &node["bounds"];
        let (x, y, w, h) = (
            b["x"].as_i64().unwrap_or(0),
            b["y"].as_i64().unwrap_or(0),
            b["width"].as_i64().unwrap_or(0),
            b["height"].as_i64().unwrap_or(0),
        );
        if let Some(text) = node["text"].as_str() {
            out.push((text.to_string(), (x + w / 2) as u32, (y + h / 2) as u32));
        }
        for child in node["children"].as_array().into_iter().flatten() {
            walk(child, out);
        }
    }
    walk(&layout["root"], &mut out);
    out
}

fn find(home: &Path, starts_with: &str) -> (String, u32, u32) {
    views(home)
        .into_iter()
        .find(|(t, _, _)| t.starts_with(starts_with))
        .unwrap_or_else(|| panic!("no view starts with {starts_with:?}"))
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
        "EV_ABS:ABS_MT_PRESSURE:600",
        "EV_KEY:BTN_TOUCH:1",
        "EV_SYN:0:0",
    ]);
    pause(0.12);
    send(&["EV_ABS:ABS_MT_TRACKING_ID:-1", "EV_ABS:ABS_MT_PRESSURE:0", "EV_KEY:BTN_TOUCH:0", "EV_SYN:0:0"]);
}

fn stop(child: &mut Child) {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    // Ctrl+C, so the program puts the device's proxy back before it ends.
    unsafe { kill(child.id() as i32, 2) };
    let _ = child.wait();
}

#[test]
#[ignore = "needs a running Android emulator with the Compose demo (tools/compose-app/install.sh)"]
fn a_webview_is_listed_read_and_driven() {
    let home = home("webview");
    start(".WebViewActivity");
    let list: serde_json::Value = serde_json::from_str(&ok(&home, &["webview", "list", "--json"])).unwrap();
    let mine =
        list.as_array().unwrap().iter().find(|w| w["socket"]["app"] == APP).expect("the demo's WebView is listed");
    assert_eq!(mine["socket"]["kind"], "webview");
    assert_eq!(mine["pages"].as_array().unwrap().len(), 1);

    assert_eq!(ok(&home, &["webview", "eval", "document.title"]).trim(), "EmuLabZip WebView demo");
    assert_eq!(ok(&home, &["webview", "eval", "6 * 7"]).trim(), "42");
    let (code, _, err) = emulabzip(&home, &["webview", "eval", "nope.x"]);
    assert_eq!(code, Some(1), "a script that throws fails the command: {err}");

    // The console shows what the page writes, and the error it does not catch.
    let console = command(&home, &["webview", "console", "--seconds", "8", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    pause(3.0);
    ok(
        &home,
        &["webview", "eval", "document.getElementById('log').click(); document.getElementById('boom').click(); 1"],
    );
    let out = console.wait_with_output().unwrap();
    let lines: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let text: Vec<&str> = lines.as_array().unwrap().iter().filter_map(|l| l["text"].as_str()).collect();
    assert!(text.iter().any(|t| t.starts_with("Tapped 1 times")), "{text:?}");
    assert!(text.iter().any(|t| t.contains("This error is thrown on purpose")), "{text:?}");

    // Its requests are seen by the page itself, with no proxy and no certificate.
    let har = home.join("page.har");
    let mut network = command(&home, &["webview", "network", "--seconds", "8", "--har", har.to_str().unwrap()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    pause(3.0);
    ok(
        &home,
        &["webview", "eval", "document.getElementById('fetch').click(); document.getElementById('fail').click(); 1"],
    );
    assert!(network.wait().unwrap().success());
    let har: serde_json::Value = serde_json::from_slice(&std::fs::read(&har).unwrap()).unwrap();
    let statuses: Vec<u64> =
        har["log"]["entries"].as_array().unwrap().iter().filter_map(|e| e["response"]["status"].as_u64()).collect();
    assert!(statuses.contains(&200) && statuses.contains(&404), "{statuses:?}");
}

#[test]
#[ignore = "needs a running Android emulator with the Compose demo; touches go through the emulator console"]
fn touches_keys_and_a_recorded_flow_play_back() {
    let home = home("flow");
    start(".MainActivity");
    let counter = |home: &Path| find(home, "Tapped").0;
    assert_eq!(counter(&home), "Tapped 0 times");
    let (_, x, y) = find(&home, "Tapped");
    ok(&home, &["control", "tap", &x.to_string(), &y.to_string()]);
    pause(1.0);
    assert_eq!(counter(&home), "Tapped 1 times");
    ok(&home, &["control", "key", "back"]);
    for bad in [["control", "key", "KEYCODE_X"], ["control", "text", "caf\u{e9}"], ["control", "tap", "99999"]] {
        let (code, _, _) = emulabzip(&home, &bad);
        assert_ne!(code, Some(0), "{bad:?} is refused");
    }

    // Record two real touches (the first is read as the counter by its id), then export and play them again.
    start(".MainActivity");
    let mut recording = command(&home, &["flow", "record", "--name", "Counter", "--app", APP, "--seconds", "16"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    pause(7.0);
    touch(x, y);
    pause(4.5);
    touch(x, y);
    assert!(recording.wait().unwrap().success());
    let shown: serde_json::Value = serde_json::from_str(&ok(&home, &["flow", "show", "counter", "--json"])).unwrap();
    let steps = shown["steps"].as_array().unwrap();
    let taps = steps.iter().filter(|s| s["type"] == "tap").count();
    assert_eq!(taps, 2, "{steps:?}");
    assert!(steps.iter().any(|s| s["type"] == "wait"), "the pause between the touches is kept");
    assert_eq!(steps[0]["type"], "launch");
    let maestro = ok(&home, &["flow", "export", "counter", "--maestro"]);
    assert!(
        maestro.contains(&format!("appId: \"{APP}\""))
            && maestro.contains("- tapOn:")
            && maestro.contains("- launchApp"),
        "{maestro}"
    );
    let script = ok(&home, &["flow", "export", "counter", "--script"]);
    assert!(script.contains("emulabzip control tap"), "{script}");

    adb(&["shell", "am", "force-stop", APP]);
    ok(&home, &["flow", "play", "counter"]);
    pause(1.0);
    assert_eq!(counter(&home), "Tapped 2 times", "the flow launched the app and tapped the counter twice");
}

#[test]
#[ignore = "needs a running Android emulator with the Compose demo, and the internet for the domain files"]
fn deep_links_are_checked_against_the_domain_and_the_device() {
    let home = home("links");
    start(".MainActivity");
    // A link of the app's own scheme: no domain file, and the app opens it.
    let own = ok(&home, &["link", "check", "emulabzip-demo://open/cart", "--app", APP]);
    assert!(own.contains("dev.emulabzip.composedemo/.WebViewActivity opens it"), "{own}");

    // An app link: the site lists the demo and its debug key, and the device verified it.
    let json = ok(&home, &["link", "check", "https://emulabzip.luispeixoto.dev/demo/cart", "--app", APP, "--json"]);
    let report: serde_json::Value = serde_json::from_str(&json).unwrap();
    let titles: Vec<&str> =
        report["findings"].as_array().unwrap().iter().map(|f| f["title"].as_str().unwrap()).collect();
    for expected in ["assetlinks.json is valid", "The fingerprint matches", "The device verified the domain"] {
        assert!(titles.contains(&expected), "{expected}: {titles:?}");
    }
    assert_ne!(report["worst"], "fail", "{titles:?}");

    // Another app is not in the file: a problem, with the way to fix it, and a failing exit code.
    let (code, out, _) = emulabzip(
        &home,
        &["link", "check", "https://emulabzip.luispeixoto.dev/demo/cart", "--app", "com.android.chrome"],
    );
    assert_eq!(code, Some(1));
    assert!(out.contains("The app is not in assetlinks.json") && out.contains("fix:"), "{out}");

    // Opening a link takes the app to its screen, and the link is remembered.
    ok(&home, &["link", "open", "emulabzip-demo://open/history", "--app", APP]);
    pause(2.0);
    assert!(adb(&["shell", "dumpsys", "activity", "activities"]).contains("WebViewActivity"));
    assert!(ok(&home, &["link", "history"]).contains("emulabzip-demo://open/history"));
}

#[test]
#[ignore = "needs a running Android emulator with the Compose demo, and the internet; it sets the device's proxy"]
fn rules_answer_with_a_file_send_elsewhere_and_run_only_approved_scripts() {
    let home = home("rules");
    let file = home.join("time.txt");
    std::fs::write(&file, "mapped by EmuLabZip").unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rule-scripts/dist/add-header.wasm")
        .canonicalize()
        .unwrap();
    let rules = home.join("rules.json");
    std::fs::write(
        &rules,
        serde_json::json!([
            { "name": "local time", "matcher": { "host": "time.akamai.com" }, "action": { "type": "mapLocal", "path": file, "contentType": "text/plain" } },
            { "name": "scripted", "matcher": { "host": "example.com" }, "action": { "type": "script", "path": script } },
        ])
        .to_string(),
    )
    .unwrap();
    let said = |home: &Path| find(home, "Server said").0;
    let run = |name: &str, tap_fetch_time: bool| -> (String, serde_json::Value) {
        start(".MainActivity");
        let har = home.join(format!("{name}.har"));
        let mut net = command(&home, &["net", "--rules", rules.to_str().unwrap(), "--har", har.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        pause(6.0);
        let (_, x, y) = find(&home, if tap_fetch_time { "Fetch the time" } else { "Fetch another page" });
        ok(&home, &["control", "tap", &x.to_string(), &y.to_string()]);
        pause(6.0);
        let shown = said(&home);
        stop(&mut net);
        (shown, serde_json::from_slice(&std::fs::read(&har).unwrap()).unwrap())
    };

    // Map local: the app is given the file, not the server's time.
    let (shown, _) = run("map-local", true);
    assert_eq!(shown, "Server said: mapped by EmuLabZip");

    // A script that has not been approved does nothing; once approved it changes the request and the response.
    let header = |har: &serde_json::Value, side: &str| -> Option<String> {
        let entry = har["log"]["entries"].as_array()?.iter().find(|e| e["request"]["url"] == "http://example.com/")?;
        entry[side]["headers"]
            .as_array()?
            .iter()
            .find(|h| h["name"] == "x-emulabzip-script")
            .and_then(|h| h["value"].as_str().map(str::to_string))
    };
    let (_, before) = run("before", false);
    assert_eq!(header(&before, "response"), None, "an unapproved script is not run");
    ok(&home, &["script", "approve", script.to_str().unwrap()]);
    let (_, after) = run("after", false);
    assert_eq!(header(&after, "response").as_deref(), Some("add-header: response"));
    assert_eq!(header(&after, "request").as_deref(), Some("add-header: request"));
}
