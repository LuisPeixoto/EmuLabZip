//! Frameworks, Flutter, React Native and network conditions: the checks that need no device.

use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_emulabzip")).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn fails_with(args: &[&str], text: &str) {
    let (ok, _, err) = run(args);
    assert!(!ok && err.contains(text), "{args:?}: {err}");
}

#[test]
fn refuses_bad_arguments_before_asking_for_a_device() {
    fails_with(&["frameworks", "not an app"], "not a package name");
    fails_with(&["flutter", "tree", "not an app"], "not a package name");
    fails_with(&["flutter", "tree", "com.example.app", "--uri", "nonsense"], "not an address");
    fails_with(&["flutter", "frames", "com.example.app", "--seconds", "0"], "--seconds is between");
    fails_with(&["flutter", "frames", "com.example.app", "--seconds", "9999"], "--seconds is between");
    fails_with(&["rn", "console", "com.example.app", "--seconds", "0"], "--seconds is between");
    fails_with(&["rn", "tree", "not an app"], "not a package name");
}

#[test]
fn react_native_says_when_metro_is_not_running() {
    let free = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    fails_with(&["rn", "tree", "com.example.app", "--metro", &free.to_string()], "Metro is not running");
}

#[test]
fn network_conditions_are_checked_before_the_proxy_starts() {
    fails_with(&["net", "--conditions", "dial-up"], "not a preset");
    fails_with(&["net", "--down", "0"], "cannot be 0");
    fails_with(&["net", "--loss", "150"], "percentage");
    fails_with(&["net", "--latency", "999999"], "30 seconds");
}

#[test]
fn lists_the_new_commands_and_options() {
    let (_, out, _) = run(&["--help"]);
    for word in ["frameworks", "flutter", "rn"] {
        assert!(out.lines().any(|l| l.trim_start().starts_with(word)), "{word} is missing");
    }
    let (_, net, _) = run(&["net", "--help"]);
    for option in ["--conditions", "--down", "--up", "--latency", "--loss"] {
        assert!(net.contains(option), "{option} is missing");
    }
    let (_, flutter, _) = run(&["flutter", "--help"]);
    for word in ["tree", "frames", "memory"] {
        assert!(flutter.lines().any(|l| l.trim_start().starts_with(word)), "{word} is missing");
    }
}
