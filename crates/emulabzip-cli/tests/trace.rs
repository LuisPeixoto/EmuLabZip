//! The trace commands that need neither a device nor a download.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emulabzip"))
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-trace-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn lists_nothing_in_a_fresh_library() {
    let home = home("list");
    let out = bin().args(["trace", "list"]).env("EMULABZIP_HOME", &home).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("No recordings"));

    let json = bin().args(["trace", "list", "--json"]).env("EMULABZIP_HOME", &home).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&json.stdout).trim(), "[]");
    std::fs::remove_dir_all(home).ok();
}

#[test]
fn refuses_a_trace_file_that_does_not_exist_before_downloading_anything() {
    let home = home("missing");
    for args in [["trace", "summary", "/no/such.pftrace"], ["trace", "open", "/no/such.pftrace"]] {
        let out = bin().args(args).env("EMULABZIP_HOME", &home).output().unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("is not a file"), "{args:?}");
    }
    assert!(!home.join("tools").exists(), "nothing may be downloaded for a missing file");
    std::fs::remove_dir_all(home).ok();
}

#[test]
fn cpu_profiles_need_a_target() {
    let out = bin().args(["trace", "cpu"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--app"));
}

#[test]
fn a_custom_preset_needs_a_configuration_file() {
    let out = bin().args(["trace", "record", "--preset", "custom"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--config"));
}
