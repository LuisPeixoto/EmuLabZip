//! A small reader for protobuf without its schema, for Proto DataStore files and for taking apart the Preferences
//! DataStore format. Fields are keyed by their number.

use emulabzip_core::{Error, Result};
use serde_json::{Map, Value};

/// How deep a message may nest inside another.
const MAX_DEPTH: usize = 12;
/// The most fields read from one message.
const MAX_FIELDS: usize = 100_000;

/// One field of a message.
#[derive(Debug, Clone, PartialEq)]
pub enum Field<'a> {
    Varint(u64),
    Fixed32(u32),
    Fixed64(u64),
    Bytes(&'a [u8]),
}

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("protobuf", detail)
}

fn varint(data: &[u8], at: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *data.get(*at).ok_or_else(|| bad("a number runs past the end"))?;
        *at += 1;
        if shift < 64 {
            value |= u64::from(byte & 0x7f) << shift;
        }
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(bad("a number is too long"))
}

/// Splits a message into its fields, in order. Group types are not protobuf of today and are refused.
pub fn fields(data: &[u8]) -> Result<Vec<(u32, Field<'_>)>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < data.len() {
        if out.len() >= MAX_FIELDS {
            return Err(bad("too many fields"));
        }
        let tag = varint(data, &mut at)?;
        let number = u32::try_from(tag >> 3).map_err(|_| bad("a field number is too big"))?;
        if number == 0 {
            return Err(bad("a field has number 0"));
        }
        let field = match tag & 7 {
            0 => Field::Varint(varint(data, &mut at)?),
            1 => {
                let end = at
                    .checked_add(8)
                    .filter(|e| *e <= data.len())
                    .ok_or_else(|| bad("a fixed value runs past the end"))?;
                let v = u64::from_le_bytes(data[at..end].try_into().expect("8 bytes"));
                at = end;
                Field::Fixed64(v)
            }
            2 => {
                let len = usize::try_from(varint(data, &mut at)?).map_err(|_| bad("a length is too big"))?;
                let end =
                    at.checked_add(len).filter(|e| *e <= data.len()).ok_or_else(|| bad("a field runs past the end"))?;
                let bytes = &data[at..end];
                at = end;
                Field::Bytes(bytes)
            }
            5 => {
                let end = at
                    .checked_add(4)
                    .filter(|e| *e <= data.len())
                    .ok_or_else(|| bad("a fixed value runs past the end"))?;
                let v = u32::from_le_bytes(data[at..end].try_into().expect("4 bytes"));
                at = end;
                Field::Fixed32(v)
            }
            other => return Err(bad(format!("wire type {other} is not one a message holds"))),
        };
        out.push((number, field));
    }
    Ok(out)
}

fn printable(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'))
}

/// Reads a message as JSON without a schema: fields by number, a number that repeats becomes a list, and bytes are shown
/// as a message when they read as one, as text when they are text, and as hex otherwise.
pub fn to_json(data: &[u8]) -> Result<Value> {
    decode(data, 0)
}

fn decode(data: &[u8], depth: usize) -> Result<Value> {
    let mut map: Map<String, Value> = Map::new();
    for (number, field) in fields(data)? {
        let value = match field {
            Field::Varint(v) => Value::from(v),
            Field::Fixed32(v) => Value::from(v),
            Field::Fixed64(v) => Value::from(v),
            Field::Bytes(b) => bytes_value(b, depth),
        };
        match map.get_mut(&number.to_string()) {
            Some(Value::Array(list)) => list.push(value),
            Some(existing) => {
                let first = existing.take();
                *existing = Value::Array(vec![first, value]);
            }
            None => {
                map.insert(number.to_string(), value);
            }
        }
    }
    Ok(Value::Object(map))
}

fn bytes_value(bytes: &[u8], depth: usize) -> Value {
    if bytes.is_empty() {
        return Value::String(String::new());
    }
    // Text first: a short string is often a valid message by accident.
    if let Ok(text) = std::str::from_utf8(bytes)
        && printable(text)
    {
        return Value::String(text.to_string());
    }
    if depth < MAX_DEPTH
        && let Ok(Value::Object(m)) = decode(bytes, depth + 1)
        && !m.is_empty()
    {
        return Value::Object(m);
    }
    if bytes.len() <= 32 {
        Value::String(format!("<{}>", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()))
    } else {
        Value::String(format!("<{} bytes>", bytes.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_wire_type() {
        // 1: varint 150, 2: "hi", 3: fixed32 7, 4: fixed64 9, 5: a nested message {1: 1}
        let data = [
            0x08, 0x96, 0x01, 0x12, 0x02, b'h', b'i', 0x1d, 7, 0, 0, 0, 0x21, 9, 0, 0, 0, 0, 0, 0, 0, 0x2a, 0x02, 0x08,
            0x01,
        ];
        let v = to_json(&data).unwrap();
        assert_eq!(v["1"], 150);
        assert_eq!(v["2"], "hi");
        assert_eq!(v["3"], 7);
        assert_eq!(v["4"], 9);
        assert_eq!(v["5"]["1"], 1);
    }

    #[test]
    fn a_field_that_repeats_becomes_a_list() {
        let v = to_json(&[0x08, 1, 0x08, 2, 0x08, 3]).unwrap();
        assert_eq!(v["1"], serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn what_is_not_protobuf_is_an_error_and_never_a_panic() {
        assert!(to_json(&[0x0a, 0x05, b'a']).is_err(), "runs past the end");
        assert!(to_json(&[0x00]).is_err(), "field 0");
        assert!(to_json(&[0x0b]).is_err(), "a group");
        assert!(to_json(&[0x08, 0x80]).is_err(), "a cut number");
        assert!(to_json(&[0xff; 12]).is_err());
        assert!(to_json(&[]).unwrap().as_object().unwrap().is_empty());
    }

    #[test]
    fn deep_nesting_is_cut_off() {
        let mut data = vec![0x08, 1];
        for _ in 0..40 {
            let mut outer = vec![0x0a, data.len() as u8];
            outer.extend(&data);
            data = outer;
        }
        // It decodes without recursing forever.
        assert!(to_json(&data).is_ok());
    }
}
