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

#[test]
fn an_unknown_kind_of_colour_blindness_is_refused_before_looking_for_a_device() {
    let dir = scratch("vision");
    let o = run(&dir, &["a11y", "--simulate", "purple"]);
    assert!(!o.status.success());
    assert!(err(&o).contains("protanopia, deuteranopia, tritanopia or achromatopsia"), "{}", err(&o));
    // A file to write only makes sense with a simulation.
    assert!(!run(&dir, &["a11y", "-o", "x.png"]).status.success());
    std::fs::remove_dir_all(dir).ok();
}

fn png_file(path: &Path, w: u32, h: u32, paint: impl Fn(u32, u32) -> [u8; 3]) {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&paint(x, y));
            }
        }
        wr.write_image_data(&data).unwrap();
    }
    std::fs::write(path, out).unwrap();
}

#[test]
fn two_screenshots_are_compared_without_a_device() {
    let dir = scratch("visual");
    let (a, b, c) = (dir.join("a.png"), dir.join("b.png"), dir.join("c.png"));
    png_file(&a, 60, 40, |_, _| [255, 255, 255]);
    png_file(&b, 60, 40, |x, y| if x < 6 && y < 4 { [0, 0, 0] } else { [255, 255, 255] });
    png_file(&c, 30, 40, |_, _| [255, 255, 255]);
    let same = run(&dir, &["visual", "diff", a.to_str().unwrap(), a.to_str().unwrap()]);
    assert!(same.status.success() && out(&same).contains("identical"), "{}", out(&same));

    let diff_png = dir.join("d.png");
    let changed =
        run(&dir, &["visual", "diff", a.to_str().unwrap(), b.to_str().unwrap(), "-o", diff_png.to_str().unwrap()]);
    assert_eq!(changed.status.code(), Some(1), "{}{}", out(&changed), err(&changed));
    assert!(
        out(&changed).contains("1.000% of the screen changed") && out(&changed).contains("6×4 area at 0,0"),
        "{}",
        out(&changed)
    );
    assert!(diff_png.exists());
    // Within a looser limit it passes.
    assert!(
        run(&dir, &["visual", "diff", a.to_str().unwrap(), b.to_str().unwrap(), "--threshold", "5"]).status.success()
    );
    let json = run(&dir, &["visual", "diff", a.to_str().unwrap(), b.to_str().unwrap(), "--json", "--threshold", "5"]);
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!((v["comparison"]["changed"].as_u64(), v["exceeds"].as_bool()), (Some(24), Some(false)));
    // Another size is explained, not compared.
    let size = run(&dir, &["visual", "diff", a.to_str().unwrap(), c.to_str().unwrap()]);
    assert!(!size.status.success() && err(&size).contains("not the same size"), "{}", err(&size));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn baselines_are_named_and_replacing_one_needs_a_yes_before_any_device() {
    let dir = scratch("baseline");
    for (args, said) in [
        (vec!["visual", "capture", "../x"], "baseline name"),
        (vec!["visual", "capture", "a b"], "baseline name"),
        (vec!["visual", "update", "home"], "--yes"),
        (vec!["visual", "compare", "home", "--threshold", "150"], "percentage"),
        (vec!["visual", "compare", "missing", "--dir", dir.to_str().unwrap()], "no baseline called missing"),
    ] {
        let o = run(&dir, &args);
        assert!(!o.status.success(), "{args:?}");
        assert!(err(&o).contains(said), "{args:?}: {}", err(&o));
    }
    // A baseline that exists is not captured over.
    png_file(&dir.join("home.png"), 8, 8, |_, _| [1, 2, 3]);
    let o = run(&dir, &["visual", "capture", "home", "--dir", dir.to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("exists already"), "{}", err(&o));
    let listed = run(&dir, &["visual", "list", "--dir", dir.to_str().unwrap()]);
    assert!(out(&listed).contains("home"), "{}", out(&listed));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn analytics_arguments_are_checked_before_the_proxy_starts() {
    let dir = scratch("analytics");
    let o = run(&dir, &["analytics", "--service", "facebook"]);
    assert!(!o.status.success());
    assert!(err(&o).contains("segment, amplitude, mixpanel or firebase"), "{}", err(&o));
    std::fs::remove_dir_all(dir).ok();
}

fn zip_file(path: &Path, files: &[(&str, usize)]) {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, size) in files {
        w.start_file(*name, opts).unwrap();
        let mut bytes = vec![0u8; *size];
        if name.ends_with(".dex") {
            bytes[..8].copy_from_slice(b"dex\n035\0");
            bytes[0x58..0x5c].copy_from_slice(&500u32.to_le_bytes());
            bytes[0x60..0x64].copy_from_slice(&40u32.to_le_bytes());
        }
        w.write_all(&bytes).unwrap();
    }
    w.finish().unwrap();
}

#[test]
fn a_build_is_broken_down_compared_and_held_to_a_budget_without_a_device() {
    let dir = scratch("size");
    let (a, b) = (dir.join("a.apk"), dir.join("b.apk"));
    zip_file(
        &a,
        &[("AndroidManifest.xml", 100), ("classes.dex", 4000), ("lib/arm64-v8a/libx.so", 3000), ("assets/a.bin", 1000)],
    );
    zip_file(
        &b,
        &[
            ("AndroidManifest.xml", 100),
            ("classes.dex", 4000),
            ("lib/arm64-v8a/libx.so", 3000),
            ("assets/a.bin", 1000),
            ("assets/new.bin", 50_000),
        ],
    );

    let shown = run(&dir, &["size", "show", a.to_str().unwrap()]);
    assert!(shown.status.success(), "{}", err(&shown));
    let text = out(&shown);
    assert!(
        text.contains("APK") && text.contains("dex") && text.contains("arm64-v8a") && text.contains("500"),
        "{text}"
    );
    let json = run(&dir, &["size", "show", a.to_str().unwrap(), "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!((v["format"].as_str(), v["methods"].as_u64()), (Some("apk"), Some(500)));

    let d = run(&dir, &["size", "diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(d.status.success() && out(&d).contains("assets/new.bin") && out(&d).contains("added"), "{}", out(&d));
    let grew = run(&dir, &["size", "diff", a.to_str().unwrap(), b.to_str().unwrap(), "--fail-on-growth", "10"]);
    assert_eq!(grew.status.code(), Some(1));
    assert!(err(&grew).contains("grew"), "{}", err(&grew));

    // A budget: [size] in an emulabzip.toml.
    let config = dir.join("emulabzip.toml");
    std::fs::write(&config, "[size]\nmax_total_mb = 1\nmax_methods = 100\nmax_growth_percent = 10\n").unwrap();
    let c = config.to_str().unwrap();
    let over = run(&dir, &["size", "show", a.to_str().unwrap(), "--budget", "--config", c]);
    assert_eq!(over.status.code(), Some(1), "{}{}", out(&over), err(&over));
    assert!(out(&over).contains("methods"), "{}", out(&over));
    // Growth needs a baseline, and says so.
    assert!(out(&over).contains("no baseline build was given"), "{}", out(&over));
    let grown =
        run(&dir, &["size", "show", b.to_str().unwrap(), "--budget", "--config", c, "--baseline", a.to_str().unwrap()]);
    assert!(out(&grown).contains("growth since the baseline"), "{}", out(&grown));
    std::fs::write(&config, "[size]\nmax_total_mb = 5\nmax_methods = 1000\n").unwrap();
    assert!(run(&dir, &["size", "show", a.to_str().unwrap(), "--budget", "--config", c]).status.success());

    let junk = dir.join("junk.apk");
    std::fs::write(&junk, b"not a zip").unwrap();
    assert!(!run(&dir, &["size", "show", junk.to_str().unwrap()]).status.success());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn ci_and_audit_check_their_arguments_before_a_device_is_looked_for() {
    let dir = scratch("ci12");
    for (args, said) in [
        (vec!["ci", "-o", "x.ezsession", "--fail-on-security", "warning", "--", "true"], "--app"),
        (vec!["ci", "-o", "x.ezsession", "--size-baseline", "a.apk", "--", "true"], "--size"),
        (vec!["ci", "-o", "x.ezsession", "--size", "a.apk", "--", "true"], "[size]"),
        (vec!["audit", "security", "a b"], "package name"),
        (vec!["audit", "security", "com.example", "--only", "wifi"], "not an area"),
        (vec!["size", "show", "missing.apk", "--budget"], "emulabzip.toml"),
    ] {
        let o = run(&dir, &args);
        assert!(!o.status.success(), "{args:?}");
        assert!(err(&o).contains(said), "{args:?}: {}", err(&o));
    }
    // --fail-on-visual only makes sense with --visual: the parser says so.
    assert!(!run(&dir, &["ci", "-o", "x.ezsession", "--fail-on-visual", "--", "true"]).status.success());
    std::fs::remove_dir_all(dir).ok();
}
