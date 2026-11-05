//! Start time, the leak finder and budgets on a real Android device. Opt in with `--ignored`:
//!
//! ```bash
//! tools/leak-app/install.sh
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_phase7 -- --ignored --nocapture
//! ```

#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emulabzip"))
}

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-live-phase7-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn adb(args: &[&str]) {
    let out = Command::new("adb").args(["-s", &device()]).args(args).output().unwrap();
    assert!(out.status.success(), "adb {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "needs a running device with tools/leak-app installed"]
fn finds_the_activity_the_leak_app_keeps() {
    adb(&["shell", "am", "force-stop", "dev.emulabzip.leaky"]);
    adb(&["shell", "am", "start", "-n", "dev.emulabzip.leaky/.MainActivity", "--es", "mode", "leak"]);
    std::thread::sleep(std::time::Duration::from_secs(4));
    let dir = scratch("leak");
    let dump = dir.join("leaky.hprof");
    let out = bin()
        .args(["-d", &device(), "heapdump", "dev.emulabzip.leaky", "--analyze", "-o"])
        .arg(&dump)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains("dev.emulabzip.leaky.MainActivity"), "{text}");
    assert!(text.contains("kept"), "the static field is missing from the path: {text}");
    let again = bin().args(["heap", "analyze", "--json"]).arg(&dump).output().unwrap();
    assert!(String::from_utf8_lossy(&again.stdout).contains("\"leaks\""));
}

#[test]
#[ignore = "needs a running device"]
fn start_times_come_with_the_kind_asked_for() {
    for kind in ["cold", "hot"] {
        let out = bin()
            .args(["-d", &device(), "startup", "com.android.settings", "--kind", kind, "--runs", "2", "--json"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{kind}: {}", String::from_utf8_lossy(&out.stderr));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["kind"], kind);
        assert_eq!(v["runs"].as_array().unwrap().len(), 2);
        assert!(v["medianMs"].as_u64().unwrap() > 0);
    }
}

#[test]
#[ignore = "needs a running device"]
fn ci_fails_when_a_budget_is_broken_and_says_which() {
    let dir = scratch("budget");
    std::fs::write(dir.join("emulabzip.toml"), "[budget]\nmemory_peak_mb = 1\n").unwrap();
    let md = dir.join("report.md");
    let out = bin()
        .current_dir(&dir)
        .args(["-d", &device(), "ci", "-a", "com.android.settings", "-o", "run.ezsession", "--summary"])
        .arg(&md)
        .args(["--", "sleep", "2"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    let report = std::fs::read_to_string(&md).unwrap();
    assert!(report.contains("Budgets") && report.contains("Memory peak"), "{report}");
}
