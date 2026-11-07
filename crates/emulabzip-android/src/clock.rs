//! The device's clock, measured against this computer's, so what the device timestamps (log lines, touch events, frame
//! times) can be placed on one axis.

use std::time::{SystemTime, UNIX_EPOCH};

use emulabzip_adb::AdbClient;
use emulabzip_core::{DeviceClock, Error, Result};

/// One shell call: the wall clock in milliseconds, the uptime in seconds, and the zone as `+hhmm`.
pub const COMMAND: &str = "date +%s%3N; cat /proc/uptime; date +%z";

fn host_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Reads the device's clock. The device is asked once, and the answer is placed at the middle of the round trip, which
/// keeps the error to about half of it.
pub async fn measure(adb: &AdbClient, serial: &str) -> Result<DeviceClock> {
    let before = host_ms();
    let out = adb.shell(serial, COMMAND).await?;
    let after = host_ms();
    parse(&out, before + (after - before) / 2)
}

/// Turns the output of [`COMMAND`] into a clock. `host_mid_ms` is this computer's time when the device answered.
pub fn parse(output: &str, host_mid_ms: i64) -> Result<DeviceClock> {
    let mut lines = output.lines().map(str::trim).filter(|l| !l.is_empty());
    let wall = lines.next().and_then(parse_wall_ms).ok_or_else(|| bad("the date"))?;
    let uptime_ms = lines
        .next()
        .and_then(|l| l.split_whitespace().next())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| (s * 1000.0).round() as i64)
        .ok_or_else(|| bad("the uptime"))?;
    // A device without a zone in its output is taken as UTC.
    let tz_minutes = lines.next().and_then(parse_zone).unwrap_or(0);
    Ok(DeviceClock { offset_ms: wall - host_mid_ms, tz_minutes, boot_epoch_ms: host_mid_ms - uptime_ms })
}

fn bad(what: &str) -> Error {
    Error::parse("the device clock", format!("could not read {what}"))
}

/// Epoch milliseconds from `date +%s%3N`. Older images ignore `%3N` and print `<seconds>3N`, or only seconds.
fn parse_wall_ms(line: &str) -> Option<i64> {
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    match digits.len() {
        13.. => digits.parse().ok(),
        10..=12 => digits.parse::<i64>().ok().map(|s| s * 1000),
        _ => None,
    }
}

/// Minutes east of UTC from `+hhmm` or `-hhmm`.
fn parse_zone(line: &str) -> Option<i32> {
    let (sign, rest) = match line.as_bytes().first()? {
        b'+' => (1, &line[1..]),
        b'-' => (-1, &line[1..]),
        _ => return None,
    };
    if rest.len() != 4 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m) = (rest[..2].parse::<i32>().ok()?, rest[2..].parse::<i32>().ok()?);
    Some(sign * (h * 60 + m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_clock_uptime_and_zone() {
        // The device is 2 s ahead, and booted 1000 s before the answer.
        let clock = parse("1700000002000\n1000.50 3900.12\n-0300\n", 1_700_000_000_000).unwrap();
        assert_eq!(clock.offset_ms, 2_000);
        assert_eq!(clock.tz_minutes, -180);
        assert_eq!(clock.boot_epoch_ms, 1_700_000_000_000 - 1_000_500);
    }

    #[test]
    fn a_device_that_prints_only_seconds_is_still_usable() {
        let clock = parse("1700000000\n10.00 5.00\n+0530\n", 1_700_000_000_000).unwrap();
        assert_eq!((clock.offset_ms, clock.tz_minutes), (0, 330));
    }

    #[test]
    fn a_missing_zone_means_utc() {
        assert_eq!(parse("1700000000000\n1.0 1.0\n", 1_700_000_000_000).unwrap().tz_minutes, 0);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse("", 0).is_err());
        assert!(parse("not a number\n1.0 1.0\n", 0).is_err());
        assert!(parse("1700000000000\nnope\n", 0).is_err());
    }

    #[test]
    fn zones_are_validated() {
        assert_eq!(parse_zone("+0000"), Some(0));
        assert_eq!(parse_zone("+05:30"), None);
        assert_eq!(parse_zone("UTC"), None);
    }
}
