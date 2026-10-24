//! `emulabzip session show`, and the checks of `ci` and `a11y` that need no device.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emulabzip"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-session-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A folder the way a recording that was killed leaves it.
fn partial_session() -> PathBuf {
    let dir = scratch("show");
    std::fs::write(
        dir.join("session.json"),
        r#"{"version":1,"tool":"EmuLabZip test","device":{"id":"emulator-5554","name":"Test Pixel","platform":"android","osVersion":"14","model":"Pixel"},"app":"com.example.app","startedMs":1000,"endedMs":null,"network":false,"screensEverySecs":null}"#,
    )
    .unwrap();
    let line = |seq: u32, level: &str, msg: &str| {
        format!(
            r#"{{"seq":{seq},"time":"t","pid":1,"tid":1,"level":"{level}","tag":"Net","message":"{msg}","subsystem":null}}"#
        )
    };
    std::fs::write(
        dir.join("logs.jsonl"),
        [line(1, "error", "timeout after 30 s"), line(2, "error", "timeout after 45 s"), line(3, "info", "ok")]
            .join("\n"),
    )
    .unwrap();
    // Samples of the device as a whole, taken with no app picked.
    std::fs::write(
        dir.join("perf.jsonl"),
        concat!(
            r#"{"tMs":0,"pid":null,"appCpu":null,"deviceCpu":12.5,"appMemoryKb":null,"deviceMemoryUsedKb":2048000,"deviceMemoryTotalKb":4096000,"fps":null,"jankPercent":null,"batteryLevel":null,"batteryTempC":null}"#,
            "\n",
            r#"{"tMs":1000,"pid":null,"appCpu":null,"deviceCpu":30.0,"appMemoryKb":null,"deviceMemoryUsedKb":2148000,"deviceMemoryTotalKb":4096000,"fps":null,"jankPercent":null,"batteryLevel":null,"batteryTempC":null}"#
        ),
    )
    .unwrap();
    dir
}

#[test]
fn session_show_summarizes_a_session_folder() {
    let dir = partial_session();
    let out = bin().args(["session", "show"]).arg(&dir).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Test Pixel") && text.contains("2 errors") && text.contains("cut short"), "{text}");
    // The two timeouts differ only by a number: one line, counted twice.
    assert!(text.contains("2×") && text.contains("timeout after # s"), "{text}");

    let json = bin().args(["session", "show", "--json"]).arg(&dir).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!((v["logs"]["errors"].as_u64(), v["partial"].as_bool()), (Some(2), Some(true)));
    // With no app sampled, the device's own numbers stand in.
    assert_eq!((v["perf"]["cpuPeak"].as_f64(), v["perf"]["memoryPeakKb"].as_u64()), (Some(30.0), Some(2_148_000)));

    let md = bin().args(["session", "show", "--markdown"]).arg(&dir).output().unwrap();
    assert!(String::from_utf8_lossy(&md.stdout).starts_with("## EmuLabZip session"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn session_show_explains_a_file_that_is_not_a_session() {
    let dir = scratch("bad");
    let file = dir.join("notes.ezsession");
    std::fs::write(&file, "hello").unwrap();
    let out = bin().args(["session", "show"]).arg(&file).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a session"), "{}", String::from_utf8_lossy(&out.stderr));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn ci_needs_a_command_and_refuses_to_overwrite() {
    let dir = scratch("ci");
    let out = dir.join("run.ezsession");
    // No command after `--`: the parser says so.
    let none = bin().args(["ci", "-o"]).arg(&out).output().unwrap();
    assert!(!none.status.success());
    assert!(String::from_utf8_lossy(&none.stderr).contains("COMMAND"), "{}", String::from_utf8_lossy(&none.stderr));

    // A file that exists is refused before any device is looked for.
    std::fs::write(&out, "x").unwrap();
    let exists = bin().args(["ci", "-o"]).arg(&out).args(["--", "true"]).output().unwrap();
    assert!(!exists.status.success());
    assert!(
        String::from_utf8_lossy(&exists.stderr).contains("exists already"),
        "{}",
        String::from_utf8_lossy(&exists.stderr)
    );
    assert_eq!(std::fs::read(&out).unwrap(), b"x");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn session_record_wants_the_right_extension() {
    let dir = scratch("ext");
    let out = bin().args(["session", "record", "-o"]).arg(dir.join("run.zip")).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains(".ezsession"), "{}", String::from_utf8_lossy(&out.stderr));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a11y_fail_on_knows_its_levels() {
    let out = bin().args(["a11y", "--fail-on", "catastrophe"]).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("note, warning or error"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
