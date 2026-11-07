//! Names the operation a request carries: a GraphQL operation, or a gRPC method.
//!
//! The list of requests shows these instead of one `POST /graphql` after another.

use serde_json::Value;

/// How long a name can be before it is cut, so a strange body cannot widen the list.
const MAX_NAME: usize = 80;

/// The operation of a request, or `None` when it is neither GraphQL nor gRPC.
pub fn operation(method: &str, path_and_query: &str, content_type: Option<&str>, body: &[u8]) -> Option<String> {
    let (path, query) = path_and_query.split_once('?').unwrap_or((path_and_query, ""));
    let content_type = content_type.unwrap_or("").to_ascii_lowercase();
    if content_type.starts_with("application/grpc") {
        return grpc_method(path);
    }
    if method.eq_ignore_ascii_case("GET") {
        let text = query_param(query, "query")?;
        let name = query_param(query, "operationName").filter(|n| !n.is_empty());
        return Some(cut(&graphql_name(&text, name.as_deref())));
    }
    if !(method.eq_ignore_ascii_case("POST") || method.eq_ignore_ascii_case("PUT")) {
        return None;
    }
    if content_type.starts_with("application/graphql") {
        return Some(cut(&graphql_name(std::str::from_utf8(body).ok()?, None)));
    }
    if !content_type.contains("json") && !content_type.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_slice(body).ok()?;
    match value {
        Value::Object(_) => one(&value).map(|n| cut(&n)),
        Value::Array(items) if !items.is_empty() => {
            let names: Vec<String> = items.iter().filter_map(one).collect();
            if names.is_empty() {
                return None;
            }
            let shown: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
            let more = names.len().saturating_sub(3);
            let mut joined = shown.join(" + ");
            if more > 0 {
                joined.push_str(&format!(" + {more} more"));
            }
            Some(cut(&joined))
        }
        _ => None,
    }
}

/// `Service/Method` from `/package.Service/Method`.
fn grpc_method(path: &str) -> Option<String> {
    let mut parts = path.trim_matches('/').split('/');
    let service = parts.next().filter(|s| !s.is_empty())?;
    let method = parts.next().filter(|s| !s.is_empty())?;
    let service = service.rsplit('.').next().unwrap_or(service);
    Some(cut(&format!("{service}/{method}")))
}

/// One GraphQL request object: `{ "query": "...", "operationName": "..." }`.
fn one(value: &Value) -> Option<String> {
    let query = value.get("query")?.as_str()?;
    let name = value.get("operationName").and_then(Value::as_str).filter(|n| !n.is_empty());
    Some(graphql_name(query, name))
}

/// The name of the operation: the given one, else the one written after `query`, `mutation` or `subscription`.
pub fn graphql_name(query: &str, given: Option<&str>) -> String {
    let kind = first_keyword(query);
    if let Some(name) = given {
        return name.to_string();
    }
    match kind {
        Some((kind, Some(name))) => format!("{name} ({kind})"),
        Some((kind, None)) => format!("anonymous {kind}"),
        None => "anonymous query".into(),
    }
}

/// The first operation's keyword and name. Comments and strings before it are skipped; a leading `{` is a query.
fn first_keyword(query: &str) -> Option<(&'static str, Option<String>)> {
    let mut chars = query.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        match c {
            '#' => {
                while let Some(&(_, c)) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            c if c.is_whitespace() || c == ',' => {
                chars.next();
            }
            '{' => return Some(("query", None)),
            c if c.is_ascii_alphabetic() => {
                let word: String = query[i..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                let kind = match word.as_str() {
                    "query" => "query",
                    "mutation" => "mutation",
                    "subscription" => "subscription",
                    // A fragment can come first.
                    "fragment" => {
                        skip_braces(&mut chars);
                        continue;
                    }
                    _ => return None,
                };
                let rest = &query[i + word.len()..];
                let name: String =
                    rest.trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                let named = !name.is_empty() && !name.chars().next().is_some_and(|c| c.is_ascii_digit());
                return Some((kind, named.then_some(name)));
            }
            _ => return None,
        }
    }
    None
}

/// Moves past the next balanced `{ ... }`.
fn skip_braces(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) {
    let mut depth = 0usize;
    let mut started = false;
    for (_, c) in chars.by_ref() {
        match c {
            '{' => {
                depth += 1;
                started = true;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if started && depth == 0 {
                    return;
                }
            }
            _ => {}
        }
    }
}

/// A GraphQL request written for people: each operation's query as it was sent, then its variables.
/// `None` when the request is not GraphQL.
pub fn graphql_view(method: &str, path_and_query: &str, content_type: Option<&str>, body: &[u8]) -> Option<String> {
    operation(method, path_and_query, content_type, body)?;
    let content_type = content_type.unwrap_or("").to_ascii_lowercase();
    let (_, query) = path_and_query.split_once('?').unwrap_or((path_and_query, ""));
    let mut operations: Vec<(String, Option<Value>)> = Vec::new();
    if method.eq_ignore_ascii_case("GET") {
        operations.push((
            query_param(query, "query")?,
            query_param(query, "variables").and_then(|v| serde_json::from_str(&v).ok()),
        ));
    } else if content_type.starts_with("application/graphql") {
        operations.push((String::from_utf8_lossy(body).into_owned(), None));
    } else {
        let value: Value = serde_json::from_slice(body).ok()?;
        let items = match value {
            Value::Array(items) => items,
            other => vec![other],
        };
        for item in items {
            let text = item.get("query")?.as_str()?.to_string();
            operations.push((text, item.get("variables").filter(|v| !v.is_null()).cloned()));
        }
    }
    let many = operations.len() > 1;
    let mut out = String::new();
    for (i, (text, variables)) in operations.iter().enumerate() {
        if many {
            out.push_str(&format!("# Operation {}\n", i + 1));
        }
        out.push_str(text.trim());
        out.push('\n');
        if let Some(v) = variables {
            out.push_str("\n# Variables\n");
            out.push_str(&serde_json::to_string_pretty(v).unwrap_or_default());
            out.push('\n');
        }
        if many && i + 1 < operations.len() {
            out.push('\n');
        }
    }
    Some(out)
}

pub(crate) fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == name).then(|| percent_decode(v))
    })
}

pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn cut(text: &str) -> String {
    if text.chars().count() <= MAX_NAME {
        return text.to_string();
    }
    let mut s: String = text.chars().take(MAX_NAME - 1).collect();
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(body: &str) -> Option<String> {
        operation("POST", "/graphql", Some("application/json"), body.as_bytes())
    }

    #[test]
    fn uses_the_operation_name_the_client_sent() {
        assert_eq!(post(r#"{"operationName":"GetUser","query":"query GetUser { me { id } }"}"#).unwrap(), "GetUser");
    }

    #[test]
    fn reads_the_name_from_the_query_text() {
        assert_eq!(post(r#"{"query":"mutation AddItem($i: ID!) { add(id: $i) }"}"#).unwrap(), "AddItem (mutation)");
        assert_eq!(post(r#"{"query":"query { me }"}"#).unwrap(), "anonymous query");
        assert_eq!(post(r#"{"query":"{ me }"}"#).unwrap(), "anonymous query");
        assert_eq!(post(r#"{"query":"subscription { tick }"}"#).unwrap(), "anonymous subscription");
    }

    #[test]
    fn skips_comments_and_fragments_before_the_operation() {
        let q = r##"{"query":"# hi\nfragment F on User { id }\nquery Feed { me { ...F } }"}"##;
        assert_eq!(post(q).unwrap(), "Feed (query)");
    }

    #[test]
    fn names_a_batch() {
        let body = r#"[{"query":"query A { a }"},{"query":"query B { b }"},{"query":"query C { c }"},{"query":"query D { d }"}]"#;
        assert_eq!(post(body).unwrap(), "A (query) + B (query) + C (query) + 1 more");
    }

    #[test]
    fn reads_a_get_request() {
        let op = operation("GET", "/graphql?query=query%20Hello%20%7B%20hi%20%7D&x=1", None, b"").unwrap();
        assert_eq!(op, "Hello (query)");
        assert_eq!(operation("GET", "/graphql?operationName=Q&query=%7Bhi%7D", None, b"").unwrap(), "Q");
        assert!(operation("GET", "/index.html?x=1", None, b"").is_none());
    }

    #[test]
    fn names_a_grpc_method_from_its_path() {
        let op = operation("POST", "/helloworld.Greeter/SayHello", Some("application/grpc+proto"), b"").unwrap();
        assert_eq!(op, "Greeter/SayHello");
        assert!(operation("POST", "/", Some("application/grpc"), b"").is_none());
    }

    #[test]
    fn leaves_other_requests_alone() {
        assert!(post(r#"{"name":"x"}"#).is_none());
        assert!(post("not json").is_none());
        assert!(post("[]").is_none());
        assert!(operation("POST", "/up", Some("image/png"), b"\x89PNG").is_none());
        assert!(operation("DELETE", "/graphql", Some("application/json"), b"{}").is_none());
    }

    #[test]
    fn shows_the_query_and_its_variables() {
        let body = r#"{"query":"query Q($id: ID!) { user(id: $id) { name } }","variables":{"id":"7"}}"#;
        let text = graphql_view("POST", "/graphql", Some("application/json"), body.as_bytes()).unwrap();
        assert!(text.starts_with("query Q($id: ID!) { user(id: $id) { name } }\n"), "{text}");
        assert!(text.contains("# Variables\n{\n  \"id\": \"7\"\n}"), "{text}");
        let batch = r#"[{"query":"{a}"},{"query":"{b}","variables":{"x":1}}]"#;
        let text = graphql_view("POST", "/g", Some("application/json"), batch.as_bytes()).unwrap();
        assert!(text.contains("# Operation 1") && text.contains("# Operation 2"), "{text}");
        assert!(graphql_view("POST", "/g", Some("application/json"), b"{}").is_none());
    }

    #[test]
    fn cuts_a_very_long_name() {
        let long = "x".repeat(300);
        let name = post(&format!(r#"{{"operationName":"{long}","query":"{{a}}"}}"#)).unwrap();
        assert_eq!(name.chars().count(), MAX_NAME);
    }

    #[test]
    fn does_not_trip_on_odd_percent_signs() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz%41"), "%zzA");
    }
}
