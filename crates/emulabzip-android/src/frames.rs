//! Frames the app drew too slowly, from `dumpsys gfxinfo <package> framestats`.
//!
//! Android keeps the timings of the last 120 frames of each window, in nanoseconds on the monotonic clock. The
//! sampler reads them about once a second, keeps the frames it has not seen before, and reports the slow ones.

use std::time::Duration;

use emulabzip_adb::{AdbClient, quote};
use emulabzip_core::{DeviceClock, Result, SlowFrame, Stream};

use crate::clock;

/// How often the frame timings are read. The device keeps about two seconds at 60 frames per second.
const POLL: Duration = Duration::from_millis(1000);
/// Without a deadline in the data, a frame is slow when it took this long: two frames at 60 Hz.
const FALLBACK_SLOW_MS: f64 = 33.3;
/// A frame has to be at least this long to count, whatever its deadline said.
const MIN_SLOW_MS: f64 = 20.0;
/// `Flags` bit 0: the window layout changed, so the frame is slow on purpose.
const FLAG_LAYOUT_CHANGED: u64 = 1;

/// One row of the `PROFILEDATA` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub flags: u64,
    pub intended_vsync_ns: u64,
    pub frame_completed_ns: u64,
    pub deadline_ns: Option<u64>,
}

impl Frame {
    pub fn duration_ms(&self) -> f64 {
        self.frame_completed_ns.saturating_sub(self.intended_vsync_ns) as f64 / 1.0e6
    }

    /// Whether the frame missed its deadline by enough to be a stutter.
    pub fn is_slow(&self) -> bool {
        if self.flags & FLAG_LAYOUT_CHANGED != 0 {
            return false;
        }
        let ms = self.duration_ms();
        match self.deadline_ns {
            Some(deadline) => self.frame_completed_ns > deadline && ms >= MIN_SLOW_MS,
            None => ms >= FALLBACK_SLOW_MS,
        }
    }
}

/// Reads every `---PROFILEDATA---` table in the output. Columns are found by their names in the header row, because
/// their order has changed between Android versions. Rows that make no sense (a frame that never completed) are left
/// out.
pub fn parse(text: &str) -> Vec<Frame> {
    let mut frames = Vec::new();
    let mut inside = false;
    let mut columns: Option<(usize, usize, usize, Option<usize>)> = None;
    for line in text.lines().map(str::trim) {
        if line == "---PROFILEDATA---" {
            inside = !inside;
            columns = None;
            continue;
        }
        if !inside || line.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.trim_end_matches(',').split(',').collect();
        let Some((flags, intended, completed, deadline)) = columns else {
            let find = |name: &str| cells.iter().position(|c| *c == name);
            columns = match (find("Flags"), find("IntendedVsync"), find("FrameCompleted")) {
                (Some(f), Some(i), Some(c)) => Some((f, i, c, find("FrameDeadline"))),
                _ => None,
            };
            continue;
        };
        let get = |i: usize| cells.get(i).and_then(|c| c.parse::<u64>().ok());
        let (Some(flags), Some(intended), Some(completed)) = (get(flags), get(intended), get(completed)) else {
            continue;
        };
        // A frame that was dropped before it was drawn has the largest possible value as its end.
        if completed <= intended || completed == i64::MAX as u64 {
            continue;
        }
        frames.push(Frame {
            flags,
            intended_vsync_ns: intended,
            frame_completed_ns: completed,
            deadline_ns: deadline.and_then(get).filter(|d| *d > 0 && *d != i64::MAX as u64),
        });
    }
    frames
}

/// Remembers what was reported, so each frame is reported once across overlapping readings.
#[derive(Debug, Default)]
pub struct Tracker {
    last_seen_ns: u64,
}

impl Tracker {
    /// Takes note of the frames that are already there, so they are not reported. Android keeps the last 120 frames of
    /// a window however old they are, so the first reading is mostly from before the recording started.
    pub fn prime(&mut self, frames: &[Frame]) {
        self.last_seen_ns = frames.iter().map(|f| f.intended_vsync_ns).max().unwrap_or(0);
    }

    /// The slow frames of `frames` that earlier readings did not have, oldest first.
    pub fn new_slow(&mut self, frames: &[Frame], clock: &DeviceClock) -> Vec<SlowFrame> {
        let newest = frames.iter().map(|f| f.intended_vsync_ns).max().unwrap_or(0);
        // The app restarted: its counters began again.
        if newest < self.last_seen_ns {
            self.last_seen_ns = 0;
        }
        let mut fresh: Vec<&Frame> =
            frames.iter().filter(|f| f.intended_vsync_ns > self.last_seen_ns && f.is_slow()).collect();
        fresh.sort_by_key(|f| f.intended_vsync_ns);
        self.last_seen_ns = self.last_seen_ns.max(newest);
        fresh
            .into_iter()
            .map(|f| SlowFrame {
                t_ms: clock.from_monotonic(f.intended_vsync_ns as f64 / 1.0e6).max(0) as u64,
                duration_ms: (f.duration_ms() * 10.0).round() as f32 / 10.0,
            })
            .collect()
    }
}

/// Follows the slow frames of `app` until the stream is dropped.
pub async fn follow(adb: &AdbClient, serial: &str, app: &str) -> Result<Stream<SlowFrame>> {
    let clock = clock::measure(adb, serial).await?;
    let command = format!("dumpsys gfxinfo {} framestats", quote::quote(app));
    let (adb, serial) = (adb.clone(), serial.to_string());
    let mut tracker = Tracker::default();
    if let Ok(out) = adb.shell(&serial, &command).await {
        tracker.prime(&parse(&out));
    }
    Ok(Stream::spawn(64, |tx| async move {
        let mut tick = tokio::time::interval(POLL);
        loop {
            tick.tick().await;
            let Ok(out) = adb.shell(&serial, &command).await else { continue };
            for frame in tracker.new_slow(&parse(&out), &clock) {
                if tx.send(frame).await.is_err() {
                    return;
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../tests/fixtures/gfxinfo-framestats.txt");

    fn clock() -> DeviceClock {
        DeviceClock { offset_ms: 0, tz_minutes: 0, boot_epoch_ms: 1_000_000 }
    }

    #[test]
    fn reads_the_rows_of_a_real_table_by_column_name() {
        let frames = parse(REAL);
        assert!(frames.len() >= 40, "{}", frames.len());
        let first = frames[0];
        assert_eq!(
            (first.flags, first.intended_vsync_ns, first.frame_completed_ns),
            (1, 216_898_299_599, 217_099_276_358)
        );
        assert_eq!(first.deadline_ns, Some(216_914_966_265));
    }

    #[test]
    fn a_frame_after_a_layout_change_is_never_slow() {
        // 200 ms, far past its deadline, but the first frame of a layout is slow on purpose.
        assert!(!parse(REAL)[0].is_slow());
    }

    #[test]
    fn a_frame_is_slow_when_it_misses_its_deadline_by_enough() {
        let frame = |duration_ms: u64, deadline_ms: u64| Frame {
            flags: 0,
            intended_vsync_ns: 1_000_000_000,
            frame_completed_ns: 1_000_000_000 + duration_ms * 1_000_000,
            deadline_ns: Some(1_000_000_000 + deadline_ms * 1_000_000),
        };
        assert!(frame(48, 16).is_slow());
        assert!(!frame(15, 16).is_slow(), "on time");
        assert!(!frame(18, 16).is_slow(), "late, but under the minimum");
    }

    #[test]
    fn without_a_deadline_two_vsyncs_is_the_line() {
        let frame = |ms: u64| Frame {
            flags: 0,
            intended_vsync_ns: 10,
            frame_completed_ns: 10 + ms * 1_000_000,
            deadline_ns: None,
        };
        assert!(frame(40).is_slow());
        assert!(!frame(30).is_slow());
    }

    #[test]
    fn rows_that_never_finished_are_skipped() {
        let text = "---PROFILEDATA---\nFlags,IntendedVsync,FrameCompleted,\n0,100,9223372036854775807,\n0,500,200,\n0,100,50000000,\n---PROFILEDATA---\n";
        let frames = parse(text);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame_completed_ns, 50_000_000);
    }

    #[test]
    fn overlapping_readings_report_each_frame_once() {
        let mut tracker = Tracker::default();
        let first = parse(REAL);
        let a = tracker.new_slow(&first, &clock());
        assert!(!a.is_empty(), "the real table has slow frames");
        assert!(a.windows(2).all(|w| w[0].t_ms <= w[1].t_ms));
        assert!(tracker.new_slow(&first, &clock()).is_empty(), "nothing is new the second time");
    }

    #[test]
    fn frames_from_before_the_recording_are_not_reported() {
        let mut tracker = Tracker::default();
        let table = parse(REAL);
        tracker.prime(&table);
        assert!(tracker.new_slow(&table, &clock()).is_empty(), "everything in the first reading is old");
        let later = Frame {
            flags: 0,
            intended_vsync_ns: table.iter().map(|f| f.intended_vsync_ns).max().unwrap() + 1_000_000_000,
            frame_completed_ns: table.iter().map(|f| f.intended_vsync_ns).max().unwrap() + 1_100_000_000,
            deadline_ns: None,
        };
        assert_eq!(tracker.new_slow(&[later], &clock()).len(), 1, "a frame after it is reported");
    }

    #[test]
    fn a_restarted_app_is_followed_from_the_start() {
        let mut tracker = Tracker::default();
        let slow = |at_ns: u64| Frame {
            flags: 0,
            intended_vsync_ns: at_ns,
            frame_completed_ns: at_ns + 60_000_000,
            deadline_ns: Some(at_ns + 16_000_000),
        };
        assert_eq!(tracker.new_slow(&[slow(9_000_000_000)], &clock()).len(), 1);
        // The counters went back to a smaller time.
        assert_eq!(tracker.new_slow(&[slow(1_000_000_000)], &clock()).len(), 1);
    }

    #[test]
    fn times_are_placed_on_the_computers_clock() {
        let mut tracker = Tracker::default();
        let frame = Frame {
            flags: 0,
            intended_vsync_ns: 5_000_000_000,
            frame_completed_ns: 5_050_000_000,
            deadline_ns: Some(5_016_000_000),
        };
        let out = tracker.new_slow(&[frame], &clock());
        assert_eq!(out, vec![SlowFrame { t_ms: 1_005_000, duration_ms: 50.0 }]);
    }

    #[test]
    fn garbage_gives_no_frames() {
        assert!(parse("").is_empty());
        assert!(parse("---PROFILEDATA---\nnot,a,header\n1,2,3\n---PROFILEDATA---").is_empty());
    }
}
