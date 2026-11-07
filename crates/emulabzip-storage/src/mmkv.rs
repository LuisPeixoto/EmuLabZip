//! MMKV files (`mmkv/<name>`), the key-value store of WeChat that many apps use.
//!
//! A file is a size and a run of entries, each a key and a value, each prefixed with its length. MMKV does not write down
//! what type a value is, so the type is worked out from the bytes, and the raw bytes are kept next to it. An entry
//! written again later wins, and one written empty is a removal. A file made with a key is encrypted, and is explained as
//! such instead of being shown as noise.

use emulabzip_core::{Error, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MmkvEntry {
    pub key: String,
    /// What the bytes look like: `string`, `set`, `int`, `double`, `float` or `bytes`.
    pub kind: String,
    pub value: String,
    /// The items of a set.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
    /// The value's bytes, in hex, because the type is only a guess.
    pub hex: String,
}

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("MMKV file", detail.into())
}

fn varint(data: &[u8], at: usize) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (i, shift) in (0..70).step_by(7).enumerate() {
        let byte = *data.get(at + i)?;
        if shift < 64 {
            value |= u64::from(byte & 0x7f) << shift;
        }
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

fn printable(text: &str) -> bool {
    text.chars().all(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'))
}

/// Strings, each with its length in front, that fill `body` exactly.
fn string_set(body: &[u8]) -> Option<Vec<String>> {
    let mut items = Vec::new();
    let mut at = 0;
    while at < body.len() {
        let (n, used) = varint(body, at)?;
        let start = at + used;
        let end = start.checked_add(usize::try_from(n).ok()?)?;
        let text = std::str::from_utf8(body.get(start..end)?).ok()?;
        if !printable(text) {
            return None;
        }
        items.push(text.to_string());
        at = end;
    }
    Some(items)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Works out what a value's bytes are. The type is a guess, which is why the hex stays.
pub fn classify(v: &[u8]) -> (&'static str, String, Vec<String>) {
    // Strings, sets and byte arrays carry their own length in front.
    if let Some((n, used)) = varint(v, 0)
        && usize::try_from(n).is_ok_and(|n| used + n == v.len())
    {
        let body = &v[used..];
        if body.is_empty() {
            return ("string", String::new(), vec![]);
        }
        if let Ok(text) = std::str::from_utf8(body)
            && printable(text)
        {
            return ("string", text.to_string(), vec![]);
        }
        if let Some(items) = string_set(body) {
            return ("set", items.join(", "), items);
        }
        return ("bytes", hex(body), vec![]);
    }
    if let Some((n, used)) = varint(v, 0)
        && used == v.len()
    {
        return ("int", (n as i64).to_string(), vec![]);
    }
    match v.len() {
        8 => ("double", format!("{:?}", f64::from_le_bytes(v.try_into().expect("8 bytes"))), vec![]),
        4 => ("float", format!("{:?}", f32::from_le_bytes(v.try_into().expect("4 bytes"))), vec![]),
        _ => ("bytes", hex(v), vec![]),
    }
}

/// Reads entries from `from` up to `end`, or `None` when they do not fit exactly.
fn entries(data: &[u8], from: usize, end: usize) -> Option<Vec<(String, Vec<u8>)>> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut at = from;
    while at < end {
        let (klen, used) = varint(data, at)?;
        at += used;
        let key_end = at.checked_add(usize::try_from(klen).ok()?).filter(|e| *e <= end)?;
        let key = std::str::from_utf8(&data[at..key_end]).ok()?;
        if key.is_empty() || !printable(key) {
            return None;
        }
        at = key_end;
        let (vlen, used) = varint(data, at)?;
        at += used;
        let val_end = at.checked_add(usize::try_from(vlen).ok()?).filter(|e| *e <= end)?;
        let value = data[at..val_end].to_vec();
        at = val_end;
        out.retain(|(k, _)| k != key);
        // A key written empty is a key removed.
        if !value.is_empty() {
            out.push((key.to_string(), value));
        }
    }
    (at == end).then_some(out)
}

/// Reads an MMKV data file.
pub fn parse(bytes: &[u8]) -> Result<Vec<MmkvEntry>> {
    if bytes.len() < 4 {
        return Err(bad("the file is too short"));
    }
    let actual = u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")) as usize;
    let end = actual.saturating_add(4).min(bytes.len());
    if actual == 0 {
        return Ok(Vec::new());
    }
    // Newer MMKV puts four more bytes before the entries; older does not.
    let found = [8usize, 4].into_iter().filter(|from| *from <= end).find_map(|from| entries(bytes, from, end));
    let Some(found) = found else {
        return Err(bad("the entries do not read as MMKV: the file is encrypted with a key, or is not an MMKV file"));
    };
    Ok(found
        .into_iter()
        .map(|(key, value)| {
            let (kind, text, items) = classify(&value);
            MmkvEntry { key, kind: kind.to_string(), value: text, items, hex: hex(&value) }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &[u8] = include_bytes!("../tests/fixtures/mmkv.default");

    #[test]
    fn reads_a_real_mmkv_file() {
        let entries = parse(REAL).unwrap();
        let get = |k: &str| entries.iter().find(|e| e.key == k).unwrap_or_else(|| panic!("{k}: {entries:?}"));
        assert_eq!(entries.len(), 7);
        assert_eq!(
            (get("session_token").kind.as_str(), get("session_token").value.as_str()),
            ("string", "tok_live_123")
        );
        assert_eq!((get("visits").kind.as_str(), get("visits").value.as_str()), ("int", "7"));
        // MMKV does not say a boolean is one: it is a 1, with its hex beside it.
        assert_eq!(
            (get("beta").kind.as_str(), get("beta").value.as_str(), get("beta").hex.as_str()),
            ("int", "1", "01")
        );
        assert_eq!((get("ratio").kind.as_str(), get("ratio").value.as_str()), ("double", "1.25"));
        assert_eq!((get("big").kind.as_str(), get("big").value.as_str()), ("int", "5000000000"));
        assert_eq!(get("tags").items, ["x", "y"]);
        assert_eq!(get("tags").kind, "set");
        assert_eq!((get("blob").kind.as_str(), get("blob").value.as_str()), ("bytes", "01020304"));
    }

    fn file(entries: &[(&str, &[u8])], header: usize) -> Vec<u8> {
        let mut body = Vec::new();
        for (k, v) in entries {
            body.push(k.len() as u8);
            body.extend(k.as_bytes());
            body.push(v.len() as u8);
            body.extend(*v);
        }
        let actual = body.len() + header - 4;
        let mut out = (actual as u32).to_le_bytes().to_vec();
        out.extend(vec![0xAA; header - 4]);
        out.extend(body);
        out.extend(vec![0; 64]);
        out
    }

    #[test]
    fn a_key_written_again_wins_and_one_written_empty_is_gone() {
        for header in [8, 4] {
            let data = file(&[("a", &[1]), ("b", &[2]), ("a", &[9]), ("b", &[])], header);
            let entries = parse(&data).unwrap();
            assert_eq!(entries.len(), 1, "header {header}: {entries:?}");
            assert_eq!((entries[0].key.as_str(), entries[0].value.as_str()), ("a", "9"));
        }
    }

    #[test]
    fn empty_or_encrypted_or_other_files_are_explained() {
        assert!(parse(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap().is_empty());
        assert!(parse(&[1, 2]).is_err());
        let noise: Vec<u8> = (0..200u32).map(|i| (i * 37 % 251) as u8).collect();
        let mut data = 190u32.to_le_bytes().to_vec();
        data.extend(noise);
        assert!(parse(&data).unwrap_err().to_string().contains("encrypted"));
    }

    #[test]
    fn values_are_typed_by_their_bytes() {
        assert_eq!(classify(&[0x05, b'h', b'e', b'l', b'l', b'o']).0, "string");
        assert_eq!(classify(&[0x00]), ("string", String::new(), vec![]));
        assert_eq!(classify(&[0xac, 0x02]).1, "300");
        assert_eq!(classify(&1.5f32.to_le_bytes()).0, "float");
        assert_eq!(classify(&[0xff; 3]).0, "bytes");
    }
}
