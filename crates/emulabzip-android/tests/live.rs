//! Checks against a real emulator. Run with a booted emulator:
//!
//! ```bash
//! cargo test -p emulabzip-android --test live -- --ignored --nocapture
//! ```

use emulabzip_android::AndroidBackend;
use emulabzip_core::{Backend, DeviceState, LogOptions};

/// The device to test on: `EMULABZIP_DEVICE` (an adb serial) or the first one that is booted.
async fn first_online(backend: &AndroidBackend) -> String {
    let devices = backend.devices().await.expect("adb server reachable");
    println!("{devices:#?}");
    let wanted = std::env::var("EMULABZIP_DEVICE").ok();
    devices
        .into_iter()
        .filter(|d| d.state == DeviceState::Online)
        .find(|d| wanted.as_deref().is_none_or(|w| d.id == w))
        .expect("a booted emulator")
        .id
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

#[tokio::test]
#[ignore = "needs a running emulator"]
async fn forwards_ports_and_pushes_files() {
    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;
    let adb = backend.adb();

    adb.reverse(&id, "tcp:9997", "tcp:9997").await.unwrap();
    assert!(adb.reverse_list(&id).await.unwrap().iter().any(|(r, _)| r == "tcp:9997"));
    adb.reverse_remove_all(&id).await.unwrap();
    assert!(adb.reverse_list(&id).await.unwrap().is_empty());

    let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    adb.push(&id, "/data/local/tmp/emulabzip-push-test.bin", 0o644, &data).await.unwrap();
    let mut back = Vec::new();
    adb.pull(&id, "/data/local/tmp/emulabzip-push-test.bin", &mut back).await.unwrap();
    adb.shell(&id, "rm -f /data/local/tmp/emulabzip-push-test.bin").await.unwrap();
    assert_eq!(back, data);
}

#[tokio::test]
#[ignore = "needs a running emulator"]
async fn routes_traffic_to_a_proxy_and_back() {
    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;
    let before = backend.network_status(&id).await.unwrap();

    // Another tool's reverse forward must survive our disconnect.
    backend.adb().reverse(&id, "tcp:8081", "tcp:8081").await.unwrap();

    let on = backend.connect_network(&id, 9091).await.unwrap();
    assert!(on.connected, "{on:?}");
    assert_eq!(on.proxy.as_deref(), Some("127.0.0.1:9091"));
    assert!(backend.adb().reverse_list(&id).await.unwrap().iter().any(|(r, _)| r == "tcp:9091"));

    let off = backend.disconnect_network(&id).await.unwrap();
    assert!(!off.connected && off.proxy.is_none(), "{off:?}");
    let remaining = backend.adb().reverse_list(&id).await.unwrap();
    assert!(!remaining.iter().any(|(r, _)| r == "tcp:9091"), "our forward must be gone: {remaining:?}");
    assert!(remaining.iter().any(|(r, _)| r == "tcp:8081"), "other forwards must stay: {remaining:?}");

    backend.adb().reverse_remove(&id, "tcp:8081").await.unwrap();
    assert_eq!(before.connected, backend.network_status(&id).await.unwrap().connected);
}

#[tokio::test]
#[ignore = "needs a running emulator; opens the Settings app on it"]
async fn guides_a_user_certificate_install_on_production_images() {
    use emulabzip_core::CaKind;

    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;
    let ca = emulabzip_net_free_ca();
    let result = backend.install_ca(&id, &ca.0, &ca.1).await.unwrap();
    println!("{result:#?}");
    // Play images cannot run adbd as root, so the certificate goes to Downloads for the user.
    if result.kind == CaKind::User {
        assert!(!result.done && !result.steps.is_empty());
        assert!(result.snippet.as_deref().unwrap().contains("<debug-overrides>"));
        let on_device = backend.adb().list(&id, "/sdcard/Download").await.unwrap();
        assert!(on_device.iter().any(|e| e.name == "emulabzip-ca.crt"));
        backend.adb().shell(&id, "rm -f /sdcard/Download/emulabzip-ca.crt").await.unwrap();
    }
}

/// A throwaway CA as PEM and DER, without depending on the proxy crate.
fn emulabzip_net_free_ca() -> (String, Vec<u8>) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    params.distinguished_name.push(rcgen::DnType::CommonName, "EmuLabZip live test CA");
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), cert.der().to_vec())
}

/// Needs an emulator whose image allows `adb root` (Google APIs or AOSP, not Google Play), for
/// example: `EMULABZIP_DEVICE=emulator-5556 cargo test … -- --ignored`.
#[tokio::test]
#[ignore = "needs a rootable emulator, selected with EMULABZIP_DEVICE"]
async fn trusts_the_certificate_system_wide_on_rootable_images() {
    use emulabzip_core::CaKind;

    let backend = AndroidBackend::new();
    let id = first_online(&backend).await;
    let adb = backend.adb();
    let (pem, der) = emulabzip_net_free_ca();
    let name = format!("{}.0", emulabzip_android::cert::subject_hash_old(&der).unwrap());

    let result = backend.install_ca(&id, &pem, &der).await.unwrap();
    println!("{result:#?}");
    assert_eq!(result.kind, CaKind::System, "this device is not rootable");
    assert!(result.done && result.steps.is_empty());

    // The certificate has to be visible where apps read it, not only in the shell's view. Start an
    // app, then look through its own mount namespace.
    adb.shell(&id, "am start -n com.android.settings/.Settings").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    adb.root(&id).await.unwrap();
    let pid = adb.shell(&id, "pidof com.android.settings").await.unwrap().trim().to_string();
    let seen = adb
        .shell(&id, &format!("ls /proc/{pid}/root/apex/com.android.conscrypt/cacerts/{name} /proc/{pid}/root/system/etc/security/cacerts/{name} 2>&1"))
        .await
        .unwrap();
    println!("{seen}");
    assert!(!seen.contains("No such file"), "the app cannot see the certificate:\n{seen}");

    // Installing it again must be harmless.
    assert!(backend.install_ca(&id, &pem, &der).await.unwrap().done);
}
