//! Screen recording with `screenrecord`, in short pieces.
//!
//! `screenrecord` stops by itself after at most three minutes, and an mp4 is only readable once the tool has closed it.
//! So the screen is recorded as a series of pieces. The next one starts before the last is pulled off the device, which
//! keeps the gap between them to a fraction of a second, and a stop (Ctrl-C to the tool) keeps what the current piece
//! has recorded.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use emulabzip_adb::{AdbClient, command};
use emulabzip_core::{Error, Result, VideoSegment};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::input::parse_wm_size;
use crate::trace::interrupt;

/// Where the shell user may write.
const REMOTE_DIR: &str = "/data/local/tmp";
/// The video bit rate. Short pieces at this rate stay small enough to attach to a report.
const BIT_RATE: u32 = 2_000_000;
/// The longest side of the recording, in pixels. The screen is scaled down to this.
const LONG_SIDE: u32 = 1280;
/// How long to wait for the tool to close its file after it was interrupted.
const CLOSE_GRACE: Duration = Duration::from_secs(15);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// The size to record at: the screen scaled so its long side is at most [`LONG_SIDE`], with both sides a multiple of
/// 16, which every AVC encoder accepts.
pub fn recording_size(width: u32, height: u32) -> (u32, u32) {
    let long = width.max(height).max(1);
    let scale = (f64::from(LONG_SIDE) / f64::from(long)).min(1.0);
    let round = |v: u32| (((f64::from(v) * scale / 16.0).round() as u32).max(1)) * 16;
    (round(width), round(height))
}

/// The shell command that records one piece, ending with what the tool printed (an error, usually).
pub fn command_for(file: &str, secs: u32, size: Option<(u32, u32)>) -> String {
    let mut args = vec![
        "screenrecord".to_string(),
        "--time-limit".into(),
        secs.to_string(),
        "--bit-rate".into(),
        BIT_RATE.to_string(),
    ];
    if let Some((w, h)) = size {
        args.push("--size".into());
        args.push(format!("{w}x{h}"));
    }
    args.push(file.to_string());
    format!("{} 2>&1", command(args))
}

/// Records pieces of `segment_secs` into `out_dir` until `stop` turns `true`, reporting each on `segments`.
pub async fn record(
    adb: &AdbClient,
    serial: &str,
    out_dir: &Path,
    segment_secs: u32,
    mut stop: watch::Receiver<bool>,
    segments: mpsc::UnboundedSender<VideoSegment>,
) -> Result<()> {
    let size =
        adb.shell(serial, "wm size").await.ok().as_deref().and_then(parse_wm_size).map(|(w, h)| recording_size(w, h));
    let token = format!("{:x}", now_ms());
    let mut pulls: JoinSet<Result<()>> = JoinSet::new();
    let mut produced = 0u32;
    let mut failure = None::<Error>;

    for n in 1u32.. {
        let remote = format!("{REMOTE_DIR}/emulabzip-video-{token}-{n}.mp4");
        let start_ms = now_ms();
        let line = command_for(&remote, segment_secs, size);
        let recording = adb.shell(serial, &line);
        tokio::pin!(recording);
        let (output, stopped) = loop {
            tokio::select! {
                out = &mut recording => break (out, *stop.borrow()),
                changed = stop.changed() => {
                    if changed.is_err() || *stop.borrow() {
                        // Ctrl-C makes screenrecord close the file properly.
                        let _ = adb.shell(serial, &interrupt("screenrecord", &remote)).await;
                        break (tokio::time::timeout(CLOSE_GRACE, &mut recording).await.unwrap_or_else(|_| {
                            Err(Error::failed("screenrecord did not close its file in time"))
                        }), true);
                    }
                }
            }
        };
        let end_ms = now_ms();
        match output {
            Ok(said) => {
                let (adb, serial, out_dir, segments) =
                    (adb.clone(), serial.to_string(), out_dir.to_path_buf(), segments.clone());
                let file = format!("{n:04}.mp4");
                pulls.spawn(async move {
                    let path = out_dir.join(&file);
                    let mut local = tokio::fs::File::create(&path).await?;
                    let size = adb.pull(&serial, &remote, &mut local).await;
                    drop(local);
                    let _ = adb.shell(&serial, &command(["rm", "-f", &remote])).await;
                    match size {
                        Ok(bytes) if bytes > 0 => {
                            let _ = segments.send(VideoSegment { file, start_ms, end_ms });
                            Ok(())
                        }
                        _ => {
                            let _ = tokio::fs::remove_file(&path).await;
                            Err(Error::failed(failure_text(&said)))
                        }
                    }
                });
                produced += 1;
            }
            Err(e) => {
                failure = Some(e);
                let _ = adb.shell(serial, &command(["rm", "-f", &remote])).await;
            }
        }
        if stopped || failure.is_some() {
            break;
        }
    }

    let mut saved = 0u32;
    while let Some(done) = pulls.join_next().await {
        match done {
            Ok(Ok(())) => saved += 1,
            Ok(Err(e)) => failure = failure.or(Some(e)),
            Err(_) => {}
        }
    }
    // A recording that produced nothing says why; one that lost its last piece is still a recording.
    match failure {
        Some(e) if saved == 0 => Err(e),
        Some(e) => {
            tracing::warn!("the last piece of video was not kept: {e}");
            Ok(())
        }
        None if produced == 0 => Err(Error::failed("nothing was recorded")),
        None => Ok(()),
    }
}

/// What `screenrecord` printed when it made no video: its own message, or a reminder of the usual causes.
fn failure_text(said: &str) -> String {
    let said = said.trim();
    if said.is_empty() {
        "screenrecord made no video. The screen may be secure or off, or the device may not support screen recording."
            .into()
    } else {
        format!("screenrecord made no video: {}", said.lines().next().unwrap_or(said))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_size_is_scaled_to_the_long_side_and_aligned_to_16() {
        assert_eq!(recording_size(1080, 2400), (576, 1280));
        assert_eq!(recording_size(2400, 1080), (1280, 576));
        // Already small: kept, only aligned.
        assert_eq!(recording_size(720, 1280), (720, 1280));
        assert_eq!(recording_size(0, 0), (16, 16));
    }

    #[test]
    fn the_command_records_one_piece_and_keeps_the_tools_message() {
        let cmd = command_for("/data/local/tmp/a.mp4", 30, Some((576, 1280)));
        assert_eq!(cmd, "screenrecord --time-limit 30 --bit-rate 2000000 --size 576x1280 /data/local/tmp/a.mp4 2>&1");
        assert!(!command_for("/x.mp4", 5, None).contains("--size"));
    }

    #[test]
    fn a_failure_names_the_tool_or_the_usual_causes() {
        assert!(failure_text("").contains("secure"));
        assert!(failure_text("ERROR: unsupported size\nmore").ends_with("unsupported size"));
    }
}
