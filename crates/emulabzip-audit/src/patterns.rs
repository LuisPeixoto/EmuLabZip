//! Finding secrets and personal data in text, and cutting what is found down so a report does not repeat it.
//!
//! One table drives every place text is scanned (the log, preferences, request URLs and bodies), so a pattern added here
//! is found everywhere.

use std::sync::LazyLock;

use emulabzip_core::Severity;
use regex::Regex;

/// What a pattern looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind {
    /// The rule name suffix: `jwt`, `bearer-token`, `api-key`, `email`, `card-number`, `secret`.
    pub name: &'static str,
    /// What it is, in words.
    pub what: &'static str,
    pub severity: Severity,
    /// Personal data rather than a credential.
    pub personal: bool,
}

const JWT: Kind = Kind { name: "jwt", what: "a JSON Web Token", severity: Severity::Error, personal: false };
const BEARER: Kind = Kind { name: "bearer-token", what: "a bearer token", severity: Severity::Error, personal: false };
const API_KEY: Kind = Kind { name: "api-key", what: "an API key", severity: Severity::Error, personal: false };
const SECRET: Kind =
    Kind { name: "secret", what: "a value named like a secret", severity: Severity::Warning, personal: false };
const EMAIL: Kind = Kind { name: "email", what: "an email address", severity: Severity::Warning, personal: true };
const CARD: Kind = Kind { name: "card-number", what: "a card number", severity: Severity::Error, personal: true };

struct Pattern {
    kind: Kind,
    regex: Regex,
    /// The capture group that holds the value, 0 for the whole match.
    group: usize,
}

fn pattern(kind: Kind, source: &str, group: usize) -> Pattern {
    Pattern { kind, regex: Regex::new(source).expect("a valid pattern"), group }
}

static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    vec![
        pattern(JWT, r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", 0),
        pattern(BEARER, r"(?i)\bbearer\s+([A-Za-z0-9._~+/=-]{16,})", 1),
        // Google, AWS, Stripe, GitHub and Slack keys have shapes of their own.
        pattern(
            API_KEY,
            r"\b(AIza[0-9A-Za-z_-]{35}|AKIA[0-9A-Z]{16}|sk_live_[0-9a-zA-Z]{16,}|ghp_[A-Za-z0-9]{36}|xox[baprs]-[A-Za-z0-9-]{10,})\b",
            1,
        ),
        pattern(
            SECRET,
            r#"(?i)\b(?:api[_-]?key|secret|access[_-]?token|auth[_-]?token|refresh[_-]?token|password|passwd|pwd)["']?\s*[:=]\s*["']?([A-Za-z0-9._~+/=!@#$%^&*-]{6,})"#,
            1,
        ),
        pattern(EMAIL, r"\b[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9-]{1,63}(?:\.[A-Za-z0-9-]{1,63})*\.[A-Za-z]{2,24}\b", 0),
        pattern(CARD, r"\b(?:\d[ -]?){12,18}\d\b", 0),
    ]
});

/// One thing found in a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub kind: Kind,
    /// The value cut down by [`redact`].
    pub redacted: String,
    /// Where it starts in the text.
    pub at: usize,
}

/// A value shortened so it can be shown without being leaked: its first two and last two characters, and how long it was.
pub fn redact(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return format!("{} ({} chars)", "*".repeat(chars.len().min(4)), chars.len());
    }
    let head: String = chars[..2].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{head}…{tail} ({} chars)", chars.len())
}

/// The digits of a card number pass the Luhn check, which keeps order numbers and timestamps from being taken for cards.
pub fn luhn(digits: &str) -> bool {
    let nums: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&nums.len()) {
        return false;
    }
    let sum: u32 = nums
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| if i % 2 == 1 { if d * 2 > 9 { d * 2 - 9 } else { d * 2 } } else { d })
        .sum();
    sum % 10 == 0
}

/// Everything in `text` that looks like a credential or personal data. A value reported by a specific pattern (a JWT)
/// is not reported again by a general one (a secret named `token`).
pub fn scan(text: &str) -> Vec<Hit> {
    let mut hits: Vec<(usize, usize, Hit)> = Vec::new();
    for p in PATTERNS.iter() {
        for caps in p.regex.captures_iter(text) {
            let Some(m) = caps.get(p.group) else { continue };
            if p.kind == CARD && !luhn(m.as_str()) {
                continue;
            }
            // A placeholder such as `password=****` or `${TOKEN}` is not a secret.
            if p.kind == SECRET && is_placeholder(m.as_str()) {
                continue;
            }
            if hits.iter().any(|(s, e, _)| m.start() < *e && *s < m.end()) {
                continue;
            }
            hits.push((m.start(), m.end(), Hit { kind: p.kind, redacted: redact(m.as_str()), at: m.start() }));
        }
    }
    hits.sort_by_key(|(s, _, _)| *s);
    hits.into_iter().map(|(_, _, h)| h).collect()
}

fn is_placeholder(value: &str) -> bool {
    let v = value.trim_matches(|c| c == '"' || c == '\'');
    v.chars().all(|c| matches!(c, '*' | 'x' | 'X' | '•' | '.' | '-'))
        || v.starts_with("${")
        || v.starts_with("{{")
        || matches!(v.to_ascii_lowercase().as_str(), "null" | "none" | "undefined" | "true" | "false" | "redacted")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<&'static str> {
        scan(text).iter().map(|h| h.kind.name).collect()
    }

    #[test]
    fn credentials_are_found_and_cut_down() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let hits = scan(&format!("auth ok token {jwt} sent"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind.name, "jwt");
        assert!(hits[0].redacted.starts_with("ey…") && !hits[0].redacted.contains("HUzI1"), "{:?}", hits[0]);

        assert_eq!(names("Authorization: Bearer abcdef0123456789abcdef"), ["bearer-token"]);
        assert_eq!(names("key=AIzaSyA1234567890abcdefghijklmnopqrstuv"), ["api-key"]);
        assert_eq!(names("AKIAIOSFODNN7EXAMPLE"), ["api-key"]);
        assert_eq!(names(r#"{"password": "hunter2hunter2"}"#), ["secret"]);
        assert_eq!(names("api_key=abc123def456"), ["secret"]);
    }

    #[test]
    fn personal_data_is_found() {
        assert_eq!(names("sent to ana.silva@example.com ok"), ["email"]);
        assert_eq!(names("card 4111 1111 1111 1111 charged"), ["card-number"]);
        assert_eq!(names("card 4111-1111-1111-1111"), ["card-number"]);
    }

    #[test]
    fn what_only_looks_like_a_secret_is_left_alone() {
        // Fails the Luhn check: an order number, not a card.
        assert!(names("order 1234567890123456").is_empty());
        assert!(names("password=********").is_empty());
        assert!(names("token=${TOKEN}").is_empty());
        assert!(names("secret: null").is_empty());
        assert!(names("nothing to see, user@localhost").is_empty());
    }

    #[test]
    fn a_value_is_reported_once_by_the_most_specific_pattern() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(names(&format!("access_token={jwt}")), ["jwt"]);
    }

    #[test]
    fn redaction_never_shows_the_middle() {
        assert_eq!(redact("short"), "***** (5 chars)".replace("*****", "****"));
        let r = redact("supersecretvalue");
        assert_eq!(r, "su…ue (16 chars)");
        assert!(luhn("4111111111111111") && !luhn("4111111111111112") && !luhn("12345"));
    }
}
