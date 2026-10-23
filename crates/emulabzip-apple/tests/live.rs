//! Checks against a booted simulator. Run on macOS with:
//!
//! ```bash
//! cargo test -p emulabzip-apple --test live -- --ignored --nocapture
//! ```

use emulabzip_apple::AppleBackend;
use emulabzip_core::{Backend, LogOptions};

#[tokio::test]
#[ignore = "needs a booted iOS simulator"]
async fn reads_a_booted_simulator() {
    let backend = AppleBackend::new();
    assert!(backend.is_available().await);
    let devices = backend.devices().await.unwrap();
    println!("{devices:#?}");
    let id = devices.first().expect("a booted simulator").id.clone();

    for section in backend.info(&id).await.unwrap() {
        println!("== {}", section.title);
        for (k, v) in &section.items {
            println!("   {k}: {v}");
        }
    }

    let apps = backend.apps(&id).await.unwrap();
    println!("{} apps, {} running", apps.len(), apps.iter().filter(|a| a.pid.is_some()).count());
    assert!(!apps.is_empty());

    assert!(backend.screenshot(&id).await.unwrap().starts_with(b"\x89PNG"));

    let mut logs = backend.logs(&id, LogOptions::default()).await.unwrap();
    let first = tokio::time::timeout(std::time::Duration::from_secs(15), logs.next()).await.unwrap().unwrap();
    println!("first log line: {}", first.to_line());
}
