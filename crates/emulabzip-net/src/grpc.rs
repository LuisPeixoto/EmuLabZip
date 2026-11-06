//! Reading gRPC bodies: the length-prefixed messages, and protobuf without its `.proto` file.
//!
//! A gRPC body is a series of messages, each with a 5-byte header: one byte saying whether the message is
//! compressed, then its length as a big-endian `u32`. A message is protobuf. Without the schema the field
//! names are lost, but the numbers, the wire types and the values are enough to read most messages.

use std::io::Read;

use serde_json::{Map, Number, Value};

/// Messages read from one body, at most.
const MAX_MESSAGES: usize = 1_000;
/// How deep nested messages are followed.
const MAX_DEPTH: usize = 8;
/// How many values one message can have, so a hostile body cannot make a huge tree.
const MAX_NODES: usize = 10_000;
/// Bytes shown of a field that is neither text nor a message.
const MAX_BYTES_SHOWN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The message was compressed and could not be unpacked, so `data` is as it was sent.
    pub compressed: bool,
    pub data: Vec<u8>,
}

/// Splits a body into its messages. A body that stops in the middle of a message gives the ones before it.
/// `encoding` is the `grpc-encoding` header, such as `gzip`.
pub fn messages(body: &[u8], encoding: Option<&str>) -> Vec<Message> {
    let gzip = encoding.is_some_and(|e| e.trim().eq_ignore_ascii_case("gzip"));
    let mut out = Vec::new();
    let mut rest = body;
    while rest.len() >= 5 && out.len() < MAX_MESSAGES {
        let flag = rest[0];
        let len = u32::from_be_bytes([rest[1], rest[2], rest[3], rest[4]]) as usize;
        if flag > 1 || rest.len() - 5 < len {
            break;
        }
        let payload = &rest[5..5 + len];
        out.push(match (flag, gzip) {
            (0, _) => Message { compressed: false, data: payload.to_vec() },
            (_, true) => match gunzip(payload) {
                Some(data) => Message { compressed: false, data },
                None => Message { compressed: true, data: payload.to_vec() },
            },
            (_, false) => Message { compressed: true, data: payload.to_vec() },
        });
        rest = &rest[5 + len..];
    }
    out
}

fn gunzip(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(data).take(50 * 1024 * 1024).read_to_end(&mut out).ok()?;
    Some(out)
}

/// Reads protobuf without a schema into JSON: fields are keyed by their number, a field that repeats becomes
/// an array, a nested message becomes an object, and bytes are shown as hex.
///
/// `None` when `data` is not protobuf at all.
pub fn decode_raw(data: &[u8]) -> Option<Value> {
    let mut budget = MAX_NODES;
    parse(data, 0, &mut budget).map(Value::Object)
}

fn parse(data: &[u8], depth: usize, budget: &mut usize) -> Option<Map<String, Value>> {
    let mut fields: Vec<(u64, Value)> = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let (tag, n) = varint(&data[i..])?;
        i += n;
        let (number, wire) = (tag >> 3, (tag & 7) as u8);
        if number == 0 || number > 536_870_911 {
            return None;
        }
        *budget = budget.checked_sub(1)?;
        let value = match wire {
            0 => {
                let (v, n) = varint(&data[i..])?;
                i += n;
                number_value(v)
            }
            1 => {
                let bytes: [u8; 8] = data.get(i..i + 8)?.try_into().ok()?;
                i += 8;
                number_value(u64::from_le_bytes(bytes))
            }
            5 => {
                let bytes: [u8; 4] = data.get(i..i + 4)?.try_into().ok()?;
                i += 4;
                Value::from(u32::from_le_bytes(bytes))
            }
            2 => {
                let (len, n) = varint(&data[i..])?;
                i += n;
                let len = usize::try_from(len).ok()?;
                let bytes = data.get(i..i.checked_add(len)?)?;
                i += len;
                length_delimited(bytes, depth, budget)
            }
            // Groups are deprecated and not worth guessing at; anything else is not protobuf.
            _ => return None,
        };
        fields.push((number, value));
    }
    let mut map = Map::new();
    for (number, value) in fields {
        let key = number.to_string();
        match map.get_mut(&key) {
            None => {
                map.insert(key, value);
            }
            // No value this reader makes is an array, so an array here is a field that already repeated.
            Some(Value::Array(items)) => items.push(value),
            Some(existing) => {
                let first = existing.take();
                *existing = Value::Array(vec![first, value]);
            }
        }
    }
    Some(map)
}

fn number_value(v: u64) -> Value {
    // JSON numbers lose precision past 2^53, so a big one is written as text.
    if v <= (1 << 53) { Value::Number(Number::from(v)) } else { Value::String(v.to_string()) }
}

/// A length-delimited field is text, a nested message or bytes. Text is tried first: a string
/// rarely parses as a message, but a message can be valid UTF-8.
fn length_delimited(bytes: &[u8], depth: usize, budget: &mut usize) -> Value {
    if bytes.is_empty() {
        return Value::String(String::new());
    }
    if let Ok(text) = std::str::from_utf8(bytes)
        && text.chars().all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
    {
        return Value::String(text.to_string());
    }
    if depth < MAX_DEPTH {
        let mut nested_budget = *budget;
        if let Some(map) = parse(bytes, depth + 1, &mut nested_budget)
            && !map.is_empty()
        {
            *budget = nested_budget;
            return Value::Object(map);
        }
    }
    let shown = &bytes[..bytes.len().min(MAX_BYTES_SHOWN)];
    let mut hex: String = shown.iter().map(|b| format!("{b:02x}")).collect();
    if bytes.len() > shown.len() {
        hex.push_str(&format!("… ({} bytes)", bytes.len()));
    }
    Value::String(format!("0x{hex}"))
}

fn varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (i, &byte) in data.iter().enumerate().take(10) {
        value |= u64::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

/// The name of a gRPC status code, as the specification writes it.
pub fn status_name(code: u32) -> &'static str {
    match code {
        0 => "OK",
        1 => "CANCELLED",
        2 => "UNKNOWN",
        3 => "INVALID_ARGUMENT",
        4 => "DEADLINE_EXCEEDED",
        5 => "NOT_FOUND",
        6 => "ALREADY_EXISTS",
        7 => "PERMISSION_DENIED",
        8 => "RESOURCE_EXHAUSTED",
        9 => "FAILED_PRECONDITION",
        10 => "ABORTED",
        11 => "OUT_OF_RANGE",
        12 => "UNIMPLEMENTED",
        13 => "INTERNAL",
        14 => "UNAVAILABLE",
        15 => "DATA_LOSS",
        16 => "UNAUTHENTICATED",
        _ => "UNKNOWN CODE",
    }
}

/// The body as readable JSON: one entry per message, decoded without a schema.
pub fn describe(body: &[u8], encoding: Option<&str>) -> Value {
    let list = messages(body, encoding);
    Value::Array(
        list.iter()
            .map(|m| {
                if m.compressed {
                    return serde_json::json!({ "compressed": true, "bytes": m.data.len() });
                }
                decode_raw(&m.data).unwrap_or_else(|| Value::String(format!("{} bytes, not protobuf", m.data.len())))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut m = vec![0u8];
        m.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        m.extend_from_slice(payload);
        m
    }

    #[test]
    fn splits_messages() {
        let body = [frame(b"abc"), frame(b""), frame(b"de")].concat();
        let got = messages(&body, None);
        assert_eq!(got.iter().map(|m| m.data.as_slice()).collect::<Vec<_>>(), [&b"abc"[..], b"", b"de"]);
    }

    #[test]
    fn a_cut_body_gives_the_messages_before_the_cut() {
        let mut body = [frame(b"abc"), frame(b"defgh")].concat();
        body.truncate(body.len() - 2);
        assert_eq!(messages(&body, None).len(), 1);
        assert!(messages(&[0, 0, 0], None).is_empty());
        assert!(messages(&[7, 0, 0, 0, 1, 1], None).is_empty(), "an unknown flag is not gRPC");
    }

    #[test]
    fn unpacks_gzip_messages() {
        use std::io::Write;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(b"\x08\x01").unwrap();
        let packed = gz.finish().unwrap();
        let mut body = vec![1u8];
        body.extend_from_slice(&(packed.len() as u32).to_be_bytes());
        body.extend_from_slice(&packed);
        let got = messages(&body, Some("gzip"));
        assert_eq!(got, vec![Message { compressed: false, data: vec![8, 1] }]);
        assert!(messages(&body, None)[0].compressed, "without grpc-encoding it stays packed");
    }

    #[test]
    fn decodes_a_message_without_its_schema() {
        // field 1 = "hello", field 2 = 150, field 3 = { field 1 = 5 }
        let data = [&[0x0a, 5][..], b"hello", &[0x10, 0x96, 0x01, 0x1a, 2, 0x08, 5]].concat();
        let v = decode_raw(&data).unwrap();
        assert_eq!(v, serde_json::json!({ "1": "hello", "2": 150, "3": { "1": 5 } }));
    }

    #[test]
    fn a_repeated_field_becomes_an_array() {
        // field 1 = 1, field 1 = 2, field 1 = 3
        let v = decode_raw(&[0x08, 1, 0x08, 2, 0x08, 3]).unwrap();
        assert_eq!(v, serde_json::json!({ "1": [1, 2, 3] }));
    }

    #[test]
    fn reads_fixed_width_fields_and_big_numbers() {
        let mut data = vec![0x09];
        data.extend_from_slice(&7u64.to_le_bytes());
        data.push(0x15);
        data.extend_from_slice(&9u32.to_le_bytes());
        data.extend_from_slice(&[0x18, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]);
        let v = decode_raw(&data).unwrap();
        assert_eq!(v["1"], 7);
        assert_eq!(v["2"], 9);
        assert_eq!(v["3"], "18446744073709551615");
    }

    #[test]
    fn shows_bytes_as_hex_and_text_as_text() {
        let v = decode_raw(&[0x0a, 3, 0xff, 0xfe, 0xfd, 0x12, 2, b'h', b'i']).unwrap();
        assert_eq!(v["1"], "0xfffefd");
        assert_eq!(v["2"], "hi");
    }

    #[test]
    fn refuses_what_is_not_protobuf() {
        assert!(decode_raw(&[0x00]).is_none(), "field 0 does not exist");
        assert!(decode_raw(&[0x0a, 9, 1]).is_none(), "a length past the end");
        assert!(decode_raw(&[0x0b]).is_none(), "groups are not read");
        assert!(decode_raw(&[0x80]).is_none(), "a varint that never ends");
        assert_eq!(decode_raw(&[]), Some(serde_json::json!({})));
    }

    #[test]
    fn deep_nesting_and_noise_do_not_panic() {
        // Each level wraps the next in field 1.
        let mut data = vec![0x08, 1];
        for _ in 0..100 {
            let mut outer = vec![0x0a, data.len() as u8];
            outer.extend(&data);
            data = outer;
            if data.len() > 120 {
                break;
            }
        }
        let _ = decode_raw(&data);
        let mut seed = 0x1234_5678u32;
        for _ in 0..2_000 {
            let noise: Vec<u8> = (0..40)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed >> 24) as u8
                })
                .collect();
            let _ = decode_raw(&noise);
            let _ = messages(&noise, Some("gzip"));
        }
    }

    #[test]
    fn names_the_status_codes() {
        assert_eq!(status_name(0), "OK");
        assert_eq!(status_name(14), "UNAVAILABLE");
        assert_eq!(status_name(99), "UNKNOWN CODE");
    }

    #[test]
    fn describes_a_body_message_by_message() {
        let body = [frame(&[0x08, 1]), frame(&[0x08, 2])].concat();
        assert_eq!(describe(&body, None), serde_json::json!([{ "1": 1 }, { "1": 2 }]));
    }
}
