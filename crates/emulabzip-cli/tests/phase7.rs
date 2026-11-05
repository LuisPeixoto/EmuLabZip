//! Start time, energy, heap analysis and budgets: the checks that need no device.

use std::path::PathBuf;
use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_emulabzip")).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-phase7-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn startup_refuses_bad_arguments_before_asking_for_a_device() {
    let (ok, _, err) = run(&["startup", "com.example.app", "--kind", "lukewarm"]);
    assert!(!ok && err.contains("error"), "{err}");
    for runs in ["0", "51"] {
        let (ok, _, err) = run(&["startup", "com.example.app", "--runs", runs]);
        assert!(!ok && !err.is_empty(), "runs {runs}: {err}");
    }
}

#[test]
fn heap_analyze_reads_a_dump_without_a_device() {
    let dir = scratch("heap");
    let file = dir.join("empty.hprof");
    std::fs::write(&file, emulabzip_heap::testing::Builder::new().finish()).unwrap();
    let (ok, out, err) = run(&["heap", "analyze", file.to_str().unwrap(), "--json"]);
    assert!(ok, "{err}");
    assert!(out.trim_start().starts_with('{'), "{out}");
}

#[test]
fn heap_analyze_fails_cleanly_on_a_missing_or_broken_file() {
    let dir = scratch("broken");
    let (ok, _, err) = run(&["heap", "analyze", dir.join("nope.hprof").to_str().unwrap()]);
    assert!(!ok && !err.is_empty());
    let junk = dir.join("junk.hprof");
    std::fs::write(&junk, b"not a heap dump at all").unwrap();
    let (ok, _, err) = run(&["heap", "analyze", junk.to_str().unwrap()]);
    assert!(!ok && !err.is_empty());
}

#[test]
fn session_compare_fails_on_a_missing_session() {
    let dir = scratch("compare");
    let a = dir.join("a.ezsession");
    let (ok, _, err) = run(&["session", "compare", a.to_str().unwrap(), a.to_str().unwrap()]);
    assert!(!ok && !err.is_empty());
}

#[test]
fn help_lists_the_new_commands() {
    let (_, out, _) = run(&["--help"]);
    for word in ["startup", "energy", "heap"] {
        assert!(out.lines().any(|l| l.trim_start().starts_with(word)), "{word} is missing");
    }
}
