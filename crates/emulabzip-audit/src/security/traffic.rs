//! What the app sent: plain HTTP, and secrets or personal data where they should not be.

use emulabzip_core::Severity;

use super::finding;
use crate::finding::{Area, Finding};
use crate::patterns::{is_empty_secret, redact, scan, sensitive_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: String,
    /// The request body as text, when it is text.
    pub body: Option<String>,
}

/// The host of an `http://` or `https://` address.
pub fn host(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    // IPv6 hosts are in brackets.
    if let Some(v6) = authority.strip_prefix('[') {
        return v6.split(']').next();
    }
    authority.split(':').next()
}

fn is_local(host: &str) -> bool {
    host == "localhost"
        || host == "::1"
        || host.starts_with("127.")
        || host == "10.0.2.2"
        || host == "10.0.3.2"
        || host.ends_with(".local")
}

/// An address without its query string or fragment, which is where secrets travel.
pub fn without_query(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn analyze(requests: &[Request]) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut cleartext_hosts: Vec<String> = Vec::new();
    let mut seen: Vec<(String, String)> = Vec::new();
    for r in requests {
        let clear = r.url.starts_with("http://") && host(&r.url).is_some_and(|h| !is_local(h));
        if clear {
            let h = host(&r.url).unwrap_or_default().to_string();
            if !cleartext_hosts.contains(&h) {
                cleartext_hosts.push(h.clone());
                let mut x = finding(
                    "cleartext-traffic",
                    Severity::Warning,
                    Area::Traffic,
                    format!(
                        "The app sent {} {} over plain HTTP: anyone on the network can read and change it.",
                        r.method, h
                    ),
                    "Use HTTPS. On Android set `android:usesCleartextTraffic=\"false\"`; on iOS keep App Transport Security on.",
                );
                x.location = Some(without_query(&r.url).to_string());
                out.push(x);
            }
        }
        // The query string: secrets in an address end up in logs, history and proxies.
        if let Some((_, query)) = r.url.split_once('?') {
            for pair in query.split('#').next().unwrap_or("").split('&') {
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                let value = decode(value);
                let by_name = sensitive_name(&decode(name)) && value.len() >= 8 && !is_empty_secret(&value);
                let by_shape = scan(&value).iter().any(|h| !h.kind.personal);
                if (by_name || by_shape) && !seen.contains(&(host(&r.url).unwrap_or("").into(), name.into())) {
                    seen.push((host(&r.url).unwrap_or("").into(), name.into()));
                    let mut x = finding(
                        "secret-in-url",
                        Severity::Error,
                        Area::Traffic,
                        format!(
                            "The parameter `{name}` of a request to {} carries what looks like a secret.",
                            host(&r.url).unwrap_or("")
                        ),
                        "Send credentials in the `Authorization` header or the body, never in the address.",
                    );
                    x.evidence = Some(format!("{name}={}", redact(&value)));
                    x.location = Some(without_query(&r.url).to_string());
                    out.push(x);
                }
            }
        }
        // A body is only a problem when anyone on the network can read it.
        if clear && let Some(body) = &r.body {
            for hit in scan(body) {
                let key = (host(&r.url).unwrap_or("").to_string(), hit.redacted.clone());
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                let mut x = finding(
                    format!("{}-in-cleartext-body", hit.kind.name),
                    Severity::Error,
                    Area::Traffic,
                    format!("A request over plain HTTP carries {}.", hit.kind.what),
                    "Use HTTPS, and send only the data the server needs.",
                );
                x.evidence = Some(hit.redacted);
                x.location = Some(without_query(&r.url).to_string());
                out.push(x);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(url: &str, body: Option<&str>) -> Request {
        Request { method: "POST".into(), url: url.into(), body: body.map(str::to_string) }
    }

    #[test]
    fn hosts_are_read_from_addresses() {
        assert_eq!(host("https://api.shop.com:8443/a?x=1"), Some("api.shop.com"));
        assert_eq!(host("http://user:pw@example.com/"), Some("example.com"));
        assert_eq!(host("http://[::1]:8080/x"), Some("::1"));
        assert_eq!(host("nonsense"), None);
        assert_eq!(without_query("https://a.com/p?token=1#x"), "https://a.com/p");
    }

    #[test]
    fn plain_http_is_reported_once_per_host_and_local_hosts_are_not() {
        let found = analyze(&[
            req("http://api.shop.com/a", None),
            req("http://api.shop.com/b", None),
            req("http://10.0.2.2:8080/dev", None),
            req("http://localhost:3000/", None),
            req("https://api.shop.com/c", None),
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, "cleartext-traffic");
        assert_eq!(found[0].location.as_deref(), Some("http://api.shop.com/a"));
    }

    #[test]
    fn secrets_in_a_query_are_found_and_redacted() {
        let found =
            analyze(&[req("https://api.shop.com/me?access_token=abcdef1234567890&page=2&token=%2A%2A%2A", None)]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].rule, "secret-in-url");
        assert_eq!(found[0].evidence.as_deref(), Some("access_token=ab…90 (16 chars)"));
        assert!(!found[0].location.as_deref().unwrap().contains("abcdef"));
    }

    #[test]
    fn a_body_matters_only_over_plain_http() {
        let card = r#"{"card":"4111 1111 1111 1111","email":"ana@example.com"}"#;
        let clear = analyze(&[req("http://pay.example.com/charge", Some(card))]);
        let rules: Vec<_> = clear.iter().map(|f| f.rule.as_str()).collect();
        assert!(
            rules.contains(&"card-number-in-cleartext-body") && rules.contains(&"email-in-cleartext-body"),
            "{rules:?}"
        );
        assert!(analyze(&[req("https://pay.example.com/charge", Some(card))]).is_empty());
    }
}
