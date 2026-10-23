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

#[tokio::test]
#[ignore = "needs a running emulator"]
async fn samples_performance_and_memory() {
    use emulabzip_core::PerfOptions;

    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;
    let app = "com.android.systemui".to_string();

    let mut perf = backend.perf(&id, PerfOptions { app: Some(app.clone()), interval_ms: 500 }).await.unwrap();
    let mut last = None;
    for _ in 0..3 {
        last = perf.next().await;
    }
    let sample = last.unwrap();
    println!("{sample:#?}");
    assert!(sample.pid.is_some());
    assert!(sample.device_cpu.is_some());
    assert!(sample.app_memory_kb.is_some());

    let report = backend.memory(&id, &app).await.unwrap();
    println!("{} {} KB: {:?}", report.metric, report.total_kb, &report.categories[..3]);
    assert!(report.total_kb > 0);

    match backend.heap_dump(&id, &app).await {
        Ok(dump) => println!("heap dump {} ({} bytes)", dump.file_name, dump.bytes.len()),
        Err(e) => println!("heap dump refused, as expected on user builds: {e}"),
    }
}

#[tokio::test]
#[ignore = "needs a running emulator"]
async fn browses_app_storage() {
    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;
    let app = "com.android.chrome";

    let roots = backend.storage_roots(&id, app).await.unwrap();
    println!("{roots:#?}");
    assert_eq!(roots.len(), 2);

    let external = roots.iter().find(|r| r.id == "external").unwrap();
    if external.unavailable.is_none() {
        let entries = backend.list_files(&id, app, "external", "").await.unwrap();
        println!("{entries:#?}");
        if let Some(file) = entries.iter().find(|e| !e.is_dir) {
            let bytes = backend.read_file(&id, app, "external", &file.path).await.unwrap();
            assert_eq!(bytes.len() as u64, file.size);
        }
    }

    let data = roots.iter().find(|r| r.id == "data").unwrap();
    if data.unavailable.is_some() {
        assert!(backend.list_files(&id, app, "data", "").await.is_err());
    }
}

/// Needs the sample app: `tools/sample-app/install.sh`.
#[tokio::test]
#[ignore = "needs a running emulator with the sample app"]
async fn reads_debuggable_app_data() {
    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;
    let app = "dev.emulabzip.sample";

    let roots = backend.storage_roots(&id, app).await.unwrap();
    let data = roots.iter().find(|r| r.id == "data").unwrap();
    assert!(data.unavailable.is_none(), "{data:?}");

    let top = backend.list_files(&id, app, "data", "").await.unwrap();
    let names: Vec<&str> = top.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"databases") && names.contains(&"shared_prefs"), "{names:?}");

    let db = backend.read_file(&id, app, "data", "databases/notes.db").await.unwrap();
    assert!(db.starts_with(b"SQLite format 3\0"));
    let png = backend.read_file(&id, app, "data", "files/logo.png").await.unwrap();
    assert!(png.starts_with(b"\x89PNG"), "binary files must survive the trip intact");
}
