//! DNS over UDP: which names the device asked for, and what it was told.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::Serialize;

/// One question and, when the capture has it, its answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lookup {
    /// Milliseconds since the Unix epoch, when the question was asked.
    pub started_ms: u64,
    pub name: String,
    /// `A`, `AAAA`, `CNAME`, … or the number.
    pub kind: String,
    /// The addresses and names in the answer.
    pub answers: Vec<String>,
    /// `NOERROR`, `NXDOMAIN`, …; `None` when no answer was captured.
    pub status: Option<String>,
    pub duration_ms: Option<u64>,
}

struct Message {
    id: u16,
    response: bool,
    rcode: u8,
    name: String,
    qtype: u16,
    answers: Vec<String>,
}

fn kind_name(t: u16) -> String {
    match t {
        1 => "A".into(),
        2 => "NS".into(),
        5 => "CNAME".into(),
        15 => "MX".into(),
        16 => "TXT".into(),
        28 => "AAAA".into(),
        33 => "SRV".into(),
        65 => "HTTPS".into(),
        n => n.to_string(),
    }
}

fn status_name(rcode: u8) -> String {
    match rcode {
        0 => "NOERROR".into(),
        1 => "FORMERR".into(),
        2 => "SERVFAIL".into(),
        3 => "NXDOMAIN".into(),
        4 => "NOTIMP".into(),
        5 => "REFUSED".into(),
        n => format!("RCODE {n}"),
    }
}

/// A name at `at`, following compression pointers. Returns it and where the name ends in place.
fn name(data: &[u8], mut at: usize) -> Option<(String, usize)> {
    let (mut labels, mut end, mut jumps) = (Vec::new(), None, 0);
    loop {
        let len = *data.get(at)? as usize;
        if len == 0 {
            return Some((labels.join("."), end.unwrap_or(at + 1)));
        }
        if len & 0xc0 == 0xc0 {
            jumps += 1;
            if jumps > 16 {
                return None;
            }
            let target = ((len & 0x3f) << 8) | *data.get(at + 1)? as usize;
            end.get_or_insert(at + 2);
            at = target;
            continue;
        }
        let label = data.get(at + 1..at + 1 + len)?;
        labels.push(String::from_utf8_lossy(label).into_owned());
        at += 1 + len;
        if labels.len() > 127 {
            return None;
        }
    }
}

fn parse(data: &[u8]) -> Option<Message> {
    if data.len() < 12 {
        return None;
    }
    let flags = u16::from_be_bytes([data[2], data[3]]);
    let (questions, answers) = (u16::from_be_bytes([data[4], data[5]]), u16::from_be_bytes([data[6], data[7]]));
    if questions == 0 {
        return None;
    }
    let (qname, mut at) = name(data, 12)?;
    let qtype = u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]);
    at += 4;
    let mut found = Vec::new();
    for _ in 0..answers {
        let Some((_, next)) = name(data, at) else { break };
        let Some(head) = data.get(next..next + 10) else { break };
        let (kind, len) = (u16::from_be_bytes([head[0], head[1]]), usize::from(u16::from_be_bytes([head[8], head[9]])));
        let rdata = next + 10;
        let Some(bytes) = data.get(rdata..rdata + len) else { break };
        match kind {
            1 if len == 4 => found.push(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]).to_string()),
            28 if len == 16 => {
                found.push(Ipv6Addr::from(<[u8; 16]>::try_from(bytes).expect("sixteen bytes")).to_string())
            }
            5 => {
                if let Some((target, _)) = name(data, rdata) {
                    found.push(target);
                }
            }
            _ => {}
        }
        at = rdata + len;
    }
    Some(Message {
        id: u16::from_be_bytes([data[0], data[1]]),
        response: flags & 0x8000 != 0,
        rcode: (flags & 0x0f) as u8,
        name: qname,
        qtype,
        answers: found,
    })
}

/// Pairs questions with answers as the datagrams arrive.
#[derive(Default)]
pub struct Tracker {
    lookups: Vec<Lookup>,
    /// Open questions by (id, name, type): the index in `lookups` and when it was asked (µs).
    open: HashMap<(u16, String, u16), (usize, u64)>,
}

impl Tracker {
    pub fn feed(&mut self, ts_us: u64, payload: &[u8]) {
        let Some(m) = parse(payload) else { return };
        let key = (m.id, m.name.to_lowercase(), m.qtype);
        if !m.response {
            let index = self.lookups.len();
            self.lookups.push(Lookup {
                started_ms: ts_us / 1000,
                name: m.name,
                kind: kind_name(m.qtype),
                answers: Vec::new(),
                status: None,
                duration_ms: None,
            });
            self.open.insert(key, (index, ts_us));
        } else if let Some((index, asked)) = self.open.remove(&key) {
            let l = &mut self.lookups[index];
            l.answers = m.answers;
            l.status = Some(status_name(m.rcode));
            l.duration_ms = Some(ts_us.saturating_sub(asked) / 1000);
        } else {
            // An answer whose question is not in the capture.
            self.lookups.push(Lookup {
                started_ms: ts_us / 1000,
                name: m.name,
                kind: kind_name(m.qtype),
                answers: m.answers,
                status: Some(status_name(m.rcode)),
                duration_ms: None,
            });
        }
    }

    pub fn into_lookups(self) -> Vec<Lookup> {
        self.lookups
    }
}

#[cfg(test)]
pub mod build {
    pub fn query(id: u16, host: &str, qtype: u16) -> Vec<u8> {
        let mut m = id.to_be_bytes().to_vec();
        m.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
        for label in host.split('.') {
            m.push(label.len() as u8);
            m.extend_from_slice(label.as_bytes());
        }
        m.push(0);
        m.extend_from_slice(&qtype.to_be_bytes());
        m.extend_from_slice(&[0, 1]);
        m
    }

    /// An answer that repeats the question and gives each record as (type, rdata), with the name
    /// pointing back to the question.
    pub fn answer(id: u16, host: &str, qtype: u16, rcode: u8, records: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut m = query(id, host, qtype);
        m[2] = 0x81;
        m[3] = 0x80 | rcode;
        m[6..8].copy_from_slice(&(records.len() as u16).to_be_bytes());
        for (kind, rdata) in records {
            m.extend_from_slice(&[0xc0, 0x0c]);
            m.extend_from_slice(&kind.to_be_bytes());
            m.extend_from_slice(&[0, 1, 0, 0, 0, 60]);
            m.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
            m.extend_from_slice(rdata);
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn pairs_a_question_with_its_answer() {
        let mut t = Tracker::default();
        t.feed(1_000_000, &query(7, "api.example.com", 1));
        t.feed(1_042_000, &answer(7, "api.example.com", 1, 0, &[(1, vec![93, 184, 216, 34])]));
        let l = t.into_lookups();
        assert_eq!(l.len(), 1);
        assert_eq!(
            (l[0].name.as_str(), l[0].kind.as_str(), l[0].status.as_deref(), l[0].duration_ms),
            ("api.example.com", "A", Some("NOERROR"), Some(42))
        );
        assert_eq!(l[0].answers, ["93.184.216.34"]);
        assert_eq!(l[0].started_ms, 1000);
    }

    #[test]
    fn reads_aaaa_cname_and_failures() {
        let mut t = Tracker::default();
        let v6 = "2001:db8::1".parse::<Ipv6Addr>().unwrap().octets().to_vec();
        let cname = vec![3, b'c', b'd', b'n', 0xc0, 0x10]; // cdn + the "example.com" part of the question
        t.feed(1, &query(1, "www.example.com", 28));
        t.feed(2, &answer(1, "www.example.com", 28, 0, &[(5, cname), (28, v6)]));
        t.feed(3, &query(2, "nothing.invalid", 1));
        t.feed(4, &answer(2, "nothing.invalid", 1, 3, &[]));
        let l = t.into_lookups();
        assert_eq!(l[0].answers, ["cdn.example.com", "2001:db8::1"]);
        assert_eq!((l[1].status.as_deref(), l[1].answers.len()), (Some("NXDOMAIN"), 0));
    }

    #[test]
    fn a_question_nobody_answered_and_an_answer_nobody_asked() {
        let mut t = Tracker::default();
        t.feed(1, &query(5, "lonely.test", 1));
        t.feed(2, &answer(9, "late.test", 1, 0, &[(1, vec![1, 2, 3, 4])]));
        let l = t.into_lookups();
        assert_eq!((l[0].status.clone(), l[0].duration_ms), (None, None));
        assert_eq!((l[1].name.as_str(), l[1].answers.clone()), ("late.test", vec!["1.2.3.4".to_string()]));
    }

    #[test]
    fn ignores_what_is_not_dns() {
        let mut t = Tracker::default();
        t.feed(1, b"short");
        t.feed(1, &[0u8; 12]);
        // A pointer that points at itself must not loop forever.
        let mut looped = query(1, "x.test", 1);
        looped[12] = 0xc0;
        looped[13] = 0x0c;
        t.feed(1, &looped);
        assert!(t.into_lookups().is_empty());
    }
}
