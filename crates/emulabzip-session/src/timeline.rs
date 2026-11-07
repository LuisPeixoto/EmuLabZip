//! One time axis for everything a session recorded: touches, log problems, requests, slow frames, garbage
//! collections, crashes, screenshots and video.
//!
//! Every stream stamps time its own way (see [`crate::format`]); this module puts them on the same axis, in
//! milliseconds from the start of the recording.

use std::collections::BTreeMap;

use emulabzip_core::{InputKind, LogLevel};
use serde::{Deserialize, Serialize};

use crate::reader::Session;

/// The most events kept for each lane. The counts of what was left out are in [`LaneSummary`].
const LANE_CAP: usize = 3000;
/// Slow frames closer than this are one burst.
const BURST_GAP_MS: u64 = 250;
/// Longest label kept, in characters.
const LABEL_MAX: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Lane {
    Taps,
    Logs,
    Requests,
    Frames,
    Gc,
    Crashes,
    Screens,
    Video,
}

impl Lane {
    pub const ALL: [Lane; 8] =
        [Lane::Taps, Lane::Logs, Lane::Requests, Lane::Frames, Lane::Gc, Lane::Crashes, Lane::Screens, Lane::Video];

    pub fn name(self) -> &'static str {
        match self {
            Lane::Taps => "taps",
            Lane::Logs => "logs",
            Lane::Requests => "requests",
            Lane::Frames => "frames",
            Lane::Gc => "gc",
            Lane::Crashes => "crashes",
            Lane::Screens => "screens",
            Lane::Video => "video",
        }
    }

    pub fn parse(name: &str) -> Option<Lane> {
        let name = name.trim().to_ascii_lowercase();
        Lane::ALL.into_iter().find(|l| l.name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Where the times of device-stamped lines came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClockSource {
    /// Measured against the device when the recording started.
    Measured,
    /// A recording made before the clock was measured: the zone and offset were worked out from the first log line.
    Estimated,
    /// Nothing in the session needed the device's clock.
    NotNeeded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Milliseconds from the start of the recording.
    pub t_ms: u64,
    /// How long it lasted, for things that take time (a request, a collection, a piece of video).
    pub dur_ms: Option<u64>,
    pub lane: Lane,
    pub severity: Severity,
    pub label: String,
    pub detail: Option<String>,
    /// Where to find it in the session: the position in its list (`logs`, the HAR entries, `crashes`, `screens`,
    /// `video`).
    pub index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneSummary {
    pub lane: Lane,
    /// How many there were.
    pub total: usize,
    /// How many are in the timeline. Fewer than `total` when the lane was cut to its cap.
    pub shown: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub start_ms: u64,
    pub duration_ms: u64,
    pub clock: ClockSource,
    pub lanes: Vec<LaneSummary>,
    /// Sorted by time.
    pub events: Vec<Event>,
}

impl Timeline {
    /// The events of some lanes (all of them when `lanes` is empty).
    pub fn only(&self, lanes: &[Lane]) -> Vec<&Event> {
        self.events.iter().filter(|e| lanes.is_empty() || lanes.contains(&e.lane)).collect()
    }
}

/// Puts everything the session recorded on one axis.
pub fn build(session: &Session) -> Timeline {
    let start = session.meta.started_ms;
    let mut lanes: BTreeMap<Lane, (usize, Vec<Event>)> = BTreeMap::new();
    let mut add = |event: Event| {
        let slot = lanes.entry(event.lane).or_default();
        slot.0 += 1;
        slot.1.push(event);
    };
    let rel = |epoch_ms: i64| (epoch_ms - start as i64).max(0) as u64;

    for (i, tap) in session.input.iter().enumerate() {
        let label = match tap.kind {
            InputKind::Tap => format!("Tap ({}, {})", tap.x, tap.y),
            InputKind::LongPress => format!("Long press ({}, {})", tap.x, tap.y),
            InputKind::Swipe => format!("Swipe ({}, {}) to ({}, {})", tap.x, tap.y, tap.x2, tap.y2),
        };
        add(Event {
            t_ms: rel(tap.t_ms as i64),
            dur_ms: Some(u64::from(tap.duration_ms)),
            lane: Lane::Taps,
            severity: Severity::Info,
            label,
            detail: None,
            index: Some(i),
        });
    }

    let (log_times, clock) = LogTimes::new(session);
    for (i, entry) in session.logs.iter().enumerate() {
        let Some(at) = log_times.epoch_ms(&entry.time) else { continue };
        if let Some(gc) = parse_gc(&entry.message) {
            let end = rel(at);
            add(Event {
                t_ms: end.saturating_sub(gc.total_ms.round() as u64),
                dur_ms: Some(gc.total_ms.round() as u64),
                lane: Lane::Gc,
                severity: Severity::Info,
                label: format!("GC {:.0} ms, paused {:.0} ms", gc.total_ms, gc.paused_ms),
                detail: Some(format!("{} freed {}", gc.kind, gc.freed)),
                index: Some(i),
            });
        }
        let severity = match entry.level {
            LogLevel::Error | LogLevel::Fatal => Severity::Error,
            LogLevel::Warn => Severity::Warning,
            _ => continue,
        };
        add(Event {
            t_ms: rel(at),
            dur_ms: None,
            lane: Lane::Logs,
            severity,
            label: clip(&format!("{}: {}", entry.tag, entry.message)),
            detail: None,
            index: Some(i),
        });
    }

    if let Some(entries) = session.har.as_ref().and_then(|h| h.pointer("/log/entries")).and_then(|v| v.as_array()) {
        for (i, e) in entries.iter().enumerate() {
            let Some(at) = e.get("startedDateTime").and_then(|v| v.as_str()).and_then(parse_iso) else { continue };
            let status = e.pointer("/response/status").and_then(|v| v.as_u64()).unwrap_or(0);
            let ms = e.get("time").and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
            let method = e.pointer("/request/method").and_then(|v| v.as_str()).unwrap_or("GET");
            let url = e.pointer("/request/url").and_then(|v| v.as_str()).unwrap_or("");
            let failure = e.get("comment").and_then(|v| v.as_str()).filter(|c| !c.is_empty());
            let severity = match (status, failure) {
                (s, _) if s >= 500 => Severity::Error,
                (0, Some(_)) => Severity::Error,
                (s, _) if s >= 400 => Severity::Warning,
                _ => Severity::Info,
            };
            add(Event {
                t_ms: rel(at),
                dur_ms: Some(ms.round() as u64),
                lane: Lane::Requests,
                severity,
                label: clip(&format!(
                    "{method} {} {}",
                    short_url(url),
                    if status == 0 { "failed".into() } else { status.to_string() }
                )),
                detail: failure.map(str::to_string),
                index: Some(i),
            });
        }
    }

    // A scroll that stutters makes dozens of slow frames in a row. They are one event: a burst.
    let mut frames: Vec<&emulabzip_core::SlowFrame> = session.slow_frames.iter().collect();
    frames.sort_by_key(|f| f.t_ms);
    let mut bursts: Vec<Vec<&emulabzip_core::SlowFrame>> = Vec::new();
    for frame in frames {
        match bursts.last_mut() {
            Some(burst)
                if burst.last().is_some_and(|l| frame.t_ms <= l.t_ms + l.duration_ms.round() as u64 + BURST_GAP_MS) =>
            {
                burst.push(frame);
            }
            _ => bursts.push(vec![frame]),
        }
    }
    for burst in bursts {
        let (first, last) = (burst[0], burst[burst.len() - 1]);
        let worst = burst.iter().map(|f| f.duration_ms).fold(0.0f32, f32::max);
        let span = (last.t_ms + last.duration_ms.round() as u64).saturating_sub(first.t_ms);
        add(Event {
            t_ms: rel(first.t_ms as i64),
            dur_ms: Some(span),
            lane: Lane::Frames,
            severity: if worst >= 100.0 { Severity::Error } else { Severity::Warning },
            label: if burst.len() == 1 {
                format!("Slow frame, {worst:.0} ms")
            } else {
                format!("{} slow frames, worst {worst:.0} ms", burst.len())
            },
            detail: None,
            index: None,
        });
    }

    let end_of_run = session.meta.duration_ms().unwrap_or(0);
    for (i, crash) in session.crashes.iter().enumerate() {
        let at = match crash.summary.time_ms {
            Some(ms) => match session.meta.clock {
                Some(c) => rel(c.from_device(ms as i64)),
                None => rel(ms as i64),
            },
            None => end_of_run,
        };
        add(Event {
            t_ms: at,
            dur_ms: None,
            lane: Lane::Crashes,
            severity: Severity::Error,
            label: clip(&crash.summary.title),
            detail: Some(crash.summary.process.clone()),
            index: Some(i),
        });
    }

    for (i, shot) in session.screen_times.iter().enumerate() {
        add(Event {
            t_ms: rel(shot.t_ms as i64),
            dur_ms: None,
            lane: Lane::Screens,
            severity: Severity::Info,
            label: "Screenshot".into(),
            detail: Some(shot.name.clone()),
            index: Some(i),
        });
    }

    for (i, piece) in session.video.iter().enumerate() {
        let (from, to) = (rel(piece.start_ms as i64), rel(piece.end_ms as i64));
        add(Event {
            t_ms: from,
            dur_ms: Some(to.saturating_sub(from)),
            lane: Lane::Video,
            severity: Severity::Info,
            label: format!("Video {}", piece.file),
            detail: Some(piece.file.clone()),
            index: Some(i),
        });
    }

    // Perf samples are drawn as charts by the viewer; the timeline only needs to know how long the run was.
    let perf_origin = session.meta.perf_started_ms.unwrap_or(start);
    let perf_end = session.perf.last().map_or(0, |s| rel((perf_origin + s.t_ms) as i64));

    let mut summaries = Vec::new();
    let mut events = Vec::new();
    for lane in Lane::ALL {
        let Some((total, mut list)) = lanes.remove(&lane) else { continue };
        list.sort_by_key(|e| e.t_ms);
        if list.len() > LANE_CAP {
            list = thin(list);
        }
        summaries.push(LaneSummary { lane, total, shown: list.len() });
        events.append(&mut list);
    }
    events.sort_by_key(|a| (a.t_ms, a.lane));
    let duration_ms =
        events.iter().map(|e| e.t_ms + e.dur_ms.unwrap_or(0)).max().unwrap_or(0).max(end_of_run).max(perf_end);
    Timeline { start_ms: start, duration_ms, clock, lanes: summaries, events }
}

/// Cuts a lane to its cap. Worse events are kept first (errors, then warnings), then the rest are spread evenly.
fn thin(list: Vec<Event>) -> Vec<Event> {
    let (mut keep, rest): (Vec<Event>, Vec<Event>) = list.into_iter().partition(|e| e.severity == Severity::Error);
    keep.truncate(LANE_CAP);
    let room = LANE_CAP - keep.len();
    if room > 0 && !rest.is_empty() {
        let step = rest.len().div_ceil(room).max(1);
        keep.extend(rest.into_iter().step_by(step).take(room));
    }
    keep.sort_by_key(|e| e.t_ms);
    keep
}

fn clip(text: &str) -> String {
    let flat: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if flat.chars().count() <= LABEL_MAX {
        flat
    } else {
        let mut out: String = flat.chars().take(LABEL_MAX - 1).collect();
        out.push('…');
        out
    }
}

/// `https://api.example.com/v1/items?x=1` as `api.example.com/v1/items`.
fn short_url(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['?', '#']).next().unwrap_or(rest).to_string()
}

// ---------------------------------------------------------------------------------------------------------------
// Time

/// Turns the printed time of a log line (`10-23 14:02:11.123`, no year, in the device's zone) into epoch milliseconds
/// on this computer's clock.
struct LogTimes {
    /// Zone plus clock offset, in milliseconds: `host epoch = local time read as UTC - fold`.
    fold_ms: i64,
    anchor_ms: i64,
}

impl LogTimes {
    fn new(session: &Session) -> (LogTimes, ClockSource) {
        let start = session.meta.started_ms as i64;
        if let Some(c) = session.meta.clock {
            return (
                LogTimes { fold_ms: i64::from(c.tz_minutes) * 60_000 + c.offset_ms, anchor_ms: start },
                ClockSource::Measured,
            );
        }
        // A recording made before the clock was measured: the first line says what the zone must have been, because
        // it was written within moments of the start.
        let first = session.logs.iter().find_map(|l| log_fields(&l.time));
        let Some(first) = first else {
            return (LogTimes { fold_ms: 0, anchor_ms: start }, ClockSource::NotNeeded);
        };
        let guess = LogTimes { fold_ms: 0, anchor_ms: start };
        let as_utc = guess.resolve(first);
        let quarter = 15 * 60_000;
        let fold = ((as_utc - start) as f64 / quarter as f64).round() as i64 * quarter;
        // Nothing sensible is further than a day from the start.
        let fold = if fold.abs() > 14 * 3_600_000 { 0 } else { fold };
        (LogTimes { fold_ms: fold, anchor_ms: start }, ClockSource::Estimated)
    }

    fn epoch_ms(&self, printed: &str) -> Option<i64> {
        Some(self.resolve(log_fields(printed)?) - self.fold_ms)
    }

    /// The printed time read as UTC, in the year that puts it closest to the start of the recording.
    fn resolve(&self, f: LogFields) -> i64 {
        let around = year_of(self.anchor_ms + self.fold_ms);
        let at = |year: i64| {
            days_from_civil(year, f.month, f.day) * 86_400_000
                + i64::from(f.hour) * 3_600_000
                + i64::from(f.minute) * 60_000
                + i64::from(f.second) * 1000
                + i64::from(f.milli)
        };
        [around - 1, around, around + 1]
            .into_iter()
            .map(at)
            .min_by_key(|t| (t - (self.anchor_ms + self.fold_ms)).abs())
            .unwrap_or_else(|| at(around))
    }
}

#[derive(Clone, Copy)]
struct LogFields {
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    milli: u32,
}

/// `MM-DD HH:MM:SS.mmm`, the layout both logcat and the simulator's log print.
fn log_fields(text: &str) -> Option<LogFields> {
    let b = text.as_bytes();
    if b.len() < 14 || b[2] != b'-' || b[5] != b' ' || b[8] != b':' || b[11] != b':' {
        return None;
    }
    let num = |from: usize, to: usize| text.get(from..to)?.parse::<u32>().ok();
    let milli = match text.get(14..).and_then(|r| r.strip_prefix('.')) {
        Some(frac) => {
            let digits: String = frac.chars().take_while(char::is_ascii_digit).take(3).collect();
            format!("{digits:0<3}").parse::<u32>().ok()?
        }
        None => 0,
    };
    let f = LogFields {
        month: num(0, 2)?,
        day: num(3, 5)?,
        hour: num(6, 8)?,
        minute: num(9, 11)?,
        second: num(12, 14)?,
        milli,
    };
    ((1..=12).contains(&f.month) && (1..=31).contains(&f.day) && f.hour < 24 && f.minute < 60 && f.second < 61)
        .then_some(f)
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn year_of(epoch_ms: i64) -> i64 {
    let z = epoch_ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    yoe + era * 400 + i64::from(month <= 2)
}

/// An ISO 8601 time as HAR writes it (`2026-11-07T12:00:00.123Z`, or with an offset), in epoch milliseconds.
pub fn parse_iso(text: &str) -> Option<i64> {
    let (date, time) = text.split_once('T')?;
    let mut d = date.split('-');
    let (year, month, day) =
        (d.next()?.parse::<i64>().ok()?, d.next()?.parse::<u32>().ok()?, d.next()?.parse::<u32>().ok()?);
    let split = time.find(['Z', '+', '-']).unwrap_or(time.len());
    let (clock, zone) = time.split_at(split);
    let mut t = clock.split(':');
    let (hour, minute) = (t.next()?.parse::<i64>().ok()?, t.next()?.parse::<i64>().ok()?);
    let seconds = t.next().unwrap_or("0");
    let (whole, frac) = seconds.split_once('.').unwrap_or((seconds, ""));
    let second = whole.parse::<i64>().ok()?;
    let milli = format!("{:0<3}", frac.chars().take(3).collect::<String>()).parse::<i64>().ok()?;
    let offset_min = match zone.as_bytes().first() {
        None | Some(b'Z') => 0,
        Some(sign @ (b'+' | b'-')) => {
            let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
            if digits.len() < 4 {
                return None;
            }
            let minutes = digits[..2].parse::<i64>().ok()? * 60 + digits[2..4].parse::<i64>().ok()?;
            if *sign == b'-' { -minutes } else { minutes }
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    Some(days * 86_400_000 + hour * 3_600_000 + minute * 60_000 + second * 1000 + milli - offset_min * 60_000)
}

// ---------------------------------------------------------------------------------------------------------------
// Garbage collection

/// What an ART collection line says.
#[derive(Debug, Clone, PartialEq)]
pub struct Gc {
    /// For example `Background concurrent mark compact`.
    pub kind: String,
    /// What was freed, as printed (`5160KB AllocSpace bytes`, `3427(284KB) AllocSpace objects`).
    pub freed: String,
    /// All the pauses added up.
    pub paused_ms: f64,
    pub total_ms: f64,
}

/// Reads a collection line: `<kind> GC freed <what>, ..., paused <a>,<b> total <t>`.
pub fn parse_gc(message: &str) -> Option<Gc> {
    let (kind, rest) = message.split_once(" GC freed ")?;
    let (freed, tail) = rest.split_once(',')?;
    let paused_at = tail.find("paused ")?;
    let after = &tail[paused_at + "paused ".len()..];
    let (pauses, total) = after.split_once(" total ")?;
    let paused_ms = pauses.split(',').map(duration_ms).sum::<Option<f64>>()?;
    Some(Gc {
        kind: kind.trim().to_string(),
        freed: freed.trim().to_string(),
        paused_ms,
        total_ms: duration_ms(total)?,
    })
}

/// `1.230ms`, `421us`, `2s` as milliseconds. Anything after the unit (a trailing space, a period) is ignored.
fn duration_ms(text: &str) -> Option<f64> {
    let text = text.trim();
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let value: f64 = text[..split].parse().ok()?;
    let unit = &text[split..];
    if unit.starts_with("ms") {
        Some(value)
    } else if unit.starts_with("us") {
        Some(value / 1000.0)
    } else if unit.starts_with('s') {
        Some(value * 1000.0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_collection_lines_of_old_and_new_android() {
        let new = parse_gc("Background concurrent mark compact GC freed 5160KB AllocSpace bytes, 0(0B) LOS objects, 49% free, 6118KB/11MB, paused 2.134ms,4.023ms total 76.451ms").unwrap();
        assert_eq!(new.kind, "Background concurrent mark compact");
        assert_eq!(new.freed, "5160KB AllocSpace bytes");
        assert!((new.paused_ms - 6.157).abs() < 1e-9 && (new.total_ms - 76.451).abs() < 1e-9, "{new:?}");

        let old = parse_gc("Background concurrent copying GC freed 3427(284KB) AllocSpace objects, 0(0B) LOS objects, 49% free, 3MB/7MB, paused 421us total 31.002ms").unwrap();
        assert!((old.paused_ms - 0.421).abs() < 1e-9 && (old.total_ms - 31.002).abs() < 1e-9, "{old:?}");
    }

    #[test]
    fn other_messages_are_not_collections() {
        assert_eq!(parse_gc("Start proc 4321:com.example/u0a190 for activity"), None);
        assert_eq!(parse_gc("GC freed nothing useful"), None);
        assert_eq!(parse_gc("x GC freed 1KB, paused soon total later"), None);
    }

    #[test]
    fn durations_have_units() {
        assert_eq!(duration_ms("1.5ms"), Some(1.5));
        assert_eq!(duration_ms("250us"), Some(0.25));
        assert_eq!(duration_ms("2s"), Some(2000.0));
        assert_eq!(duration_ms("12"), None);
    }

    #[test]
    fn dates_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(year_of(days_from_civil(2026, 11, 7) * 86_400_000), 2026);
        assert_eq!(year_of(days_from_civil(2024, 2, 29) * 86_400_000 + 5), 2024);
        assert_eq!(year_of(days_from_civil(2025, 1, 1) * 86_400_000 - 1), 2024);
    }

    #[test]
    fn iso_times_with_and_without_offsets() {
        let utc = parse_iso("2026-11-07T12:00:00.123Z").unwrap();
        assert_eq!(utc, days_from_civil(2026, 11, 7) * 86_400_000 + 12 * 3_600_000 + 123);
        assert_eq!(parse_iso("2026-11-07T09:00:00.123-03:00"), Some(utc));
        assert_eq!(parse_iso("2026-11-07T12:00:00Z"), Some(utc - 123));
        assert_eq!(parse_iso("yesterday"), None);
    }

    #[test]
    fn log_times_are_read_in_both_layouts() {
        let f = log_fields("10-23 14:02:11.123").unwrap();
        assert_eq!((f.month, f.day, f.hour, f.minute, f.second, f.milli), (10, 23, 14, 2, 11, 123));
        assert_eq!(log_fields("10-23 14:02:11.5").unwrap().milli, 500);
        assert!(log_fields("not a time").is_none());
        assert!(log_fields("13-40 25:00:00.000").is_none());
    }

    #[test]
    fn labels_are_flat_and_short() {
        assert_eq!(clip("a\nb"), "a b");
        assert_eq!(clip(&"x".repeat(500)).chars().count(), LABEL_MAX);
        assert_eq!(short_url("https://api.example.com/v1/items?x=1#top"), "api.example.com/v1/items");
    }
}
