//! Putting the bytes of a TCP connection back in order.
//!
//! A capture has segments out of order, repeated (retransmissions) or missing. Each direction of a
//! connection gets a [`Stream`] that keeps what is contiguous and holds the rest back until the gap
//! fills. Every chunk remembers when it was captured, so a parsed message can be given a time.

use std::collections::BTreeMap;

/// The most one direction of one connection keeps, in bytes.
pub const STREAM_LIMIT: usize = 32 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct Stream {
    /// The sequence number the stream starts at, once it is known.
    isn: Option<u32>,
    /// The next offset that is expected.
    next: u64,
    pending: BTreeMap<u64, (Vec<u8>, u64)>,
    pub data: Vec<u8>,
    /// For each chunk appended to `data`: where it ends, and when it was captured (µs).
    marks: Vec<(u64, u64)>,
    /// Some segments never arrived, so `data` skips over them.
    pub gaps: bool,
    /// More arrived than [`STREAM_LIMIT`]; the rest was dropped.
    pub truncated: bool,
    /// When the first and the last bytes were captured (µs).
    pub first_ts: Option<u64>,
    pub last_ts: Option<u64>,
    /// The bytes that crossed the wire, counting repeats once.
    pub wire_bytes: u64,
}

impl Stream {
    /// Adds a segment. `syn` marks the one that opens the connection: it takes up one sequence number.
    pub fn add(&mut self, seq: u32, syn: bool, payload: &[u8], ts: u64) {
        if syn {
            self.isn = Some(seq.wrapping_add(1));
        }
        if payload.is_empty() {
            return;
        }
        let isn = *self.isn.get_or_insert(seq);
        let mut offset = u64::from(seq.wrapping_sub(isn));
        let mut payload = payload;
        let end = offset + payload.len() as u64;
        if end <= self.next {
            return; // a segment we already have
        }
        if offset < self.next {
            payload = &payload[(self.next - offset) as usize..];
            offset = self.next;
        }
        self.first_ts.get_or_insert(ts);
        self.last_ts = Some(self.last_ts.map_or(ts, |t| t.max(ts)));
        if offset == self.next {
            self.append(payload, ts);
            while let Some((&at, _)) = self.pending.first_key_value() {
                if at > self.next {
                    break;
                }
                let (chunk, t) = self.pending.remove(&at).expect("just seen");
                if at + chunk.len() as u64 > self.next {
                    let skip = (self.next - at) as usize;
                    self.append(&chunk[skip..], t);
                }
            }
        } else if self.pending.get(&offset).is_none_or(|(old, _)| old.len() < payload.len()) {
            self.pending.insert(offset, (payload.to_vec(), ts));
        }
    }

    fn append(&mut self, payload: &[u8], ts: u64) {
        self.next += payload.len() as u64;
        self.wire_bytes += payload.len() as u64;
        let room = STREAM_LIMIT.saturating_sub(self.data.len());
        self.data.extend_from_slice(&payload[..payload.len().min(room)]);
        self.truncated |= payload.len() > room;
        self.marks.push((self.data.len() as u64, ts));
    }

    /// Called when the capture ends: whatever is still held back is added, with the gaps skipped.
    pub fn finish(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for (at, (chunk, ts)) in pending {
            self.gaps |= at > self.next;
            let skip = self.next.saturating_sub(at) as usize;
            if skip < chunk.len() {
                self.next = at.max(self.next) + (chunk.len() - skip) as u64;
                self.wire_bytes += (chunk.len() - skip) as u64;
                let room = STREAM_LIMIT.saturating_sub(self.data.len());
                let part = &chunk[skip..];
                self.data.extend_from_slice(&part[..part.len().min(room)]);
                self.truncated |= part.len() > room;
                self.marks.push((self.data.len() as u64, ts));
            }
        }
    }

    /// When the first and the last of the bytes in `start..end` were captured (µs). Segments that
    /// arrived out of order make these differ from the times of the first and last byte.
    pub fn ts_range(&self, start: usize, end: usize) -> Option<(u64, u64)> {
        let (start, end) = (start as u64, end as u64);
        let mut from = 0u64;
        let mut found: Option<(u64, u64)> = None;
        for &(until, ts) in &self.marks {
            if until > start && from < end.max(start + 1) {
                found = Some(found.map_or((ts, ts), |(lo, hi)| (lo.min(ts), hi.max(ts))));
            }
            from = until;
        }
        found
    }

    /// When the byte at `offset` of `data` was captured (µs).
    pub fn ts_at(&self, offset: usize) -> Option<u64> {
        let offset = offset as u64;
        self.marks.iter().find(|(end, _)| *end > offset).or(self.marks.last()).map(|(_, ts)| *ts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_in_order_are_joined() {
        let mut s = Stream::default();
        s.add(100, true, &[], 1);
        s.add(101, false, b"hello ", 2);
        s.add(107, false, b"world", 3);
        assert_eq!(s.data, b"hello world");
        assert_eq!((s.first_ts, s.last_ts, s.wire_bytes), (Some(2), Some(3), 11));
        assert!(!s.gaps);
    }

    #[test]
    fn segments_out_of_order_wait_for_the_gap() {
        let mut s = Stream::default();
        s.add(0, true, &[], 1);
        s.add(7, false, b"world", 3);
        s.add(12, false, b"!", 4);
        assert!(s.data.is_empty(), "nothing is contiguous yet");
        s.add(1, false, b"hello ", 2);
        assert_eq!(s.data, b"hello world!");
        assert!(!s.gaps);
    }

    #[test]
    fn retransmissions_and_overlaps_do_not_repeat_bytes() {
        let mut s = Stream::default();
        s.add(0, true, &[], 1);
        s.add(1, false, b"abcdef", 2);
        s.add(1, false, b"abcdef", 3); // sent again
        s.add(4, false, b"defghi", 4); // overlaps the end
        s.add(2, false, b"bc", 5); // inside what we have
        assert_eq!(s.data, b"abcdefghi");
        assert_eq!(s.wire_bytes, 9);
    }

    #[test]
    fn a_missing_segment_is_skipped_at_the_end_and_noted() {
        let mut s = Stream::default();
        s.add(0, true, &[], 1);
        s.add(1, false, b"abc", 2);
        s.add(10, false, b"xyz", 3); // 4..10 never arrived
        s.finish();
        assert_eq!(s.data, b"abcxyz");
        assert!(s.gaps);
    }

    #[test]
    fn a_capture_that_starts_in_the_middle_starts_at_the_first_segment_it_has() {
        let mut s = Stream::default();
        s.add(5_000, false, b"mid", 1);
        s.add(5_003, false, b"dle", 2);
        assert_eq!(s.data, b"middle");
    }

    #[test]
    fn sequence_numbers_wrap_around() {
        let mut s = Stream::default();
        s.add(u32::MAX - 2, true, &[], 1);
        s.add(u32::MAX - 1, false, b"abc", 2);
        s.add(1, false, b"def", 3);
        assert_eq!(s.data, b"abcdef");
    }

    #[test]
    fn a_range_spans_the_times_of_the_segments_it_covers() {
        let mut s = Stream::default();
        s.add(0, true, &[], 0);
        s.add(5, false, b"bbbb", 20); // arrives before the segment in front of it
        s.add(1, false, b"aaaa", 30);
        // `bbbb` is appended after `aaaa` but was captured first.
        assert_eq!(s.ts_range(0, 8), Some((20, 30)));
        assert_eq!(s.ts_range(0, 4), Some((30, 30)));
        assert_eq!(s.ts_range(4, 8), Some((20, 20)));
        assert_eq!(s.ts_range(100, 120), None);
    }

    #[test]
    fn every_byte_has_the_time_of_its_segment() {
        let mut s = Stream::default();
        s.add(0, true, &[], 0);
        s.add(1, false, b"aaaa", 10);
        s.add(5, false, b"bbbb", 20);
        assert_eq!(
            (s.ts_at(0), s.ts_at(3), s.ts_at(4), s.ts_at(7), s.ts_at(99)),
            (Some(10), Some(10), Some(20), Some(20), Some(20))
        );
    }
}
