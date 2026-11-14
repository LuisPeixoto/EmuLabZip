//! Deep links and the files that tie an app to a website.
//!
//! A web link opens an app (instead of the browser) when the domain says so: Android reads
//! `/.well-known/assetlinks.json`, iOS reads `/.well-known/apple-app-site-association`, and the app has to claim the
//! domain too (an `autoVerify` intent filter, an Associated Domains entitlement). This module reads those files and the
//! device's own report of what it decided, and turns them into findings that say what is wrong and how to fix it.
//! Everything here is pure: fetching and asking the device is done by the callers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Platform, Result};

// ---------------------------------------------------------------------------------------------
// The link
// ---------------------------------------------------------------------------------------------

/// A link taken apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub scheme: String,
    pub host: Option<String>,
    /// The path, without the query: `/demo/cart`.
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl Link {
    pub fn parse(url: &str) -> Option<Link> {
        let (scheme, rest) = url.split_once(':')?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme.is_empty() {
            return None;
        }
        let (rest, fragment) = match rest.split_once('#') {
            Some((r, f)) => (r, Some(f.to_string())),
            None => (rest, None),
        };
        let (rest, query) = match rest.split_once('?') {
            Some((r, q)) => (r, Some(q.to_string())),
            None => (rest, None),
        };
        let (host, path) = match rest.strip_prefix("//") {
            Some(after) => {
                let (authority, path) = after.find('/').map_or((after, ""), |i| (&after[..i], &after[i..]));
                // No credentials and no port in the host that a domain file is read from.
                let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
                let host = host
                    .rsplit_once(':')
                    .filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit()))
                    .map_or(host, |(h, _)| h);
                ((!host.is_empty()).then(|| host.to_ascii_lowercase()), path.to_string())
            }
            None => (None, rest.to_string()),
        };
        let path = if path.is_empty() && host.is_some() { "/".to_string() } else { path };
        Some(Link { scheme, host, path, query, fragment })
    }

    /// An `http` or `https` link, which a domain file can claim.
    pub fn is_web(&self) -> bool {
        matches!(self.scheme.as_str(), "http" | "https") && self.host.is_some()
    }
}

/// `*` matches any run of characters, `?` one character. Both Apple's path patterns and its query patterns use it.
pub fn glob(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None::<usize>, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) && p[pi] != '*' {
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
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

// ---------------------------------------------------------------------------------------------
// Android: assetlinks.json
// ---------------------------------------------------------------------------------------------

/// An app a domain lets handle its links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetStatement {
    pub package: String,
    /// SHA-256 fingerprints of the signing certificates, as `AA:BB:…`.
    pub fingerprints: Vec<String>,
}

/// The statements of `assetlinks.json` that grant an app the domain's links.
pub fn parse_assetlinks(text: &str) -> Result<Vec<AssetStatement>> {
    let value: Value = serde_json::from_str(text).map_err(|e| Error::failed(format!("It is not JSON: {e}")))?;
    let list = value.as_array().ok_or_else(|| Error::failed("It must be a JSON array of statements."))?;
    let mut out = Vec::new();
    for item in list {
        let relations = item.get("relation").and_then(Value::as_array);
        if !relations
            .is_some_and(|r| r.iter().any(|x| x.as_str() == Some("delegate_permission/common.handle_all_urls")))
        {
            continue;
        }
        let Some(target) = item.get("target") else { continue };
        if target.get("namespace").and_then(Value::as_str) != Some("android_app") {
            continue;
        }
        let Some(package) = target.get("package_name").and_then(Value::as_str) else { continue };
        let fingerprints = target
            .get("sha256_cert_fingerprints")
            .and_then(Value::as_array)
            .map(|f| f.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        out.push(AssetStatement { package: package.to_string(), fingerprints });
    }
    Ok(out)
}

/// A fingerprint without colons, in upper case, so two spellings compare equal.
pub fn normalize_fingerprint(f: &str) -> String {
    f.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_ascii_uppercase()
}

// ---------------------------------------------------------------------------------------------
// iOS: apple-app-site-association
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AasaRule {
    pub path: Option<String>,
    pub query: Option<Vec<(String, String)>>,
    pub fragment: Option<String>,
    pub exclude: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AasaDetail {
    /// `TEAMID.bundle.id`
    pub app_ids: Vec<String>,
    pub rules: Vec<AasaRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aasa {
    pub details: Vec<AasaDetail>,
}

impl Aasa {
    pub fn app_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.details.iter().flat_map(|d| d.app_ids.iter().map(String::as_str)).collect();
        ids.dedup();
        ids
    }

    /// Whether the app's links include this one. `None` when the app is not listed at all. The first rule that
    /// matches decides, and a link no rule matches is not handled.
    pub fn allows(&self, app_id: &str, link: &Link) -> Option<bool> {
        let mut listed = false;
        for detail in self.details.iter().filter(|d| d.app_ids.iter().any(|a| a == app_id)) {
            listed = true;
            for rule in &detail.rules {
                if rule.matches(link) {
                    return Some(!rule.exclude);
                }
            }
        }
        listed.then_some(false)
    }
}

impl AasaRule {
    fn matches(&self, link: &Link) -> bool {
        if let Some(p) = &self.path
            && !glob(p, &link.path)
        {
            return false;
        }
        if let Some(pairs) = &self.query {
            let query = link.query.as_deref().unwrap_or("");
            let whole = pairs.iter().find(|(k, _)| k.is_empty());
            if let Some((_, pattern)) = whole {
                if !glob(pattern, query) {
                    return false;
                }
            } else {
                for (key, pattern) in pairs {
                    let found = query.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| k == key);
                    if !found.is_some_and(|(_, v)| glob(pattern, v)) {
                        return false;
                    }
                }
            }
        }
        if let Some(f) = &self.fragment
            && !glob(f, link.fragment.as_deref().unwrap_or(""))
        {
            return false;
        }
        true
    }
}

/// Reads `apple-app-site-association`: the current `components` format and the older `paths` one.
pub fn parse_aasa(text: &str) -> Result<Aasa> {
    let value: Value = serde_json::from_str(text).map_err(|e| Error::failed(format!("It is not JSON: {e}")))?;
    let applinks = value
        .get("applinks")
        .ok_or_else(|| Error::failed("It has no `applinks` section, which is what universal links read."))?;
    let mut details = Vec::new();
    for detail in applinks.get("details").and_then(Value::as_array).into_iter().flatten() {
        let mut app_ids: Vec<String> = detail
            .get("appIDs")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        if let Some(one) = detail.get("appID").and_then(Value::as_str) {
            app_ids.push(one.to_string());
        }
        let mut rules = Vec::new();
        for c in detail.get("components").and_then(Value::as_array).into_iter().flatten() {
            let query = match c.get("?") {
                Some(Value::String(s)) => Some(vec![(String::new(), s.clone())]),
                Some(Value::Object(o)) => {
                    Some(o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect())
                }
                _ => None,
            };
            rules.push(AasaRule {
                path: c.get("/").and_then(Value::as_str).map(str::to_string),
                query,
                fragment: c.get("#").and_then(Value::as_str).map(str::to_string),
                exclude: c.get("exclude").and_then(Value::as_bool).unwrap_or(false),
            });
        }
        for path in detail.get("paths").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            let (exclude, pattern) = path.strip_prefix("NOT ").map_or((false, path), |p| (true, p));
            rules.push(AasaRule { path: Some(pattern.to_string()), query: None, fragment: None, exclude });
        }
        details.push(AasaDetail { app_ids, rules });
    }
    // The oldest form lists the apps directly under `apps`, with the paths beside them.
    if details.is_empty() {
        let apps: Vec<String> = applinks
            .get("apps")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        if !apps.is_empty() {
            details.push(AasaDetail {
                app_ids: apps,
                rules: vec![AasaRule { path: Some("*".into()), query: None, fragment: None, exclude: false }],
            });
        }
    }
    Ok(Aasa { details })
}

/// The team part of `TEAMID.bundle.id`, and the bundle id.
pub fn split_app_id(app_id: &str) -> Option<(&str, &str)> {
    let (team, bundle) = app_id.split_once('.')?;
    (!team.is_empty() && !bundle.is_empty()).then_some((team, bundle))
}

/// What an app's entitlements say about links, read from `codesign -d --entitlements :-`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entitlements {
    /// The `applinks:` entries of Associated Domains: `applinks:example.com`.
    pub associated_domains: Vec<String>,
    /// `TEAMID.bundle.id`.
    pub app_identifier: Option<String>,
}

/// Reads the two values that matter out of an entitlements property list.
pub fn parse_entitlements(plist: &str) -> Entitlements {
    fn after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
        let marker = format!("<key>{key}</key>");
        text.find(&marker).map(|i| &text[i + marker.len()..])
    }
    let strings = |block: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = block;
        while let Some(start) = rest.find("<string>") {
            let after = &rest[start + 8..];
            let Some(end) = after.find("</string>") else { break };
            out.push(after[..end].to_string());
            rest = &after[end + 9..];
        }
        out
    };
    let associated_domains = after(plist, "com.apple.developer.associated-domains")
        .and_then(|rest| {
            let rest = rest.trim_start();
            let end = rest.find("</array>")?;
            rest.starts_with("<array>").then(|| strings(&rest[..end]))
        })
        .unwrap_or_default();
    let app_identifier = after(plist, "application-identifier").and_then(|rest| {
        strings(rest.trim_start().split("</string>").next().map_or("", |s| s)).into_iter().next().or_else(|| {
            let rest = rest.trim_start().strip_prefix("<string>")?;
            Some(rest.split("</string>").next()?.to_string())
        })
    });
    Entitlements { associated_domains, app_identifier }
}

// ---------------------------------------------------------------------------------------------
// Android: what the device decided
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainState {
    pub domain: String,
    /// `verified`, `none`, `approved`, `denied`, `legacy_failure`, or a code of 1024 and up (a failure that says why).
    pub state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLinks {
    pub package: String,
    /// The SHA-256 fingerprints of the signing certificates the verifier uses.
    pub signatures: Vec<String>,
    /// The user's switch "Open supported links".
    pub link_handling_allowed: Option<bool>,
    pub domains: Vec<DomainState>,
}

impl AppLinks {
    pub fn state_of(&self, host: &str) -> Option<&str> {
        self.domains.iter().find(|d| d.domain.eq_ignore_ascii_case(host)).map(|d| d.state.as_str())
    }
}

/// Reads `pm get-app-links <package>` (Android 12 and later).
pub fn parse_get_app_links(text: &str) -> Option<AppLinks> {
    let mut links = AppLinks::default();
    let mut in_states = false;
    let mut seen = false;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // The package is a line of its own at the first level: "  dev.x.app:".
        let indent = line.len() - line.trim_start().len();
        if indent <= 2 && t.ends_with(':') && !t.contains(' ') {
            links.package = t.trim_end_matches(':').to_string();
            seen = true;
            continue;
        }
        if let Some(sigs) = t.strip_prefix("Signatures:") {
            links.signatures = sigs
                .trim()
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            in_states = false;
        } else if t.starts_with("Domain verification state") {
            in_states = true;
        } else if let Some(allowed) = t.strip_prefix("Verification link handling allowed:") {
            links.link_handling_allowed = Some(allowed.trim() == "true");
            in_states = false;
        } else if t.starts_with("User ") || t.starts_with("Selection state") {
            in_states = false;
        } else if in_states && let Some((domain, state)) = t.split_once(':') {
            let (domain, state) = (domain.trim(), state.trim());
            if !domain.is_empty() && !domain.contains(' ') && !state.is_empty() {
                links.domains.push(DomainState { domain: domain.to_string(), state: state.to_string() });
            }
        }
    }
    (seen && (!links.domains.is_empty() || !links.signatures.is_empty())).then_some(links)
}

/// Who opens a link on the device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    /// `dev.example.app/.MainActivity`
    pub component: String,
    pub package: String,
    /// Several apps match, so Android shows a chooser.
    pub chooser: bool,
}

/// Reads `cmd package resolve-activity --brief`.
pub fn parse_resolve_activity(text: &str) -> Option<Resolved> {
    let component =
        text.lines().map(str::trim).rev().find(|l| l.contains('/') && !l.contains(' ') && !l.contains('='))?;
    let package = component.split('/').next()?.to_string();
    let chooser = package == "android" || component.contains("ResolverActivity");
    Some(Resolved { component: component.to_string(), package, chooser })
}

// ---------------------------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------------------------

/// A file read from the domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub location: Option<String>,
    pub body: String,
    pub truncated: bool,
}

/// What was asked of the domain and the device.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    /// Android: who opens the link.
    pub resolved: Option<Resolved>,
    /// Android: `pm get-app-links` for the app.
    pub app_links: Option<AppLinks>,
    /// iOS: the app's entitlements.
    pub entitlements: Option<Entitlements>,
    /// Why something is missing.
    pub notes: Vec<String>,
}

/// The outcome of fetching a domain file: the file, or why not.
pub type Fetch = std::result::Result<RemoteFile, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Pass,
    Info,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub level: Level,
    pub title: String,
    pub detail: String,
    /// What to do about it.
    pub fix: Option<String>,
}

fn f(level: Level, title: &str, detail: impl Into<String>, fix: Option<&str>) -> Finding {
    Finding { level, title: title.to_string(), detail: detail.into(), fix: fix.map(str::to_string) }
}

/// The address of the file a platform reads from a domain.
pub fn domain_file_url(host: &str, platform: Platform) -> String {
    match platform {
        Platform::Android => format!("https://{host}/.well-known/assetlinks.json"),
        Platform::Ios => format!("https://{host}/.well-known/apple-app-site-association"),
    }
}

/// What a link check found, in order.
pub fn evaluate(
    link: &Link,
    platform: Platform,
    app: Option<&str>,
    file: Option<&Fetch>,
    facts: &Facts,
) -> Vec<Finding> {
    let mut out = Vec::new();
    if !link.is_web() {
        out.push(f(
            Level::Info,
            "A link of the app's own scheme",
            format!("`{}:` links need no domain file; any app can claim a scheme, so the one that opens is the one the system picks.", link.scheme),
            None,
        ));
        if platform == Platform::Android {
            resolution(&mut out, facts, app);
        }
        return out;
    }
    if link.scheme == "http" {
        out.push(f(Level::Warn, "The link is http", "App links and universal links are for https. Android can verify http only if the domain redirects, and iOS ignores it.", Some("Use an https link.")));
    }
    let host = link.host.as_deref().unwrap_or("");
    match platform {
        Platform::Android => android(&mut out, link, host, app, file, facts),
        Platform::Ios => ios(&mut out, link, host, app, file, facts),
    }
    out
}

/// Findings about the status, redirect and type of a domain file. Returns the body when it can be read.
fn file_checks<'a>(out: &mut Vec<Finding>, name: &str, file: Option<&'a Fetch>) -> Option<&'a RemoteFile> {
    let file = match file {
        Some(Ok(file)) => file,
        Some(Err(why)) => {
            out.push(f(
                Level::Fail,
                &format!("{name} could not be read"),
                why.clone(),
                Some("Check that the domain is reachable over https with a valid certificate."),
            ));
            return None;
        }
        None => return None,
    };
    if (300..400).contains(&file.status) {
        let to = file.location.as_deref().unwrap_or("somewhere else");
        out.push(f(Level::Fail, &format!("{name} redirects"), format!("{} answers {} and points to {to}. The system does not follow redirects for this file.", file.url, file.status), Some("Serve the file directly at that address, with no redirect (including http to https, or the bare domain to www).")));
        return None;
    }
    if file.status != 200 {
        out.push(f(
            Level::Fail,
            &format!("{name} is not there"),
            format!("{} answers {}.", file.url, file.status),
            Some("Publish the file at that address."),
        ));
        return None;
    }
    out.push(f(Level::Pass, &format!("{name} is served"), format!("{} answers 200.", file.url), None));
    let kind = file.content_type.as_deref().unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if kind != "application/json" && kind != "application/pkcs7-mime" {
        out.push(f(
            Level::Warn,
            &format!("{name} has an unusual content type"),
            format!(
                "It is served as `{}`; `application/json` is what both systems expect.",
                if kind.is_empty() { "no type" } else { &kind }
            ),
            Some("Serve the file with Content-Type: application/json."),
        ));
    }
    if file.truncated {
        out.push(f(Level::Warn, &format!("{name} is very large"), "Only the first part was read.", None));
    }
    Some(file)
}

fn resolution(out: &mut Vec<Finding>, facts: &Facts, app: Option<&str>) {
    match &facts.resolved {
        Some(r) if r.chooser => out.push(f(
            Level::Warn,
            "Several apps open this link",
            "Android shows a chooser, so the link does not go straight to one app.",
            Some("For a web link, make sure the domain verifies for your app only."),
        )),
        Some(r) => {
            let mine = app.is_none_or(|a| a == r.package);
            let level = if mine { Level::Pass } else { Level::Warn };
            let detail = if mine {
                format!("{} opens it.", r.component)
            } else {
                format!("{} opens it, not {}.", r.component, app.unwrap_or(""))
            };
            out.push(f(level, "Who opens the link", detail, None));
        }
        None if facts.notes.is_empty() => out.push(f(
            Level::Fail,
            "No app opens this link",
            "No app on the device has an intent filter for it, so it opens a browser or nothing.",
            Some("Add an intent filter for the link (scheme, host and path) to the activity."),
        )),
        None => {}
    }
}

fn android(out: &mut Vec<Finding>, link: &Link, host: &str, app: Option<&str>, file: Option<&Fetch>, facts: &Facts) {
    if let Some(remote) = file_checks(out, "assetlinks.json", file) {
        match parse_assetlinks(&remote.body) {
            Err(e) => out.push(f(
                Level::Fail,
                "assetlinks.json is not valid",
                e.to_string(),
                Some("It must be a JSON array of statements."),
            )),
            Ok(statements) if statements.is_empty() => out.push(f(
                Level::Fail,
                "assetlinks.json grants no app",
                "No statement has the relation `delegate_permission/common.handle_all_urls` for an `android_app`.",
                Some("Add a statement for your package and the SHA-256 of its signing certificate."),
            )),
            Ok(statements) => {
                out.push(f(
                    Level::Pass,
                    "assetlinks.json is valid",
                    format!(
                        "It grants {}.",
                        statements.iter().map(|s| s.package.as_str()).collect::<Vec<_>>().join(", ")
                    ),
                    None,
                ));
                if let Some(app) = app {
                    match statements.iter().find(|s| s.package == app) {
                        None => out.push(f(
                            Level::Fail,
                            "The app is not in assetlinks.json",
                            format!(
                                "It lists {}, not {app}.",
                                statements.iter().map(|s| s.package.as_str()).collect::<Vec<_>>().join(", ")
                            ),
                            Some("Add a statement with this package name."),
                        )),
                        Some(statement) => {
                            let signed: Vec<String> = facts
                                .app_links
                                .iter()
                                .flat_map(|a| a.signatures.iter().map(|s| normalize_fingerprint(s)))
                                .collect();
                            let listed: Vec<String> =
                                statement.fingerprints.iter().map(|s| normalize_fingerprint(s)).collect();
                            if signed.is_empty() {
                                out.push(f(
                                    Level::Info,
                                    "The signing certificate could not be read",
                                    "`pm get-app-links` did not report the app's fingerprints (Android 12 and later).",
                                    None,
                                ));
                            } else if signed.iter().any(|s| listed.contains(s)) {
                                out.push(f(
                                    Level::Pass,
                                    "The fingerprint matches",
                                    "The app's signing certificate is the one assetlinks.json lists.",
                                    None,
                                ));
                            } else {
                                out.push(f(Level::Fail, "The fingerprint does not match", format!("The app is signed with {}; the file lists {}.", facts.app_links.as_ref().map(|a| a.signatures.join(", ")).unwrap_or_default(), statement.fingerprints.join(", ")), Some("Put the SHA-256 of the key that signed this build in sha256_cert_fingerprints. A debug build is signed with the debug key, not the release key.")));
                            }
                        }
                    }
                }
            }
        }
    }
    if let (Some(app), Some(links)) = (app, &facts.app_links) {
        match links.state_of(host) {
            Some("verified") | Some("1") => out.push(f(Level::Pass, "The device verified the domain", format!("{host} is verified for {app}."), None)),
            Some(state) => out.push(f(Level::Warn, "The device has not verified the domain", format!("{host} is `{state}` for {app}."), Some(&format!("Ask the device to try again: adb shell pm verify-app-links --re-verify {app}")))),
            None => out.push(f(Level::Fail, "The app does not claim this domain", format!("{app} has no autoVerify intent filter for {host}."), Some(&format!("Add <intent-filter android:autoVerify=\"true\"> with <data android:scheme=\"https\" android:host=\"{host}\"/> to the activity.")))),
        }
        if links.link_handling_allowed == Some(false) {
            out.push(f(
                Level::Warn,
                "Opening supported links is off for the app",
                "The user turned off \"Open supported links\", so web links go to the browser.",
                Some(&format!("adb shell pm set-app-links-user-selection --user 0 --package {app} true {host}")),
            ));
        }
    }
    resolution(out, facts, app);
    if link.path.is_empty() {
        out.push(f(Level::Info, "The link has no path", "Some intent filters only match links with a path.", None));
    }
}

fn ios(out: &mut Vec<Finding>, link: &Link, host: &str, app: Option<&str>, file: Option<&Fetch>, facts: &Facts) {
    let mut team_of_file: Option<String> = None;
    if let Some(remote) = file_checks(out, "apple-app-site-association", file) {
        match parse_aasa(&remote.body) {
            Err(e) => out.push(f(
                Level::Fail,
                "apple-app-site-association is not valid",
                e.to_string(),
                Some("It needs an `applinks` section with `details`."),
            )),
            Ok(aasa) if aasa.details.is_empty() => out.push(f(
                Level::Fail,
                "apple-app-site-association lists no app",
                "`applinks.details` is empty.",
                Some("Add a detail with `appIDs` (TEAMID.bundle.id) and `components`."),
            )),
            Ok(aasa) => {
                out.push(f(
                    Level::Pass,
                    "apple-app-site-association is valid",
                    format!("It lists {}.", aasa.app_ids().join(", ")),
                    None,
                ));
                if let Some(app) = app {
                    match aasa.app_ids().into_iter().find(|id| split_app_id(id).is_some_and(|(_, b)| b == app)) {
                        None => out.push(f(
                            Level::Fail,
                            "The app is not in apple-app-site-association",
                            format!("It lists {}, none for the bundle id {app}.", aasa.app_ids().join(", ")),
                            Some("Add TEAMID.<bundle id> to `appIDs`."),
                        )),
                        Some(id) => {
                            team_of_file = split_app_id(id).map(|(t, _)| t.to_string());
                            match aasa.allows(id, link) {
                                Some(true) => out.push(f(Level::Pass, "The link is handled by the app", format!("A rule of {id} matches {}.", link.path), None)),
                                Some(false) => out.push(f(Level::Fail, "The link is not handled by the app", format!("No rule of {id} lets {} through, or an exclude rule catches it.", link.path), Some("Add a component that matches the path, and check the order: the first match decides."))),
                                None => {}
                            }
                        }
                    }
                }
            }
        }
    }
    match &facts.entitlements {
        Some(e) => {
            let wanted = format!("applinks:{host}");
            let covered = e.associated_domains.iter().any(|d| {
                d.eq_ignore_ascii_case(&wanted)
                    || d.eq_ignore_ascii_case(&format!("{wanted}?mode=developer"))
                    || (d.starts_with("applinks:*.") && host.ends_with(&d["applinks:*".len()..]))
            });
            if covered {
                out.push(f(
                    Level::Pass,
                    "The app claims the domain",
                    format!("Associated Domains has {wanted}."),
                    None,
                ));
            } else {
                out.push(f(
                    Level::Fail,
                    "The app does not claim the domain",
                    format!(
                        "Associated Domains has {}.",
                        if e.associated_domains.is_empty() {
                            "no applinks entries".to_string()
                        } else {
                            e.associated_domains.join(", ")
                        }
                    ),
                    Some(&format!("Add {wanted} to the Associated Domains capability.")),
                ));
            }
            if let (Some(file_team), Some(id)) = (&team_of_file, &e.app_identifier)
                && let Some((team, _)) = split_app_id(id)
            {
                if team == file_team {
                    out.push(f(Level::Pass, "The team id matches", format!("Both say {team}."), None));
                } else {
                    out.push(f(
                        Level::Fail,
                        "The team id does not match",
                        format!("The app is signed by {team}; the file says {file_team}."),
                        Some("Use the app's own Team ID in `appIDs`."),
                    ));
                }
            }
        }
        None => {
            for n in &facts.notes {
                out.push(f(Level::Info, "The app's entitlements were not read", n.clone(), None));
            }
        }
    }
    out.push(f(Level::Info, "The simulator does not verify domains the way a device does", "Associated Domains are read at install time from Apple's CDN on a device; on the simulator a link opens the app only when its entitlement is present and the file is reachable.", None));
}

// ---------------------------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------------------------

/// A link that was opened or checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub url: String,
    /// Milliseconds since the Unix epoch.
    pub at_ms: u64,
    pub app: Option<String>,
    pub pinned: bool,
    /// The worst finding of its last check.
    pub result: Option<Level>,
}

pub const MAX_HISTORY: usize = 100;

/// Puts a link at the top of the history, keeping its pin, and drops the oldest ones that are not pinned.
pub fn remember(history: &mut Vec<HistoryEntry>, url: &str, app: Option<&str>, at_ms: u64, result: Option<Level>) {
    let previous = history.iter().position(|h| h.url == url).map(|i| history.remove(i));
    history.insert(
        0,
        HistoryEntry {
            url: url.to_string(),
            at_ms,
            app: app.map(str::to_string).or_else(|| previous.as_ref().and_then(|p| p.app.clone())),
            pinned: previous.as_ref().is_some_and(|p| p.pinned),
            result: result.or_else(|| previous.as_ref().and_then(|p| p.result)),
        },
    );
    while history.len() > MAX_HISTORY {
        match history.iter().rposition(|h| !h.pinned) {
            Some(i) => {
                history.remove(i);
            }
            None => break,
        }
    }
}

/// Where the history is kept, shared by the app and the command line.
pub fn history_path() -> std::path::PathBuf {
    crate::data_dir().join("links.json")
}

/// The saved history, newest first. A missing or damaged file is an empty history.
pub fn load_history() -> Vec<HistoryEntry> {
    std::fs::read_to_string(history_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_history(history: &[HistoryEntry]) -> Result<()> {
    let path = history_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_string_pretty(history).map_err(|e| Error::failed(e.to_string()))?)?;
    std::fs::rename(&temp, &path)?;
    Ok(())
}

/// Remembers a link in the saved history.
pub fn note_link(url: &str, app: Option<&str>, result: Option<Level>) -> Result<()> {
    let mut history = load_history();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    remember(&mut history, url, app, now, result);
    save_history(&history)
}

/// Pins or unpins a link.
pub fn pin_link(url: &str, pinned: bool) -> Result<()> {
    let mut history = load_history();
    if let Some(entry) = history.iter_mut().find(|h| h.url == url) {
        entry.pinned = pinned;
    }
    save_history(&history)
}

/// Forgets a link.
pub fn forget_link(url: &str) -> Result<()> {
    let mut history = load_history();
    history.retain(|h| h.url != url);
    save_history(&history)
}

/// The worst level among findings.
pub fn worst(findings: &[Finding]) -> Level {
    findings
        .iter()
        .map(|x| x.level)
        .max_by_key(|l| match l {
            Level::Pass => 0,
            Level::Info => 1,
            Level::Warn => 2,
            Level::Fail => 3,
        })
        .unwrap_or(Level::Pass)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(url: &str) -> Link {
        Link::parse(url).unwrap()
    }

    #[test]
    fn a_link_is_taken_apart() {
        let l = link("https://User@Shop.Example.com:8443/demo/cart?id=3&x=y#top");
        assert_eq!(l.scheme, "https");
        assert_eq!(l.host.as_deref(), Some("shop.example.com"));
        assert_eq!(
            (l.path.as_str(), l.query.as_deref(), l.fragment.as_deref()),
            ("/demo/cart", Some("id=3&x=y"), Some("top"))
        );
        assert!(l.is_web());
        let own = link("emulabzip-demo://open/cart");
        assert_eq!((own.host.as_deref(), own.path.as_str()), (Some("open"), "/cart"));
        assert!(!own.is_web() || own.scheme == "http");
        assert_eq!(link("https://example.com").path, "/");
        assert!(Link::parse("no scheme").is_none());
        assert!(Link::parse(":x").is_none());
    }

    #[test]
    fn globs_have_a_star_and_a_question_mark() {
        assert!(glob("/demo/*", "/demo/cart/1"));
        assert!(glob("/d?mo", "/demo"));
        assert!(glob("*", ""));
        assert!(glob("/a/*/c", "/a/b/c"));
        assert!(!glob("/demo", "/demo/"));
        assert!(!glob("/d?mo", "/dmo"));
        assert!(!glob("/a/*/c", "/a/b/d"));
    }

    const ASSETLINKS: &str = r#"[
      { "relation": ["delegate_permission/common.handle_all_urls"],
        "target": { "namespace": "android_app", "package_name": "dev.x.app",
                    "sha256_cert_fingerprints": ["2A:6F:BF:F7"] } },
      { "relation": ["delegate_permission/common.get_login_creds"],
        "target": { "namespace": "android_app", "package_name": "dev.x.other", "sha256_cert_fingerprints": ["00"] } },
      { "relation": ["delegate_permission/common.handle_all_urls"], "target": { "namespace": "web", "site": "https://x" } }
    ]"#;

    #[test]
    fn assetlinks_grants_are_read() {
        let s = parse_assetlinks(ASSETLINKS).unwrap();
        assert_eq!(s, [AssetStatement { package: "dev.x.app".into(), fingerprints: vec!["2A:6F:BF:F7".into()] }]);
        assert!(parse_assetlinks("{}").is_err());
        assert!(parse_assetlinks("nope").is_err());
        assert_eq!(normalize_fingerprint("2a:6f:Bf"), "2A6FBF");
    }

    const AASA: &str = r#"{ "applinks": { "details": [
      { "appIDs": ["ABCDE12345.dev.x.app"],
        "components": [
          { "/": "/demo/private/*", "exclude": true },
          { "/": "/demo/*", "?": { "ref": "*" } },
          { "/": "/shop/?*" } ] },
      { "appID": "ZZZZZ99999.dev.x.old", "paths": ["NOT /x/*", "/x*"] } ] } }"#;

    #[test]
    fn aasa_components_and_paths_decide_in_order() {
        let a = parse_aasa(AASA).unwrap();
        assert_eq!(a.app_ids(), ["ABCDE12345.dev.x.app", "ZZZZZ99999.dev.x.old"]);
        let id = "ABCDE12345.dev.x.app";
        assert_eq!(a.allows(id, &link("https://h/demo/cart?ref=a")), Some(true));
        assert_eq!(a.allows(id, &link("https://h/demo/cart")), Some(false), "the query rule needs ref");
        assert_eq!(a.allows(id, &link("https://h/demo/private/x?ref=a")), Some(false), "the exclude comes first");
        assert_eq!(a.allows(id, &link("https://h/shop/42")), Some(true));
        assert_eq!(a.allows(id, &link("https://h/other")), Some(false));
        assert_eq!(a.allows("NOBODY.x", &link("https://h/x")), None);
        let old = "ZZZZZ99999.dev.x.old";
        assert_eq!(a.allows(old, &link("https://h/x/1")), Some(false));
        assert_eq!(a.allows(old, &link("https://h/xy")), Some(true));
        assert!(parse_aasa("{}").is_err());
        assert!(parse_aasa("[").is_err());
        assert_eq!(split_app_id("ABCDE12345.dev.x.app"), Some(("ABCDE12345", "dev.x.app")));
        assert_eq!(split_app_id("nodot"), None);
    }

    #[test]
    fn entitlements_give_the_domains_and_the_team() {
        let plist = r#"<?xml version="1.0"?><plist version="1.0"><dict>
          <key>application-identifier</key><string>ABCDE12345.dev.x.app</string>
          <key>com.apple.developer.associated-domains</key>
          <array><string>applinks:shop.example.com</string><string>webcredentials:shop.example.com</string></array>
        </dict></plist>"#;
        let e = parse_entitlements(plist);
        assert_eq!(e.app_identifier.as_deref(), Some("ABCDE12345.dev.x.app"));
        assert_eq!(e.associated_domains, ["applinks:shop.example.com", "webcredentials:shop.example.com"]);
        assert_eq!(parse_entitlements("<dict></dict>"), Entitlements::default());
    }

    const APP_LINKS: &str = "  dev.x.app:
    ID: 01234567-89ab-cdef-0123-456789abcdef
    Signatures: [2A:6F:BF:F7:1F:5B]
    Domain verification state:
      shop.example.com: verified
      www.example.com: 1024
      other.example.com: none
    User 0:
      Verification link handling allowed: true
      Selection state:
        Enabled:
          shop.example.com
";

    #[test]
    fn the_devices_decision_is_read() {
        let l = parse_get_app_links(APP_LINKS).unwrap();
        assert_eq!(l.package, "dev.x.app");
        assert_eq!(l.signatures, ["2A:6F:BF:F7:1F:5B"]);
        assert_eq!(l.link_handling_allowed, Some(true));
        assert_eq!(l.state_of("SHOP.example.com"), Some("verified"));
        assert_eq!(l.state_of("www.example.com"), Some("1024"));
        assert_eq!(l.domains.len(), 3, "{:?}", l.domains);
        assert!(parse_get_app_links("Error: no package").is_none());
        let r = parse_resolve_activity(
            "priority=0 preferredOrder=0 match=0x600000 specificIndex=-1 isDefault=true\ndev.x.app/.WebViewActivity\n",
        )
        .unwrap();
        assert_eq!((r.package.as_str(), r.chooser), ("dev.x.app", false));
        let chooser =
            parse_resolve_activity("priority=0 isDefault=true\nandroid/com.android.internal.app.ResolverActivity\n")
                .unwrap();
        assert!(chooser.chooser);
        assert!(parse_resolve_activity("No activity found").is_none());
    }

    fn served(url: &str, body: &str) -> Fetch {
        Ok(RemoteFile {
            url: url.into(),
            status: 200,
            content_type: Some("application/json".into()),
            location: None,
            body: body.into(),
            truncated: false,
        })
    }

    fn levels(findings: &[Finding]) -> Vec<(Level, &str)> {
        findings.iter().map(|x| (x.level, x.title.as_str())).collect()
    }

    #[test]
    fn a_good_android_setup_passes_and_a_wrong_key_fails_with_the_fix() {
        let url = "https://shop.example.com/demo/cart";
        let file = served("https://shop.example.com/.well-known/assetlinks.json", ASSETLINKS);
        let facts = Facts {
            app_links: parse_get_app_links(APP_LINKS),
            resolved: Some(Resolved { component: "dev.x.app/.W".into(), package: "dev.x.app".into(), chooser: false }),
            ..Facts::default()
        };
        // The fixture's file lists only the start of the fingerprint, so make it match the device.
        let good = served(
            "https://shop.example.com/.well-known/assetlinks.json",
            &ASSETLINKS.replace("2A:6F:BF:F7", "2A:6F:BF:F7:1F:5B"),
        );
        let ok = evaluate(&link(url), Platform::Android, Some("dev.x.app"), Some(&good), &facts);
        assert_eq!(worst(&ok), Level::Pass, "{:#?}", levels(&ok));
        let bad = evaluate(&link(url), Platform::Android, Some("dev.x.app"), Some(&file), &facts);
        let wrong = bad.iter().find(|x| x.title == "The fingerprint does not match").expect("a mismatch is found");
        assert_eq!(wrong.level, Level::Fail);
        assert!(wrong.fix.as_deref().unwrap().contains("sha256_cert_fingerprints"));
        assert_eq!(worst(&bad), Level::Fail);
    }

    #[test]
    fn android_problems_with_the_file_the_package_and_the_device_are_said() {
        let url = link("https://shop.example.com/x");
        let redirect: Fetch = Ok(RemoteFile {
            url: "https://shop.example.com/.well-known/assetlinks.json".into(),
            status: 301,
            content_type: None,
            location: Some("https://www.shop.example.com/.well-known/assetlinks.json".into()),
            body: String::new(),
            truncated: false,
        });
        let r = evaluate(&url, Platform::Android, Some("dev.x.app"), Some(&redirect), &Facts::default());
        assert!(r.iter().any(|x| x.title == "assetlinks.json redirects" && x.level == Level::Fail));
        let missing: Fetch = Ok(RemoteFile { status: 404, ..redirect.clone().unwrap() });
        assert!(
            evaluate(&url, Platform::Android, None, Some(&missing), &Facts::default())
                .iter()
                .any(|x| x.title == "assetlinks.json is not there")
        );
        let down: Fetch = Err("connection refused".into());
        assert!(
            evaluate(&url, Platform::Android, None, Some(&down), &Facts::default())
                .iter()
                .any(|x| x.detail.contains("refused"))
        );
        let text: Fetch =
            Ok(RemoteFile { content_type: Some("text/plain".into()), ..served("u", ASSETLINKS).unwrap() });
        assert!(
            evaluate(&url, Platform::Android, None, Some(&text), &Facts::default())
                .iter()
                .any(|x| x.title.contains("content type"))
        );
        let other = served("u", ASSETLINKS);
        let wrong_app = evaluate(&url, Platform::Android, Some("dev.y.app"), Some(&other), &Facts::default());
        assert!(wrong_app.iter().any(|x| x.title == "The app is not in assetlinks.json"));
        let facts = Facts { app_links: parse_get_app_links(APP_LINKS), ..Facts::default() };
        let unclaimed =
            evaluate(&link("https://nowhere.example.org/x"), Platform::Android, Some("dev.x.app"), None, &facts);
        let f = unclaimed.iter().find(|x| x.title == "The app does not claim this domain").unwrap();
        assert!(f.fix.as_deref().unwrap().contains("autoVerify"));
        let unverified =
            evaluate(&link("https://other.example.com/x"), Platform::Android, Some("dev.x.app"), None, &facts);
        assert!(unverified.iter().any(|x| x.fix.as_deref().is_some_and(|f| f.contains("--re-verify"))));
        let nobody = evaluate(&url, Platform::Android, None, None, &Facts::default());
        assert!(nobody.iter().any(|x| x.title == "No app opens this link"));
    }

    #[test]
    fn a_link_of_the_apps_own_scheme_needs_no_domain_file() {
        let facts = Facts {
            resolved: Some(Resolved { component: "dev.x.app/.W".into(), package: "dev.x.app".into(), chooser: false }),
            ..Facts::default()
        };
        let r = evaluate(&link("emulabzip-demo://open/cart"), Platform::Android, Some("dev.x.app"), None, &facts);
        assert_eq!(levels(&r), [(Level::Info, "A link of the app's own scheme"), (Level::Pass, "Who opens the link")]);
        let http = evaluate(&link("http://shop.example.com/x"), Platform::Android, None, None, &Facts::default());
        assert!(http.iter().any(|x| x.title == "The link is http"));
    }

    #[test]
    fn ios_needs_the_file_the_bundle_the_path_the_entitlement_and_the_team() {
        let url = link("https://shop.example.com/demo/cart?ref=a");
        let file = served("https://shop.example.com/.well-known/apple-app-site-association", AASA);
        let entitlements = |domain: &str, team: &str| Facts {
            entitlements: Some(Entitlements {
                associated_domains: vec![format!("applinks:{domain}")],
                app_identifier: Some(format!("{team}.dev.x.app")),
            }),
            ..Facts::default()
        };
        let ok = evaluate(
            &url,
            Platform::Ios,
            Some("dev.x.app"),
            Some(&file),
            &entitlements("shop.example.com", "ABCDE12345"),
        );
        assert!(ok.iter().all(|x| x.level != Level::Fail), "{:#?}", levels(&ok));
        let wrong_team = evaluate(
            &url,
            Platform::Ios,
            Some("dev.x.app"),
            Some(&file),
            &entitlements("shop.example.com", "OTHER00000"),
        );
        assert!(wrong_team.iter().any(|x| x.title == "The team id does not match" && x.level == Level::Fail));
        let no_domain =
            evaluate(&url, Platform::Ios, Some("dev.x.app"), Some(&file), &entitlements("elsewhere.com", "ABCDE12345"));
        assert!(no_domain.iter().any(|x| x.title == "The app does not claim the domain"));
        let blocked = evaluate(
            &link("https://shop.example.com/demo/private/x?ref=a"),
            Platform::Ios,
            Some("dev.x.app"),
            Some(&file),
            &Facts::default(),
        );
        assert!(blocked.iter().any(|x| x.title == "The link is not handled by the app"));
        let absent = evaluate(&url, Platform::Ios, Some("dev.nobody"), Some(&file), &Facts::default());
        assert!(absent.iter().any(|x| x.title == "The app is not in apple-app-site-association"));
    }

    #[test]
    fn the_history_keeps_pins_and_forgets_the_oldest() {
        let mut h = Vec::new();
        remember(&mut h, "https://a/1", Some("dev.x"), 1, Some(Level::Warn));
        h[0].pinned = true;
        remember(&mut h, "https://a/2", None, 2, None);
        remember(&mut h, "https://a/1", None, 3, None);
        assert_eq!(h.iter().map(|e| e.url.as_str()).collect::<Vec<_>>(), ["https://a/1", "https://a/2"]);
        assert!(
            h[0].pinned && h[0].app.as_deref() == Some("dev.x") && h[0].result == Some(Level::Warn),
            "a repeat keeps what it knew"
        );
        for i in 0..(MAX_HISTORY + 20) {
            remember(&mut h, &format!("https://b/{i}"), None, i as u64, None);
        }
        assert_eq!(h.len(), MAX_HISTORY);
        assert!(h.iter().any(|e| e.url == "https://a/1" && e.pinned), "a pinned link stays");
        assert_eq!(worst(&[]), Level::Pass);
    }
}
