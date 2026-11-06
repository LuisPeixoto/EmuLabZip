//! Network conditions: a slow, laggy, lossy or missing connection between the device and the proxy.
//!
//! The proxy sits in the middle of everything the device sends, so it can shape the link: limit how fast bytes
//! go each way, delay the answers, stall now and then like a lost packet would, or cut the connection.
//!
//! A TCP proxy cannot lose packets (the device's TCP does the retransmitting on its side of the link), so loss is
//! shown the way an app feels it: a burst of data stalls for about a retransmission timeout, now and then.
//! HTTP/3 over QUIC does not go through the proxy's TCP link and is not shaped.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

/// A burst of data that comes after this long a pause pays the latency again.
const IDLE: Duration = Duration::from_millis(50);
/// How long a stalled burst waits, besides the round trip: a retransmission timeout.
const RETRANSMIT: Duration = Duration::from_millis(200);
/// The size of a packet, for working out how likely a burst is to lose one.
const PACKET: usize = 1400;

/// What the link is like. The default is no shaping at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Conditions {
    /// What the preset is called, when it came from one.
    pub name: Option<String>,
    /// Speed toward the device, in kilobits per second. `None` is as fast as it goes.
    pub down_kbps: Option<u32>,
    /// Speed from the device, in kilobits per second.
    pub up_kbps: Option<u32>,
    /// Time added to every answer, in milliseconds.
    pub latency_ms: u32,
    /// How much of the traffic stalls like a lost packet, in percent (0 to 100).
    pub loss_percent: f32,
    /// No connection at all.
    pub offline: bool,
}

/// The presets: id, label.
pub const PRESETS: &[(&str, &str)] = &[
    ("fast-3g", "Fast 3G"),
    ("slow-3g", "Slow 3G"),
    ("slow-wifi", "Slow Wi-Fi"),
    ("high-latency", "High latency"),
    ("packet-loss", "Packet loss"),
    ("offline", "Offline"),
];

impl Conditions {
    pub fn preset(id: &str) -> Option<Conditions> {
        let named = |down, up, latency_ms, loss_percent, offline| {
            let label = PRESETS.iter().find(|(i, _)| *i == id).map(|(_, l)| l.to_string());
            Conditions { name: label, down_kbps: down, up_kbps: up, latency_ms, loss_percent, offline }
        };
        Some(match id {
            "fast-3g" => named(Some(1_440), Some(675), 150, 0.0, false),
            "slow-3g" => named(Some(500), Some(500), 400, 0.0, false),
            "slow-wifi" => named(Some(2_000), Some(1_000), 80, 0.0, false),
            "high-latency" => named(None, None, 600, 0.0, false),
            "packet-loss" => named(None, None, 0, 5.0, false),
            "offline" => named(None, None, 0, 0.0, true),
            _ => return None,
        })
    }

    /// Whether anything is changed about the link.
    pub fn is_active(&self) -> bool {
        self.offline
            || self.down_kbps.is_some()
            || self.up_kbps.is_some()
            || self.latency_ms > 0
            || self.loss_percent > 0.0
    }

    /// A short text for a badge: `Slow 3G`, or the numbers when it is custom.
    pub fn summary(&self) -> String {
        if !self.is_active() {
            return "No limits".into();
        }
        if let Some(name) = &self.name {
            return name.clone();
        }
        let mut parts = Vec::new();
        if let Some(d) = self.down_kbps {
            parts.push(format!("{d} kb/s down"));
        }
        if let Some(u) = self.up_kbps {
            parts.push(format!("{u} kb/s up"));
        }
        if self.latency_ms > 0 {
            parts.push(format!("{} ms", self.latency_ms));
        }
        if self.loss_percent > 0.0 {
            parts.push(format!("{}% loss", self.loss_percent));
        }
        parts.join(", ")
    }

    /// Checks the numbers, so a typo cannot freeze the device.
    pub fn check(&self) -> Result<(), String> {
        for (what, v) in [("download", self.down_kbps), ("upload", self.up_kbps)] {
            if v == Some(0) {
                return Err(format!("The {what} speed cannot be 0: use Offline to cut the connection."));
            }
        }
        if self.latency_ms > 30_000 {
            return Err("Latency can be 30 seconds at most.".into());
        }
        if !(0.0..=100.0).contains(&self.loss_percent) {
            return Err("Loss is a percentage from 0 to 100.".into());
        }
        Ok(())
    }
}

/// The conditions in force, shared by every connection and changeable while they run.
#[derive(Debug, Default)]
pub struct Shaper {
    current: RwLock<Conditions>,
}

impl Shaper {
    pub fn new(conditions: Conditions) -> Arc<Shaper> {
        Arc::new(Shaper { current: RwLock::new(conditions) })
    }

    pub fn get(&self) -> Conditions {
        self.current.read().clone()
    }

    pub fn set(&self, conditions: Conditions) {
        *self.current.write() = conditions;
    }

    pub fn offline(&self) -> bool {
        self.current.read().offline
    }
}

/// A connection with the shared conditions applied to it. Reads are what the device sends; writes are what it receives.
pub struct Shaped<S> {
    inner: S,
    shaper: Arc<Shaper>,
    write: Lane,
    read: Lane,
    rng: Mutex<u64>,
}

/// One direction of the link.
#[derive(Default)]
struct Lane {
    sleep: Option<Pin<Box<Sleep>>>,
    /// The wait for this chunk is over.
    ready: bool,
    /// When the link is free for the next chunk.
    free_at: Option<Instant>,
    /// When the last chunk went by.
    last: Option<Instant>,
}

impl<S> Shaped<S> {
    pub fn new(inner: S, shaper: Arc<Shaper>) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15)
            | 1;
        Shaped { inner, shaper, write: Lane::default(), read: Lane::default(), rng: Mutex::new(seed) }
    }

    fn chance(&self) -> f64 {
        let mut x = self.rng.lock();
        *x ^= *x << 13;
        *x ^= *x >> 7;
        *x ^= *x << 17;
        (*x >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// How much to move at once so that the pace is smooth: about 20 ms of data at the speed.
fn chunk_for(kbps: Option<u32>, want: usize) -> usize {
    match kbps {
        None => want.min(64 * 1024),
        Some(k) => (k as usize * 1000 / 8 / 50).clamp(256, 16 * 1024).min(want),
    }
}

fn transfer_time(bytes: usize, kbps: Option<u32>) -> Duration {
    match kbps {
        None => Duration::ZERO,
        Some(k) => Duration::from_secs_f64(bytes as f64 * 8.0 / (k as f64 * 1000.0)),
    }
}

impl<S> Shaped<S> {
    /// Waits for the link: the latency of a new burst, the time the previous chunk is still using, a stall
    /// for loss. Returns `Ready` when the chunk can go.
    fn wait(lane: &mut Lane, cx: &mut Context<'_>, latency: Duration, loss: Option<(f32, usize, f64)>) -> Poll<()> {
        loop {
            if let Some(sleep) = lane.sleep.as_mut() {
                ready!(sleep.as_mut().poll(cx));
                lane.sleep = None;
                lane.ready = true;
            }
            if lane.ready {
                return Poll::Ready(());
            }
            let now = Instant::now();
            let mut start = now;
            if lane.last.is_none_or(|l| now.duration_since(l) > IDLE) {
                start += latency;
            }
            if let Some(free) = lane.free_at
                && free > start
            {
                start = free;
            }
            if let Some((percent, bytes, roll)) = loss {
                let packets = bytes.div_ceil(PACKET) as i32;
                let p = 1.0 - (1.0 - f64::from(percent) / 100.0).powi(packets);
                if roll < p {
                    start += RETRANSMIT + latency * 2;
                }
            }
            if start > now {
                lane.sleep = Some(Box::pin(tokio::time::sleep_until(start.into())));
            } else {
                lane.ready = true;
            }
        }
    }

    fn done(lane: &mut Lane, bytes: usize, kbps: Option<u32>) {
        let now = Instant::now();
        lane.ready = false;
        lane.last = Some(now);
        lane.free_at = Some(lane.free_at.map_or(now, |f| f.max(now)) + transfer_time(bytes, kbps));
    }
}

fn reset() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionReset, "The network conditions are set to offline.")
}

impl<S: AsyncRead + Unpin> AsyncRead for Shaped<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let c = self.shaper.get();
        if c.offline {
            return Poll::Ready(Err(reset()));
        }
        if c.up_kbps.is_none() {
            return Pin::new(&mut self.inner).poll_read(cx, buf);
        }
        let this = &mut *self;
        // Only the pace is limited on the way up: the latency is added once, on the way back.
        ready!(Shaped::<S>::wait(&mut this.read, cx, Duration::ZERO, None));
        let room = chunk_for(c.up_kbps, buf.remaining());
        let slice = buf.initialize_unfilled_to(room);
        let mut limited = ReadBuf::new(slice);
        match Pin::new(&mut this.inner).poll_read(cx, &mut limited) {
            Poll::Ready(Ok(())) => {
                let n = limited.filled().len();
                buf.advance(n);
                Shaped::<S>::done(&mut this.read, n, c.up_kbps);
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Shaped<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let c = self.shaper.get();
        if c.offline {
            return Poll::Ready(Err(reset()));
        }
        if !c.is_active() || buf.is_empty() {
            return Pin::new(&mut self.inner).poll_write(cx, buf);
        }
        let n = chunk_for(c.down_kbps, buf.len());
        let roll = if c.loss_percent > 0.0 { self.chance() } else { 1.0 };
        let this = &mut *self;
        let loss = (c.loss_percent > 0.0).then_some((c.loss_percent, n, roll));
        ready!(Shaped::<S>::wait(&mut this.write, cx, Duration::from_millis(u64::from(c.latency_ms)), loss));
        match Pin::new(&mut this.inner).poll_write(cx, &buf[..n]) {
            Poll::Ready(Ok(written)) => {
                Shaped::<S>::done(&mut this.write, written, c.down_kbps);
                Poll::Ready(Ok(written))
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

    use super::*;

    fn shaped(c: Conditions) -> (Shaped<tokio::io::DuplexStream>, tokio::io::DuplexStream) {
        let (a, b) = duplex(1 << 20);
        (Shaped::new(a, Shaper::new(c)), b)
    }

    #[test]
    fn presets_exist_and_are_checked() {
        for (id, label) in PRESETS {
            let c = Conditions::preset(id).unwrap_or_else(|| panic!("{id}"));
            assert!(c.is_active(), "{id}");
            assert_eq!(c.name.as_deref(), Some(*label));
            c.check().unwrap();
        }
        assert!(Conditions::preset("nope").is_none());
        assert_eq!(Conditions::preset("slow-3g").unwrap().latency_ms, 400);
        assert!(!Conditions::default().is_active());
        assert_eq!(Conditions::default().summary(), "No limits");
        let custom = Conditions { down_kbps: Some(100), latency_ms: 5, ..Default::default() };
        assert_eq!(custom.summary(), "100 kb/s down, 5 ms");
    }

    #[test]
    fn refuses_numbers_that_would_freeze_the_link() {
        assert!(Conditions { down_kbps: Some(0), ..Default::default() }.check().is_err());
        assert!(Conditions { latency_ms: 60_000, ..Default::default() }.check().is_err());
        assert!(Conditions { loss_percent: 120.0, ..Default::default() }.check().is_err());
    }

    #[tokio::test]
    async fn without_conditions_bytes_pass_at_full_speed() {
        let (mut s, mut peer) = shaped(Conditions::default());
        let data = vec![7u8; 500_000];
        let started = Instant::now();
        s.write_all(&data).await.unwrap();
        let mut got = vec![0u8; data.len()];
        peer.read_exact(&mut got).await.unwrap();
        assert!(started.elapsed() < Duration::from_millis(300));
    }

    #[tokio::test]
    async fn download_speed_is_limited() {
        // 25,000 bytes at 200 kb/s: one second.
        let (mut s, mut peer) = shaped(Conditions { down_kbps: Some(200), ..Default::default() });
        let data = vec![1u8; 25_000];
        let started = Instant::now();
        let reader = tokio::spawn(async move {
            let mut got = vec![0u8; 25_000];
            peer.read_exact(&mut got).await.unwrap();
        });
        s.write_all(&data).await.unwrap();
        reader.await.unwrap();
        let took = started.elapsed();
        assert!(took > Duration::from_millis(800) && took < Duration::from_millis(1800), "{took:?}");
    }

    #[tokio::test]
    async fn upload_speed_is_limited() {
        let (mut s, mut peer) = shaped(Conditions { up_kbps: Some(200), ..Default::default() });
        peer.write_all(&vec![2u8; 25_000]).await.unwrap();
        let started = Instant::now();
        let mut got = vec![0u8; 25_000];
        s.read_exact(&mut got).await.unwrap();
        assert!(got.iter().all(|b| *b == 2));
        let took = started.elapsed();
        assert!(took > Duration::from_millis(800) && took < Duration::from_millis(1800), "{took:?}");
    }

    #[tokio::test]
    async fn latency_is_paid_once_per_burst() {
        let (mut s, mut peer) = shaped(Conditions { latency_ms: 300, ..Default::default() });
        let started = Instant::now();
        s.write_all(b"hello").await.unwrap();
        s.write_all(b" again").await.unwrap();
        let mut got = [0u8; 11];
        peer.read_exact(&mut got).await.unwrap();
        let first = started.elapsed();
        assert!(first > Duration::from_millis(280) && first < Duration::from_millis(600), "{first:?}");
        // After a pause, it is a new burst.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let again = Instant::now();
        s.write_all(b"more").await.unwrap();
        assert!(again.elapsed() > Duration::from_millis(280));
    }

    #[tokio::test]
    async fn offline_resets_the_connection_and_can_be_lifted() {
        let (a, mut peer) = duplex(1024);
        let shaper = Shaper::new(Conditions::preset("offline").unwrap());
        let mut s = Shaped::new(a, shaper.clone());
        assert_eq!(s.write_all(b"x").await.unwrap_err().kind(), io::ErrorKind::ConnectionReset);
        assert_eq!(s.read(&mut [0u8; 4]).await.unwrap_err().kind(), io::ErrorKind::ConnectionReset);
        shaper.set(Conditions::default());
        s.write_all(b"back").await.unwrap();
        let mut got = [0u8; 4];
        peer.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"back");
    }

    #[tokio::test]
    async fn a_change_applies_to_a_connection_that_is_open() {
        let (mut s, mut peer) = shaped(Conditions::default());
        let shaper = s.shaper.clone();
        s.write_all(b"fast").await.unwrap();
        shaper.set(Conditions { latency_ms: 250, ..Default::default() });
        tokio::time::sleep(Duration::from_millis(80)).await;
        let started = Instant::now();
        s.write_all(b"slow").await.unwrap();
        assert!(started.elapsed() > Duration::from_millis(230));
        let mut got = [0u8; 8];
        peer.read_exact(&mut got).await.unwrap();
    }

    #[tokio::test]
    async fn loss_stalls_some_bursts() {
        let (mut s, mut peer) = shaped(Conditions { loss_percent: 100.0, ..Default::default() });
        let started = Instant::now();
        s.write_all(b"data").await.unwrap();
        assert!(started.elapsed() > Duration::from_millis(180), "every burst stalls at 100%");
        let mut got = [0u8; 4];
        peer.read_exact(&mut got).await.unwrap();
    }
}
