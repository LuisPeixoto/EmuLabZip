//! The Phase 12 audits on a real Android emulator. Opt in with `--ignored`:
//!
//! ```bash
//! tools/compose-app/install.sh                 # the demo, with its deliberately weak mode
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_phase12 -- --ignored --nocapture --test-threads=1
//! ```
//!
//! They change the text size, the screen size and the language of the demo, and put them back. Use a throwaway emulator.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const APP: &str = "dev.emulabzip.composedemo";
const MAIN: &str = "dev.emulabzip.composedemo/.MainActivity";

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-live12-{name}-{}", std::process::id()));
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

fn start_weak() {
    adb(&["shell", "am", "force-stop", APP]);
    adb(&["shell", "am", "start", "-n", MAIN, "--es", "weak", "1"]);
    std::thread::sleep(Duration::from_secs(5));
}

#[test]
#[ignore = "needs a running emulator with the Compose demo"]
fn the_weak_demo_is_audited_and_every_planted_problem_is_found() {
    let h = home("audit");
    start_weak();
    let audit = ok(&h, &["audit", "security", APP, "--json"]);
    let v: serde_json::Value = serde_json::from_str(&audit).unwrap();
    let rules: Vec<&str> = v["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap()).collect();
    for expected in [
        "debuggable",
        "allow-backup",
        "exposed-component",
        "bearer-token-in-log",
        "secret-in-storage",
        "email-in-storage",
        "sensitive-column",
    ] {
        assert!(rules.contains(&expected), "{expected} is missing from {rules:?}");
    }
    // Nothing the audit prints repeats a secret.
    assert!(!audit.contains("abcdef0123456789abcdef0123456789") && !audit.contains("s3cr3t-session-token"), "{audit}");
    let ping = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["location"].as_str().is_some_and(|l| l.ends_with("PingReceiver")));
    assert!(ping, "the exported receiver is named");

    // --fail-on decides the exit code.
    assert_eq!(emulabzip(&h, &["audit", "security", APP, "--fail-on", "error"]).0, Some(1));
    assert_eq!(emulabzip(&h, &["audit", "security", APP, "--only", "manifest", "--fail-on", "error"]).0, Some(0));
    std::fs::remove_dir_all(h).ok();
}

#[test]
#[ignore = "needs a running emulator with the Compose demo"]
fn text_size_display_and_language_are_changed_checked_and_put_back() {
    let h = home("text");
    start_weak();
    let check = ok(&h, &["i18n", "check", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&check).unwrap();
    let cut: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule"] == "text-cut-off")
        .map(|f| f["message"].as_str().unwrap())
        .collect();
    assert_eq!(cut.len(), 1, "{cut:?}");
    assert!(cut[0].contains("order confirmation"), "{cut:?}");

    let sweep = ok(
        &h,
        &["i18n", "sweep", "--app", APP, "--locales", "ar-XB", "--scales", "100,200", "--settle-ms", "2000", "--json"],
    );
    let s: serde_json::Value = serde_json::from_str(&sweep).unwrap();
    let rows = s["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "device, device 200%, ar-XB, ar-XB 200%");
    assert!(rows[2]["rtl"].as_bool().unwrap());
    assert!(rows[2]["byRule"].get("rtl-not-mirrored").is_some(), "the demo is not mirrored: {}", rows[2]);
    // The sweep put everything back.
    assert!(adb(&["shell", "settings", "get", "system", "font_scale"]).trim().starts_with('1'));
    assert!(adb(&["shell", "cmd", "locale", "get-app-locales", APP, "--user", "0"]).contains("[]"));

    ok(&h, &["control", "display", "--width", "720", "--height", "1600", "--density", "320"]);
    assert!(adb(&["shell", "wm", "size"]).contains("Override size: 720x1600"));
    ok(&h, &["control", "reset-display"]);
    assert!(!adb(&["shell", "wm", "size"]).contains("Override"));
    std::fs::remove_dir_all(h).ok();
}

#[test]
#[ignore = "needs a running emulator with the Compose demo"]
fn colour_vision_is_simulated_on_the_real_screen() {
    let h = home("vision");
    start_weak();
    let png = h.join("deut.png");
    ok(&h, &["a11y", "--simulate", "deuteranopia", "-o", png.to_str().unwrap()]);
    let bytes = std::fs::read(&png).unwrap();
    assert!(bytes.starts_with(b"\x89PNG"), "a PNG was written");
    assert!(ok(&h, &["a11y", "--simulate", "achromatopsia"]).len() > 10);
    std::fs::remove_dir_all(h).ok();
}

#[test]
#[ignore = "needs a running emulator with the Compose demo"]
fn a_baseline_is_captured_compared_and_updated() {
    let h = home("visual");
    let dir = h.join("baselines");
    let d = dir.to_str().unwrap();
    adb(&["shell", "am", "force-stop", APP]);
    adb(&["shell", "am", "start", "-n", MAIN]);
    std::thread::sleep(Duration::from_secs(3));
    ok(&h, &["visual", "capture", "home", "--dir", d, "--status-bar"]);
    assert!(ok(&h, &["visual", "compare", "home", "--dir", d, "--status-bar"]).contains("identical"));
    // Tapping the counter changes its label.
    adb(&["shell", "input", "tap", "540", "270"]);
    std::thread::sleep(Duration::from_secs(1));
    let (code, out, _) = emulabzip(&h, &["visual", "compare", "home", "--dir", d, "--status-bar"]);
    assert_eq!(code, Some(1), "{out}");
    assert!(dir.join("diffs/home.png").exists());
    assert_eq!(emulabzip(&h, &["visual", "update", "home", "--dir", d]).0, Some(1), "update needs --yes");
    ok(&h, &["visual", "update", "home", "--dir", d, "--status-bar", "--yes"]);
    assert!(ok(&h, &["visual", "compare", "home", "--dir", d, "--status-bar"]).contains("identical"));
    std::fs::remove_dir_all(h).ok();
}

fn spawn(home: &Path, args: &[&str]) -> Child {
    command(home, args).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap()
}

#[test]
#[ignore = "needs a running emulator with the Compose demo, and the internet (api.mixpanel.com)"]
fn analytics_calls_of_the_app_are_read_through_the_proxy() {
    use std::io::Read;
    let h = home("analytics");
    let mut child = spawn(&h, &["analytics", "--port", "9393", "--service", "mixpanel"]);
    std::thread::sleep(Duration::from_secs(6));
    start_weak();
    std::thread::sleep(Duration::from_secs(12));
    let _ = child.kill();
    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    let _ = child.wait();
    assert!(out.contains("Mixpanel") && out.contains("Signed Up"), "{out}");
}

#[test]
#[ignore = "needs the Compose demo's debug APK built (tools/compose-app/install.sh)"]
fn the_demos_apk_is_taken_apart() {
    let apk =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/compose-app/app/build/outputs/apk/debug/app-debug.apk");
    assert!(apk.exists(), "build the demo first");
    let out = Command::new(env!("CARGO_BIN_EXE_emulabzip"))
        .args(["size", "show", apk.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["format"], "apk");
    assert!(v["methods"].as_u64().unwrap() > 1000);
    assert!(v["native"].get("arm64-v8a").is_some());
}
