//! Screen recording with `simctl io recordVideo`, in short pieces, and the simulator's clock.
//!
//! The Simulator runs on this Mac, so its clock is ours and its log lines are printed in our time zone.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use emulabzip_core::{DeviceClock, Error, Result, VideoSegment};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

use crate::simctl;

/// How long to wait for `recordVideo` to say it has its first frame.
const START_WAIT: Duration = Duration::from_secs(20);
/// How long to wait for the tool to close its file after it was interrupted.
const CLOSE_WAIT: Duration = Duration::from_secs(20);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// The Mac's clock. The offset is zero by definition; the zone is read from `date +%z`.
pub async fn clock() -> Result<DeviceClock> {
    let out = Command::new("date").arg("+%z").output().await?;
    let tz = parse_zone(String::from_utf8_lossy(&out.stdout).trim()).unwrap_or(0);
    Ok(DeviceClock { offset_ms: 0, tz_minutes: tz, boot_epoch_ms: 0 })
}

/// Minutes east of UTC from `+hhmm` or `-hhmm`.
pub fn parse_zone(text: &str) -> Option<i32> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = &text[1..];
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(sign * (digits[..2].parse::<i32>().ok()? * 60 + digits[2..].parse::<i32>().ok()?))
}

/// Records pieces of `segment_secs` until `stop` turns `true`. Each piece is a separate `recordVideo`, so there is a
/// gap of about a second between two of them.
pub async fn record(
    udid: &str,
    out_dir: &Path,
    segment_secs: u32,
    mut stop: watch::Receiver<bool>,
    segments: mpsc::UnboundedSender<VideoSegment>,
) -> Result<()> {
    let mut saved = 0u32;
    for n in 1u32.. {
        if *stop.borrow() {
            break;
        }
        let file = format!("{n:04}.mp4");
        let path = out_dir.join(&file);
        let mut child = simctl::spawn_with_stderr([
            "io".as_ref(),
            udid.as_ref(),
            "recordVideo".as_ref(),
            "--codec=h264".as_ref(),
            "--force".as_ref(),
            path.as_os_str(),
        ])?;
        let stderr = child.stderr.take().expect("stderr is piped");
        let mut lines = BufReader::new(stderr).lines();

        // The recording starts when the tool says so; that is the time the piece begins.
        let mut said = String::new();
        let started = tokio::time::timeout(START_WAIT, async {
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("Recording started") {
                    return true;
                }
                said.push_str(&line);
                said.push('\n');
            }
            false
        })
        .await
        .unwrap_or(false);
        if !started {
            let _ = child.kill().await;
            return if saved > 0 { Ok(()) } else { Err(Error::Simctl(first_line(&said, "recordVideo did not start"))) };
        }
        let start_ms = now_ms();

        let stopped = tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(u64::from(segment_secs))) => false,
            _ = wait_for_stop(&mut stop) => true,
        };
        if let Some(pid) = child.id() {
            // Ctrl-C makes recordVideo finish the movie.
            let _ = Command::new("kill").args(["-INT", &pid.to_string()]).status().await;
        }
        if tokio::time::timeout(CLOSE_WAIT, child.wait()).await.is_err() {
            let _ = child.kill().await;
        }
        let end_ms = now_ms();
        match tokio::fs::metadata(&path).await {
            Ok(m) if m.len() > 0 => {
                saved += 1;
                let _ = segments.send(VideoSegment { file, start_ms, end_ms });
            }
            _ => {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }
        if stopped {
            break;
        }
    }
    if saved == 0 { Err(Error::failed("nothing was recorded")) } else { Ok(()) }
}

async fn wait_for_stop(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow() {
        if stop.changed().await.is_err() {
            return;
        }
    }
}

fn first_line(text: &str, fallback: &str) -> String {
    text.lines().find(|l| !l.trim().is_empty()).map_or_else(|| fallback.to_string(), |l| l.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_zone() {
        assert_eq!(parse_zone("-0300"), Some(-180));
        assert_eq!(parse_zone("+0530"), Some(330));
        assert_eq!(parse_zone("UTC"), None);
        assert_eq!(parse_zone(""), None);
    }

    #[test]
    fn the_first_line_of_a_message_is_used() {
        assert_eq!(first_line("\n  Invalid device\nmore", "x"), "Invalid device");
        assert_eq!(first_line("", "fallback"), "fallback");
    }
}
