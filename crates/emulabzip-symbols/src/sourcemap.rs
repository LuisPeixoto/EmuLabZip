//! Positions in a React Native bundle, looked up in a source map.
//!
//! A release build of a React Native app ships one JavaScript bundle (`index.android.bundle`, `main.jsbundle`), often
//! compiled to Hermes bytecode, and an error in it points at a place in that bundle: `index.android.bundle:1:123456`.
//! The source map written next to the bundle says which file, line and name that place came from. For Hermes the map is
//! the one `hermes` composes with Metro's: its columns on line 1 are offsets into the bytecode.
//!
//! This reads source map version 3 (`mappings` in base64 VLQ). It never panics on a map that is wrong.

use serde_json::Value;

/// A place in the original source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Original {
    pub source: String,
    /// 1-based.
    pub line: u32,
    /// 1-based.
    pub column: u32,
    /// The name of the function or variable the map gives for the place, when it does.
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    generated_column: u32,
    /// The source, line, column and name, when the segment says them.
    source: Option<(u32, u32, u32, Option<u32>)>,
}

/// A parsed source map.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    sources: Vec<String>,
    names: Vec<String>,
    /// One list of segments per line of the generated file, in column order.
    lines: Vec<Vec<Segment>>,
}

/// Why a map was not read.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NotJson,
    NotAMap,
    /// An indexed map (`sections`), which no React Native toolchain writes for a bundle.
    Indexed,
    Bad(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotJson => write!(f, "this is not a source map: it is not JSON"),
            Error::NotAMap => write!(f, "this is not a source map: it has no `mappings`"),
            Error::Indexed => write!(f, "indexed source maps (`sections`) are not supported"),
            Error::Bad(why) => write!(f, "this source map is damaged: {why}"),
        }
    }
}

impl std::error::Error for Error {}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn digit(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Reads one base64 VLQ number, and how many characters it took.
fn vlq(bytes: &[u8]) -> Option<(i64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    for (i, &b) in bytes.iter().enumerate() {
        let d = digit(b)?;
        let more = d & 32 != 0;
        let value = u64::from(d & 31);
        if shift > 55 {
            return None;
        }
        result |= value << shift;
        shift += 5;
        if !more {
            let negative = result & 1 == 1;
            let n = (result >> 1) as i64;
            return Some((if negative { -n } else { n }, i + 1));
        }
    }
    None
}

/// How the digits of a number are written, for tests and for anyone who wants to build a map.
pub fn encode_vlq(value: i64) -> String {
    let mut v = if value < 0 { ((-value) as u64) << 1 | 1 } else { (value as u64) << 1 };
    let mut out = String::new();
    loop {
        let mut digit = (v & 31) as usize;
        v >>= 5;
        if v > 0 {
            digit |= 32;
        }
        out.push(BASE64[digit] as char);
        if v == 0 {
            break;
        }
    }
    out
}

impl SourceMap {
    /// Reads a source map from its JSON.
    pub fn parse(text: &str) -> Result<SourceMap, Error> {
        let json: Value = serde_json::from_str(text).map_err(|_| Error::NotJson)?;
        if json.get("sections").is_some() {
            return Err(Error::Indexed);
        }
        let mappings = json.get("mappings").and_then(Value::as_str).ok_or(Error::NotAMap)?;
        let root = json.get("sourceRoot").and_then(Value::as_str).unwrap_or("");
        let strings = |key: &str| -> Vec<String> {
            json.get(key)
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect())
                .unwrap_or_default()
        };
        let sources: Vec<String> = strings("sources")
            .into_iter()
            .map(|s| {
                if root.is_empty() || s.starts_with('/') || s.contains("://") {
                    s
                } else {
                    format!("{}/{s}", root.trim_end_matches('/'))
                }
            })
            .collect();
        let names = strings("names");

        let mut lines: Vec<Vec<Segment>> = Vec::new();
        let (mut source, mut src_line, mut src_col, mut name) = (0i64, 0i64, 0i64, 0i64);
        for line in mappings.split(';') {
            let mut generated_column = 0i64;
            let mut segments = Vec::new();
            for segment in line.split(',').filter(|s| !s.is_empty()) {
                let bytes = segment.as_bytes();
                let mut fields = [0i64; 5];
                let (mut count, mut at) = (0usize, 0usize);
                while at < bytes.len() {
                    if count == 5 {
                        return Err(Error::Bad("a segment with more than five fields"));
                    }
                    let (value, used) = vlq(&bytes[at..]).ok_or(Error::Bad("a number that does not end"))?;
                    fields[count] = value;
                    count += 1;
                    at += used;
                }
                if count != 1 && count != 4 && count != 5 {
                    return Err(Error::Bad("a segment of two or three fields"));
                }
                generated_column += fields[0];
                if generated_column < 0 {
                    return Err(Error::Bad("a negative column"));
                }
                let origin = if count >= 4 {
                    source += fields[1];
                    src_line += fields[2];
                    src_col += fields[3];
                    let named = if count == 5 {
                        name += fields[4];
                        u32::try_from(name).ok()
                    } else {
                        None
                    };
                    match (u32::try_from(source), u32::try_from(src_line), u32::try_from(src_col)) {
                        (Ok(s), Ok(l), Ok(c)) => Some((s, l, c, named)),
                        _ => return Err(Error::Bad("a position before the start")),
                    }
                } else {
                    None
                };
                segments.push(Segment { generated_column: generated_column as u32, source: origin });
            }
            lines.push(segments);
        }
        Ok(SourceMap { sources, names, lines })
    }

    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// How many lines of the bundle the map covers.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// The original place of `line` and `column` of the bundle, both counted from 1 as stacks print them.
    pub fn lookup(&self, line: u32, column: u32) -> Option<Original> {
        let segments = self.lines.get(line.checked_sub(1)? as usize)?;
        let wanted = column.saturating_sub(1);
        // The last segment that starts at or before the column.
        let at = segments.partition_point(|s| s.generated_column <= wanted);
        let segment = segments.get(at.checked_sub(1)?)?;
        let (source, src_line, src_col, name) = segment.source?;
        Some(Original {
            source: self.sources.get(source as usize)?.clone(),
            line: src_line + 1,
            column: src_col + 1,
            name: name.and_then(|n| self.names.get(n as usize)).cloned().filter(|n| !n.is_empty()),
        })
    }

    /// Rewrites the frames of a JavaScript stack, one per line, to point at the original files. Lines that are not
    /// frames, and frames the map does not cover, are left as they are.
    pub fn stack(&self, stack: &str) -> String {
        let mut out = String::with_capacity(stack.len());
        for line in stack.lines() {
            match parse_js_frame(line).and_then(|f| self.lookup(f.line, f.column).map(|o| (f, o))) {
                Some((frame, original)) => {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    let function = original.name.as_deref().or(frame.function.as_deref()).unwrap_or("anonymous");
                    out.push_str(&format!(
                        "{indent}at {function} ({}:{}:{})\n",
                        original.source, original.line, original.column
                    ));
                }
                None => {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        out
    }
}

pub use emulabzip_core::{JsFrame, parse_js_frame};

#[cfg(test)]
mod tests {
    use super::*;

    /// A segment of a map built by a test: generated line and column, source, source line and column, name.
    type Seg = (usize, i64, i64, i64, i64, Option<i64>);

    /// A map for a bundle, built segment by segment.
    fn build(segments: &[Seg], sources: &[&str], names: &[&str]) -> String {
        let mut lines: Vec<Vec<String>> = Vec::new();
        let (mut source, mut line, mut col, mut name) = (0, 0, 0, 0);
        let mut last_gen_col = [0i64; 8];
        for &(gl, gc, s, sl, sc, n) in segments {
            while lines.len() <= gl {
                lines.push(Vec::new());
            }
            let mut seg = encode_vlq(gc - last_gen_col[gl]);
            last_gen_col[gl] = gc;
            seg += &encode_vlq(s - source);
            seg += &encode_vlq(sl - line);
            seg += &encode_vlq(sc - col);
            (source, line, col) = (s, sl, sc);
            if let Some(n) = n {
                seg += &encode_vlq(n - name);
                name = n;
            }
            lines[gl].push(seg);
        }
        let mappings: Vec<String> = lines.iter().map(|l| l.join(",")).collect();
        serde_json::json!({ "version": 3, "sources": sources, "names": names, "mappings": mappings.join(";") })
            .to_string()
    }

    #[test]
    fn numbers_are_written_and_read_the_way_the_spec_says() {
        for (n, text) in [(0, "A"), (1, "C"), (-1, "D"), (16, "gB"), (-16, "hB"), (123, "2H"), (1_000_000, "gkh9B")] {
            assert_eq!(encode_vlq(n), text, "{n}");
            assert_eq!(vlq(text.as_bytes()), Some((n, text.len())), "{n}");
        }
        assert_eq!(vlq(b"g"), None, "a number that does not end");
        assert_eq!(vlq(b"!"), None);
        assert_eq!(vlq(b""), None);
        assert_eq!(vlq(b"gggggggggggggggggggg"), None, "too long to be a number");
    }

    #[test]
    fn finds_the_original_place() {
        let text = build(
            &[(0, 0, 0, 0, 0, None), (0, 40, 0, 11, 4, Some(0)), (0, 90, 1, 3, 8, Some(1)), (1, 5, 1, 20, 2, None)],
            &["App.tsx", "src/util.ts"],
            &["onPress", "pad"],
        );
        let map = SourceMap::parse(&text).unwrap();
        assert_eq!((map.sources(), map.line_count()), (&["App.tsx".to_string(), "src/util.ts".to_string()][..], 2));
        let o = map.lookup(1, 41).unwrap();
        assert_eq!((o.source.as_str(), o.line, o.column, o.name.as_deref()), ("App.tsx", 12, 5, Some("onPress")));
        // Inside the span of a segment, the segment before is the one.
        let o = map.lookup(1, 80).unwrap();
        assert_eq!((o.line, o.name.as_deref()), (12, Some("onPress")));
        let o = map.lookup(1, 91).unwrap();
        assert_eq!((o.source.as_str(), o.line, o.name.as_deref()), ("src/util.ts", 4, Some("pad")));
        let o = map.lookup(2, 6).unwrap();
        assert_eq!((o.source.as_str(), o.line, o.column, o.name), ("src/util.ts", 21, 3, None));
        // Before the first segment, past the last line, and line 0 say nothing.
        assert!(map.lookup(2, 1).is_none());
        assert!(map.lookup(3, 1).is_none());
        assert!(map.lookup(0, 1).is_none());
    }

    #[test]
    fn hermes_bytecode_offsets_are_columns_of_line_one() {
        // A composed Hermes map has one line, and the columns are offsets in the bytecode.
        let text = build(
            &[(0, 100, 0, 7, 2, None), (0, 5000, 0, 30, 8, Some(0)), (0, 90_000, 0, 55, 0, None)],
            &["App.tsx"],
            &["boom"],
        );
        let map = SourceMap::parse(&text).unwrap();
        let o = map.lookup(1, 5001).unwrap();
        assert_eq!((o.line, o.column, o.name.as_deref()), (31, 9, Some("boom")));
        assert_eq!(map.lookup(1, 95_000).unwrap().line, 56);
    }

    #[test]
    fn the_source_root_goes_in_front_of_relative_sources() {
        let text = r#"{"version":3,"sourceRoot":"/repo/","sources":["a.ts","/abs/b.ts","http://x/c.ts"],"names":[],"mappings":"AAAA"}"#;
        let map = SourceMap::parse(text).unwrap();
        assert_eq!(map.sources(), ["/repo/a.ts", "/abs/b.ts", "http://x/c.ts"]);
    }

    #[test]
    fn says_what_is_wrong_with_a_map() {
        assert_eq!(SourceMap::parse("nope").unwrap_err(), Error::NotJson);
        assert_eq!(SourceMap::parse("{}").unwrap_err(), Error::NotAMap);
        assert_eq!(SourceMap::parse(r#"{"sections":[]}"#).unwrap_err(), Error::Indexed);
        assert!(SourceMap::parse(r#"{"mappings":"AAAA,,AAAA,"}"#).is_ok(), "an empty segment is skipped");
        assert!(matches!(SourceMap::parse(r#"{"mappings":"AAAAAAA"}"#).unwrap_err(), Error::Bad(_)));
        assert!(matches!(SourceMap::parse(r#"{"mappings":"AA"}"#).unwrap_err(), Error::Bad(_)));
        assert!(matches!(SourceMap::parse(r#"{"mappings":"g"}"#).unwrap_err(), Error::Bad(_)));
        assert!(SourceMap::parse(r#"{"mappings":"D"}"#).is_err(), "a negative column");
        assert!(SourceMap::parse(r#"{"mappings":"AADD"}"#).is_err(), "a line before the start");
        assert!(SourceMap::parse("{}").unwrap_err().to_string().contains("not a source map"));
    }

    #[test]
    fn rewrites_a_stack_and_leaves_the_rest() {
        let text = build(&[(0, 0, 0, 0, 0, None), (0, 5000, 0, 30, 8, Some(0))], &["src/App.tsx"], &["crashDeep"]);
        let map = SourceMap::parse(&text).unwrap();
        let stack = "Error: boom\n    at a (address at index.android.bundle:1:5001)\n    at b (address at other.bundle:9:9)\n    at c (native)\n";
        let out = map.stack(stack);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "Error: boom");
        assert_eq!(lines[1], "    at crashDeep (src/App.tsx:31:9)");
        assert_eq!(lines[2], "    at b (address at other.bundle:9:9)");
        assert_eq!(lines[3], "    at c (native)");
    }

    #[test]
    fn garbage_does_not_panic() {
        let mut seed = 0xBEEF_u32;
        for _ in 0..400 {
            let mappings: String = (0..60)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let alphabet = b"AAAAggBCD;,/+zZ09!";
                    alphabet[(seed >> 24) as usize % alphabet.len()] as char
                })
                .collect();
            let json =
                serde_json::json!({ "version": 3, "sources": ["a"], "names": ["n"], "mappings": mappings }).to_string();
            if let Ok(map) = SourceMap::parse(&json) {
                let _ = map.lookup(1, 1);
                let _ = map.lookup(u32::MAX, u32::MAX);
                let _ = map.stack("at x (b:1:1)");
            }
        }
    }
}
