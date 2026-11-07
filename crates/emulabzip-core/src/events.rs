//! What happens on a device over time: taps, slow frames, screen video, and the device's clock.
//!
//! All times here are milliseconds since the Unix epoch **on this computer's clock**. A backend
//! converts what the device reports with [`DeviceClock`], so streams from different sources line
//! up on one axis.

use serde::{Deserialize, Serialize};

/// How the clock of a device relates to this computer's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceClock {
    /// The device's wall clock minus this computer's, in milliseconds.
    pub offset_ms: i64,
    /// The device's time zone, as minutes east of UTC. Log lines are printed in it.
    pub tz_minutes: i32,
    /// This computer's epoch time at the moment the device's monotonic clock read zero. It turns
    /// `CLOCK_MONOTONIC` readings (touch events, frame times) into epoch milliseconds.
    pub boot_epoch_ms: i64,
}

impl DeviceClock {
    /// A device wall-clock reading as this computer's epoch time.
    pub fn from_device(&self, device_ms: i64) -> i64 {
        device_ms - self.offset_ms
    }

    /// A reading of the device's monotonic clock as this computer's epoch time.
    pub fn from_monotonic(&self, monotonic_ms: f64) -> i64 {
        self.boot_epoch_ms + monotonic_ms.round() as i64
    }
}

/// What a finger did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputKind {
    Tap,
    LongPress,
    Swipe,
}

/// A touch gesture, with screen coordinates in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputEvent {
    /// When the finger went down.
    pub t_ms: u64,
    pub kind: InputKind,
    pub x: u32,
    pub y: u32,
    /// Where the finger came up, for a swipe.
    pub x2: u32,
    pub y2: u32,
    /// How long the finger stayed down.
    pub duration_ms: u32,
}

/// A frame the app took too long to draw.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlowFrame {
    /// When the frame was due.
    pub t_ms: u64,
    pub duration_ms: f32,
}

/// One piece of the screen recording. Recordings are cut into short pieces so the one around a
/// crash can be used as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSegment {
    /// File name inside the recording's folder.
    pub file: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// The longest piece of video, in seconds.
pub const MAX_SEGMENT_SECS: u32 = 180;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_device_and_monotonic_times() {
        let clock = DeviceClock { offset_ms: 2_000, tz_minutes: -180, boot_epoch_ms: 1_000_000 };
        assert_eq!(clock.from_device(10_000), 8_000);
        assert_eq!(clock.from_monotonic(500.4), 1_000_500);
    }
}
