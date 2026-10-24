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

/// Every proxy setting of every network service, as text, to compare before and after.
fn proxy_snapshot() -> String {
    let run = |args: &[&str]| {
        let out = std::process::Command::new("networksetup").args(args).output().unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let services = run(&["-listallnetworkservices"]);
    services
        .lines()
        .skip(1)
        .filter(|s| !s.starts_with('*'))
        .map(|s| {
            format!(
                "[{s}]\n{}{}{}",
                run(&["-getwebproxy", s]),
                run(&["-getsecurewebproxy", s]),
                run(&["-getproxybypassdomains", s])
            )
        })
        .collect()
}

/// Changes the proxy settings of this Mac while it runs, and puts them back.
#[tokio::test]
#[ignore = "changes this Mac's proxy settings while it runs, then restores them"]
async fn routes_the_mac_through_the_proxy_and_restores_it() {
    let dir = std::env::temp_dir().join(format!("emulabzip-apple-live-{}", std::process::id()));
    let backend = AppleBackend::with_state_dir(&dir);
    let saved = dir.join(emulabzip_apple::network::STATE_FILE);
    let id = backend.devices().await.unwrap().first().expect("a booted simulator").id.clone();
    let before = proxy_snapshot();
    assert!(
        !backend.network_status(&id).await.unwrap().connected,
        "the Mac already uses a local proxy; turn it off first"
    );

    // A failed assertion inside the task must not leave the Mac's traffic going nowhere.
    let task = {
        let (backend, id, saved) = (backend.clone(), id.clone(), saved.clone());
        tokio::spawn(async move {
            let on = backend.connect_network(&id, 9094).await.unwrap();
            assert!(on.connected && on.proxy.as_deref() == Some("127.0.0.1:9094"), "{on:?}");
            assert!(saved.exists(), "the original settings must be saved before changing them");

            // Connecting again keeps the original settings, not our own.
            backend.connect_network(&id, 9094).await.unwrap();
        })
    };
    let outcome = task.await;
    let off = backend.disconnect_network(&id).await.unwrap();
    outcome.unwrap();

    assert!(!off.connected && off.proxy.is_none(), "{off:?}");
    assert_eq!(proxy_snapshot(), before, "the Mac's proxy settings must be exactly as they were");
    assert!(!saved.exists(), "the saved copy is removed once it is restored");
    assert!(!backend.restore_system_proxy().await.unwrap(), "nothing is left to restore");
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
#[ignore = "adds a throwaway root certificate to the simulator, and simctl cannot remove it again"]
async fn trusts_a_certificate_in_the_simulator() {
    use emulabzip_core::CaKind;

    let backend = AppleBackend::new();
    let id = backend.devices().await.unwrap().first().expect("a booted simulator").id.clone();
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    params.distinguished_name.push(rcgen::DnType::CommonName, "EmuLabZip live test CA");
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let cert = params.self_signed(&key).unwrap();

    let result = backend.install_ca(&id, &cert.pem(), cert.der()).await.unwrap();
    assert_eq!(result.kind, CaKind::Simulator);
    assert!(result.done);
}

/// Lists the real templates. Recording needs Developer Mode (`sudo DevToolsSecurity -enable`).
#[tokio::test]
#[ignore = "needs Xcode"]
async fn lists_instruments_templates_and_records_when_allowed() {
    use emulabzip_core::{Backend, TraceRequest};

    let backend = emulabzip_apple::AppleBackend::new();
    let templates = backend.trace_templates("any").await.unwrap();
    println!("{templates:?}");
    assert!(templates.iter().any(|t| t == "Time Profiler"));

    let Some(udid) = std::env::var("EMULABZIP_DEVICE").ok() else { return };
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let out = std::env::temp_dir().join(format!("emulabzip-xctrace-{}", std::process::id()));
    let request = TraceRequest::Instruments { template: "Time Profiler".into(), app: None, duration_s: 5 };
    match backend.record_trace(&udid, &request, &out, rx).await {
        Ok(file) => {
            println!("{file:#?}");
            for xml in &file.extras {
                let profile = emulabzip_trace::xctrace::parse(&std::fs::read_to_string(xml).unwrap()).unwrap();
                println!("{} samples, {:?}", profile.samples.len(), profile.processes().first());
            }
        }
        Err(e) => println!("not recorded: {e}"),
    }
    std::fs::remove_dir_all(out).ok();
}

/// Needs the crash app (`tools/crash-app-ios/install.sh`) to have crashed on the simulator in
/// `EMULABZIP_DEVICE` (see its README).
#[tokio::test]
#[ignore = "needs a simulator where the crash app has crashed"]
async fn lists_and_reads_real_crashes() {
    use emulabzip_core::Backend;

    let udid = std::env::var("EMULABZIP_DEVICE").expect("EMULABZIP_DEVICE=<simulator udid>");
    let backend = emulabzip_apple::AppleBackend::new();
    let list = backend.crashes(&udid, Some("dev.emulabzip.crashy")).await.unwrap();
    println!("{list:#?}");
    assert!(!list.is_empty());
    let report = backend.crash(&udid, &list[0].id).await.unwrap();
    println!("{}: {} threads", report.summary.title, report.threads.len());
    assert!(
        report.threads[0].crashed
            && report.threads[0].frames.iter().any(|f| f.origin == emulabzip_core::FrameOrigin::App)
    );
    assert!(backend.crashes(&udid, Some("com.not.installed")).await.unwrap().is_empty());
}

/// Applies the controls on a real simulator, reads back what simctl lets us read, and puts it the
/// way it was. Needs the crash app installed: `EMULABZIP_DEVICE=<udid>`.
#[tokio::test]
#[ignore = "changes the simulator's settings and puts them back"]
async fn applies_every_control_and_resets_it() {
    use emulabzip_core::{Appearance, Backend, Control, Grant, StatusBar};

    let udid = std::env::var("EMULABZIP_DEVICE").expect("EMULABZIP_DEVICE=<simulator udid>");
    let backend = emulabzip_apple::AppleBackend::new();
    let app = "dev.emulabzip.crashy";
    let simctl = |args: &[&str]| {
        let out = std::process::Command::new("xcrun").arg("simctl").args(args).output().unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    let info = backend.controls(&udid, None).await.unwrap();
    let original = info.appearance.expect("the simulator reports its appearance");
    let other = if original == Appearance::Dark { Appearance::Light } else { Appearance::Dark };
    backend.control(&udid, &Control::Appearance { appearance: other }).await.unwrap();
    assert_eq!(backend.controls(&udid, None).await.unwrap().appearance, Some(other));
    backend.control(&udid, &Control::Appearance { appearance: original }).await.unwrap();
    assert_eq!(backend.controls(&udid, None).await.unwrap().appearance, Some(original));

    let bar = StatusBar { time: Some("9:41".into()), battery: Some(100), wifi: Some(true), cellular: Some(true) };
    backend.control(&udid, &Control::StatusBar { status_bar: bar }).await.unwrap();
    let listed = simctl(&["status_bar", &udid, "list"]);
    assert!(listed.contains("9:41"), "{listed}");
    backend.control(&udid, &Control::ClearStatusBar).await.unwrap();
    assert!(!simctl(&["status_bar", &udid, "list"]).contains("9:41"));

    for grant in [Grant::Grant, Grant::Revoke, Grant::Reset] {
        let c = Control::Permission { app: app.into(), permission: "photos".into(), grant };
        backend.control(&udid, &c).await.unwrap();
    }
    let unknown = Control::Permission { app: app.into(), permission: "teleport".into(), grant: Grant::Grant };
    assert!(backend.control(&udid, &unknown).await.is_err());

    let push = Control::Notification {
        app: app.into(),
        title: "EmuLabZip".into(),
        body: "A simulated push".into(),
        payload: None,
    };
    backend.control(&udid, &push).await.unwrap();
    backend.control(&udid, &Control::OpenUrl { url: "https://example.com".into() }).await.unwrap();
    backend.control(&udid, &Control::Location { lat: "48.8584".into(), lon: "2.2945".into() }).await.unwrap();
    backend.control(&udid, &Control::ClearLocation).await.unwrap();
    assert!(backend.control(&udid, &Control::Battery { level: 10, charging: false }).await.is_err());
}

/// Needs Simulator.app open on the device and the Accessibility permission for the test runner.
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "needs a booted iOS simulator with its window open"]
async fn reads_the_layout_and_audits_it() {
    let backend = AppleBackend::new();
    let devices = backend.devices().await.unwrap();
    let id = devices.first().expect("a booted simulator").id.clone();
    let snapshot = backend.layout(&id).await.unwrap();
    println!("{} views on a {}×{} pt screen", snapshot.root.count(), snapshot.screen.width, snapshot.screen.height);
    snapshot.root.walk(&mut |n| {
        if n.name().is_some() && n.clickable {
            println!("{} {:?} {:?} {:?}", n.class, n.name(), n.resource_id, n.bounds);
        }
    });
    assert!(snapshot.root.count() > 3);
    assert!(snapshot.screenshot.starts_with(b"\x89PNG"));
    let audit = emulabzip_core::a11y::audit(&snapshot);
    for f in &audit.findings {
        println!("{:?} {} {} {}", f.severity, f.rule, f.class, f.message);
    }
    assert!(!audit.contrast_skipped);
}
