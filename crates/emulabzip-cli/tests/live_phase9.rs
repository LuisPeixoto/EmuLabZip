//! Readable crashes and the bug report on a real device. Opt in with `--ignored`:
//!
//! ```bash
//! tools/compose-app/install.sh --release       # the R8 build; its mapping.txt is next to the build
//! EMULABZIP_DEVICE=emulator-5554 cargo test -p emulabzip-cli --test live_phase9 -- --ignored --nocapture --test-threads=1
//! ```

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

const APP: &str = "dev.emulabzip.composedemo";

fn device() -> String {
    std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-live9-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn adb(args: &[&str]) {
    let out = Command::new("adb").args(["-s", &device()]).args(args).output().unwrap();
    assert!(out.status.success(), "adb {args:?}: {}", String::from_utf8_lossy(&out.stderr));
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

fn mapping() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/compose-app/app/build/outputs/mapping/release/mapping.txt");
    assert!(path.exists(), "build the release app first: tools/compose-app/install.sh --release");
    path
}

/// Crashes the release build, and returns the id of the newest crash of the app.
fn crash_the_app(home: &Path) -> String {
    adb(&["shell", "am", "force-stop", APP]);
    adb(&["shell", "am", "start", "-n", &format!("{APP}/.MainActivity"), "--es", "crash", "1"]);
    std::thread::sleep(std::time::Duration::from_secs(6));
    let (ok, out, err) = emulabzip(home, &["crashes", "--app", APP, "--json"]);
    assert!(ok, "{err}");
    let list: serde_json::Value = serde_json::from_str(&out).unwrap();
    list[0]["id"].as_str().expect("the app crashed and the crash is listed").to_string()
}

#[test]
#[ignore = "needs a running device with the Compose release build (tools/compose-app/install.sh --release)"]
fn an_r8_crash_reads_as_the_source_once_its_mapping_is_stored() {
    let home = home("r8");
    let id = crash_the_app(&home);

    // Scrambled, with nothing stored: the app's own frames are one name made of R8's id.
    let (ok, before, err) = emulabzip(&home, &["crash", &id]);
    assert!(ok, "{err}");
    assert!(!before.contains("Cart.average"), "{before}");
    assert!(before.contains("MainActivity.onCreate"), "{before}");

    // By naming the file once...
    let file = mapping();
    let (ok, named, err) = emulabzip(&home, &["crash", &id, "--mapping", file.to_str().unwrap()]);
    assert!(ok, "{err}");
    assert!(named.contains("Cart.average") && named.contains("Checkout.total"), "{named}");
    assert!(named.contains("MainActivity.kt"), "files come from the mapping: {named}");

    // ...or by keeping it for the app and version, as the crash says them.
    let (ok, _, err) = emulabzip(&home, &["mappings", "add", APP, "1.0 (1)", file.to_str().unwrap()]);
    assert!(ok, "{err}");
    let (ok, stored, err) = emulabzip(&home, &["crash", &id]);
    assert!(ok, "{err}");
    assert!(stored.contains("Cart.average") && stored.contains("Made readable with"), "{stored}");

    // --raw is always what the device wrote.
    let (ok, raw, err) = emulabzip(&home, &["crash", &id, "--raw"]);
    assert!(ok, "{err}");
    assert!(!raw.contains("Cart.average"), "{raw}");

    // A mapping of another version is not used.
    let (ok, _, _) = emulabzip(&home, &["mappings", "remove", APP, "1.0 (1)"]);
    assert!(ok);
    let (ok, _, _) = emulabzip(&home, &["mappings", "add", APP, "9.9 (9)", file.to_str().unwrap()]);
    assert!(ok);
    let (_, other, _) = emulabzip(&home, &["crash", &id]);
    assert!(!other.contains("Cart.average"), "a mapping of the wrong build reads worse than none: {other}");
}

#[test]
#[ignore = "needs a running device with the Compose release build (tools/compose-app/install.sh --release)"]
fn the_bug_report_holds_the_readable_crash_and_the_screen() {
    let home = home("report");
    let _ = crash_the_app(&home);
    let file = mapping();
    let (ok, _, err) = emulabzip(&home, &["mappings", "add", APP, "1.0 (1)", file.to_str().unwrap()]);
    assert!(ok, "{err}");

    let out = home.join("report.html");
    let (ok, stdout, err) = emulabzip(
        &home,
        &[
            "report",
            "--app",
            APP,
            "-o",
            out.to_str().unwrap(),
            "--issue",
            "github:LuisPeixoto/EmuLabZip",
            "--description",
            "It closes at once",
        ],
    );
    assert!(ok, "{err}");
    assert!(stdout.contains("https://github.com/LuisPeixoto/EmuLabZip/issues/new?title="), "{stdout}");
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(
        html.contains("Cart.average") && html.contains("MainActivity.kt:39"),
        "the crash is readable in the report"
    );
    assert!(html.contains("data:image/png;base64,"), "the screenshot is inside the file");
    assert!(html.contains("API level") && html.contains("It closes at once"));
    assert!(!html.contains("<script"));
}
