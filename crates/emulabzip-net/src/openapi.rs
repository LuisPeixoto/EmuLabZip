//! Checking responses against an OpenAPI description: does the server answer the way its contract says?
//!
//! A spec (OpenAPI 3.0 or 3.1, JSON or YAML) is read once and kept. For a response, the operation is found from the
//! method and the path (`/users/{id}`), the schema from the status and the content type, and the body is validated
//! against it. What is wrong is listed with a JSON pointer into the body and a message. Only responses are checked.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use emulabzip_core::{Error, Result};
use jsonschema::Validator;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The largest spec that is read.
pub const MAX_SPEC_BYTES: u64 = 5 * 1024 * 1024;
/// The most violations listed for one response.
pub const MAX_VIOLATIONS: usize = 50;

/// One thing wrong with a response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    /// Where in the body (`/items/0/name`), or empty for the response as a whole.
    pub pointer: String,
    pub message: String,
}

/// What checking a response found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    /// The spec's file name.
    pub spec: String,
    /// `GET /users/{id}`, when the spec describes the request.
    pub operation: Option<String>,
    /// `ok`, `violations`, `unknown` (the spec does not describe this request) or `unchecked` (it does, but with no
    /// schema to check this response against).
    pub outcome: String,
    pub violations: Vec<Violation>,
    pub note: Option<String>,
}

impl Validation {
    fn new(spec: &str, operation: Option<String>, outcome: &str, note: Option<String>) -> Self {
        Validation { spec: spec.to_string(), operation, outcome: outcome.to_string(), violations: Vec::new(), note }
    }
}

#[derive(Debug)]
enum Segment {
    Literal(String),
    Parameter,
}

struct Route {
    template: String,
    segments: Vec<Segment>,
    /// Upper-case method to operation.
    operations: Vec<(String, Value)>,
}

pub struct Spec {
    name: String,
    document: Value,
    openapi_30: bool,
    /// Paths that the servers add in front of every route: `/v1`.
    base_paths: Vec<String>,
    routes: Vec<Route>,
    validators: Mutex<HashMap<String, Option<Arc<Validator>>>>,
}

impl std::fmt::Debug for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spec").field("name", &self.name).field("routes", &self.routes.len()).finish()
    }
}

const METHODS: [&str; 8] = ["get", "put", "post", "delete", "options", "head", "patch", "trace"];

/// Follows a local `$ref` (`#/components/responses/NotFound`) a few steps, so the object it names is returned.
fn resolve<'a>(document: &'a Value, mut value: &'a Value) -> &'a Value {
    for _ in 0..8 {
        let Some(target) = value.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix('#')) else {
            return value;
        };
        match document.pointer(target) {
            Some(next) => value = next,
            None => return value,
        }
    }
    value
}

/// OpenAPI 3.0's schema dialect is draft 4's with a few changes. This rewrites what differs into what draft 2020-12
/// understands: `nullable`, and the boolean `exclusiveMinimum` and `exclusiveMaximum`.
fn upgrade_30(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.remove("nullable").is_some_and(|n| n == Value::Bool(true)) {
                match map.get("type").cloned() {
                    Some(Value::String(t)) => {
                        map.insert("type".into(), Value::Array(vec![Value::String(t), Value::String("null".into())]));
                    }
                    Some(Value::Array(mut types)) => {
                        types.push(Value::String("null".into()));
                        map.insert("type".into(), Value::Array(types));
                    }
                    _ => {}
                }
                if let Some(Value::Array(values)) = map.get_mut("enum")
                    && !values.contains(&Value::Null)
                {
                    values.push(Value::Null);
                }
            }
            for (flag, bound) in [("exclusiveMinimum", "minimum"), ("exclusiveMaximum", "maximum")] {
                match map.get(flag) {
                    Some(Value::Bool(true)) => {
                        if let Some(limit) = map.remove(bound) {
                            map.insert(flag.into(), limit);
                        } else {
                            map.remove(flag);
                        }
                    }
                    Some(Value::Bool(false)) => {
                        map.remove(flag);
                    }
                    _ => {}
                }
            }
            map.values_mut().for_each(upgrade_30);
        }
        Value::Array(items) => items.iter_mut().for_each(upgrade_30),
        _ => {}
    }
}

fn base_path(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    // A server URL may have variables (`{region}`); they are left as the text says.
    let path = rest.find('/').map(|i| &rest[i..])?;
    let path = path.trim_end_matches('/');
    (!path.is_empty() && !path.contains('{')).then(|| path.to_string())
}

impl Spec {
    /// Reads a spec from text, JSON or YAML.
    pub fn parse(name: &str, text: &str) -> Result<Spec> {
        let document: Value = if text.trim_start().starts_with('{') {
            serde_json::from_str(text).map_err(|e| Error::failed(format!("{name} is not valid JSON: {e}")))?
        } else {
            serde_yaml_ng::from_str(text).map_err(|e| Error::failed(format!("{name} is not valid YAML: {e}")))?
        };
        let version = document.get("openapi").and_then(Value::as_str).unwrap_or("");
        if document.get("swagger").is_some() {
            return Err(Error::failed(format!(
                "{name} is a Swagger 2.0 file. Convert it to OpenAPI 3 (the swagger converter or editor.swagger.io)."
            )));
        }
        let openapi_30 = version.starts_with("3.0");
        if !openapi_30 && !version.starts_with("3.1") {
            return Err(Error::failed(format!(
                "{name} has no `openapi: 3.0.x` or `3.1.x` field, so it is not an OpenAPI 3 file."
            )));
        }
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::failed(format!("{name} has no `paths`.")))?;
        let mut routes = Vec::new();
        for (template, item) in paths {
            let item = resolve(&document, item);
            let operations: Vec<(String, Value)> =
                METHODS.iter().filter_map(|m| item.get(*m).map(|op| (m.to_ascii_uppercase(), op.clone()))).collect();
            if operations.is_empty() {
                continue;
            }
            let segments = template
                .split('/')
                .filter(|s| !s.is_empty())
                .map(|s| {
                    if s.starts_with('{') && s.ends_with('}') {
                        Segment::Parameter
                    } else {
                        Segment::Literal(s.to_string())
                    }
                })
                .collect();
            routes.push(Route { template: template.clone(), segments, operations });
        }
        let base_paths = document
            .get("servers")
            .and_then(Value::as_array)
            .map(|s| s.iter().filter_map(|s| s.get("url").and_then(Value::as_str)).filter_map(base_path).collect())
            .unwrap_or_default();
        Ok(Spec {
            name: name.to_string(),
            document,
            openapi_30,
            base_paths,
            routes,
            validators: Mutex::new(HashMap::new()),
        })
    }

    /// Reads a spec from a file, at most [`MAX_SPEC_BYTES`].
    pub fn load(path: &Path) -> Result<Spec> {
        let meta =
            std::fs::metadata(path).map_err(|e| Error::failed(format!("Could not read {}: {e}", path.display())))?;
        if !meta.is_file() || meta.len() > MAX_SPEC_BYTES {
            return Err(Error::failed(format!(
                "{} is not a file of at most {} MB.",
                path.display(),
                MAX_SPEC_BYTES / 1024 / 1024
            )));
        }
        let name = path.file_name().map_or_else(|| "spec".to_string(), |n| n.to_string_lossy().into_owned());
        Spec::parse(&name, &std::fs::read_to_string(path)?)
    }

    /// Like [`Spec::load`], and remembered until the file changes, so a busy proxy reads a spec once.
    pub fn cached(path: &Path) -> Result<Arc<Spec>> {
        type Cache = Mutex<HashMap<PathBuf, (SystemTime, Arc<Spec>)>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        if let Some((at, spec)) = cache.lock().unwrap().get(path)
            && *at == modified
        {
            return Ok(spec.clone());
        }
        let spec = Arc::new(Spec::load(path)?);
        cache.lock().unwrap().insert(path.to_path_buf(), (modified, spec.clone()));
        Ok(spec)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// How many operations the spec describes.
    pub fn operation_count(&self) -> usize {
        self.routes.iter().map(|r| r.operations.len()).sum()
    }

    /// The operation for a request: its route template and the operation object.
    fn find(&self, method: &str, path: &str) -> Option<(&Route, &Value)> {
        let path = path.split(['?', '#']).next().unwrap_or(path);
        let mut candidates: Vec<&str> = vec![path];
        for base in &self.base_paths {
            if let Some(rest) = path.strip_prefix(base.as_str()) {
                candidates.push(if rest.is_empty() { "/" } else { rest });
            }
        }
        let method = method.to_ascii_uppercase();
        let mut best: Option<(usize, &Route, &Value)> = None;
        for candidate in candidates {
            let parts: Vec<&str> = candidate.split('/').filter(|s| !s.is_empty()).collect();
            for route in &self.routes {
                if route.segments.len() != parts.len() {
                    continue;
                }
                let mut literals = 0;
                let fits = route.segments.iter().zip(&parts).all(|(seg, part)| match seg {
                    Segment::Literal(l) => {
                        literals += 1;
                        l == part
                    }
                    Segment::Parameter => true,
                });
                if !fits {
                    continue;
                }
                if let Some((_, op)) = route.operations.iter().find(|(m, _)| *m == method)
                    && best.is_none_or(|(n, _, _)| literals > n)
                {
                    best = Some((literals, route, op));
                }
            }
        }
        best.map(|(_, r, o)| (r, o))
    }

    /// The response object a status gets: the exact code, then `2XX`, then `default`.
    fn response_for<'a>(&'a self, operation: &'a Value, status: u16) -> Option<&'a Value> {
        let responses = resolve(&self.document, operation.get("responses")?).as_object()?;
        let exact = status.to_string();
        let class = format!("{}XX", status / 100);
        [exact.as_str(), class.as_str(), "default"]
            .iter()
            .find_map(|k| responses.get(*k).or_else(|| responses.get(&k.to_ascii_lowercase())))
            .map(|r| resolve(&self.document, r))
    }

    /// A validator for a response's schema, built once. `None` when the schema cannot be compiled.
    fn validator(&self, key: &str, schema: &Value) -> Option<Arc<Validator>> {
        if let Some(found) = self.validators.lock().unwrap().get(key) {
            return found.clone();
        }
        // The schema may point into `components`, so the document's own components travel with it.
        let mut root = Map::new();
        root.insert("$ref".into(), Value::String("#/$defs/response".into()));
        let mut defs = Map::new();
        defs.insert("response".into(), schema.clone());
        root.insert("$defs".into(), Value::Object(defs));
        if let Some(components) = self.document.get("components") {
            root.insert("components".into(), components.clone());
        }
        let mut root = Value::Object(root);
        if self.openapi_30 {
            upgrade_30(&mut root);
        }
        let built = jsonschema::draft202012::new(&root).ok().map(Arc::new);
        self.validators.lock().unwrap().insert(key.to_string(), built.clone());
        built
    }

    /// Checks one response against the spec.
    pub fn check(&self, method: &str, path: &str, status: u16, content_type: Option<&str>, body: &[u8]) -> Validation {
        let Some((route, operation)) = self.find(method, path) else {
            return Validation::new(
                &self.name,
                None,
                "unknown",
                Some("The spec does not describe this request.".into()),
            );
        };
        let label = Some(format!("{} {}", method.to_ascii_uppercase(), route.template));
        let Some(response) = self.response_for(operation, status) else {
            let mut v = Validation::new(&self.name, label, "violations", None);
            v.violations.push(Violation {
                pointer: String::new(),
                message: format!("The spec does not describe a {status} response."),
            });
            return v;
        };
        let wanted = content_type.unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        let Some(content) = response.get("content").and_then(Value::as_object) else {
            let note =
                if body.is_empty() { None } else { Some("The spec says this response has no body.".to_string()) };
            return Validation::new(&self.name, label, if body.is_empty() { "ok" } else { "unchecked" }, note);
        };
        // The media type the response used, else a wildcard, else the one JSON type the spec has.
        let media = content
            .get(&wanted)
            .or_else(|| {
                content
                    .iter()
                    .find(|(k, _)| k.as_str() == "*/*" || (k.ends_with("/*") && wanted.starts_with(&k[..k.len() - 1])))
                    .map(|(_, v)| v)
            })
            .or_else(|| {
                let json: Vec<_> = content.iter().filter(|(k, _)| k.contains("json")).collect();
                (json.len() == 1 && wanted.contains("json")).then(|| json[0].1)
            });
        let Some(media) = media else {
            let mut v = Validation::new(&self.name, label, "violations", None);
            v.violations.push(Violation {
                pointer: String::new(),
                message: format!(
                    "The response is `{}`; the spec describes {}.",
                    if wanted.is_empty() { "of no type" } else { &wanted },
                    content.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            });
            return v;
        };
        let json_type =
            wanted.contains("json") || wanted.is_empty() || content.keys().any(|k| k.contains("json") && *k == wanted);
        let Some(schema) = media.get("schema").map(|s| resolve(&self.document, s)) else {
            return Validation::new(
                &self.name,
                label,
                "unchecked",
                Some("The spec has no schema for this response.".into()),
            );
        };
        if !json_type {
            return Validation::new(
                &self.name,
                label,
                "unchecked",
                Some(format!("Only JSON bodies are checked, not `{wanted}`.")),
            );
        }
        let instance: Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => {
                let mut v = Validation::new(&self.name, label, "violations", None);
                v.violations
                    .push(Violation { pointer: String::new(), message: format!("The body is not valid JSON: {e}") });
                return v;
            }
        };
        let key = format!("{} {} {status} {wanted}", method.to_ascii_uppercase(), route.template);
        let Some(validator) = self.validator(&key, schema) else {
            return Validation::new(
                &self.name,
                label,
                "unchecked",
                Some("The schema in the spec could not be compiled.".into()),
            );
        };
        let violations: Vec<Violation> = validator
            .iter_errors(&instance)
            .take(MAX_VIOLATIONS)
            .map(|e| Violation { pointer: e.instance_path().to_string(), message: e.to_string() })
            .collect();
        let outcome = if violations.is_empty() { "ok" } else { "violations" };
        let mut v = Validation::new(&self.name, label, outcome, None);
        v.violations = violations;
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = r##"
openapi: 3.0.3
info: { title: Shop, version: "1" }
servers:
  - url: https://api.shop.example/v1
paths:
  /users/{id}:
    get:
      responses:
        "200":
          description: a user
          content:
            application/json:
              schema: { $ref: "#/components/schemas/User" }
        "404": { $ref: "#/components/responses/NotFound" }
  /users/me:
    get:
      responses:
        "200":
          description: the caller
          content:
            application/json:
              schema: { type: object, required: [email], properties: { email: { type: string } } }
  /items:
    get:
      responses:
        2XX:
          description: items
          content:
            application/json:
              schema:
                type: array
                items: { $ref: "#/components/schemas/Item" }
        default: { description: anything else }
  /ping:
    get:
      responses:
        "204": { description: nothing }
components:
  responses:
    NotFound:
      description: missing
      content:
        application/json:
          schema: { type: object, required: [error], properties: { error: { type: string } } }
  schemas:
    User:
      type: object
      required: [id, name]
      properties:
        id: { type: integer }
        name: { type: string }
        nickname: { type: string, nullable: true }
        age: { type: integer, minimum: 0, exclusiveMinimum: true }
    Item:
      type: object
      required: [sku]
      properties:
        sku: { type: string }
        price: { type: number }
"##;

    fn spec() -> Spec {
        Spec::parse("shop.yaml", YAML).unwrap()
    }

    fn check(s: &Spec, method: &str, path: &str, status: u16, body: &str) -> Validation {
        s.check(method, path, status, Some("application/json; charset=utf-8"), body.as_bytes())
    }

    #[test]
    fn a_response_that_fits_the_schema_is_ok() {
        let s = spec();
        let v = check(&s, "GET", "/v1/users/7?x=1", 200, r#"{"id": 7, "name": "Ana", "nickname": null, "age": 3}"#);
        assert_eq!((v.outcome.as_str(), v.operation.as_deref()), ("ok", Some("GET /users/{id}")), "{v:?}");
        assert!(v.violations.is_empty());
        assert_eq!(s.operation_count(), 4);
    }

    #[test]
    fn what_is_wrong_is_listed_with_where() {
        let s = spec();
        let v = check(&s, "GET", "/users/7", 200, r#"{"id": "seven", "age": 0}"#);
        assert_eq!(v.outcome, "violations");
        let at: Vec<&str> = v.violations.iter().map(|x| x.pointer.as_str()).collect();
        assert!(at.contains(&"/id"), "a wrong type points at the field: {v:?}");
        assert!(v.violations.iter().any(|x| x.message.contains("name") && x.message.contains("required")), "{v:?}");
        assert!(at.contains(&"/age"), "the boolean exclusiveMinimum of 3.0 is understood: {v:?}");
    }

    #[test]
    fn a_literal_route_beats_a_parameter_and_a_response_can_be_a_reference() {
        let s = spec();
        assert_eq!(
            check(&s, "GET", "/users/me", 200, r#"{"email": "a@b.c"}"#).operation.as_deref(),
            Some("GET /users/me")
        );
        let bad = check(&s, "GET", "/users/me", 200, r#"{"id": 1, "name": "x"}"#);
        assert_eq!(bad.outcome, "violations", "/users/me is not /users/{{id}}");
        let missing = check(&s, "GET", "/users/9", 404, r#"{"error": "no such user"}"#);
        assert_eq!(missing.outcome, "ok", "{missing:?}");
        assert_eq!(check(&s, "GET", "/users/9", 404, "{}").outcome, "violations");
    }

    #[test]
    fn status_classes_defaults_and_undocumented_statuses() {
        let s = spec();
        assert_eq!(check(&s, "GET", "/items", 201, r#"[{"sku": "a"}]"#).outcome, "ok", "2XX covers 201");
        assert_eq!(check(&s, "GET", "/items", 200, r#"[{"price": 1}]"#).violations[0].pointer, "/0");
        assert_eq!(check(&s, "GET", "/items", 500, "oops").outcome, "unchecked", "default has no body described");
        let none = check(&s, "GET", "/users/1", 500, "");
        assert_eq!(none.outcome, "violations");
        assert!(none.violations[0].message.contains("500"));
        assert_eq!(s.check("GET", "/ping", 204, None, b"").outcome, "ok");
    }

    #[test]
    fn requests_and_bodies_the_spec_does_not_describe_are_said() {
        let s = spec();
        assert_eq!(check(&s, "GET", "/nothing", 200, "{}").outcome, "unknown");
        assert_eq!(check(&s, "DELETE", "/users/1", 200, "{}").outcome, "unknown", "no such method");
        let html = s.check("GET", "/users/1", 200, Some("text/html"), b"<p>");
        assert_eq!(html.outcome, "violations");
        assert!(html.violations[0].message.contains("application/json"));
        let broken = check(&s, "GET", "/users/1", 200, "{not json");
        assert!(broken.violations[0].message.contains("not valid JSON"));
    }

    #[test]
    fn openapi_31_is_read_without_the_30_rewrites() {
        let text = r#"{"openapi":"3.1.0","paths":{"/a":{"get":{"responses":{"200":{"description":"x","content":{"application/json":{"schema":{"type":["string","null"]}}}}}}}}}"#;
        let s = Spec::parse("a.json", text).unwrap();
        assert_eq!(check(&s, "GET", "/a", 200, "null").outcome, "ok");
        assert_eq!(check(&s, "GET", "/a", 200, "1").outcome, "violations");
    }

    #[test]
    fn files_that_are_not_an_openapi_3_description_are_refused_with_the_reason() {
        let why = |text: &str| Spec::parse("x", text).unwrap_err().to_string();
        assert!(why("swagger: '2.0'\npaths: {}").contains("Swagger 2.0"));
        assert!(why("title: nope").contains("OpenAPI 3"));
        assert!(why("openapi: 3.0.0").contains("paths"));
        assert!(why("{ broken").contains("JSON"));
        assert!(why("a: [").contains("YAML"));
    }

    #[test]
    fn a_spec_file_is_read_once_until_it_changes() {
        let dir = std::env::temp_dir().join(format!("emulabzip-openapi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("api.yaml");
        std::fs::write(&path, YAML).unwrap();
        let a = Spec::cached(&path).unwrap();
        let b = Spec::cached(&path).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(a.name(), "api.yaml");
        assert!(Spec::load(&dir).is_err(), "a folder is not a spec");
        std::fs::remove_dir_all(dir).ok();
    }
}
