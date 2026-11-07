//! App state, editing and the schema diff on a real Android emulator. Opt in with `--ignored`:
//!
//! ```bash
//! tools/compose-app/install.sh                 # the demo with the data screen
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_phase11 -- --ignored --nocapture --test-threads=1
//! ```
//!
//! They write the demo's data, change it, save and restore it. Use a throwaway emulator.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const APP: &str = "dev.emulabzip.composedemo";
const MAIN: &str = "dev.emulabzip.composedemo/.MainActivity";

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-live11-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn adb(args: &[&str]) -> String {
    let out = Command::new("adb").args(["-s", &device()]).args(args).output().unwrap();
    assert!(out.status.success(), "adb {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn emulabzip(home: &Path, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_emulabzip"))
        .env("EMULABZIP_HOME", home)
        .args(["-d", &device()])
        .args(args)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ok(home: &Path, args: &[&str]) -> String {
    let (good, out, err) = emulabzip(home, args);
    assert!(good, "emulabzip {args:?} failed: {err}");
    out
}

fn write_demo_data(version: &str) {
    adb(&["shell", "am", "force-stop", APP]);
    adb(&["shell", "am", "start", "-n", MAIN, "--es", "data", version]);
    std::thread::sleep(Duration::from_secs(4));
    adb(&["shell", "am", "force-stop", APP]);
}

fn query(home: &Path, sql: &str) -> String {
    ok(home, &["db", APP, "databases/shop.db", "--query", sql, "--json"])
}

#[test]
#[ignore = "needs a running emulator with the Compose demo"]
fn state_is_saved_changed_restored_and_compared() {
    let home = home("state");
    // A database already at version 2 cannot be opened as version 1, so start from nothing.
    adb(&["shell", "pm", "clear", APP]);
    write_demo_data("1");
    let v1 = home.join("v1.ezstate");
    let v2 = home.join("v2.ezstate");

    // Save, and see what is inside.
    ok(&home, &["state", "save", APP, "-o", v1.to_str().unwrap()]);
    let shown = ok(&home, &["state", "show", v1.to_str().unwrap()]);
    assert!(shown.contains("databases/shop.db") && shown.contains("shared_prefs/settings.xml"), "{shown}");

    // Without --yes nothing changes.
    let (good, _, err) = emulabzip(
        &home,
        &["edit", "cell", APP, "databases/shop.db", "-t", "customers", "--rowid", "1", "--text", "name=Edited"],
    );
    assert!(!good && err.contains("--yes"), "{err}");

    // Change a cell and a preference.
    let before = query(&home, "SELECT name FROM customers WHERE id = 1");
    let edit = ok(
        &home,
        &[
            "edit",
            "cell",
            APP,
            "databases/shop.db",
            "-t",
            "customers",
            "--rowid",
            "1",
            "--text",
            "name=Edited",
            "--yes",
        ],
    );
    assert!(edit.contains("Undo with:"), "{edit}");
    assert!(query(&home, "SELECT name FROM customers WHERE id = 1").contains("Edited"));
    ok(&home, &["edit", "pref-set", APP, "shared_prefs/settings.xml", "theme", "light", "--yes"]);
    let prefs =
        ok(&home, &["pull", APP, "shared_prefs/settings.xml", "-o", home.join("settings.xml").to_str().unwrap()]);
    assert!(prefs.contains("Saved"), "{prefs}");
    let xml = std::fs::read_to_string(home.join("settings.xml")).unwrap();
    assert!(xml.contains("light") && xml.contains("launches"), "{xml}");

    // The undo folder the edit printed puts the cell back.
    let folder = edit
        .lines()
        .find_map(|l| l.strip_prefix("Undo with: emulabzip edit undo "))
        .unwrap()
        .trim_end_matches(" --yes");
    ok(&home, &["edit", "undo", folder, "--yes"]);
    assert_eq!(query(&home, "SELECT name FROM customers WHERE id = 1"), before);

    // Restore the saved state: the preference comes back too.
    ok(&home, &["state", "restore", v1.to_str().unwrap(), "--yes"]);
    ok(&home, &["pull", APP, "shared_prefs/settings.xml", "-o", home.join("again.xml").to_str().unwrap()]);
    assert!(std::fs::read_to_string(home.join("again.xml")).unwrap().contains("dark"));

    // The migration shows up in the schema diff.
    write_demo_data("2");
    ok(&home, &["state", "save", APP, "-o", v2.to_str().unwrap()]);
    let diff = ok(&home, &["state", "diff", v1.to_str().unwrap(), v2.to_str().unwrap()]);
    assert!(diff.contains("orders") && diff.contains("idx_cart_sku"), "{diff}");
    assert!(ok(&home, &["state", "diff", v1.to_str().unwrap()]).contains("databases/shop.db"));

    // Leave the demo as it was found.
    ok(&home, &["state", "restore", v1.to_str().unwrap(), "--yes"]);
    std::fs::remove_dir_all(home).ok();
}

#[test]
#[ignore = "needs a running emulator with the Compose demo"]
fn other_stores_are_read_from_the_device() {
    let home = home("stores");
    write_demo_data("1");
    let files = ok(&home, &["files", APP, "files/datastore"]);
    assert!(files.contains("demo.preferences_pb"), "{files}");
    let mmkv = ok(&home, &["files", APP, "files/mmkv"]);
    assert!(mmkv.contains("mmkv.default") || mmkv.contains("crc"), "{mmkv}");
    std::fs::remove_dir_all(home).ok();
}
