//! How long an app takes to open in the iOS Simulator, read from the system's own log.
//!
//! SpringBoard logs when it is asked to open an app, and again when the app's scene is fully active
//! (the launch animation is over and the app takes touches). The time between the two is what is
//! measured. It is the time a person waits, animation included, so compare runs with each other.
//!
//! iOS has two kinds of start that can be told apart: **cold** (the process is gone) and **hot** (it is
//! alive in the background). There is no separate "warm" start.

use std::time::Duration;

use emulabzip_core::{Error, Result, StartKind, StartupReport, StartupRun};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::simctl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// SpringBoard received the request to open the app.
    Requested,
    /// The app's scene became active.
    Active,
}

/// The milliseconds since midnight of a log line that starts `2026-11-05 19:23:59.290`.
pub fn timestamp_ms(line: &str) -> Option<u64> {
    let time = line.get(11..23)?;
    if line.as_bytes().get(4) != Some(&b'-') || line.as_bytes().get(10) != Some(&b' ') {
        return None;
    }
    let (h, rest) = time.split_once(':')?;
    let (m, rest) = rest.split_once(':')?;
    let (sec, ms) = rest.split_once('.')?;
    let (h, m, sec, ms) =
        (h.parse::<u64>().ok()?, m.parse::<u64>().ok()?, sec.parse::<u64>().ok()?, ms.parse::<u64>().ok()?);
    Some(((h * 60 + m) * 60 + sec) * 1000 + ms)
}

/// Whether a log line is one of the two moments, and when it happened.
pub fn marker(bundle: &str, line: &str) -> Option<(Marker, u64)> {
    let kind = if line.contains(&format!("Received request to open \"{bundle}\"")) {
        Marker::Requested
    } else if line.contains(&format!("[sceneID:{bundle}-default] Setting deactivation reasons to: '(none)'")) {
        Marker::Active
    } else {
        return None;
    };
    Some((kind, timestamp_ms(line)?))
}

/// Milliseconds from the request to the scene being active, across midnight too.
pub fn elapsed(start: u64, end: u64) -> u64 {
    if end >= start { end - start } else { end + 86_400_000 - start }
}

async fn simctl_timed<const N: usize>(args: [&str; N]) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(25), simctl::run(args))
        .await
        .map_err(|_| Error::Simctl("the simulator did not answer in time".into()))?
        .map(|_| ())
}

/// Opens the app `runs` times and times each opening.
pub async fn measure(udid: &str, app: &str, kind: StartKind, runs: u32) -> Result<StartupReport> {
    if kind == StartKind::Warm {
        return Err(Error::Unsupported("A warm start on iOS, which has cold and hot starts"));
    }
    if !emulabzip_core::control::is_app_id(app) {
        return Err(Error::invalid("bundle id", app));
    }
    let mut stream = Command::new("xcrun")
        .args([
            "simctl",
            "spawn",
            udid,
            "log",
            "stream",
            "--style",
            "compact",
            "--predicate",
            &format!("eventMessage CONTAINS \"{app}\""),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = stream.stdout.take().ok_or_else(|| Error::Simctl("could not read the log".into()))?;
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    // The stream says what it filters on before it reports anything.
    let ready = async {
        while let Some(line) = rx.recv().await {
            if line.starts_with("Filtering the log data") {
                return true;
            }
        }
        false
    };
    if !tokio::time::timeout(Duration::from_secs(15), ready).await.unwrap_or(false) {
        return Err(Error::Simctl("could not read the simulator's log".into()));
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut done = Vec::new();
    for _ in 0..runs {
        match kind {
            StartKind::Cold => {
                let _ = simctl_timed(["terminate", udid, app]).await;
                tokio::time::sleep(Duration::from_millis(1200)).await;
            }
            _ => {
                // Running, then covered by another app (Settings), so the next opening resumes it.
                simctl_timed(["launch", udid, app]).await?;
                tokio::time::sleep(Duration::from_millis(1500)).await;
                simctl_timed(["launch", udid, "com.apple.Preferences"]).await?;
                tokio::time::sleep(Duration::from_millis(1500)).await;
            }
        }
        while rx.try_recv().is_ok() {} // what the preparation logged does not count
        simctl_timed(["launch", udid, app]).await?;

        let (mut requested, mut active) = (None, None);
        let wait = async {
            while let Some(line) = rx.recv().await {
                match marker(app, &line) {
                    Some((Marker::Requested, t)) if requested.is_none() => requested = Some(t),
                    Some((Marker::Active, t)) if requested.is_some() && active.is_none() => {
                        active = Some(t);
                        return;
                    }
                    _ => {}
                }
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(15), wait).await;
        let (Some(start), Some(end)) = (requested, active) else {
            return Err(Error::Simctl(
                "the log did not show the app opening. Is the simulator's log working, and is the app installed?"
                    .into(),
            ));
        };
        done.push(StartupRun {
            total_ms: elapsed(start, end),
            wait_ms: None,
            launch_state: Some(kind.label().to_ascii_uppercase()),
        });
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    StartupReport::new(app, kind, done).ok_or_else(|| Error::failed("there was nothing to measure"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = include_str!("../tests/fixtures/startup-log.txt");
    const APP: &str = "dev.emulabzip.crashy";

    #[test]
    fn finds_the_two_moments_in_a_real_log() {
        let found: Vec<(Marker, u64)> = LOG.lines().filter_map(|l| marker(APP, l)).collect();
        // A cold start, then a hot one.
        assert_eq!(found.len(), 4, "{found:?}");
        assert_eq!(found[0], (Marker::Requested, ((19 * 60 + 23) * 60 + 59) * 1000 + 290));
        assert_eq!(elapsed(found[0].1, found[1].1), 382, "cold: 19:23:59.290 to 19:23:59.672");
        assert_eq!(elapsed(found[2].1, found[3].1), 503, "hot: 19:30:54.203 to 19:30:54.706");
    }

    #[test]
    fn the_intermediate_deactivation_is_not_the_end() {
        let line = "2026-11-05 19:23:59.418 Df SpringBoard[1:2] [com.apple.UIKit:SceneDeactivation] [sceneID:dev.emulabzip.crashy-default] Setting deactivation reasons to: 'systemAnimation' for reason: x";
        assert_eq!(marker(APP, line), None);
    }

    #[test]
    fn other_apps_are_not_mistaken_for_this_one() {
        let line =
            "2026-11-05 19:23:59.290 Df SpringBoard[1:2] [x] Received request to open \"dev.emulabzip.other\" from y";
        assert_eq!(marker(APP, line), None);
    }

    #[test]
    fn timestamps_and_midnight() {
        assert_eq!(timestamp_ms("2026-11-05 00:00:01.250 rest"), Some(1250));
        assert_eq!(timestamp_ms("Timestamp               Ty Process[PID:TID]"), None);
        assert_eq!(timestamp_ms(""), None);
        assert_eq!(elapsed(86_399_900, 100), 200);
    }

    #[tokio::test]
    async fn a_warm_start_is_not_a_thing_on_ios() {
        let err = measure("00000000-0000-0000-0000-000000000000", APP, StartKind::Warm, 1).await.unwrap_err();
        assert!(matches!(err, Error::Unsupported(_)), "{err}");
    }
}
