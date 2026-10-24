//! `adb start-server` leaves a server behind that keeps the command's output open. Starting it must
//! not wait for that server to end. This test is alone in its binary because it sets an environment
//! variable.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

fn fake_adb(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let path = dir.join("adb");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[tokio::test]
async fn does_not_wait_for_the_server_it_starts() {
    let dir = std::env::temp_dir().join(format!("emulabzip-adb-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // A "server" that stays alive holding the output, like the real one does.
    let adb = fake_adb(&dir, "echo 'daemon started successfully'\n(sleep 20) &\nexit 0");
    // SAFETY: the only test in this binary, so nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("EMULABZIP_ADB", &adb) };
    let started = Instant::now();
    emulabzip_adb::server::start_server().await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(10), "waited {:?} for the server to end", started.elapsed());

    // A failure still says why.
    let adb = fake_adb(&dir, "echo 'cannot bind port 5037' >&2\nexit 1");
    unsafe { std::env::set_var("EMULABZIP_ADB", &adb) };
    let err = emulabzip_adb::server::start_server().await.unwrap_err().to_string();
    assert!(err.contains("cannot bind port 5037"), "{err}");
    std::fs::remove_dir_all(dir).ok();
}
