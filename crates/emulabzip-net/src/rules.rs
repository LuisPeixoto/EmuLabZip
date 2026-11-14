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
/// Largest file a Map local rule serves.
pub const MAX_MAPPED_FILE: u64 = 50 * 1024 * 1024;

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
    /// After this rule, keep looking at the rules below it. Only a rule that changes the request (a delay, a map to
    /// another server, a check) can continue; one that answers, blocks or holds the request always ends the search.
    #[serde(default)]
    pub then_continue: bool,
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
    /// Answer with a file on this computer, or with the file of a folder that the request path names. The server is
    /// not asked. The part of the path before the rule's own pattern (`/assets/*`) is left off, so `/assets/app.js`
    /// is `app.js` in the folder.
    #[serde(rename_all = "camelCase")]
    MapLocal {
        /// A file or a folder.
        path: String,
        /// The type to say. Left out, it comes from the file's extension.
        #[serde(default)]
        content_type: Option<String>,
        /// The status to answer with (200 when left out).
        #[serde(default)]
        status: Option<u16>,
    },
    /// Check the response against an OpenAPI description (a file on this computer, JSON or YAML). The answer is not
    /// changed: what is wrong shows on the flow. Add `thenContinue` to let the rules below apply too.
    Validate { spec: String },
    /// Send the request to another server, as if the app had asked for it. What is left out stays as it was.
    #[serde(rename_all = "camelCase")]
    MapRemote {
        #[serde(default)]
        scheme: Option<String>,
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        port: Option<u16>,
        /// Replaces the part of the path that the rule's own pattern names before its first `*`: with the pattern
        /// `/api/*` and `/v2/`, `/api/users` becomes `/v2/users`.
        #[serde(default)]
        path_prefix: Option<String>,
        /// Keep saying the original host to the new server. Leave it off to say the new one.
        #[serde(default)]
        preserve_host: bool,
    },
}

impl Action {
    /// An action that answers, blocks or holds the request ends the search for rules, whatever `then_continue` says.
    pub fn is_final(&self) -> bool {
        matches!(self, Action::Mock { .. } | Action::MapLocal { .. } | Action::Block | Action::Breakpoint { .. })
    }
}

/// The part of a pattern before its first `*` or `?`.
pub fn literal_prefix(pattern: &str) -> &str {
    pattern.find(['*', '?']).map_or(pattern, |i| &pattern[..i])
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The file a Map local rule serves for a request. A folder gives the file the request path names under it, and never
/// anything outside it: `..`, a drive, a symbolic link that leads out, all fail. A request for a folder gets its
/// `index.html`.
pub fn resolve_local(root: &Path, matcher_path: Option<&str>, request_path: &str) -> Result<PathBuf> {
    let root_meta =
        std::fs::metadata(root).map_err(|_| Error::failed(format!("{} does not exist.", root.display())))?;
    if root_meta.is_file() {
        return Ok(root.to_path_buf());
    }
    let only = request_path.split('?').next().unwrap_or(request_path);
    let rest = matcher_path.map(literal_prefix).and_then(|p| only.strip_prefix(p)).unwrap_or(only);
    let mut target = root.to_path_buf();
    for segment in rest.split('/') {
        let segment = percent_decode(segment).ok_or_else(|| Error::invalid("path", request_path))?;
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains(['/', '\\', '\0', ':']) {
            return Err(Error::invalid("path", request_path));
        }
        target.push(segment);
    }
    if target.is_dir() {
        target.push("index.html");
    }
    let real = std::fs::canonicalize(&target).map_err(|_| Error::failed("There is no such file in the folder."))?;
    let base = std::fs::canonicalize(root)?;
    if !real.starts_with(&base) {
        return Err(Error::invalid("path", request_path));
    }
    Ok(real)
}

/// A content type for a file name, for the common web files.
pub fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("xml") => "application/xml",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("wasm") => "application/wasm",
        Some("pdf") => "application/pdf",
        Some("mp4") => "video/mp4",
        Some("mp3") => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

/// Where a Map remote rule sends a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    /// Path and query.
    pub path: String,
}

/// Applies a Map remote rule to the address the app asked for.
pub fn map_remote(
    rule_scheme: Option<&str>,
    rule_host: Option<&str>,
    rule_port: Option<u16>,
    path_prefix: Option<&str>,
    matcher_path: Option<&str>,
    from: &Mapped,
) -> Mapped {
    let scheme = rule_scheme.map_or_else(|| from.scheme.clone(), str::to_string);
    let host = rule_host.map_or_else(|| from.host.clone(), str::to_string);
    // A new scheme without a port takes the scheme's usual one; the port the app used belonged to the old scheme.
    let default_port = |s: &str| if s == "https" { 443 } else { 80 };
    let port = rule_port.unwrap_or(
        if from.port == default_port(&from.scheme) && (rule_scheme.is_some() || rule_host.is_some()) {
            default_port(&scheme)
        } else {
            from.port
        },
    );
    let path = match path_prefix {
        Some(prefix) => {
            let literal = matcher_path.map(literal_prefix).unwrap_or("");
            let rest = from.path.strip_prefix(literal).unwrap_or(&from.path);
            let rest = rest.strip_prefix('/').filter(|_| prefix.ends_with('/')).unwrap_or(rest);
            format!("{prefix}{rest}")
        }
        None => from.path.clone(),
    };
    Mapped { scheme, host, port, path }
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
            Action::MapLocal { path, status, .. } => {
                if path.trim().is_empty() || path.contains('\0') || !Path::new(path).is_absolute() {
                    return Err(Error::invalid("local path", "give the full path of a file or folder"));
                }
                match std::fs::metadata(path) {
                    Err(_) => return Err(Error::invalid("local path", format!("{path} does not exist"))),
                    Ok(m) if m.is_file() && m.len() > MAX_MAPPED_FILE => {
                        return Err(Error::invalid(
                            "local file",
                            format!("{} bytes (at most {MAX_MAPPED_FILE})", m.len()),
                        ));
                    }
                    Ok(_) => {}
                }
                if status.is_some_and(|s| !(100..=599).contains(&s)) {
                    return Err(Error::invalid("status", status.unwrap_or(0).to_string()));
                }
            }
            Action::MapRemote { scheme, host, port, path_prefix, .. } => {
                if scheme.is_none() && host.is_none() && port.is_none() && path_prefix.is_none() {
                    return Err(Error::invalid("map to another server", "give a scheme, host, port or path"));
                }
                if scheme.as_deref().is_some_and(|s| s != "http" && s != "https") {
                    return Err(Error::invalid("scheme", scheme.clone().unwrap_or_default()));
                }
                if let Some(h) = host
                    && (h.is_empty()
                        || h.len() > 253
                        || !h.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                {
                    return Err(Error::invalid("host", h.clone()));
                }
                if *port == Some(0) {
                    return Err(Error::invalid("port", "0"));
                }
                if let Some(p) = path_prefix
                    && (!p.starts_with('/')
                        || p.len() > 500
                        || p.chars().any(|c| c.is_control() || c.is_whitespace() || c == '?' || c == '#'))
                {
                    return Err(Error::invalid("path", p.clone()));
                }
            }
            Action::Validate { spec } => {
                if !Path::new(spec).is_absolute() {
                    return Err(Error::invalid("OpenAPI file", "give the full path of the file"));
                }
                // The file is read now, so a spec that cannot be used is said when the rule is saved.
                crate::openapi::Spec::load(Path::new(spec))?;
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

    /// The rules that apply to a request, in order: the first match, and, while a rule says `then_continue` and does
    /// not end the search by what it does, the ones after it.
    pub fn matching(&self, method: &str, host: &str, path: &str) -> Vec<Rule> {
        let mut chain = Vec::new();
        for rule in self.rules.read().iter().filter(|r| r.enabled && r.matcher.matches(method, host, path)) {
            chain.push(rule.clone());
            if rule.action.is_final() || !rule.then_continue {
                break;
            }
        }
        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, matcher: Matcher, action: Action) -> Rule {
        Rule { id: 0, name: name.into(), enabled: true, matcher, action, then_continue: false }
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

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-maplocal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("app.js"), "x").unwrap();
        std::fs::write(dir.join("index.html"), "i").unwrap();
        std::fs::write(dir.join("sub/index.html"), "s").unwrap();
        dir
    }

    #[test]
    fn a_folder_serves_what_the_path_names_and_never_what_is_outside_it() {
        let dir = scratch("serve");
        let at = |path: &str| {
            resolve_local(&dir, Some("/assets/*"), path)
                .map(|p| p.strip_prefix(std::fs::canonicalize(&dir).unwrap()).unwrap().to_path_buf())
        };
        assert_eq!(at("/assets/app.js?v=3").unwrap(), PathBuf::from("app.js"));
        assert_eq!(at("/assets/").unwrap(), PathBuf::from("index.html"), "a folder gives its index");
        assert_eq!(at("/assets/sub").unwrap(), PathBuf::from("sub/index.html"));
        assert!(at("/assets/missing.js").is_err());
        for bad in [
            "/assets/../x",
            "/assets/%2e%2e/x",
            "/assets/a%2Fb",
            "/assets/..%5Cx",
            "/assets/C:/x",
            "/assets/%zz",
            "/assets/a%00b",
        ] {
            assert!(at(bad).is_err(), "{bad}");
        }
        // A symbolic link that leads out of the folder is not followed.
        #[cfg(unix)]
        {
            let outside = scratch("outside");
            std::os::unix::fs::symlink(outside.join("app.js"), dir.join("link.js")).unwrap();
            assert!(at("/assets/link.js").is_err());
            std::fs::remove_dir_all(outside).ok();
        }
        // A file is served whatever the path.
        assert_eq!(resolve_local(&dir.join("app.js"), None, "/anything").unwrap(), dir.join("app.js"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn content_types_come_from_the_extension() {
        assert_eq!(mime_for(Path::new("a/b.JSON")), "application/json");
        assert_eq!(mime_for(Path::new("x.png")), "image/png");
        assert_eq!(mime_for(Path::new("noext")), "application/octet-stream");
    }

    fn from(scheme: &str, host: &str, port: u16, path: &str) -> Mapped {
        Mapped { scheme: scheme.into(), host: host.into(), port, path: path.into() }
    }

    #[test]
    fn a_map_remote_rule_changes_only_what_it_names() {
        let app = from("https", "api.prod.com", 443, "/api/users?page=2");
        let to = |s, h, p, prefix, pattern| map_remote(s, h, p, prefix, pattern, &app);
        // The same path on a local server over plain http.
        let local = to(Some("http"), Some("localhost"), Some(3000), None, None);
        assert_eq!(local, from("http", "localhost", 3000, "/api/users?page=2"));
        // Another host keeps the scheme, and takes its usual port unless one is given.
        assert_eq!(
            to(None, Some("staging.prod.com"), None, None, None),
            from("https", "staging.prod.com", 443, "/api/users?page=2")
        );
        // The part of the path the pattern names is replaced.
        assert_eq!(
            to(None, None, None, Some("/v2/"), Some("/api/*")),
            from("https", "api.prod.com", 443, "/v2/users?page=2")
        );
        assert_eq!(
            to(None, None, None, Some("/v2"), Some("/api")),
            from("https", "api.prod.com", 443, "/v2/users?page=2")
        );
        // A port the app used on a custom port stays unless the rule says another.
        let custom = from("http", "dev.test", 8080, "/x");
        assert_eq!(map_remote(None, Some("other.test"), None, None, None, &custom).port, 8080);
        assert_eq!(map_remote(Some("https"), None, None, None, None, &custom).port, 8080);
    }

    #[test]
    fn rules_that_only_change_the_request_can_let_the_next_one_apply() {
        let set = RuleSet::new();
        let any = Matcher::default();
        let mut slow = rule("slow", any.clone(), Action::Delay { ms: 10 });
        slow.then_continue = true;
        let mut mapped = rule(
            "mapped",
            any.clone(),
            Action::MapRemote {
                scheme: None,
                host: Some("x.test".into()),
                port: None,
                path_prefix: None,
                preserve_host: false,
            },
        );
        mapped.then_continue = true;
        let mut mock_after = rule("answer", any.clone(), mock(200));
        mock_after.then_continue = true; // an answer ends the search whatever it says
        set.set(vec![slow, mapped, mock_after, rule("never", any, mock(500))]).unwrap();
        let names: Vec<String> = set.matching("GET", "h", "/").into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["slow", "mapped", "answer"]);
        // Without then_continue only the first applies.
        let one = RuleSet::new();
        one.set(vec![rule("a", Matcher::default(), Action::Delay { ms: 1 }), rule("b", Matcher::default(), mock(200))])
            .unwrap();
        assert_eq!(one.matching("GET", "h", "/").len(), 1);
        assert!(one.matching("GET", "h", "/").first().is_some_and(|r| r.name == "a"));
    }

    #[test]
    fn map_rules_that_cannot_work_are_refused() {
        let dir = scratch("check");
        let set = RuleSet::new();
        let bad = |action: Action| set.set(vec![rule("r", Matcher::default(), action)]).is_err();
        let local = |path: &str, status| Action::MapLocal { path: path.into(), content_type: None, status };
        assert!(!bad(local(&dir.display().to_string(), Some(404))));
        assert!(bad(local("relative/path", None)));
        assert!(bad(local("/no/such/folder/at/all", None)));
        assert!(bad(local(&dir.display().to_string(), Some(99))));
        assert!(bad(local("", None)));
        let remote = |scheme: Option<&str>, host: Option<&str>, port, prefix: Option<&str>| Action::MapRemote {
            scheme: scheme.map(str::to_string),
            host: host.map(str::to_string),
            port,
            path_prefix: prefix.map(str::to_string),
            preserve_host: false,
        };
        assert!(!bad(remote(None, Some("localhost"), Some(3000), None)));
        assert!(bad(remote(None, None, None, None)), "it must change something");
        assert!(bad(remote(Some("ftp"), None, None, None)));
        assert!(bad(remote(None, Some("a b"), None, None)));
        assert!(bad(remote(None, Some("a/b"), None, None)));
        assert!(bad(remote(None, None, Some(0), None)));
        assert!(bad(remote(None, None, None, Some("no-slash"))));
        assert!(bad(remote(None, None, None, Some("/a?b=1"))));
        std::fs::remove_dir_all(dir).ok();
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
