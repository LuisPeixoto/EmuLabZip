//! Records sessions on a real device and runs `ci` around a command. Opt in with `--ignored`:
//!
//! ```bash
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_session -- --ignored --nocapture
//! ```
//!
//! For the crash check the crash app has to be installed (see `tools/crash-app`), on an Android device.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emulabzip"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-live-session-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

#[test]
#[ignore = "needs a running device"]
fn records_a_session_and_reads_it_back() {
    let dir = scratch("record");
    let out = dir.join("run.ezsession");
    let rec = bin()
        .args(["-d", &device(), "session", "record", "--duration", "6", "--screens", "2", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(rec.status.success(), "{}", String::from_utf8_lossy(&rec.stderr));
    assert!(out.is_file());
    assert!(!dir.join("run.ezsession.partial").exists());

    let show = bin().args(["session", "show", "--json"]).arg(&out).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert!(v["logs"]["total"].as_u64().unwrap() > 0, "{v}");
    assert!(v["screens"].as_u64().unwrap() >= 1, "{v}");
    assert_eq!(v["partial"], false);
    assert!(v["durationMs"].as_u64().unwrap() >= 5000);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
#[ignore = "needs a running device"]
fn ci_keeps_the_exit_code_and_writes_the_report() {
    let dir = scratch("ci");
    let (out, summary) = (dir.join("ci.ezsession"), dir.join("summary.md"));
    let run = bin()
        .args(["-d", &device(), "ci", "--summary"])
        .arg(&summary)
        .arg("-o")
        .arg(&out)
        .args(["--", "sh", "-c", "sleep 3; exit 7"])
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(7), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(out.is_file());
    assert!(std::fs::read_to_string(&summary).unwrap().starts_with("## EmuLabZip session"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
#[ignore = "needs an Android device with the crash app installed"]
fn ci_can_fail_a_run_when_the_app_crashes() {
    let serial = device();
    let adb = std::env::var("EMULABZIP_ADB").unwrap_or_else(|_| "adb".to_string());
    let dir = scratch("crash");
    let out = dir.join("crash.ezsession");
    let trigger =
        format!("{adb} -s {serial} shell am start -n dev.emulabzip.crashy/.MainActivity --es mode crash; sleep 12");
    let run = bin()
        .args(["-d", &serial, "ci", "--fail-on-crash", "--app", "dev.emulabzip.crashy", "-o"])
        .arg(&out)
        .args(["--", "sh", "-c", &trigger])
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(1), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(String::from_utf8_lossy(&run.stderr).contains("crash"), "{}", String::from_utf8_lossy(&run.stderr));
    let show = bin().args(["session", "show", "--json"]).arg(&out).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(v["crashes"].as_array().unwrap().len(), 1, "{v}");
    std::fs::remove_dir_all(dir).ok();
}
