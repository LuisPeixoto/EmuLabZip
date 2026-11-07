//! Symbolication and bug reports without a device: the arguments are checked first, and a stack or a session on disk
//! needs nothing else.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-phase9-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_emulabzip")).env("EMULABZIP_HOME", home).args(args).output().unwrap()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

const MAPPING: &str = "\
com.example.shop.Checkout -> a.b:
    int total() -> a
    1:1:int discounted(int):12:12 -> b
com.example.shop.Cart -> a.c:
    3:3:int average():21:21 -> a
";

#[test]
fn symbolicate_reads_a_pasted_stack() {
    let dir = scratch("stack");
    std::fs::write(dir.join("mapping.txt"), MAPPING).unwrap();
    std::fs::write(
        dir.join("stack.txt"),
        "java.lang.ArithmeticException: / by zero\n\tat a.c.a(SourceFile:3)\n\tat a.b.b(SourceFile:1)\n",
    )
    .unwrap();
    let o = run(
        &dir,
        &[
            "symbolicate",
            dir.join("stack.txt").to_str().unwrap(),
            "--mapping",
            dir.join("mapping.txt").to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", err(&o));
    let text = out(&o);
    assert!(
        text.contains("com.example.shop.Cart.average") && text.contains("com.example.shop.Checkout.discounted"),
        "{text}"
    );
    assert!(!text.contains("a.c.a"), "{text}");
}

#[test]
fn symbolicate_needs_a_mapping_and_a_stack() {
    let dir = scratch("nomap");
    std::fs::write(dir.join("stack.txt"), "x").unwrap();
    let o = run(&dir, &["symbolicate", dir.join("stack.txt").to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("--mapping"), "{}", err(&o));
    let o = run(
        &dir,
        &["symbolicate", dir.join("missing.txt").to_str().unwrap(), "-m", dir.join("stack.txt").to_str().unwrap()],
    );
    assert!(!o.status.success());
}

#[test]
fn mappings_are_kept_per_app_and_version() {
    let dir = scratch("store");
    std::fs::write(dir.join("mapping.txt"), MAPPING).unwrap();
    let file = dir.join("mapping.txt");

    let o = run(&dir, &["mappings", "list"]);
    assert!(o.status.success() && out(&o).contains("Nothing stored"), "{}", out(&o));

    let o = run(&dir, &["mappings", "add", "com.example.shop", "1.2.0 (42)", file.to_str().unwrap()]);
    assert!(o.status.success(), "{}", err(&o));
    assert!(out(&o).contains("mapping.txt") && out(&o).contains("1.2.0 (42)"), "{}", out(&o));

    let o = run(&dir, &["mappings", "list", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v[0]["app"], "com.example.shop");
    assert_eq!(v[0]["kind"], "mapping");

    let o = run(&dir, &["mappings", "remove", "com.example.shop", "1.2.0 (42)"]);
    assert!(o.status.success(), "{}", err(&o));
    let o = run(&dir, &["mappings", "remove", "com.example.shop", "1.2.0 (42)"]);
    assert!(!o.status.success() && err(&o).contains("nothing is stored"), "{}", err(&o));
}

#[test]
fn mappings_refuse_what_cannot_be_an_app_or_a_file() {
    let dir = scratch("bad");
    let note = dir.join("notes.txt");
    std::fs::write(&note, "just some text").unwrap();
    let o = run(&dir, &["mappings", "add", "../evil", "1", note.to_str().unwrap()]);
    assert!(!o.status.success(), "an app id cannot name a path");
    let o = run(&dir, &["mappings", "add", "com.example.shop", "1", dir.join("nope").to_str().unwrap()]);
    assert!(!o.status.success());
    let o = run(&dir, &["mappings", "add", "com.example.shop", "1", note.to_str().unwrap(), "--kind", "zip"]);
    assert!(!o.status.success() && err(&o).contains("use mapping, sourcemap or dsym"), "{}", err(&o));
}

#[test]
fn crash_checks_its_arguments_before_asking_for_a_device() {
    let dir = scratch("crash");
    let o = run(&dir, &["crash", "x", "--raw", "--mapping", dir.join("m.txt").to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("--raw"), "{}", err(&o));
    let o = run(&dir, &["crash", "x", "--mapping", dir.join("missing.txt").to_str().unwrap()]);
    assert!(!o.status.success(), "{}", err(&o));
    assert!(!err(&o).to_lowercase().contains("device"), "a missing file is found before the device: {}", err(&o));
}

#[test]
fn report_checks_its_arguments_before_asking_for_a_device() {
    let dir = scratch("args");
    for (args, word) in [
        (vec!["report", "--issue", "github:onlyone"], "github:owner/repo"),
        (vec!["report", "--issue", "bitbucket:a/b"], "unknown tracker"),
        (vec!["report", "--without", "everything"], "screenshot, logs, requests, crashes or video"),
        (vec!["report", "--session", "x.ezsession", "--app", "a.b"], "--app"),
    ] {
        let o = run(&dir, &args);
        assert!(!o.status.success() && err(&o).contains(word), "{args:?}: {}", err(&o));
        assert!(!err(&o).to_lowercase().contains("no device"), "{args:?}: {}", err(&o));
    }
    let o =
        run(&dir, &["report", "--session", "x.ezsession", "-o", dir.join("no/such/folder/r.html").to_str().unwrap()]);
    assert!(!o.status.success() && err(&o).contains("is not a folder"), "{}", err(&o));
}

/// A recording that was cut short, with a failed request and an error in the log.
fn session_folder() -> PathBuf {
    let dir = scratch("session");
    std::fs::write(
        dir.join("session.json"),
        r#"{"version":1,"tool":"EmuLabZip test","device":{"id":"emulator-5554","name":"Test <Pixel>","platform":"android","osVersion":"14","model":"Pixel"},"app":"com.example.shop","startedMs":1000,"endedMs":null,"network":true,"screensEverySecs":null}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("logs.jsonl"),
        r#"{"seq":1,"time":"t","pid":1,"tid":1,"level":"error","tag":"Net","message":"<b>timeout</b> after 30 s","subsystem":null}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("network.har"),
        r#"{"log":{"version":"1.2","creator":{"name":"x","version":"1"},"entries":[{"startedDateTime":"2026-01-01T00:00:00Z","time":42.0,"request":{"method":"GET","url":"https://api.example.com/cart","headers":[],"queryString":[],"cookies":[],"headersSize":-1,"bodySize":0,"httpVersion":"HTTP/1.1"},"response":{"status":503,"statusText":"","headers":[],"cookies":[],"content":{"size":0,"mimeType":"text/plain","text":"down"},"redirectURL":"","headersSize":-1,"bodySize":0,"httpVersion":"HTTP/1.1"},"cache":{},"timings":{"send":0,"wait":42,"receive":0}}]}}"#,
    )
    .unwrap();
    dir
}

#[test]
fn report_from_a_session_writes_one_safe_html_file() {
    let session = session_folder();
    let file = session.join("report.html");
    let o = run(
        &session,
        &[
            "report",
            "--session",
            session.to_str().unwrap(),
            "-o",
            file.to_str().unwrap(),
            "--title",
            "Cart fails",
            "--issue",
            "github:LuisPeixoto/EmuLabZip",
        ],
    );
    assert!(o.status.success(), "{}", err(&o));
    let text = out(&o);
    assert!(text.contains("https://github.com/LuisPeixoto/EmuLabZip/issues/new?title=Cart%20fails&body="), "{text}");
    let html = std::fs::read_to_string(&file).unwrap();
    assert!(html.contains("<h1>Cart fails</h1>") && html.contains("Test &lt;Pixel&gt;"), "device is escaped");
    assert!(html.contains("https://api.example.com/cart") && html.contains("503"), "the failed request is in it");
    assert!(html.contains("&lt;b&gt;timeout&lt;/b&gt;") && !html.contains("<b>timeout"), "log text is escaped");
    assert!(!html.contains("<script"));
}

#[test]
fn report_can_leave_parts_out() {
    let session = session_folder();
    let file = session.join("lean.html");
    let o = run(
        &session,
        &[
            "report",
            "--session",
            session.to_str().unwrap(),
            "-o",
            file.to_str().unwrap(),
            "--without",
            "requests",
            "--without",
            "logs",
        ],
    );
    assert!(o.status.success(), "{}", err(&o));
    let html = std::fs::read_to_string(&file).unwrap();
    assert!(!html.contains("api.example.com") && !html.contains("timeout"), "{html}");
}
