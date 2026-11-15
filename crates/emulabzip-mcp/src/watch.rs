//! Background watches: a stream of something the device does (log lines, performance samples, a page's console), kept in a
//! buffer the assistant reads from whenever it likes.
//!
//! A tool call cannot stay open while the assistant uses the app, so a watch runs in the background and `watch_read` hands
//! over what came in since the last read. Each watch keeps at most [`CAPACITY`] items, stops by itself when nobody reads it
//! for [`IDLE`], and at most [`MAX_WATCHES`] run at once.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokio::task::JoinHandle;

pub const CAPACITY: usize = 5_000;
pub const IDLE: Duration = Duration::from_secs(600);
pub const MAX_WATCHES: usize = 8;

/// What a watch has gathered.
#[derive(Default)]
pub struct Ring {
    items: VecDeque<(u64, Value)>,
    next_seq: u64,
    /// Items that were pushed out because nobody read them in time.
    dropped: u64,
    /// The watch ended, and why.
    pub ended: Option<String>,
}

impl Ring {
    pub fn push(&mut self, item: Value) {
        self.next_seq += 1;
        self.items.push_back((self.next_seq, item));
        if self.items.len() > CAPACITY {
            self.items.pop_front();
            self.dropped += 1;
        }
    }

    /// The items after `since` (everything kept when `None`), at most `limit` and about `budget` bytes, and the sequence
    /// number to ask for next. What does not fit stays for the next read, so a reply is never cut in the middle.
    pub fn read(&self, since: Option<u64>, limit: usize, budget: usize) -> (Vec<&Value>, u64, bool) {
        let after = since.unwrap_or(0);
        let missed = since.is_some_and(|s| self.items.front().is_some_and(|(first, _)| *first > s + 1));
        let mut out = Vec::new();
        let mut last = after;
        let mut size = 0;
        for (seq, item) in self.items.iter().filter(|(seq, _)| *seq > after).take(limit) {
            size += item.to_string().len();
            // One item always goes, so a single large one cannot block the watch.
            if size > budget && !out.is_empty() {
                break;
            }
            out.push(item);
            last = *seq;
        }
        // With nothing new the place is kept, so a later read finds what arrives.
        (out, last, missed)
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

pub struct Watch {
    pub id: u64,
    pub kind: String,
    pub device: String,
    pub detail: String,
    pub started: Instant,
    pub ring: Arc<Mutex<Ring>>,
    pub last_read: Arc<Mutex<Instant>>,
    pub task: JoinHandle<()>,
}

impl Watch {
    pub fn finished(&self) -> bool {
        self.task.is_finished()
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchInfo {
    pub id: u64,
    pub kind: String,
    pub device: String,
    pub detail: String,
    pub running: bool,
    pub ended: Option<String>,
    pub seconds: u64,
    pub buffered: usize,
    pub dropped: u64,
}

impl Watch {
    pub fn info(&self) -> WatchInfo {
        let ring = self.ring.lock().unwrap();
        WatchInfo {
            id: self.id,
            kind: self.kind.clone(),
            device: self.device.clone(),
            detail: self.detail.clone(),
            running: !self.finished(),
            ended: ring.ended.clone(),
            seconds: self.started.elapsed().as_secs(),
            buffered: ring.len(),
            dropped: ring.dropped(),
        }
    }
}

/// Whether a watch has been left alone long enough that it should end.
pub fn idle(last_read: &Mutex<Instant>) -> bool {
    last_read.lock().unwrap().elapsed() > IDLE
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_read_stops_before_it_gets_too_big_and_the_rest_waits() {
        let mut ring = Ring::default();
        for i in 0..10 {
            ring.push(json!({ "line": "x".repeat(100), "n": i }));
        }
        let (items, next, _) = ring.read(None, 100, 350);
        assert!(items.len() < 10 && !items.is_empty(), "{}", items.len());
        let (rest, _, _) = ring.read(Some(next), 100, usize::MAX);
        assert_eq!(items.len() + rest.len(), 10, "nothing is lost");
        // One big item still goes, so a watch cannot get stuck.
        let (one, _, _) = ring.read(None, 100, 1);
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn a_read_hands_over_what_is_new_and_says_where_to_continue() {
        let mut ring = Ring::default();
        for n in 1..=5 {
            ring.push(json!(n));
        }
        let (items, next, missed) = ring.read(None, 3, usize::MAX);
        assert_eq!((items.len(), next, missed), (3, 3, false));
        let (items, next, _) = ring.read(Some(next), 10, usize::MAX);
        assert_eq!((items, next), (vec![&json!(4), &json!(5)], 5));
        let (items, next, _) = ring.read(Some(next), 10, usize::MAX);
        assert!(items.is_empty());
        assert_eq!(next, 5, "an empty read keeps its place");
        ring.push(json!(6));
        assert_eq!(ring.read(Some(next), 10, usize::MAX).0, vec![&json!(6)]);
    }

    #[test]
    fn what_nobody_reads_is_dropped_and_said() {
        let mut ring = Ring::default();
        for n in 0..(CAPACITY + 10) {
            ring.push(json!(n));
        }
        assert_eq!((ring.len(), ring.dropped()), (CAPACITY, 10));
        let (items, _, missed) = ring.read(Some(2), 5, usize::MAX);
        assert!(missed, "the reader asked from before the oldest kept item");
        assert_eq!(items[0], &json!(10));
        assert!(!ring.read(Some(20), 5, usize::MAX).2);
    }
}
