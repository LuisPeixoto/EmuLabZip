//! Reading WebSocket traffic (RFC 6455) as it passes through the proxy, without disturbing it.
//!
//! A [`FrameParser`] is fed the bytes of one direction in whatever pieces they arrive in, and hands
//! back whole messages: fragments are joined, masks are removed, and `permessage-deflate`
//! (RFC 7692) is inflated when the handshake agreed to it. The bytes themselves are forwarded as
//! they are by the caller; this only looks. Anything that does not parse as WebSocket makes the
//! parser stop quietly, so the connection keeps working.

use std::io::Write;

use base64::Engine;
use flate2::{Decompress, FlushDecompress, Status};
use serde::Serialize;

/// The most of one message that is kept. The rest is skipped, and the message says so.
pub const MAX_MESSAGE: usize = 64 * 1024;
/// The most messages a flow keeps.
pub const MAX_MESSAGES: usize = 1000;
/// The most a compressed message may inflate to, for the parser's own memory.
const MAX_INFLATED: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

/// Which way a message went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// From the client (the app) to the server.
    Up,
    /// From the server to the client.
    Down,
}

/// A whole message, as the parser found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub kind: Kind,
    pub payload: Vec<u8>,
    /// The size of the message, even when only part of it was kept.
    pub len: u64,
    pub truncated: bool,
}

/// A message as a flow stores it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsMessage {
    /// Milliseconds since the flow started.
    pub t_ms: u64,
    pub direction: Direction,
    pub kind: Kind,
    /// The text, for a text message.
    pub text: Option<String>,
    /// The bytes, for anything that is not text (and for text that is not valid UTF-8).
    pub base64: Option<String>,
    pub len: u64,
    pub truncated: bool,
}

impl Message {
    pub fn into_stored(self, t_ms: u64, direction: Direction) -> WsMessage {
        let (text, base64) = match (self.kind, std::str::from_utf8(&self.payload)) {
            (Kind::Text, Ok(text)) => (Some(text.to_string()), None),
            _ if self.payload.is_empty() => (None, None),
            _ => (None, Some(base64::engine::general_purpose::STANDARD.encode(&self.payload))),
        };
        WsMessage { t_ms, direction, kind: self.kind, text, base64, len: self.len, truncated: self.truncated }
    }
}

/// What the handshake agreed for `permessage-deflate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deflate {
    pub client_no_context_takeover: bool,
    pub server_no_context_takeover: bool,
}

/// Reads the `Sec-WebSocket-Extensions` header of the server's answer.
pub fn parse_extensions(value: &str) -> Option<Deflate> {
    let offer = value
        .split(',')
        .map(str::trim)
        .find(|e| e.split(';').next().is_some_and(|n| n.trim().eq_ignore_ascii_case("permessage-deflate")))?;
    let has = |name: &str| {
        offer
            .split(';')
            .skip(1)
            .any(|p| p.trim().split('=').next().is_some_and(|k| k.trim().eq_ignore_ascii_case(name)))
    };
    Some(Deflate {
        client_no_context_takeover: has("client_no_context_takeover"),
        server_no_context_takeover: has("server_no_context_takeover"),
    })
}

struct Inflater {
    d: Decompress,
    /// Start every message with an empty window.
    reset: bool,
}

impl Inflater {
    /// Inflates one whole message. `None` when it is not valid deflate data.
    fn inflate(&mut self, data: &[u8]) -> Option<(Vec<u8>, bool)> {
        let mut input = data.to_vec();
        // The sender cuts the empty block that ends every message; put it back.
        input.write_all(&[0x00, 0x00, 0xff, 0xff]).ok()?;
        let (mut out, mut consumed, mut truncated) = (Vec::new(), 0usize, false);
        loop {
            let (before_in, before_out) = (self.d.total_in(), out.len());
            out.reserve(8192);
            let status = self.d.decompress_vec(&input[consumed..], &mut out, FlushDecompress::Sync).ok()?;
            consumed += (self.d.total_in() - before_in) as usize;
            if out.len() > MAX_INFLATED {
                out.truncate(MAX_INFLATED);
                truncated = true;
                break;
            }
            let progress = self.d.total_in() > before_in || out.len() > before_out;
            if matches!(status, Status::StreamEnd)
                || (consumed >= input.len() && out.len() < out.capacity())
                || !progress
            {
                break;
            }
        }
        if self.reset {
            self.d.reset(false);
        }
        Some((out, truncated))
    }
}

struct Frame {
    fin: bool,
    opcode: u8,
    mask: Option<[u8; 4]>,
    remaining: u64,
    done: u64,
}

struct Pending {
    kind: Kind,
    compressed: bool,
    data: Vec<u8>,
    total: u64,
    truncated: bool,
}

/// Turns the bytes of one direction into messages.
pub struct FrameParser {
    head: Vec<u8>,
    frame: Option<Frame>,
    message: Option<Pending>,
    control: Vec<u8>,
    inflater: Option<Inflater>,
    broken: bool,
}

/// The header of a frame, once all of it has arrived.
fn parse_header(head: &[u8]) -> Option<Result<Frame, ()>> {
    if head.len() < 2 {
        return None;
    }
    let (b0, b1) = (head[0], head[1]);
    let masked = b1 & 0x80 != 0;
    let (len_bytes, short) = match b1 & 0x7f {
        126 => (2, 0),
        127 => (8, 0),
        n => (0, u64::from(n)),
    };
    let need = 2 + len_bytes + if masked { 4 } else { 0 };
    if head.len() < need {
        return None;
    }
    let len = match len_bytes {
        0 => short,
        2 => u64::from(u16::from_be_bytes([head[2], head[3]])),
        _ => u64::from_be_bytes(head[2..10].try_into().expect("eight bytes")),
    };
    let mask = masked.then(|| head[2 + len_bytes..need].try_into().expect("four bytes"));
    let (fin, rsv, opcode) = (b0 & 0x80 != 0, b0 & 0x70, b0 & 0x0f);
    let control = opcode >= 8;
    let valid_opcode = matches!(opcode, 0..=2 | 8..=10);
    // Only RSV1 (compression) is ever set, never on a control frame, and control frames are short
    // and whole. Anything else is not WebSocket.
    if !valid_opcode || rsv & 0x30 != 0 || (control && (!fin || len > 125 || rsv != 0)) || len >> 62 != 0 {
        return Some(Err(()));
    }
    Some(Ok(Frame { fin, opcode, mask, remaining: len, done: 0 }))
}

impl FrameParser {
    /// `deflate` is set when this direction's messages may be compressed: `Some(true)` when each
    /// message starts with an empty window (`no_context_takeover`), `Some(false)` when the window
    /// carries over.
    pub fn new(deflate: Option<bool>) -> Self {
        FrameParser {
            head: Vec::new(),
            frame: None,
            message: None,
            control: Vec::new(),
            inflater: deflate.map(|reset| Inflater { d: Decompress::new(false), reset }),
            broken: false,
        }
    }

    /// True once the bytes stopped being WebSocket frames. Nothing more is parsed.
    pub fn is_broken(&self) -> bool {
        self.broken
    }

    /// Feeds the next bytes of the stream and returns the messages they completed.
    pub fn feed(&mut self, mut bytes: &[u8]) -> Vec<Message> {
        let mut done = Vec::new();
        while !bytes.is_empty() && !self.broken {
            let Some(frame) = &mut self.frame else {
                self.head.push(bytes[0]);
                bytes = &bytes[1..];
                match parse_header(&self.head) {
                    None => {}
                    Some(Err(())) => self.broken = true,
                    Some(Ok(frame)) => {
                        let first = self.head[0];
                        self.head.clear();
                        self.start(frame, first & 0x40 != 0, &mut done);
                    }
                }
                continue;
            };
            let take = (bytes.len() as u64).min(frame.remaining) as usize;
            let (chunk, rest) = bytes.split_at(take);
            bytes = rest;
            let mut piece = chunk.to_vec();
            if let Some(mask) = frame.mask {
                for (i, b) in piece.iter_mut().enumerate() {
                    *b ^= mask[((frame.done + i as u64) % 4) as usize];
                }
            }
            frame.done += take as u64;
            frame.remaining -= take as u64;
            let (opcode, finished) = (frame.opcode, frame.remaining == 0);
            if opcode >= 8 {
                self.control.extend_from_slice(&piece);
            } else if let Some(m) = &mut self.message {
                m.total += take as u64;
                let room = MAX_MESSAGE.saturating_sub(m.data.len());
                m.data.extend_from_slice(&piece[..piece.len().min(room)]);
                m.truncated |= piece.len() > room;
            }
            if finished {
                self.end_frame(&mut done);
            }
        }
        done
    }

    /// A frame's header is complete: begin the message it belongs to, or note a control frame.
    fn start(&mut self, frame: Frame, compressed: bool, done: &mut Vec<Message>) {
        match frame.opcode {
            1 | 2 => {
                if self.message.is_some() {
                    self.broken = true;
                    return;
                }
                let kind = if frame.opcode == 1 { Kind::Text } else { Kind::Binary };
                self.message = Some(Pending { kind, compressed, data: Vec::new(), total: 0, truncated: false });
            }
            0 if self.message.is_none() => {
                self.broken = true;
                return;
            }
            _ => {}
        }
        let empty = frame.remaining == 0;
        self.frame = Some(frame);
        if empty {
            self.end_frame(done);
        }
    }

    fn end_frame(&mut self, done: &mut Vec<Message>) {
        let Some(frame) = self.frame.take() else { return };
        match frame.opcode {
            8..=10 => {
                let kind = match frame.opcode {
                    8 => Kind::Close,
                    9 => Kind::Ping,
                    _ => Kind::Pong,
                };
                let payload = std::mem::take(&mut self.control);
                done.push(Message { kind, len: payload.len() as u64, payload, truncated: false });
            }
            _ if frame.fin => {
                let Some(m) = self.message.take() else { return };
                done.push(self.finish(m));
            }
            _ => {}
        }
    }

    fn finish(&mut self, m: Pending) -> Message {
        if !m.compressed {
            return Message { kind: m.kind, len: m.total, truncated: m.truncated, payload: m.data };
        }
        let inflated = match (&mut self.inflater, m.truncated) {
            (Some(inflater), false) => inflater.inflate(&m.data),
            _ => None,
        };
        match inflated {
            Some((payload, truncated)) => {
                let len = payload.len() as u64;
                let truncated = truncated || payload.len() > MAX_MESSAGE;
                let mut payload = payload;
                payload.truncate(MAX_MESSAGE);
                Message { kind: m.kind, len, truncated, payload }
            }
            // Too large to inflate here, or not deflate data: its size is known, its content is not.
            None => Message { kind: m.kind, len: m.total, truncated: true, payload: Vec::new() },
        }
    }
}

#[cfg(test)]
mod tests {
    use flate2::{Compress, Compression, FlushCompress};

    use super::*;

    /// One frame as a peer would send it.
    fn frame(fin: bool, rsv1: bool, opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
        let mut out = vec![(u8::from(fin) << 7) | (u8::from(rsv1) << 6) | opcode];
        let m = if mask.is_some() { 0x80 } else { 0 };
        match payload.len() {
            n if n < 126 => out.push(m | n as u8),
            n if n <= 0xffff => {
                out.push(m | 126);
                out.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                out.push(m | 127);
                out.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        match mask {
            Some(key) => {
                out.extend_from_slice(&key);
                out.extend(payload.iter().enumerate().map(|(i, b)| b ^ key[i % 4]));
            }
            None => out.extend_from_slice(payload),
        }
        out
    }

    fn text(s: &str) -> Vec<u8> {
        frame(true, false, 1, s.as_bytes(), None)
    }

    #[test]
    fn reads_a_text_message_and_a_masked_one() {
        let mut p = FrameParser::new(None);
        let got = p.feed(&text("hello"));
        assert_eq!(got, [Message { kind: Kind::Text, payload: b"hello".to_vec(), len: 5, truncated: false }]);
        let got = p.feed(&frame(true, false, 1, b"from the client", Some([0x12, 0x34, 0x56, 0x78])));
        assert_eq!(got[0].payload, b"from the client");
    }

    #[test]
    fn reads_every_length_form() {
        let mut p = FrameParser::new(None);
        for size in [0usize, 1, 125, 126, 300, 65_535, 65_536] {
            let payload = vec![b'x'; size];
            let got = p.feed(&frame(true, false, 2, &payload, None));
            assert_eq!(got.len(), 1, "size {size}");
            assert_eq!((got[0].kind, got[0].len), (Kind::Binary, size as u64));
            assert_eq!(got[0].payload.len(), size.min(MAX_MESSAGE));
        }
    }

    #[test]
    fn a_message_that_arrives_in_pieces_is_the_same_message() {
        let bytes = frame(true, false, 1, "pieces of a message".as_bytes(), Some([1, 2, 3, 4]));
        let mut p = FrameParser::new(None);
        let mut got = Vec::new();
        for b in &bytes {
            got.extend(p.feed(std::slice::from_ref(b)));
        }
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].payload, b"pieces of a message");
        // Two messages in one read.
        let mut both = text("one");
        both.extend(text("two"));
        let got = FrameParser::new(None).feed(&both);
        assert_eq!(got.iter().map(|m| m.payload.clone()).collect::<Vec<_>>(), [b"one".to_vec(), b"two".to_vec()]);
    }

    #[test]
    fn fragments_are_joined_and_control_frames_can_come_between() {
        let mut bytes = frame(false, false, 1, b"Hel", None);
        bytes.extend(frame(true, false, 9, b"ping!", None));
        bytes.extend(frame(false, false, 0, b"lo, ", None));
        bytes.extend(frame(true, false, 0, b"world", None));
        let got = FrameParser::new(None).feed(&bytes);
        assert_eq!(
            got.iter().map(|m| (m.kind, m.payload.clone())).collect::<Vec<_>>(),
            [(Kind::Ping, b"ping!".to_vec()), (Kind::Text, b"Hello, world".to_vec())]
        );
    }

    #[test]
    fn a_close_frame_carries_its_code() {
        let mut payload = 1000u16.to_be_bytes().to_vec();
        payload.extend_from_slice(b"bye");
        let got = FrameParser::new(None).feed(&frame(true, false, 8, &payload, None));
        assert_eq!((got[0].kind, &got[0].payload[..2]), (Kind::Close, &[0x03, 0xe8][..]));
    }

    #[test]
    fn a_large_message_keeps_its_size_and_a_part_of_it() {
        let payload = vec![b'a'; MAX_MESSAGE * 3 + 7];
        let got = FrameParser::new(None).feed(&frame(true, false, 2, &payload, None));
        assert_eq!((got[0].len, got[0].payload.len(), got[0].truncated), (payload.len() as u64, MAX_MESSAGE, true));
    }

    #[test]
    fn bytes_that_are_not_websocket_stop_the_parser_without_a_fuss() {
        // What the proxy test sends through a "WebSocket": plain bytes.
        for junk in [&b"hello"[..], b"GET / HTTP/1.1\r\n", &[0xff; 16], &[0x03, 0x00]] {
            let mut p = FrameParser::new(None);
            assert!(p.feed(junk).is_empty());
            assert!(p.is_broken(), "{junk:?}");
            assert!(p.feed(&text("ignored")).is_empty(), "a broken parser stays quiet");
        }
        // A continuation with nothing to continue, and a data frame in the middle of a message.
        assert!(FrameParser::new(None).feed(&frame(true, false, 0, b"x", None)).is_empty());
        let mut p = FrameParser::new(None);
        p.feed(&frame(false, false, 1, b"a", None));
        p.feed(&frame(true, false, 1, b"b", None));
        assert!(p.is_broken());
    }

    /// Compresses messages the way `permessage-deflate` does: raw deflate, ended with a sync
    /// flush whose trailing `00 00 ff ff` is dropped.
    struct Deflater {
        c: Compress,
        reset: bool,
    }

    impl Deflater {
        fn new(reset: bool) -> Self {
            Deflater { c: Compress::new(Compression::default(), false), reset }
        }

        fn message(&mut self, text: &str) -> Vec<u8> {
            let mut out = Vec::with_capacity(text.len() + 64);
            self.c.compress_vec(text.as_bytes(), &mut out, FlushCompress::Sync).unwrap();
            assert!(out.ends_with(&[0x00, 0x00, 0xff, 0xff]));
            out.truncate(out.len() - 4);
            if self.reset {
                self.c.reset();
            }
            frame(true, true, 1, &out, None)
        }
    }

    #[test]
    fn inflates_permessage_deflate_with_a_window_that_carries_over() {
        let mut sender = Deflater::new(false);
        let mut p = FrameParser::new(Some(false));
        let first = r#"{"event":"price","symbol":"EMU","value":101.25,"event2":"price"}"#;
        let second = r#"{"event":"price","symbol":"EMU","value":101.30,"event2":"price"}"#;
        let (a, b) = (sender.message(first), sender.message(second));
        assert!(b.len() < a.len(), "the second message is smaller because it refers to the first");
        assert_eq!(p.feed(&a)[0].payload, first.as_bytes());
        let got = p.feed(&b);
        assert_eq!((got[0].payload.as_slice(), got[0].len), (second.as_bytes(), second.len() as u64));
    }

    #[test]
    fn inflates_permessage_deflate_with_no_context_takeover() {
        let mut sender = Deflater::new(true);
        let mut p = FrameParser::new(Some(true));
        for text in ["first message", "second message, the same window start", "third"] {
            assert_eq!(p.feed(&sender.message(text))[0].payload, text.as_bytes());
        }
    }

    #[test]
    fn a_compressed_message_that_is_not_deflate_data_is_reported_as_unreadable() {
        let mut p = FrameParser::new(Some(true));
        let got = p.feed(&frame(true, true, 1, &[0xff, 0xff, 0xff, 0xff, 0xff], None));
        assert_eq!((got[0].payload.len(), got[0].truncated, got[0].len), (0, true, 5));
    }

    #[test]
    fn reads_the_extensions_the_server_agreed_to() {
        assert_eq!(
            parse_extensions("permessage-deflate; client_no_context_takeover; server_max_window_bits=15"),
            Some(Deflate { client_no_context_takeover: true, server_no_context_takeover: false })
        );
        assert_eq!(
            parse_extensions("x-webkit-deflate-frame, permessage-deflate"),
            Some(Deflate { client_no_context_takeover: false, server_no_context_takeover: false })
        );
        assert_eq!(parse_extensions("x-other"), None);
        assert_eq!(parse_extensions(""), None);
    }

    #[test]
    fn stores_text_as_text_and_the_rest_as_base64() {
        let t = Message { kind: Kind::Text, payload: b"hi".to_vec(), len: 2, truncated: false }
            .into_stored(5, Direction::Up);
        assert_eq!((t.text.as_deref(), t.base64.as_deref(), t.t_ms), (Some("hi"), None, 5));
        let b = Message { kind: Kind::Binary, payload: vec![0xff, 0xfe, 0], len: 3, truncated: false }
            .into_stored(0, Direction::Down);
        assert_eq!((b.text, b.base64.as_deref()), (None, Some("//4A")));
        let bad_text =
            Message { kind: Kind::Text, payload: vec![0xff], len: 1, truncated: false }.into_stored(0, Direction::Down);
        assert_eq!((bad_text.text, bad_text.base64.as_deref()), (None, Some("/w==")));
        let empty =
            Message { kind: Kind::Pong, payload: vec![], len: 0, truncated: false }.into_stored(0, Direction::Down);
        assert_eq!((empty.text, empty.base64), (None, None));
    }
}
