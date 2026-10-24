//! Rules that change what the proxy does with a request: answer it with a made-up response, delay
//! it, fail it, or hold it at a breakpoint so it can be edited.
//!
//! The first enabled rule that matches decides. Rules live in a [`RuleSet`] that the app and the
//! proxy share, and that is saved to a file so they survive a restart.

use std::path::{Path, PathBuf};

use emulabzip_core::{Error, Result, data_dir};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::flow::Headers;

/// Largest mock body accepted, in bytes.
pub const MAX_MOCK_BODY: usize = 5 * 1024 * 1024;
/// Longest delay a rule may add, in milliseconds.
pub const MAX_DELAY_MS: u64 = 60_000;

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// Assigned when the rules are saved.
    #[serde(default)]
    pub id: u64,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub matcher: Matcher,
    pub action: Action,
}

/// Which requests a rule applies to. Every field that is set has to match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Matcher {
    /// Methods such as `GET`. Empty means any.
    pub methods: Vec<String>,
    /// A host pattern, where `*` is any run of characters and `?` any one: `api.*.example.com`.
    pub host: Option<String>,
    /// A path pattern, without the query: `/v1/users/*`.
    pub path: Option<String>,
    /// Text the whole URL (host, path and query) has to contain.
    pub url_contains: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Answer without asking the server.
    #[serde(rename_all = "camelCase")]
    Mock {
        status: u16,
        #[serde(default)]
        headers: Headers,
        #[serde(default)]
        body: String,
        /// The body is base64 of the bytes, for content that is not text.
        #[serde(default)]
        base64: bool,
        #[serde(default)]
        delay_ms: u64,
    },
    /// Wait before forwarding.
    Delay { ms: u64 },
    /// Fail the request, as if the network had.
    Block,
    /// Hold the request and/or the response until someone continues it.
    Breakpoint {
        #[serde(default)]
        request: bool,
        #[serde(default)]
        response: bool,
    },
}

/// `*` and `?` patterns, anchored at both ends.
pub fn glob_match(pattern: &str, text: &str, ignore_case: bool) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = if ignore_case {
        (pattern.to_lowercase().chars().collect(), text.to_lowercase().chars().collect())
    } else {
        (pattern.chars().collect(), text.chars().collect())
    };
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

impl Matcher {
    /// `path` is the path and query as the client sent it.
    pub fn matches(&self, method: &str, host: &str, path: &str) -> bool {
        let only_path = path.split('?').next().unwrap_or(path);
        (self.methods.is_empty() || self.methods.iter().any(|m| m.eq_ignore_ascii_case(method)))
            && self.host.as_deref().filter(|h| !h.is_empty()).is_none_or(|h| glob_match(h, host, true))
            && self.path.as_deref().filter(|p| !p.is_empty()).is_none_or(|p| glob_match(p, only_path, false))
            && self
                .url_contains
                .as_deref()
                .filter(|u| !u.is_empty())
                .is_none_or(|u| format!("{host}{path}").contains(u))
    }
}

impl Rule {
    fn check(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.len() > 120 {
            return Err(Error::invalid("rule name", self.name.clone()));
        }
        match &self.action {
            Action::Mock { status, headers, body, base64, delay_ms } => {
                if !(100..=599).contains(status) {
                    return Err(Error::invalid("status", status.to_string()));
                }
                if *delay_ms > MAX_DELAY_MS {
                    return Err(Error::invalid("delay", format!("{delay_ms} ms (at most {MAX_DELAY_MS})")));
                }
                for (k, v) in headers {
                    if http::HeaderName::from_bytes(k.as_bytes()).is_err() || http::HeaderValue::from_str(v).is_err() {
                        return Err(Error::invalid("header", format!("{k}: {v}")));
                    }
                }
                let size = if *base64 { decode_base64(body)?.len() } else { body.len() };
                if size > MAX_MOCK_BODY {
                    return Err(Error::invalid("mock body", format!("{size} bytes (at most {MAX_MOCK_BODY})")));
                }
            }
            Action::Delay { ms } if *ms > MAX_DELAY_MS => {
                return Err(Error::invalid("delay", format!("{ms} ms (at most {MAX_DELAY_MS})")));
            }
            Action::Breakpoint { request: false, response: false } => {
                return Err(Error::invalid("breakpoint", "stop at the request, the response, or both"));
            }
            _ => {}
        }
        Ok(())
    }
}

/// The bytes of a mock body.
pub fn mock_bytes(body: &str, base64: bool) -> Result<Vec<u8>> {
    if base64 { decode_base64(body) } else { Ok(body.as_bytes().to_vec()) }
}

fn decode_base64(body: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|e| Error::invalid("base64 body", e.to_string()))
}

/// The rules, shared between the proxy and whoever edits them.
#[derive(Debug, Default)]
pub struct RuleSet {
    rules: RwLock<Vec<Rule>>,
    path: Option<PathBuf>,
}

impl RuleSet {
    /// Rules kept in memory only.
    pub fn new() -> Self {
        RuleSet::default()
    }

    /// The rules in `net-rules.json` in the data folder, saved back there on every change.
    pub fn load_default() -> Self {
        RuleSet::load(data_dir().join("net-rules.json"))
    }

    /// Reads the rules in `path` (none when it is missing or damaged) and saves changes back to it.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let rules = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| match serde_json::from_str::<Vec<Rule>>(&text) {
                Ok(rules) => Some(rules),
                Err(e) => {
                    tracing::warn!("ignoring {}: {e}", path.display());
                    None
                }
            })
            .unwrap_or_default();
        RuleSet { rules: RwLock::new(rules), path: Some(path) }
    }

    pub fn rules(&self) -> Vec<Rule> {
        self.rules.read().clone()
    }

    /// Replaces every rule, after checking them. Ids are renumbered from 1.
    pub fn set(&self, mut rules: Vec<Rule>) -> Result<()> {
        for rule in &rules {
            rule.check()?;
        }
        for (i, rule) in rules.iter_mut().enumerate() {
            rule.id = i as u64 + 1;
        }
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let text = serde_json::to_vec_pretty(&rules).map_err(|e| Error::failed(e.to_string()))?;
            let temp = path.with_extension("tmp");
            std::fs::write(&temp, text)?;
            std::fs::rename(temp, path)?;
        }
        *self.rules.write() = rules;
        Ok(())
    }

    /// The first enabled rule that matches. `path` is the path and query.
    pub fn find(&self, method: &str, host: &str, path: &str) -> Option<Rule> {
        self.rules.read().iter().find(|r| r.enabled && r.matcher.matches(method, host, path)).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, matcher: Matcher, action: Action) -> Rule {
        Rule { id: 0, name: name.into(), enabled: true, matcher, action }
    }

    fn mock(status: u16) -> Action {
        Action::Mock { status, headers: vec![], body: String::new(), base64: false, delay_ms: 0 }
    }

    #[test]
    fn globs_match_whole_strings() {
        assert!(glob_match("api.*.example.com", "api.eu.example.com", true));
        assert!(glob_match("*", "", true) && glob_match("a*", "a", true) && glob_match("*a*", "banana", true));
        assert!(glob_match("/v1/users/?", "/v1/users/7", false));
        assert!(!glob_match("/v1/users/?", "/v1/users/77", false));
        assert!(!glob_match("example.com", "api.example.com", true), "anchored at the start");
        assert!(!glob_match("api", "api.example.com", true), "and at the end");
        assert!(glob_match("EXAMPLE.com", "example.COM", true) && !glob_match("A", "a", false));
        assert!(glob_match("a*b*c", "aXXbYYc", false) && !glob_match("a*b*c", "aXXbYY", false));
    }

    #[test]
    fn a_matcher_needs_every_set_field_to_match() {
        let m = Matcher {
            methods: vec!["get".into(), "POST".into()],
            host: Some("api.*".into()),
            path: Some("/users/*".into()),
            url_contains: Some("page=2".into()),
        };
        assert!(m.matches("GET", "api.example.com", "/users/7?page=2"));
        assert!(!m.matches("DELETE", "api.example.com", "/users/7?page=2"), "method");
        assert!(!m.matches("GET", "www.example.com", "/users/7?page=2"), "host");
        assert!(!m.matches("GET", "api.example.com", "/orders/7?page=2"), "path");
        assert!(!m.matches("GET", "api.example.com", "/users/7?page=3"), "url text");
        assert!(Matcher::default().matches("PUT", "anything", "/"), "no fields set matches everything");
    }

    #[test]
    fn the_query_is_not_part_of_the_path_pattern() {
        let m = Matcher { path: Some("/search".into()), ..Default::default() };
        assert!(m.matches("GET", "h", "/search?q=x"));
        assert!(!m.matches("GET", "h", "/search/more"));
    }

    #[test]
    fn the_first_enabled_matching_rule_wins() {
        let set = RuleSet::new();
        let everything = Matcher::default();
        let mut off = rule("off", everything.clone(), mock(500));
        off.enabled = false;
        set.set(vec![
            off,
            rule("a", Matcher { host: Some("a.test".into()), ..Default::default() }, mock(201)),
            rule("b", everything, mock(202)),
        ])
        .unwrap();
        let hit = |host: &str| set.find("GET", host, "/").map(|r| r.name);
        assert_eq!(hit("a.test").as_deref(), Some("a"));
        assert_eq!(hit("other.test").as_deref(), Some("b"));
        assert_eq!(set.rules().iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2, 3], "ids are renumbered");
    }

    #[test]
    fn rejects_rules_that_cannot_work() {
        let set = RuleSet::new();
        let bad = |action: Action| set.set(vec![rule("r", Matcher::default(), action)]).is_err();
        assert!(bad(mock(99)) && bad(mock(600)));
        assert!(bad(Action::Delay { ms: MAX_DELAY_MS + 1 }));
        assert!(bad(Action::Breakpoint { request: false, response: false }));
        assert!(bad(Action::Mock {
            status: 200,
            headers: vec![("bad header".into(), "x".into())],
            body: String::new(),
            base64: false,
            delay_ms: 0
        }));
        assert!(bad(Action::Mock {
            status: 200,
            headers: vec![],
            body: "not base64!".into(),
            base64: true,
            delay_ms: 0
        }));
        assert!(set.set(vec![rule(" ", Matcher::default(), Action::Block)]).is_err(), "a rule needs a name");
        assert!(set.set(vec![rule("ok", Matcher::default(), Action::Block)]).is_ok());
        assert!(set.rules().len() == 1, "a rejected set leaves the rules as they were");
    }

    #[test]
    fn rules_are_saved_and_loaded() {
        let path = std::env::temp_dir().join(format!("emulabzip-rules-{}.json", std::process::id()));
        let set = RuleSet::load(&path);
        assert!(set.rules().is_empty());
        set.set(vec![rule("health", Matcher { path: Some("/health".into()), ..Default::default() }, mock(503))])
            .unwrap();
        let again = RuleSet::load(&path);
        assert_eq!(again.rules(), set.rules());
        std::fs::write(&path, "not json").unwrap();
        assert!(RuleSet::load(&path).rules().is_empty(), "a damaged file means no rules");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn rules_read_from_json_with_defaults() {
        let rules: Vec<Rule> = serde_json::from_str(
            r#"[{"name":"slow","matcher":{"host":"*.test"},"action":{"type":"delay","ms":500}},
                {"name":"down","action":{"type":"block"}},
                {"name":"stub","action":{"type":"mock","status":404,"body":"gone"}}]"#,
        )
        .unwrap();
        assert!(rules.iter().all(|r| r.enabled));
        assert_eq!(rules[1].matcher, Matcher::default());
        assert_eq!(mock_bytes("aGk=", true).unwrap(), b"hi");
    }
}
