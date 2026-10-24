//! The plugin commands that need no device, in a private data folder.

use std::path::PathBuf;
use std::process::Command;

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-plugin-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(home: &PathBuf, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_emulabzip")).args(args).env("EMULABZIP_HOME", home).output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn creates_lists_enables_and_disables_a_plugin() {
    let home = home("flow");
    assert!(run(&home, &["plugin", "list"]).1.contains("No plugins"));

    let (ok, out, err) = run(&home, &["plugin", "new", "my-plugin"]);
    assert!(ok, "{err}");
    assert!(out.contains("Created") && home.join("plugins/my-plugin/plugin.json").is_file());
    assert!(!run(&home, &["plugin", "new", "my-plugin"]).0, "an existing plugin is not overwritten");
    assert!(!run(&home, &["plugin", "new", "../escape"]).0);

    let listed = run(&home, &["plugin", "list"]).1;
    assert!(listed.starts_with("disabled") && listed.contains("my-plugin"), "{listed}");

    let (ok, out, _) = run(&home, &["plugin", "enable", "my-plugin"]);
    assert!(ok && out.contains("runs `python3 run.py` as you"), "the user is told what will run: {out}");
    assert!(run(&home, &["plugin", "list"]).1.starts_with("enabled"));
    let json = run(&home, &["plugin", "list", "--json"]).1;
    assert!(json.contains("\"enabled\": true"), "{json}");

    // Editing the manifest withdraws the approval.
    let manifest = home.join("plugins/my-plugin/plugin.json");
    std::fs::write(&manifest, std::fs::read_to_string(&manifest).unwrap().replace("0.1.0", "0.2.0")).unwrap();
    assert!(run(&home, &["plugin", "list"]).1.starts_with("disabled"));

    assert!(run(&home, &["plugin", "disable", "my-plugin"]).0);
    std::fs::remove_dir_all(home).ok();
}

#[test]
fn installs_a_folder_only_when_it_is_a_plugin() {
    let home = home("install");
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/top-processes");
    let (ok, out, err) = run(&home, &["plugin", "install", example.to_str().unwrap()]);
    assert!(ok, "{err}");
    assert!(out.contains("disabled until you enable it") && home.join("plugins/top-processes/run.py").is_file());
    assert!(!run(&home, &["plugin", "install", example.to_str().unwrap()]).0, "no silent overwrite");
    assert!(run(&home, &["plugin", "install", example.to_str().unwrap(), "--force"]).0);

    let not_a_plugin = home.join("plain");
    std::fs::create_dir_all(&not_a_plugin).unwrap();
    let (ok, _, err) = run(&home, &["plugin", "install", not_a_plugin.to_str().unwrap()]);
    assert!(!ok && err.contains("is not a plugin"), "{err}");
    std::fs::remove_dir_all(home).ok();
}

#[test]
fn unknown_plugins_are_reported() {
    let home = home("unknown");
    for args in [["plugin", "enable", "nope"], ["plugin", "run", "nope"]] {
        let (ok, _, err) = run(&home, &args);
        assert!(!ok && err.contains("there is no plugin called nope"), "{args:?}: {err}");
    }
    std::fs::remove_dir_all(home).ok();
}
