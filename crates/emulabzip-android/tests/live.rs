//! Checks against a real emulator. Run with a booted emulator:
//!
//! ```bash
//! cargo test -p emulabzip-android --test live -- --ignored --nocapture
//! ```

use emulabzip_android::AndroidBackend;
use emulabzip_core::{Backend, DeviceState, LogOptions};

async fn first_online(backend: &AndroidBackend) -> String {
    let devices = backend.devices().await.expect("adb server reachable");
    println!("{devices:#?}");
    devices.into_iter().find(|d| d.state == DeviceState::Online).expect("a booted emulator").id
}

#[tokio::test]
#[ignore = "needs a running emulator"]
async fn reads_a_real_device() {
    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;

    let info = backend.info(&id).await.unwrap();
    for section in &info {
        println!("== {}", section.title);
        for (k, v) in &section.items {
            println!("   {k}: {v}");
        }
    }
    assert!(info.iter().any(|s| s.title == "Software"));

    let apps = backend.apps(&id).await.unwrap();
    println!("{} apps, {} running", apps.len(), apps.iter().filter(|a| a.pid.is_some()).count());
    assert!(!apps.is_empty());

    let png = backend.screenshot(&id).await.unwrap();
    assert!(png.starts_with(b"\x89PNG"));

    let mut logs = backend.logs(&id, LogOptions { tail: 20, verbose: true }).await.unwrap();
    let first = tokio::time::timeout(std::time::Duration::from_secs(10), logs.next()).await.unwrap().unwrap();
    println!("first log line: {}", first.to_line());
}
