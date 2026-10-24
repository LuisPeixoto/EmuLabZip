//! Captures real traffic from a real emulator. Opt in with `--ignored`.
//!
//! The emulator needs Chrome (finished with its first run) and to trust the certificate for
//! every app, which means an image `adb root` works on (Google APIs, not Google Play):
//!
//! ```bash
//! EMULABZIP_DEVICE=emulator-5556 cargo test -p emulabzip-cli --test live_net -- --ignored --nocapture
//! ```
//!
//! It uses a private data folder, so it never touches your own certificate.

#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emulabzip"))
}

fn adb(serial: &str, args: &[&str]) -> String {
    let adb = std::env::var("EMULABZIP_ADB").unwrap_or_else(|_| "adb".to_string());
    let out = Command::new(adb).args(["-s", serial]).args(args).output().expect("adb is on the PATH");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[ignore = "needs an emulator with Chrome that trusts the certificate system-wide"]
fn captures_what_chrome_loads_and_cleans_up() {
    let serial = std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE to the emulator's serial");
    let home: PathBuf = std::env::temp_dir().join(format!("emulabzip-live-net-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();

    // Trust the certificate system-wide.
    let install = bin().args(["-d", &serial, "ca", "install"]).env("EMULABZIP_HOME", &home).output().unwrap();
    assert!(install.status.success(), "{}", String::from_utf8_lossy(&install.stderr));
    assert!(
        String::from_utf8_lossy(&install.stdout).contains("trusted by every app"),
        "{}",
        String::from_utf8_lossy(&install.stdout)
    );

    let mut child = bin()
        .args(["-d", &serial, "net", "--json", "--port", "9098"])
        .env("EMULABZIP_HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });

    // The proxy is on the device as soon as `net` says so.
    let deadline = Instant::now() + Duration::from_secs(15);
    while !adb(&serial, &["shell", "settings", "get", "global", "http_proxy"]).starts_with("127.0.0.1:9098") {
        assert!(Instant::now() < deadline, "the device was never pointed at the proxy");
        std::thread::sleep(Duration::from_millis(300));
    }

    // A URL nobody else has loaded, so the flow can only come from this test.
    let marker = format!("emulabzip-live-{}", std::process::id());
    adb(
        &serial,
        &[
            "shell",
            "am",
            "start",
            "-a",
            "android.intent.action.VIEW",
            "-d",
            &format!("https://example.com/?{marker}"),
            "-p",
            "com.android.chrome",
        ],
    );

    let deadline = Instant::now() + Duration::from_secs(60);
    let flow = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = rx.recv_timeout(left).expect("Chrome's request never showed up");
        let flow: serde_json::Value = serde_json::from_str(&line).unwrap();
        if flow["url"].as_str().is_some_and(|u| u.contains(&marker)) {
            break flow;
        }
    };
    assert_eq!(flow["status"], 200);
    assert_eq!(flow["scheme"], "https");
    assert_eq!(flow["host"], "example.com");

    // Ctrl+C puts the device back.
    Command::new("kill").args(["-INT", &child.id().to_string()]).status().unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(adb(&serial, &["shell", "settings", "get", "global", "http_proxy"]), ":0");
    assert!(!adb(&serial, &["reverse", "--list"]).contains("tcp:9098"));
    std::fs::remove_dir_all(home).ok();
}

/// Reads flows from the command's JSON lines until `done` accepts one.
fn wait_for_flow(
    rx: &mpsc::Receiver<String>,
    seconds: u64,
    what: &str,
    done: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = rx.recv_timeout(left).unwrap_or_else(|_| panic!("{what} never showed up"));
        let flow: serde_json::Value = serde_json::from_str(&line).unwrap();
        if done(&flow) {
            return flow;
        }
    }
}

#[test]
#[ignore = "needs an emulator with Chrome that trusts the certificate system-wide, and the internet"]
fn mocks_what_chrome_asks_for_and_talks_http3_to_servers() {
    let serial = std::env::var("EMULABZIP_DEVICE").expect("set EMULABZIP_DEVICE to the emulator's serial");
    let home: PathBuf = std::env::temp_dir().join(format!("emulabzip-live-rules-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let install = bin().args(["-d", &serial, "ca", "install"]).env("EMULABZIP_HOME", &home).output().unwrap();
    assert!(install.status.success(), "{}", String::from_utf8_lossy(&install.stderr));

    let rules = home.join("rules.json");
    std::fs::write(
        &rules,
        r#"[{"name":"teapot","matcher":{"host":"example.com","path":"/teapot"},
             "action":{"type":"mock","status":418,"headers":[["content-type","text/plain"]],"body":"I am a mock"}}]"#,
    )
    .unwrap();
    let mut child = bin()
        .args(["-d", &serial, "net", "--json", "--port", "9097", "--http3", "--rules"])
        .arg(&rules)
        .env("EMULABZIP_HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    while !adb(&serial, &["shell", "settings", "get", "global", "http_proxy"]).starts_with("127.0.0.1:9097") {
        assert!(Instant::now() < deadline, "the device was never pointed at the proxy");
        std::thread::sleep(Duration::from_millis(300));
    }

    let open = |url: &str| {
        adb(
            &serial,
            &["shell", "am", "start", "-a", "android.intent.action.VIEW", "-d", url, "-p", "com.android.chrome"],
        );
    };
    let marker = std::process::id();

    // A mock: the server is never asked.
    open(&format!("https://example.com/teapot?{marker}"));
    let mocked = wait_for_flow(&rx, 60, "the mocked request", |f| {
        f["url"].as_str().is_some_and(|u| u.contains(&format!("teapot?{marker}")))
    });
    assert_eq!(
        (mocked["status"].as_u64(), mocked["mocked"].as_bool(), mocked["rule"].as_str()),
        (Some(418), Some(true), Some("teapot"))
    );

    // HTTP/3: the first answer says the server offers it, the next request goes over QUIC.
    open(&format!("https://cloudflare-quic.com/?first-{marker}"));
    wait_for_flow(&rx, 60, "the first request to cloudflare-quic.com", |f| {
        f["url"].as_str().is_some_and(|u| u.contains(&format!("first-{marker}")))
    });
    open(&format!("https://cloudflare-quic.com/?second-{marker}"));
    let second = wait_for_flow(&rx, 60, "the second request to cloudflare-quic.com", |f| {
        f["url"].as_str().is_some_and(|u| u.contains(&format!("second-{marker}")))
    });
    assert_eq!(second["serverProtocol"], "h3", "{second}");

    Command::new("kill").args(["-INT", &child.id().to_string()]).status().unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(adb(&serial, &["shell", "settings", "get", "global", "http_proxy"]), ":0");
    std::fs::remove_dir_all(home).ok();
}
