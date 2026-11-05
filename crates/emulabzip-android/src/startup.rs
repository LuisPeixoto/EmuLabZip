//! How long an app takes to open, timed with `am start -W`.
//!
//! `am start -W` waits until the first screen is drawn and prints how long that took. The three kinds
//! differ in what is done to the app before each opening:
//!
//! * **cold**: the process is stopped (`am force-stop`), so the system starts it from nothing;
//! * **warm**: the app is opened, then closed with the back button, so its process is alive but its screen is gone;
//! * **hot**: the app is opened, then sent to the background with the home button.

use std::time::Duration;

use emulabzip_adb::{AdbClient, command, is_package_name};
use emulabzip_core::{Error, Result, StartKind, StartupReport, StartupRun};

/// The launcher activity from the output of `cmd package resolve-activity --brief`: its last line.
pub fn parse_resolve(out: &str) -> Option<String> {
    let line = out.lines().map(str::trim).rfind(|l| !l.is_empty())?;
    (line.contains('/') && !line.contains(' ')).then(|| line.to_string())
}

/// One opening from the output of `am start -W`.
pub fn parse_start(out: &str) -> Result<StartupRun> {
    let field = |name: &str| out.lines().find_map(|l| l.trim().strip_prefix(name)).map(|v| v.trim().to_string());
    // `Error type 3` says little; the `Error:` line after it says why.
    let errors = || out.lines().filter(|l| l.trim_start().starts_with("Error"));
    if let Some(error) = errors().find(|l| l.trim_start().starts_with("Error:")).or_else(|| errors().next()) {
        return Err(Error::Adb(error.trim().to_string()));
    }
    match field("Status:").as_deref() {
        Some("ok") => {}
        Some(other) => return Err(Error::Adb(format!("the app did not open: {other}"))),
        None => return Err(Error::parse("am start output", out.lines().next().unwrap_or("it is empty"))),
    }
    let total_ms = field("TotalTime:")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Error::parse("am start output", "there is no TotalTime"))?;
    Ok(StartupRun {
        total_ms,
        wait_ms: field("WaitTime:").and_then(|v| v.parse().ok()),
        launch_state: field("LaunchState:"),
    })
}

async fn launcher(adb: &AdbClient, serial: &str, app: &str) -> Result<String> {
    let out = adb
        .shell(
            serial,
            &command([
                "cmd",
                "package",
                "resolve-activity",
                "--brief",
                "-a",
                "android.intent.action.MAIN",
                "-c",
                "android.intent.category.LAUNCHER",
                app,
            ]),
        )
        .await?;
    parse_resolve(&out).ok_or_else(|| Error::NotFound(format!("{app} is not installed, or has no screen to open")))
}

async fn open(adb: &AdbClient, serial: &str, component: &str) -> Result<StartupRun> {
    parse_start(&adb.shell(serial, &command(["am", "start", "-W", "-n", component])).await?)
}

async fn pause(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Opens the app `runs` times, preparing it before each opening according to `kind`.
pub async fn measure(adb: &AdbClient, serial: &str, app: &str, kind: StartKind, runs: u32) -> Result<StartupReport> {
    if !is_package_name(app) {
        return Err(Error::invalid("package name", app));
    }
    let component = launcher(adb, serial, app).await?;
    let mut done = Vec::new();
    for _ in 0..runs {
        match kind {
            StartKind::Cold => {
                adb.shell(serial, &command(["am", "force-stop", app])).await?;
                pause(700).await;
            }
            StartKind::Warm | StartKind::Hot => {
                // The app has to be running, and then taken off the screen in the right way. This opening is not
                // timed, and when the app is already in front the system prints no time for it.
                adb.shell(serial, &command(["am", "start", "-W", "-n", &component])).await?;
                pause(1200).await;
                let key = if kind == StartKind::Warm { "KEYCODE_BACK" } else { "KEYCODE_HOME" };
                adb.shell(serial, &command(["input", "keyevent", key])).await?;
                pause(1200).await;
            }
        }
        done.push(open(adb, serial, &component).await?);
        pause(800).await;
    }
    // Leave the device at its home screen, as it was found.
    let _ = adb.shell(serial, &command(["input", "keyevent", "KEYCODE_HOME"])).await;
    StartupReport::new(app, kind, done).ok_or_else(|| Error::failed("there was nothing to measure"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLD: &str = include_str!("../tests/fixtures/startup/am-start-cold.txt");
    const WARM: &str = include_str!("../tests/fixtures/startup/am-start-warm.txt");
    const HOT: &str = include_str!("../tests/fixtures/startup/am-start-hot.txt");

    #[test]
    fn reads_the_three_launch_states_of_a_real_device() {
        let cold = parse_start(COLD).unwrap();
        assert_eq!((cold.total_ms, cold.wait_ms, cold.launch_state.as_deref()), (797, Some(799), Some("COLD")));
        let warm = parse_start(WARM).unwrap();
        assert_eq!((warm.total_ms, warm.launch_state.as_deref()), (561, Some("WARM")));
        // A hot start prints a warning line before the numbers.
        let hot = parse_start(HOT).unwrap();
        assert_eq!((hot.total_ms, hot.wait_ms, hot.launch_state.as_deref()), (98, Some(104), Some("HOT")));
    }

    #[test]
    fn explains_an_app_that_would_not_open() {
        let err =
            parse_start("Starting: Intent { cmp=x/.Y }\nError type 3\nError: Activity class {x/x.Y} does not exist.\n")
                .unwrap_err();
        assert!(err.to_string().contains("does not exist"), "{err}");
        assert!(parse_start("Status: timeout\n").unwrap_err().to_string().contains("timeout"));
        assert!(parse_start("").is_err());
        assert!(parse_start("Status: ok\nLaunchState: COLD\n").is_err(), "no TotalTime");
    }

    #[test]
    fn finds_the_launcher_activity() {
        let found = include_str!("../tests/fixtures/startup/resolve-activity.txt");
        assert_eq!(parse_resolve(found).as_deref(), Some("dev.emulabzip.leaky/.MainActivity"));
        assert_eq!(parse_resolve(include_str!("../tests/fixtures/startup/resolve-activity-missing.txt")), None);
        assert_eq!(parse_resolve(""), None);
    }
}
