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

#[tokio::test]
#[ignore = "needs a booted iOS simulator"]
async fn samples_memory_and_storage() {
    use emulabzip_core::PerfOptions;

    let backend = AppleBackend::new();
    let id = backend.devices().await.unwrap().first().expect("a booted simulator").id.clone();
    let apps = backend.apps(&id).await.unwrap();
    let app = apps.iter().find(|a| a.pid.is_some()).expect("a running app").id.clone();
    println!("sampling {app}");

    let mut perf = backend.perf(&id, PerfOptions { app: Some(app.clone()), interval_ms: 500 }).await.unwrap();
    let mut last = None;
    for _ in 0..3 {
        last = perf.next().await;
    }
    let sample = last.unwrap();
    println!("{sample:?}");
    assert!(sample.pid.is_some());
    assert!(sample.app_memory_kb.is_some());
    assert!(sample.app_cpu.is_some());

    let report = backend.memory(&id, &app).await.unwrap();
    println!("{} {} KB, top: {:?}", report.metric, report.total_kb, report.categories.first());
    assert!(report.total_kb > 0);

    let roots = backend.storage_roots(&id, &app).await.unwrap();
    println!("{roots:#?}");
    let data = roots.iter().find(|r| r.id == "data").expect("a data container");
    let entries = backend.list_files(&id, &app, &data.id, "").await.unwrap();
    println!("{:?}", entries.iter().map(|e| &e.name).collect::<Vec<_>>());
    assert!(entries.iter().any(|e| e.name == "Library"));
}
