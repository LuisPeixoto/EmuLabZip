//! Restoring the names ProGuard and R8 changed, from a `mapping.txt`.
//!
//! The file lists every class that was renamed, with its members:
//!
//! ```text
//! com.example.app.MainActivity -> a.b.c:
//! # {"id":"sourceFile","fileName":"MainActivity.kt"}
//!     1:3:void crashDeep(int):40:42 -> a
//!     4:4:void helper():10:10 -> a
//!     4:4:void crashDeep(int):43:43 -> a
//!     void onCreate(android.os.Bundle) -> onCreate
//! ```
//!
//! A method line has the lines of the shrunk code (`1:3`), the original method and its lines (`40:42`) and the new
//! name. Several entries for the same shrunk lines mean the compiler **inlined** a method: the first is the method that
//! was inlined, the last the one it was inlined into, and one frame of the stack becomes several.

use std::collections::HashMap;

/// What a frame of a stack was before the code was shrunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retraced {
    /// The original class, `com.example.app.MainActivity`.
    pub class: String,
    pub method: String,
    /// The source file the mapping names (`MainActivity.kt`), when it says.
    pub file: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone)]
struct Entry {
    /// The lines in the shrunk code this entry covers.
    obf: Option<(u32, u32)>,
    /// The class of the original method, when it is not the class of the block (an inlined method).
    class: Option<String>,
    method: String,
    /// The lines in the original source.
    orig: Option<(u32, u32)>,
}

#[derive(Debug, Default, Clone)]
struct Class {
    original: String,
    source_file: Option<String>,
    methods: HashMap<String, Vec<Entry>>,
}

/// A parsed `mapping.txt`.
#[derive(Debug, Default, Clone)]
pub struct Mapping {
    classes: HashMap<String, Class>,
    /// Source files by original class, so an inlined method can name its own file.
    files: HashMap<String, String>,
}

impl Mapping {
    /// Reads a whole mapping.
    pub fn parse(text: &str) -> Mapping {
        Mapping::parse_with(text, |_| true)
    }

    /// Reads a mapping, keeping the members only of the classes `wanted` says yes to. Every class is still known by
    /// name, so a big mapping costs memory only for the classes of the stack.
    pub fn parse_with(text: &str, wanted: impl Fn(&str) -> bool) -> Mapping {
        let mut mapping = Mapping::default();
        let mut current: Option<(String, Class, bool)> = None;
        let finish = |m: &mut Mapping, c: Option<(String, Class, bool)>| {
            if let Some((obf, class, _)) = c {
                if let Some(file) = &class.source_file {
                    m.files.insert(class.original.clone(), file.clone());
                }
                m.classes.insert(obf, class);
            }
        };
        for line in text.lines() {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            if !trimmed.starts_with([' ', '\t', '#']) {
                // `original -> obfuscated:`
                finish(&mut mapping, current.take());
                if let Some((original, rest)) = trimmed.split_once(" -> ") {
                    let obf = rest.trim_end_matches(':').trim().to_string();
                    let keep = wanted(&obf);
                    let class = Class { original: original.trim().to_string(), ..Default::default() };
                    current = Some((obf, class, keep));
                }
                continue;
            }
            let Some((_, class, keep)) = current.as_mut() else { continue };
            let body = trimmed.trim_start();
            if let Some(comment) = body.strip_prefix('#') {
                if let Some(file) = source_file_of(comment) {
                    class.source_file = Some(file);
                }
                continue;
            }
            if !*keep {
                continue;
            }
            if let Some((name, entry)) = parse_member(body) {
                class.methods.entry(name).or_default().push(entry);
            }
        }
        finish(&mut mapping, current.take());
        mapping
    }

    /// How many classes the mapping renames.
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    /// The original name of an obfuscated class.
    pub fn class(&self, obfuscated: &str) -> Option<&str> {
        self.classes.get(obfuscated).map(|c| c.original.as_str())
    }

    /// Whether this mapping knows `obfuscated` as a class. A stack is only read with the mapping of its own build,
    /// and this is how to tell.
    pub fn knows(&self, obfuscated: &str) -> bool {
        self.classes.contains_key(obfuscated)
    }

    /// The frames a frame of a stack stands for: the inlined methods first, then the method they were inlined into.
    /// A frame the mapping does not know comes back as it is.
    pub fn frame(&self, class: &str, method: &str, line: Option<u32>) -> Vec<Retraced> {
        let Some(c) = self.classes.get(class) else {
            return vec![Retraced { class: class.into(), method: method.into(), file: None, line }];
        };
        let file_of = |original: &str| self.files.get(original).cloned();
        let own = |entry: &Entry, line: Option<u32>| Retraced {
            class: entry.class.clone().unwrap_or_else(|| c.original.clone()),
            method: entry.method.clone(),
            file: match &entry.class {
                Some(other) => file_of(other),
                None => c.source_file.clone(),
            },
            line,
        };
        let Some(entries) = c.methods.get(method) else {
            return vec![Retraced {
                class: c.original.clone(),
                method: method.into(),
                file: c.source_file.clone(),
                line,
            }];
        };
        // The entries that cover the line. Without a line, the ones that have no lines are all there is to go by.
        let covering: Vec<&Entry> = match line {
            Some(l) => entries.iter().filter(|e| e.obf.is_some_and(|(a, b)| a <= l && l <= b)).collect(),
            None => entries.iter().filter(|e| e.obf.is_none()).collect(),
        };
        if covering.is_empty() {
            // The line is in no range: the method is known, the line is not. Name the method of the first entry.
            let e = entries.first().expect("a method has entries");
            let mut r = own(e, None);
            if entries.iter().any(|e2| e2.method != e.method) {
                // Overloads or inlining: only the class is safe to say.
                r.method = method.into();
            }
            return vec![r];
        }
        covering
            .into_iter()
            .map(|e| {
                // When both ranges are as long as each other, the line moves by the same offset; otherwise the range
                // maps to a single place, its start.
                let original_line = match (line, e.obf, e.orig) {
                    (Some(l), Some((a, b)), Some((oa, ob)))
                        if b.checked_sub(a).is_some() && b - a == ob.wrapping_sub(oa) =>
                    {
                        Some(oa.saturating_add(l.saturating_sub(a)))
                    }
                    (_, _, Some((oa, _))) => Some(oa),
                    (l, _, None) => l,
                };
                own(e, original_line)
            })
            .collect()
    }

    /// Replaces every obfuscated class name found in `text` (an exception title, say) with its original.
    pub fn text(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if is_start(bytes[i]) && (i == 0 || !is_part(bytes[i - 1])) {
                let mut j = i;
                while j < bytes.len() && is_part(bytes[j]) {
                    j += 1;
                }
                // A trailing dot ends a sentence, not a name.
                let mut word = &text[i..j];
                let mut tail = "";
                while let Some(stripped) = word.strip_suffix('.') {
                    word = stripped;
                    tail = &text[i + word.len()..j];
                }
                match self.class(word) {
                    Some(original) => out.push_str(original),
                    None => out.push_str(word),
                }
                out.push_str(tail);
                i = j;
            } else {
                // Not the start of a name: copy one character.
                let ch = text[i..].chars().next().expect("not at the end");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
        out
    }

    /// Retraces a stack trace as the JVM prints it, line by line: `at a.b.c.a(SourceFile:4)` becomes one `at` line
    /// for each frame it stands for. Other lines (the exception, `Caused by:`) have their class names restored.
    pub fn stack(&self, stack: &str) -> String {
        let mut out = String::with_capacity(stack.len());
        for line in stack.lines() {
            let t = line.trim_start();
            let indent = &line[..line.len() - t.len()];
            match parse_at_line(t) {
                Some((symbol, place)) => {
                    let (class, method) = symbol.rsplit_once('.').unwrap_or(("", symbol));
                    let line_no = place.rsplit_once(':').and_then(|(_, l)| l.parse::<u32>().ok());
                    for r in self.frame(class, method, line_no) {
                        let file =
                            r.file.as_deref().or_else(|| place.rsplit_once(':').map(|(f, _)| f)).unwrap_or(place);
                        let location = match r.line {
                            Some(l) => format!("{file}:{l}"),
                            None => file.to_string(),
                        };
                        out.push_str(&format!("{indent}at {}.{}({location})\n", r.class, r.method));
                    }
                }
                None => {
                    out.push_str(&self.text(line));
                    out.push('\n');
                }
            }
        }
        out
    }
}

/// `at a.b.c.a(SourceFile:4)`: the symbol and what is in the parentheses.
pub fn parse_at_line(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("at ")?;
    let (symbol, place) = rest.split_once('(')?;
    Some((symbol, place.trim_end_matches(')')))
}

fn is_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

fn is_part(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'.')
}

/// `{"id":"sourceFile","fileName":"MainActivity.kt"}`
fn source_file_of(comment: &str) -> Option<String> {
    let json = comment.trim();
    if !json.starts_with('{') || !json.contains("\"sourceFile\"") {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v.get("id")?.as_str()? != "sourceFile" {
        return None;
    }
    v.get("fileName")?.as_str().map(str::to_string)
}

/// A member line. Fields (no parentheses) are not needed for stacks, so they give `None`.
///
/// `1:3:void crashDeep(int):40:42 -> a`, `void onCreate(android.os.Bundle) -> onCreate`,
/// `4:4:void com.x.Other.helper():10:10 -> a`
fn parse_member(line: &str) -> Option<(String, Entry)> {
    let (left, new_name) = line.rsplit_once(" -> ")?;
    let new_name = new_name.trim().to_string();
    if !left.contains('(') {
        return None;
    }
    // The leading `a:b:` of the shrunk lines.
    let mut rest = left;
    let mut obf = None;
    if let Some((a, r)) = rest.split_once(':')
        && let Ok(a) = a.parse::<u32>()
    {
        let (b, r2) = r.split_once(':')?;
        let b = b.parse::<u32>().ok()?;
        obf = Some((a, b));
        rest = r2;
    }
    // `type name(args)` then optionally `:a:b` of the original lines.
    let close = rest.rfind(')')?;
    let (signature, tail) = rest.split_at(close + 1);
    let orig = tail.strip_prefix(':').and_then(|t| {
        let mut p = t.split(':');
        let a = p.next()?.parse::<u32>().ok()?;
        let b = p.next().and_then(|b| b.parse::<u32>().ok()).unwrap_or(a);
        Some((a, b))
    });
    let (before_args, _) = signature.split_once('(')?;
    let name = before_args.rsplit(' ').next()?;
    // A method of another class is written with its class: `com.x.Other.helper`.
    let (class, method) = match name.rsplit_once('.') {
        Some((c, m)) => (Some(c.to_string()), m.to_string()),
        None => (None, name.to_string()),
    };
    Some((new_name, Entry { obf, class, method, orig }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAPPING: &str = r#"# compiler: R8
# compiler_version: 8.13.1
com.example.app.MainActivity -> a.b.c:
# {"id":"sourceFile","fileName":"MainActivity.kt"}
    int taps -> a
    1:3:void crashDeep(int):40:42 -> a
    4:4:void helper():10:10 -> a
    4:4:void crashDeep(int):43:43 -> a
    5:5:java.lang.String com.example.util.Strings.pad(java.lang.String):7:7 -> a
    5:5:void crashDeep(int):44:44 -> a
    6:9:void tap():60:63 -> b
    void onCreate(android.os.Bundle) -> onCreate
com.example.util.Strings -> a.b.d:
# {"id":"sourceFile","fileName":"Strings.kt"}
    7:7:java.lang.String pad(java.lang.String):7:7 -> a
com.example.Plain -> com.example.Plain:
    void run() -> run
"#;

    fn mapping() -> Mapping {
        Mapping::parse(MAPPING)
    }

    #[test]
    fn restores_class_names() {
        let m = mapping();
        assert_eq!(m.class("a.b.c"), Some("com.example.app.MainActivity"));
        assert_eq!(m.class("not.there"), None);
        assert_eq!(m.class_count(), 3);
        assert!(m.knows("a.b.d") && !m.knows("com.example.app.MainActivity"));
    }

    #[test]
    fn maps_a_line_inside_a_range_by_offset() {
        let r = mapping().frame("a.b.c", "a", Some(2));
        assert_eq!(
            r,
            vec![Retraced {
                class: "com.example.app.MainActivity".into(),
                method: "crashDeep".into(),
                file: Some("MainActivity.kt".into()),
                line: Some(41)
            }]
        );
        // A range that maps to a range of another length lands on its start.
        let r = mapping().frame("a.b.c", "b", Some(8));
        assert_eq!((r[0].method.as_str(), r[0].line), ("tap", Some(62)));
    }

    #[test]
    fn an_inlined_method_becomes_several_frames_inner_first() {
        let r = mapping().frame("a.b.c", "a", Some(4));
        let names: Vec<(&str, Option<u32>)> = r.iter().map(|f| (f.method.as_str(), f.line)).collect();
        assert_eq!(names, [("helper", Some(10)), ("crashDeep", Some(43))]);
    }

    #[test]
    fn a_method_inlined_from_another_class_names_that_class_and_its_file() {
        let r = mapping().frame("a.b.c", "a", Some(5));
        assert_eq!(r.len(), 2);
        assert_eq!(
            (r[0].class.as_str(), r[0].method.as_str(), r[0].file.as_deref(), r[0].line),
            ("com.example.util.Strings", "pad", Some("Strings.kt"), Some(7))
        );
        assert_eq!((r[1].class.as_str(), r[1].method.as_str()), ("com.example.app.MainActivity", "crashDeep"));
    }

    #[test]
    fn a_line_outside_every_range_keeps_the_method_it_cannot_be_sure_of() {
        let r = mapping().frame("a.b.c", "a", Some(99));
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].class.as_str(), r[0].line), ("com.example.app.MainActivity", None));
        // A method with no lines in the mapping is found by its new name.
        let r = mapping().frame("a.b.c", "onCreate", None);
        assert_eq!((r[0].method.as_str(), r[0].line), ("onCreate", None));
    }

    #[test]
    fn what_the_mapping_does_not_know_comes_back_unchanged() {
        let r = mapping().frame("android.app.Activity", "performCreate", Some(8000));
        assert_eq!(
            r,
            vec![Retraced {
                class: "android.app.Activity".into(),
                method: "performCreate".into(),
                file: None,
                line: Some(8000)
            }]
        );
        let r = mapping().frame("a.b.c", "zzz", Some(3));
        assert_eq!((r[0].class.as_str(), r[0].method.as_str()), ("com.example.app.MainActivity", "zzz"));
    }

    #[test]
    fn restores_class_names_inside_text() {
        let m = mapping();
        assert_eq!(
            m.text("java.lang.IllegalStateException: a.b.c was destroyed (see a.b.d)."),
            "java.lang.IllegalStateException: com.example.app.MainActivity was destroyed (see com.example.util.Strings)."
        );
        assert_eq!(m.text("xa.b.c a.b.cx"), "xa.b.c a.b.cx", "only whole names");
        assert_eq!(m.text(""), "");
        assert_eq!(m.text("é a.b.c ü"), "é com.example.app.MainActivity ü");
    }

    #[test]
    fn retraces_a_whole_stack() {
        let stack = "java.lang.RuntimeException: boom in a.b.c\n\tat a.b.c.a(SourceFile:4)\n\tat a.b.c.b(SourceFile:7)\n\tat android.app.Activity.performCreate(Activity.java:8000)\nCaused by: a.b.d: nested\n\t... 3 more\n";
        let out = mapping().stack(stack);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "java.lang.RuntimeException: boom in com.example.app.MainActivity");
        assert_eq!(lines[1], "\tat com.example.app.MainActivity.helper(MainActivity.kt:10)");
        assert_eq!(lines[2], "\tat com.example.app.MainActivity.crashDeep(MainActivity.kt:43)");
        assert_eq!(lines[3], "\tat com.example.app.MainActivity.tap(MainActivity.kt:61)");
        assert_eq!(lines[4], "\tat android.app.Activity.performCreate(Activity.java:8000)");
        assert_eq!(lines[5], "Caused by: com.example.util.Strings: nested");
        assert_eq!(lines[6], "\t... 3 more");
    }

    #[test]
    fn reading_only_some_classes_still_knows_all_of_them() {
        let m = Mapping::parse_with(MAPPING, |obf| obf == "a.b.c");
        assert_eq!(m.class_count(), 3);
        assert_eq!(m.frame("a.b.c", "a", Some(2))[0].method, "crashDeep");
        // The members of a class that was not asked for are not kept.
        assert_eq!(m.frame("a.b.d", "a", Some(7))[0].method, "a");
        // The file of an inlined method comes from the class header, which is always read.
        assert_eq!(m.frame("a.b.c", "a", Some(5))[0].file.as_deref(), Some("Strings.kt"));
    }

    #[test]
    fn backwards_ranges_do_not_overflow() {
        let m = Mapping::parse("a.B -> b:\n    9:3:void m():7:1 -> a\n    3:9:void n():1:7 -> a\n");
        for line in [0, 3, 5, 9, u32::MAX] {
            let _ = m.frame("b", "a", Some(line));
        }
    }

    #[test]
    fn garbage_does_not_panic() {
        let mut seed = 0x1234_5678u32;
        for _ in 0..300 {
            let text: String = (0..200)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let alphabet = b" ->:()#{}\n\t.0123456789abcxyz$";
                    alphabet[(seed >> 24) as usize % alphabet.len()] as char
                })
                .collect();
            let m = Mapping::parse(&text);
            let _ = m.stack(&text);
            let _ = m.frame("a", "b", Some(3));
        }
    }

    #[test]
    fn odd_input_is_not_a_problem() {
        for text in [
            "",
            "\n\n",
            "   indented but no class\n",
            "a -> b\n  1:x:void m() -> a\n",
            "# only a comment",
            "a -> b:\n    :::: -> \n",
        ] {
            let m = Mapping::parse(text);
            let _ = m.frame("b", "a", Some(1));
            let _ = m.text(text);
            let _ = m.stack(text);
        }
    }
}
