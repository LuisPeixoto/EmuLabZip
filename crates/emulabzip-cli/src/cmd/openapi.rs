//! `emulabzip openapi check`: do the responses in a HAR file match an OpenAPI description?

use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::Engine;
use emulabzip_net::openapi::{Spec, Validation};
use owo_colors::{OwoColorize, Stream::Stdout};
use serde::Serialize;
use serde_json::Value;

use crate::output;

/// One response of the HAR, and what checking it found.
#[derive(Debug, Serialize)]
pub struct Row {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub validation: Validation,
}

/// Checks every HTTP response in a HAR file against the spec.
pub fn check_har(spec: &Spec, har: &Value) -> Result<Vec<Row>> {
    let entries = har
        .pointer("/log/entries")
        .and_then(Value::as_array)
        .context("that is not a HAR file: it has no log.entries")?;
    let mut rows = Vec::new();
    for entry in entries {
        let (Some(method), Some(url)) = (
            entry.pointer("/request/method").and_then(Value::as_str),
            entry.pointer("/request/url").and_then(Value::as_str),
        ) else {
            continue;
        };
        let status = entry.pointer("/response/status").and_then(Value::as_u64).unwrap_or(0) as u16;
        if status == 0 {
            continue;
        }
        let mime = entry.pointer("/response/content/mimeType").and_then(Value::as_str);
        let text = entry.pointer("/response/content/text").and_then(Value::as_str).unwrap_or("");
        let body: Vec<u8> = if entry.pointer("/response/content/encoding").and_then(Value::as_str) == Some("base64") {
            base64::engine::general_purpose::STANDARD.decode(text).unwrap_or_default()
        } else {
            text.as_bytes().to_vec()
        };
        let path = url.split_once("://").map_or(url, |(_, rest)| rest.find('/').map_or("/", |i| &rest[i..]));
        let validation = spec.check(method, path, status, mime, &body);
        rows.push(Row { method: method.to_string(), url: url.to_string(), status, validation });
    }
    Ok(rows)
}

pub fn run(spec_path: &Path, har_path: &Path, json: bool) -> Result<()> {
    let spec = Spec::load(spec_path)?;
    let text = std::fs::read_to_string(har_path).with_context(|| format!("could not read {}", har_path.display()))?;
    let har: Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", har_path.display()))?;
    let rows = check_har(&spec, &har)?;
    let problems = rows.iter().filter(|r| r.validation.outcome == "violations").count();
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for r in &rows {
            let v = &r.validation;
            let mark = match v.outcome.as_str() {
                "ok" => "✓".if_supports_color(Stdout, |t| t.green()).to_string(),
                "violations" => "✗".if_supports_color(Stdout, |t| t.red()).to_string(),
                _ => "·".if_supports_color(Stdout, |t| t.dimmed()).to_string(),
            };
            println!("{mark} {} {} {}", r.method, r.url, output::dim(&r.status.to_string()));
            for x in &v.violations {
                let at = if x.pointer.is_empty() { "the response".to_string() } else { x.pointer.clone() };
                println!("    {at}: {}", x.message);
            }
            if let Some(note) = &v.note {
                println!("    {}", output::dim(note));
            }
        }
        let ok = rows.iter().filter(|r| r.validation.outcome == "ok").count();
        println!(
            "\n{} responses: {ok} match {}, {problems} do not, {} could not be checked",
            rows.len(),
            spec.name(),
            rows.len() - ok - problems
        );
    }
    if problems > 0 {
        bail!("{problems} responses do not match the spec");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = "openapi: 3.1.0\ninfo: {title: t, version: '1'}\npaths:\n  /a:\n    get:\n      responses:\n        '200':\n          description: x\n          content:\n            application/json:\n              schema: {type: object, required: [n], properties: {n: {type: integer}}}\n";

    #[test]
    fn the_responses_of_a_har_are_checked() {
        let spec = Spec::parse("api.yaml", SPEC).unwrap();
        let har = serde_json::json!({ "log": { "entries": [
            { "request": { "method": "GET", "url": "https://h.test/a?x=1" },
              "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"n\": 1}" } } },
            { "request": { "method": "GET", "url": "https://h.test/a" },
              "response": { "status": 200, "content": { "mimeType": "application/json", "encoding": "base64", "text": "eyJuIjogIngifQ==" } } },
            { "request": { "method": "GET", "url": "https://h.test/other" },
              "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{}" } } },
            { "request": { "method": "GET", "url": "https://h.test/failed" }, "response": { "status": 0 } }
        ] } });
        let rows = check_har(&spec, &har).unwrap();
        let outcomes: Vec<&str> = rows.iter().map(|r| r.validation.outcome.as_str()).collect();
        assert_eq!(outcomes, ["ok", "violations", "unknown"], "{rows:#?}");
        assert!(check_har(&spec, &serde_json::json!({})).is_err());
    }
}
