//! Argument checks of the crash and control commands, which fail before any device is asked.

use std::process::Command;

fn run(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_emulabzip")).args(args).output().unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn rejects_a_kind_that_does_not_exist() {
    let (ok, err) = run(&["crashes", "--kind", "boom"]);
    assert!(!ok && err.contains("use crash, native or anr"), "{err}");
}

#[test]
fn rejects_values_clap_can_check() {
    for args in [
        ["control", "appearance", "purple"].as_slice(),
        &["control", "permission", "a.b", "android.permission.CAMERA", "allow"],
        &["control", "battery", "set", "150"],
        &["control", "location", "set", "1"],
    ] {
        let (ok, err) = run(args);
        assert!(!ok && err.contains("error"), "{args:?}: {err}");
    }
}

#[test]
fn lists_the_new_commands_in_the_help() {
    let out = Command::new(env!("CARGO_BIN_EXE_emulabzip")).arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    for word in ["crashes", "crash", "control", "trace"] {
        assert!(help.lines().any(|l| l.trim_start().starts_with(word)), "{word} is missing from the help");
    }
}
