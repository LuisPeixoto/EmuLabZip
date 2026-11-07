//! Touches on the screen, read from the kernel's input device with `getevent` and grouped into gestures.
//!
//! Only touches that arrive through the input device are seen: a finger on the emulator window, or events sent through
//! the emulator console. `adb shell input tap` is injected higher up, inside Android, and never reaches the device.

use emulabzip_adb::{AdbClient, quote};
use emulabzip_core::{DeviceClock, Error, InputEvent, InputKind, Result, Stream};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::clock;

/// A finger held down longer than this, without moving, is a long press.
const LONG_PRESS_MS: u32 = 500;
/// A finger that travels further than this is a swipe, in screen pixels.
const SWIPE_PX: f64 = 24.0;

/// The input device that reports where fingers are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touchscreen {
    pub path: String,
    pub max_x: u32,
    pub max_y: u32,
}

/// Finds the touchscreen in the output of `getevent -pl`: the device with `ABS_MT_POSITION_X`.
pub fn find_touchscreen(text: &str) -> Option<Touchscreen> {
    let mut path = None::<String>;
    let (mut max_x, mut max_y) = (None, None);
    let mut found = None;
    let mut finish = |path: &mut Option<String>, max_x: &mut Option<u32>, max_y: &mut Option<u32>| {
        if found.is_none()
            && let (Some(p), Some(x), Some(y)) = (path.take(), max_x.take(), max_y.take())
        {
            found = Some(Touchscreen { path: p, max_x: x, max_y: y });
        }
        *path = None;
        *max_x = None;
        *max_y = None;
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("add device ") {
            finish(&mut path, &mut max_x, &mut max_y);
            path = rest.split_once(": ").map(|(_, p)| p.trim().to_string());
        } else if line.contains("ABS_MT_POSITION_X") {
            max_x = axis_max(line);
        } else if line.contains("ABS_MT_POSITION_Y") {
            max_y = axis_max(line);
        }
    }
    finish(&mut path, &mut max_x, &mut max_y);
    found.filter(|t| t.path.starts_with("/dev/input/") && t.max_x > 0 && t.max_y > 0)
}

fn axis_max(line: &str) -> Option<u32> {
    let after = line.split("max ").nth(1)?;
    after.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

/// The screen size in pixels from `wm size`. An override (set with `wm size WxH`) wins over the physical size.
pub fn parse_wm_size(text: &str) -> Option<(u32, u32)> {
    let pick = |label: &str| {
        text.lines().find_map(|l| {
            let size = l.trim().strip_prefix(label)?.trim();
            let (w, h) = size.split_once('x')?;
            Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
    };
    pick("Override size:").or_else(|| pick("Physical size:"))
}

/// Groups the lines of `getevent -lt` into gestures, one finger at a time.
pub struct Grouper {
    touchscreen: Touchscreen,
    screen: (u32, u32),
    clock: DeviceClock,
    slot: i64,
    // What has been read since the last SYN_REPORT.
    tracking: Option<i64>,
    x: Option<i64>,
    y: Option<i64>,
    time: f64,
    // The finger on the screen, if there is one.
    down: Option<Down>,
}

struct Down {
    t_ms: u64,
    x0: f64,
    y0: f64,
    x: f64,
    y: f64,
    travelled: f64,
    started: f64,
}

impl Grouper {
    pub fn new(touchscreen: Touchscreen, screen: (u32, u32), clock: DeviceClock) -> Self {
        Grouper { touchscreen, screen, clock, slot: 0, tracking: None, x: None, y: None, time: 0.0, down: None }
    }

    fn to_px(&self, x: i64, y: i64) -> (f64, f64) {
        let sx = f64::from(self.screen.0.saturating_sub(1)) / f64::from(self.touchscreen.max_x);
        let sy = f64::from(self.screen.1.saturating_sub(1)) / f64::from(self.touchscreen.max_y);
        (x as f64 * sx, y as f64 * sy)
    }

    /// The host epoch time of a timestamp printed by `getevent`. A small number is seconds since boot; a big one is
    /// the wall clock of the device.
    fn epoch_ms(&self, seconds: f64) -> u64 {
        let t = if seconds > 1.0e9 {
            self.clock.from_device((seconds * 1000.0) as i64)
        } else {
            self.clock.from_monotonic(seconds * 1000.0)
        };
        t.max(0) as u64
    }

    /// Reads one line. Returns a gesture when the finger has just come up.
    pub fn feed(&mut self, line: &str) -> Option<InputEvent> {
        let (time, rest) = line.trim().strip_prefix('[')?.split_once(']')?;
        let time: f64 = time.trim().parse().ok()?;
        let mut words = rest.split_whitespace();
        let mut kind = words.next()?;
        // With several devices, the path comes first: `/dev/input/event2: EV_ABS ...`.
        if kind.ends_with(':') {
            kind = words.next()?;
        }
        let code = words.next()?;
        let value = words.next();
        self.time = time;
        match (kind, code) {
            ("EV_ABS", "ABS_MT_SLOT") => self.slot = i64::from_str_radix(value?, 16).ok()?,
            ("EV_ABS", _) if self.slot != 0 => {}
            // The id is printed as a 32-bit hex number, and -1 (no finger) comes out as ffffffff.
            ("EV_ABS", "ABS_MT_TRACKING_ID") => {
                self.tracking = Some(i64::from(u32::from_str_radix(value?, 16).ok()? as i32));
            }
            ("EV_ABS", "ABS_MT_POSITION_X") => self.x = Some(i64::from_str_radix(value?, 16).ok()?),
            ("EV_ABS", "ABS_MT_POSITION_Y") => self.y = Some(i64::from_str_radix(value?, 16).ok()?),
            ("EV_SYN", "SYN_REPORT") => return self.report(),
            _ => {}
        }
        None
    }

    fn report(&mut self) -> Option<InputEvent> {
        let (tracking, x, y) = (self.tracking.take(), self.x.take(), self.y.take());
        let moved = x.is_some() || y.is_some();
        let mut finished = None;
        match (&mut self.down, tracking) {
            (None, Some(id)) if id != -1 && moved => {
                let (px, py) = self.to_px(x.unwrap_or(0), y.unwrap_or(0));
                self.down = Some(Down {
                    t_ms: self.epoch_ms(self.time),
                    x0: px,
                    y0: py,
                    x: px,
                    y: py,
                    travelled: 0.0,
                    started: self.time,
                });
            }
            (Some(_), Some(-1)) => finished = self.down.take(),
            (Some(_), _) if moved => {
                let (nx, ny) = (x, y);
                let last = self.down.as_ref().map(|d| (d.x, d.y)).unwrap_or_default();
                let sx = f64::from(self.screen.0.saturating_sub(1)) / f64::from(self.touchscreen.max_x);
                let sy = f64::from(self.screen.1.saturating_sub(1)) / f64::from(self.touchscreen.max_y);
                if let Some(d) = self.down.as_mut() {
                    d.x = nx.map_or(last.0, |v| v as f64 * sx);
                    d.y = ny.map_or(last.1, |v| v as f64 * sy);
                    d.travelled = d.travelled.max(((d.x - d.x0).powi(2) + (d.y - d.y0).powi(2)).sqrt());
                }
            }
            _ => {}
        }
        let d = finished?;
        let held = ((self.time - d.started) * 1000.0).round().max(0.0) as u32;
        let kind = if d.travelled > SWIPE_PX {
            InputKind::Swipe
        } else if held >= LONG_PRESS_MS {
            InputKind::LongPress
        } else {
            InputKind::Tap
        };
        Some(InputEvent {
            t_ms: d.t_ms,
            kind,
            x: d.x0.round() as u32,
            y: d.y0.round() as u32,
            x2: d.x.round() as u32,
            y2: d.y.round() as u32,
            duration_ms: held,
        })
    }
}

/// Follows the touches until the stream is dropped.
pub async fn follow(adb: &AdbClient, serial: &str) -> Result<Stream<InputEvent>> {
    let axes = adb.shell(serial, "getevent -pl").await?;
    let touchscreen =
        find_touchscreen(&axes).ok_or(Error::Unsupported("Recording touches on a device with no touchscreen"))?;
    let size = adb.shell(serial, "wm size").await?;
    let screen = parse_wm_size(&size).ok_or_else(|| Error::failed("could not read the screen size"))?;
    let clock = clock::measure(adb, serial).await?;
    let socket = adb.shell_stream(serial, &format!("getevent -lt {}", quote::quote(&touchscreen.path))).await?;
    Ok(Stream::spawn(64, |tx| async move {
        let mut reader = BufReader::new(socket);
        let mut grouper = Grouper::new(touchscreen, screen, clock);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Some(event) = grouper.feed(&line)
                && tx.send(event).await.is_err()
            {
                break;
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PL: &str = include_str!("../tests/fixtures/getevent-pl.txt");
    const TAP: &str = include_str!("../tests/fixtures/getevent-tap.txt");

    fn grouper() -> Grouper {
        let screen = Touchscreen { path: "/dev/input/event2".into(), max_x: 32767, max_y: 32767 };
        // Boot at epoch 1_000_000 ms, so a reading of 134.837795 s is at 1_134_838.
        Grouper::new(screen, (1080, 2400), DeviceClock { offset_ms: 0, tz_minutes: 0, boot_epoch_ms: 1_000_000 })
    }

    fn gestures(text: &str) -> Vec<InputEvent> {
        let mut g = grouper();
        text.lines().filter_map(|l| g.feed(l)).collect()
    }

    #[test]
    fn finds_the_touchscreen_among_the_input_devices() {
        let t = find_touchscreen(PL).unwrap();
        assert_eq!(t, Touchscreen { path: "/dev/input/event2".into(), max_x: 32767, max_y: 32767 });
    }

    #[test]
    fn a_device_with_no_touchscreen_has_none() {
        assert_eq!(find_touchscreen("add device 1: /dev/input/event0\n  name: \"Power Button\"\n"), None);
        assert_eq!(find_touchscreen(""), None);
    }

    #[test]
    fn reads_the_screen_size_and_prefers_an_override() {
        assert_eq!(parse_wm_size("Physical size: 1080x2400\n"), Some((1080, 2400)));
        assert_eq!(parse_wm_size("Physical size: 1080x2400\nOverride size: 720x1600\n"), Some((720, 1600)));
        assert_eq!(parse_wm_size("nonsense"), None);
    }

    #[test]
    fn a_real_tap_becomes_one_event_at_the_right_place_and_time() {
        let events = gestures(TAP);
        assert_eq!(events.len(), 1, "{events:?}");
        let e = events[0];
        assert_eq!(e.kind, InputKind::Tap);
        // 16000 / 32767 of 1079 and 20000 / 32767 of 2399.
        assert_eq!((e.x, e.y), (527, 1464));
        assert_eq!(e.t_ms, 1_134_838);
        assert_eq!(e.duration_ms, 184);
    }

    fn lines(down: f64, up: f64, moves: &[(f64, i64, i64)], start: (i64, i64), end: (i64, i64)) -> String {
        let mut s = format!(
            "[ {down:.6}] EV_ABS ABS_MT_TRACKING_ID 00000001\n[ {down:.6}] EV_ABS ABS_MT_POSITION_X {:08x}\n[ {down:.6}] EV_ABS ABS_MT_POSITION_Y {:08x}\n[ {down:.6}] EV_SYN SYN_REPORT 00000000\n",
            start.0, start.1
        );
        for (t, x, y) in moves {
            s += &format!(
                "[ {t:.6}] EV_ABS ABS_MT_POSITION_X {x:08x}\n[ {t:.6}] EV_ABS ABS_MT_POSITION_Y {y:08x}\n[ {t:.6}] EV_SYN SYN_REPORT 00000000\n"
            );
        }
        s += &format!(
            "[ {up:.6}] EV_ABS ABS_MT_POSITION_X {:08x}\n[ {up:.6}] EV_ABS ABS_MT_TRACKING_ID ffffffff\n[ {up:.6}] EV_SYN SYN_REPORT 00000000\n",
            end.0
        );
        s
    }

    #[test]
    fn a_long_hold_is_a_long_press() {
        let events = gestures(&lines(10.0, 10.9, &[], (16000, 16000), (16000, 16000)));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, InputKind::LongPress);
        assert_eq!(events[0].duration_ms, 900);
    }

    #[test]
    fn a_finger_that_travels_is_a_swipe() {
        let events =
            gestures(&lines(10.0, 10.3, &[(10.1, 20000, 8000), (10.2, 24000, 4000)], (6000, 28000), (26000, 3000)));
        assert_eq!(events.len(), 1);
        let e = events[0];
        assert_eq!(e.kind, InputKind::Swipe);
        assert!(e.x2 > e.x + 400 && e.y2 + 400 < e.y, "{e:?}");
    }

    #[test]
    fn a_second_finger_is_ignored() {
        let mut text = lines(10.0, 10.1, &[], (16000, 16000), (16000, 16000));
        // A finger in slot 1 that comes and goes inside the first one's gesture changes nothing.
        text = text.replace(
            "[ 10.100000] EV_ABS ABS_MT_POSITION_X",
            "[ 10.050000] EV_ABS ABS_MT_SLOT 00000001\n[ 10.050000] EV_ABS ABS_MT_TRACKING_ID 00000009\n[ 10.050000] EV_ABS ABS_MT_POSITION_X 00000001\n[ 10.050000] EV_SYN SYN_REPORT 00000000\n[ 10.060000] EV_ABS ABS_MT_SLOT 00000000\n[ 10.100000] EV_ABS ABS_MT_POSITION_X",
        );
        let events = gestures(&text);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].kind, InputKind::Tap);
    }

    #[test]
    fn lines_with_a_device_path_and_noise_are_read_too() {
        let mut g = grouper();
        let text = "add device 1: /dev/input/event2\n[ 5.0] /dev/input/event2: EV_ABS ABS_MT_TRACKING_ID 00000002\n[ 5.0] /dev/input/event2: EV_ABS ABS_MT_POSITION_X 00001000\n[ 5.0] /dev/input/event2: EV_SYN SYN_REPORT 00000000\n[ 5.1] /dev/input/event2: EV_ABS ABS_MT_TRACKING_ID ffffffff\n[ 5.1] /dev/input/event2: EV_SYN SYN_REPORT 00000000\n???\n";
        assert_eq!(text.lines().filter_map(|l| g.feed(l)).count(), 1);
    }
}
