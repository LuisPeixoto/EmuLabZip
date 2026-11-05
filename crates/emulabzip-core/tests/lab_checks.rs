//! What the Lab refuses before any device is asked.

use emulabzip_core::{Error, Lab, Platform, StartKind, Target};

fn run<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(f)
}

fn target() -> Target {
    Target { platform: Platform::Android, id: "emulator-5554".into() }
}

#[test]
fn start_time_refuses_a_bad_app_or_a_bad_number_of_runs_before_looking_for_a_device() {
    // The Lab has no backends: any answer but `Invalid` would mean a device was looked up first.
    let lab = Lab::new();
    for (app, runs) in [("not an app; rm -rf /", 3), ("com.example.app", 0), ("com.example.app", 51)] {
        let err = run(lab.startup(&target(), app, StartKind::Cold, runs)).unwrap_err();
        assert!(matches!(err, Error::Invalid { .. }), "{app} x{runs}: {err}");
    }
}

#[test]
fn energy_refuses_a_bad_app_before_looking_for_a_device() {
    let err = run(Lab::new().energy_end(&target(), "../../etc", 10)).unwrap_err();
    assert!(matches!(err, Error::Invalid { .. }), "{err}");
}
