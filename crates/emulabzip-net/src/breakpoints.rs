//! Breakpoints: requests and responses the proxy holds until someone continues them.
//!
//! The proxy calls [`Breakpoints::pause`] and waits. Whoever shows the held message (the app)
//! lists them with [`Breakpoints::pending`], and answers with [`Breakpoints::resume`], possibly with
//! edits. A message nobody answers continues unchanged after [`TIMEOUT`], so a forgotten
//! breakpoint cannot hang an app for good.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tokio::sync::{broadcast, oneshot};

use crate::flow::{FlowId, Headers};

/// How long a message is held before it continues by itself.
pub const TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Request,
    Response,
}

/// A request or response that is being held.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paused {
    pub id: u64,
    pub flow: FlowId,
    pub phase: Phase,
    pub method: String,
    pub url: String,
    /// Set for a response.
    pub status: Option<u16>,
    pub headers: Headers,
    /// The body as text, when it is text.
    pub body_text: Option<String>,
    /// The body as base64, always.
    pub body_base64: String,
}

/// What to do with a held message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Edits {
    /// Replaces the headers when set.
    pub headers: Option<Headers>,
    /// Replaces the body when set.
    pub body: Option<Vec<u8>>,
    /// Replaces the status of a response when set.
    pub status: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resume {
    /// Go on, with these edits (none to send it as it is).
    Continue(Edits),
    /// Fail the request.
    Abort,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BreakpointEvent {
    Paused { paused: Box<Paused> },
    Resumed { id: u64 },
}

/// What a caller gives to [`Breakpoints::pause`].
pub struct Hold {
    pub flow: FlowId,
    pub phase: Phase,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub headers: Headers,
    pub body: Vec<u8>,
}

pub struct Breakpoints {
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, (Paused, oneshot::Sender<Resume>)>>,
    events: broadcast::Sender<BreakpointEvent>,
    timeout: Duration,
}

impl std::fmt::Debug for Breakpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Breakpoints").field("waiting", &self.waiting.lock().len()).finish()
    }
}

impl Default for Breakpoints {
    fn default() -> Self {
        Breakpoints::with_timeout(TIMEOUT)
    }
}

/// What a held message looks like to whoever shows it.
fn describe(id: u64, hold: Hold) -> Paused {
    use base64::Engine;
    Paused {
        id,
        flow: hold.flow,
        phase: hold.phase,
        method: hold.method,
        url: hold.url,
        status: hold.status,
        headers: hold.headers,
        body_text: std::str::from_utf8(&hold.body).ok().map(str::to_string),
        body_base64: base64::engine::general_purpose::STANDARD.encode(&hold.body),
    }
}

impl Breakpoints {
    pub fn new() -> Self {
        Breakpoints::default()
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Breakpoints {
            next: AtomicU64::new(1),
            waiting: Mutex::new(HashMap::new()),
            events: broadcast::channel(64).0,
            timeout,
        }
    }

    /// Holds a message until it is resumed or the timeout passes.
    pub async fn pause(&self, hold: Hold) -> Resume {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let paused = describe(id, hold);
        let (tx, rx) = oneshot::channel();
        self.waiting.lock().insert(id, (paused.clone(), tx));
        let _ = self.events.send(BreakpointEvent::Paused { paused: Box::new(paused) });
        let answer = tokio::time::timeout(self.timeout, rx).await;
        self.waiting.lock().remove(&id);
        let _ = self.events.send(BreakpointEvent::Resumed { id });
        // A timeout, or nobody left to answer, lets the message through as it was.
        answer.ok().and_then(|a| a.ok()).unwrap_or(Resume::Continue(Edits::default()))
    }

    /// Answers a held message. False when it is not held any more.
    pub fn resume(&self, id: u64, answer: Resume) -> bool {
        match self.waiting.lock().remove(&id) {
            Some((_, tx)) => tx.send(answer).is_ok(),
            None => false,
        }
    }

    /// Lets everything held through unchanged.
    pub fn resume_all(&self) {
        let ids: Vec<u64> = self.waiting.lock().keys().copied().collect();
        for id in ids {
            self.resume(id, Resume::Continue(Edits::default()));
        }
    }

    /// What is held now, oldest first.
    pub fn pending(&self) -> Vec<Paused> {
        let mut all: Vec<Paused> = self.waiting.lock().values().map(|(p, _)| p.clone()).collect();
        all.sort_by_key(|p| p.id);
        all
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BreakpointEvent> {
        self.events.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn hold(body: &[u8]) -> Hold {
        Hold {
            flow: 7,
            phase: Phase::Request,
            method: "POST".into(),
            url: "https://x.test/a".into(),
            status: None,
            headers: vec![("a".into(), "b".into())],
            body: body.to_vec(),
        }
    }

    #[tokio::test]
    async fn a_held_message_waits_for_an_answer_with_edits() {
        let bp = Arc::new(Breakpoints::new());
        let waiter = tokio::spawn({
            let bp = bp.clone();
            async move { bp.pause(hold(b"{\"a\":1}")).await }
        });
        let held = loop {
            if let Some(p) = bp.pending().first().cloned() {
                break p;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        };
        assert_eq!((held.flow, held.method.as_str(), held.body_text.as_deref()), (7, "POST", Some("{\"a\":1}")));
        assert_eq!(held.body_base64, "eyJhIjoxfQ==");
        let edits = Edits { body: Some(b"{\"a\":2}".to_vec()), ..Default::default() };
        assert!(bp.resume(held.id, Resume::Continue(edits.clone())));
        assert_eq!(waiter.await.unwrap(), Resume::Continue(edits));
        assert!(bp.pending().is_empty());
        assert!(!bp.resume(held.id, Resume::Abort), "a message can only be answered once");
    }

    #[tokio::test]
    async fn aborting_and_continuing_everything() {
        let bp = Arc::new(Breakpoints::new());
        let (a, b) = (
            tokio::spawn({
                let bp = bp.clone();
                async move { bp.pause(hold(b"1")).await }
            }),
            tokio::spawn({
                let bp = bp.clone();
                async move { bp.pause(hold(b"2")).await }
            }),
        );
        while bp.pending().len() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let first = bp.pending()[0].id;
        bp.resume(first, Resume::Abort);
        bp.resume_all();
        let (a, b) = (a.await.unwrap(), b.await.unwrap());
        assert!(a == Resume::Abort || b == Resume::Abort);
        assert!(a == Resume::Continue(Edits::default()) || b == Resume::Continue(Edits::default()));
    }

    #[tokio::test]
    async fn a_forgotten_breakpoint_lets_the_message_through() {
        let bp = Breakpoints::with_timeout(Duration::from_millis(50));
        assert_eq!(bp.pause(hold(b"x")).await, Resume::Continue(Edits::default()));
        assert!(bp.pending().is_empty());
    }

    #[tokio::test]
    async fn watchers_hear_about_held_and_resumed_messages() {
        let bp = Arc::new(Breakpoints::new());
        let mut events = bp.subscribe();
        let waiter = tokio::spawn({
            let bp = bp.clone();
            async move { bp.pause(hold(b"x")).await }
        });
        let BreakpointEvent::Paused { paused } = events.recv().await.unwrap() else { panic!("expected a pause") };
        bp.resume(paused.id, Resume::Abort);
        waiter.await.unwrap();
        assert!(matches!(events.recv().await.unwrap(), BreakpointEvent::Resumed { id } if id == paused.id));
    }

    #[test]
    fn binary_bodies_have_no_text() {
        let held = describe(1, hold(&[0xff, 0xfe, 0x00]));
        assert_eq!(held.body_text, None);
        assert_eq!(held.body_base64, "//4A");
        assert_eq!(describe(2, hold(b"")).body_text.as_deref(), Some(""));
    }
}
