//! Frameworks, network conditions and the network views on a real device. Opt in with `--ignored`:
//!
//! ```bash
//! tools/compose-app/install.sh                 # Compose demo
//! tools/flutter-app/install.sh android         # Flutter demo, started with `am start`
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_phase8 -- --ignored --nocapture --test-threads=1
//! ```

#![cfg(unix)]

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emulabzip"))
}

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

fn adb(args: &[&str]) {
    let out = Command::new("adb").args(["-s", &device()]).args(args).output().unwrap();
    assert!(out.status.success(), "adb {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn emulabzip(args: &[&str]) -> (bool, String, String) {
    let out = bin().args(["-d", &device()]).args(args).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
#[ignore = "needs a running device with tools/compose-app installed"]
fn finds_compose_and_marks_its_views() {
    adb(&["shell", "am", "start", "-n", "dev.emulabzip.composedemo/.MainActivity"]);
    std::thread::sleep(std::time::Duration::from_secs(2));
    let (ok, out, err) = emulabzip(&["frameworks", "dev.emulabzip.composedemo", "--json"]);
    assert!(ok, "{err}");
    let found: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(found[0]["id"], "compose");

    let (ok, tree, err) = emulabzip(&["layout"]);
    assert!(ok, "{err}");
    assert!(tree.contains("ComposeView [compose]"), "{tree}");
    assert!(tree.contains("#counter") && tree.contains("#row-Apples"), "test tags are shown as ids: {tree}");
}

#[test]
#[ignore = "needs a running device with tools/flutter-app installed"]
fn reads_a_flutter_app() {
    adb(&["shell", "am", "start", "-n", "dev.emulabzip.flutterdemo/.MainActivity"]);
    std::thread::sleep(std::time::Duration::from_secs(8));
    let (ok, out, err) = emulabzip(&["frameworks", "dev.emulabzip.flutterdemo", "--json"]);
    assert!(ok, "{err}");
    assert!(out.contains("\"flutter\""), "{out}");

    let (ok, tree, err) = emulabzip(&["flutter", "tree", "dev.emulabzip.flutterdemo", "--json"]);
    assert!(ok, "{err}");
    let tree: serde_json::Value = serde_json::from_str(&tree).unwrap();
    assert!(tree.to_string().contains("HomePage"), "the app's own widgets are in the tree");

    let (ok, memory, err) = emulabzip(&["flutter", "memory", "dev.emulabzip.flutterdemo", "--json"]);
    assert!(ok, "{err}");
    let memory: serde_json::Value = serde_json::from_str(&memory).unwrap();
    assert!(memory["heapUsed"].as_u64().unwrap() > 1_000_000);
}

#[test]
#[ignore = "needs a running device with tools/flutter-app installed"]
fn flutter_frames_are_timed_while_the_app_is_used() {
    adb(&["shell", "am", "start", "-n", "dev.emulabzip.flutterdemo/.MainActivity"]);
    std::thread::sleep(std::time::Duration::from_secs(6));
    // Scroll the list while the frames are recorded.
    let dev = device();
    let scroller = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        for _ in 0..8 {
            let _ = Command::new("adb")
                .args(["-s", &dev, "shell", "input", "swipe", "540", "1800", "540", "600", "250"])
                .output();
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    });
    let (ok, out, err) = emulabzip(&["flutter", "frames", "dev.emulabzip.flutterdemo", "--seconds", "12", "--json"]);
    scroller.join().unwrap();
    assert!(ok, "{err}");
    let stats: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(stats["frames"].as_u64().unwrap() > 5, "{out}");
}

#[test]
#[ignore = "needs a device with tools/rn-app installed, and Metro running (tools/rn-app/install.sh --start)"]
fn reads_a_react_native_app_through_metro() {
    adb(&["reverse", "tcp:8081", "tcp:8081"]);
    adb(&["shell", "am", "start", "-n", "dev.emulabzip.rndemo/.MainActivity"]);
    std::thread::sleep(std::time::Duration::from_secs(12));
    let (ok, out, err) = emulabzip(&["frameworks", "dev.emulabzip.rndemo", "--json"]);
    assert!(ok, "{err}");
    assert!(out.contains("\"react-native\"") && out.contains("\"hermes\""), "{out}");

    let (ok, tree, err) = bin()
        .args(["rn", "tree", "dev.emulabzip.rndemo", "--json"])
        .output()
        .map(|o| {
            (
                o.status.success(),
                String::from_utf8_lossy(&o.stdout).into_owned(),
                String::from_utf8_lossy(&o.stderr).into_owned(),
            )
        })
        .unwrap();
    assert!(ok, "{err}");
    for component in ["App", "Counter", "Fruit"] {
        assert!(tree.contains(&format!("\"name\": \"{component}\"")), "{component} is missing from the tree");
    }

    let heap = bin().args(["rn", "heap", "dev.emulabzip.rndemo", "--json"]).output().unwrap();
    assert!(heap.status.success(), "{}", String::from_utf8_lossy(&heap.stderr));
    let heap: serde_json::Value = serde_json::from_slice(&heap.stdout).unwrap();
    assert!(heap["usedBytes"].as_u64().unwrap() > 100_000);
}
