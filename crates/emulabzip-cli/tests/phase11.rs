//! App state, the device lifecycle and editing without a device: the arguments are checked first, and a state file on
//! disk needs nothing else.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use emulabzip_core::{DATA_TAR, Platform, StateFiles, tarball};
use emulabzip_storage::schema::scan_dir;
use emulabzip_storage::snapshot::{StateMeta, VERSION, write};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-phase11-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(home: &Path, args: &[&str]) -> Output {
    // No device exists for these: an argument that needs one but is wrong must fail before it is looked for.
    Command::new(env!("CARGO_BIN_EXE_emulabzip")).env("EMULABZIP_HOME", home).args(args).output().unwrap()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../emulabzip-storage/tests/fixtures").join(name)
}

/// A state file holding `databases/shop.db` from a fixture.
fn state(dir: &Path, name: &str, db: &str) -> PathBuf {
    let sandbox = dir.join(format!("sandbox-{name}"));
    std::fs::create_dir_all(sandbox.join("databases")).unwrap();
    std::fs::copy(fixture(db), sandbox.join("databases/shop.db")).unwrap();
    let data = dir.join(format!("{name}-{DATA_TAR}"));
    tarball::pack_dir(&sandbox, &data, &[]).unwrap();
    let schemas = serde_json::to_vec(&scan_dir(&sandbox)).unwrap();
    let meta = StateMeta {
        version: VERSION,
        tool: "test".into(),
        platform: Platform::Android,
        app: "dev.x".into(),
        app_version: Some("1.0".into()),
        device: "Fake".into(),
        taken_ms: 0,
        files: 1,
        bytes: 1,
        external: false,
    };
    let out = dir.join(format!("{name}.ezstate"));
    write(&out, &meta, &StateFiles { data, external: None, app_version: None }, Some(&schemas)).unwrap();
    out
}

#[test]
fn a_state_file_is_shown_and_two_are_compared() {
    let dir = scratch("compare");
    let a = state(&dir, "a", "shop-v1.db");
    let b = state(&dir, "b", "shop-v2.db");
    let shown = run(&dir, &["state", "show", a.to_str().unwrap()]);
    assert!(shown.status.success(), "{}", err(&shown));
    assert!(out(&shown).contains("dev.x") && out(&shown).contains("databases/shop.db"), "{}", out(&shown));

    let diff = run(&dir, &["state", "diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(diff.status.success(), "{}", err(&diff));
    assert!(out(&diff).contains("databases/shop.db"), "{}", out(&diff));
    let json = run(&dir, &["state", "diff", a.to_str().unwrap(), b.to_str().unwrap(), "--json"]);
    assert!(
        serde_json::from_slice::<serde_json::Value>(&json.stdout).unwrap().as_array().is_some_and(|l| !l.is_empty())
    );

    let same = run(&dir, &["state", "diff", a.to_str().unwrap(), a.to_str().unwrap()]);
    assert!(out(&same).contains("No difference"));
    assert!(!run(&dir, &["state", "show", dir.join("missing.ezstate").to_str().unwrap()]).status.success());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn what_changes_a_device_needs_a_yes_before_any_device_is_looked_for() {
    let dir = scratch("yes");
    let a = state(&dir, "a", "shop-v1.db");
    let cases: Vec<Vec<&str>> = vec![
        vec!["state", "restore", a.to_str().unwrap()],
        vec!["avd", "wipe", "Some_AVD"],
        vec!["avd", "delete", "Some_AVD"],
        vec!["avd", "boot", "Some_AVD", "--wipe"],
        vec!["sim", "erase", "iPhone 17"],
        vec!["sim", "delete", "iPhone 17"],
        vec!["snapshot", "load", "snap1"],
        vec!["snapshot", "delete", "snap1"],
        vec!["edit", "cell", "dev.x", "databases/shop.db", "-t", "items", "--rowid", "1", "--set", "a=b"],
        vec!["edit", "insert", "dev.x", "databases/shop.db", "-t", "items", "--set", "a=b"],
        vec!["edit", "delete", "dev.x", "databases/shop.db", "-t", "items", "--rowid", "1"],
        vec!["edit", "pref-set", "dev.x", "shared_prefs/a.xml", "k", "v"],
        vec!["edit", "pref-remove", "dev.x", "shared_prefs/a.xml", "k"],
        vec!["edit", "undo", "somewhere"],
    ];
    for case in cases {
        let o = run(&dir, &case);
        assert!(!o.status.success(), "{case:?}");
        assert!(err(&o).contains("--yes"), "{case:?}: {}", err(&o));
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn bad_names_and_rows_are_refused_without_a_device() {
    let dir = scratch("bad");
    for (args, said) in [
        (
            vec![
                "avd",
                "create",
                "bad name",
                "--image",
                "system-images;android-35;google_apis;x86_64",
                "--model",
                "pixel_6",
            ],
            "identifiers",
        ),
        (vec!["avd", "create", "ok", "--image", "x; rm -rf /", "--model", "pixel_6"], "identifiers"),
        (vec!["snapshot", "save", "../x"], "snapshot name"),
        (vec!["edit", "cell", "dev.x", "a.db", "-t", "t", "--set", "a=b", "--yes"], "which row"),
        (vec!["edit", "cell", "dev.x", "a.db", "-t", "t", "--rowid", "1", "--yes"], "nothing to set"),
        (vec!["edit", "cell", "dev.x", "a.db", "-t", "t", "--rowid", "1", "--set", "nopair", "--yes"], "column=value"),
        (vec!["edit", "pref-set", "dev.x", "p.xml", "k", "x", "--as", "int", "--yes"], "preference value"),
    ] {
        let o = run(&dir, &args);
        assert!(!o.status.success(), "{args:?}");
        assert!(err(&o).contains(said), "{args:?}: {}", err(&o));
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn bad_languages_sizes_and_displays_are_refused_without_a_device() {
    let dir = scratch("locale");
    for (args, said) in [
        (vec!["control", "locale", "de_DE", "--app", "com.example"], "locale"),
        (vec!["control", "locale", "de;reboot", "--app", "com.example"], "locale"),
        (vec!["control", "locale", "de", "--app", "com.example; rm"], "app"),
        (vec!["control", "font-scale", "20"], "text size"),
        (vec!["control", "font-scale", "900"], "text size"),
        (vec!["control", "display", "--density", "5"], "density"),
        (vec!["control", "display"], "display"),
    ] {
        let o = run(&dir, &args);
        assert!(!o.status.success(), "{args:?}");
        assert!(err(&o).contains(said), "{args:?}: {}", err(&o));
    }
    // A width needs its height, which the parser says before anything else happens.
    assert!(!run(&dir, &["control", "display", "--width", "720"]).status.success());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_sweep_checks_its_arguments_before_looking_for_a_device() {
    let dir = scratch("sweep");
    for (args, said) in [
        (vec!["i18n", "sweep", "--locales", "de_DE"], "language tag"),
        (vec!["i18n", "sweep", "--locales", "en-XA;reboot"], "language tag"),
        (vec!["i18n", "sweep", "--scales", "10"], "text size"),
        (vec!["i18n", "sweep", "--scales", "100,130,150,175,200,250,300"], "at most"),
        (vec!["i18n", "sweep", "--app", "a b"], "package name"),
        (vec!["i18n", "sweep", "--settle-ms", "99999"], "settle"),
    ] {
        let o = run(&dir, &args);
        assert!(!o.status.success(), "{args:?}");
        assert!(err(&o).contains(said), "{args:?}: {}", err(&o));
    }
    std::fs::remove_dir_all(dir).ok();
}
