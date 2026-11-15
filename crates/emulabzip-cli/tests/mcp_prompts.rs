//! Prompts and resources over MCP.

mod common;

use std::collections::HashSet;

use common::Server;
use serde_json::{Value, json};

fn prompt_text(s: &mut Server, id: u32, name: &str, args: Value) -> Value {
    s.call(id, "prompts/get", json!({ "name": name, "arguments": args }))
}

#[test]
fn prompts_are_listed_and_return_a_plan() {
    let mut s = Server::start_session();
    let list = s.call(2, "prompts/list", json!({}));
    let prompts = list["result"]["prompts"].as_array().unwrap();
    let names: Vec<&str> = prompts.iter().map(|p| p["name"].as_str().unwrap()).collect();
    for expected in [
        "explain_crash",
        "why_is_it_slow",
        "summarize_session",
        "reproduce_bug",
        "debug_request",
        "release_check",
        "check_deep_link",
        "mock_backend",
    ] {
        assert!(names.contains(&expected), "{expected} is missing: {names:?}");
    }
    let r =
        prompt_text(&mut s, 3, "reproduce_bug", json!({ "description": "crash on login", "app": "com.example.app" }));
    let text = r["result"]["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(text.contains("crash on login") && text.contains("session_start"), "{r}");
    assert!(prompt_text(&mut s, 4, "reproduce_bug", json!({}))["error"].is_object());
    assert!(prompt_text(&mut s, 5, "nope", json!({}))["error"].is_object());
}

#[test]
fn every_tool_a_prompt_or_the_guide_names_exists() {
    let mut s = Server::start_session_with(&["--allow-writes"], None);
    let tools = s.call(2, "tools/list", json!({}));
    let offered: HashSet<String> =
        tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    let list = s.call(3, "prompts/list", json!({}));
    let mut texts = Vec::new();
    for (i, p) in list["result"]["prompts"].as_array().unwrap().iter().enumerate() {
        let mut args = serde_json::Map::new();
        for a in p["arguments"].as_array().unwrap_or(&Vec::new()) {
            if a["required"] == true {
                args.insert(a["name"].as_str().unwrap().to_string(), json!("x"));
            }
        }
        let r = prompt_text(&mut s, 10 + i as u32, p["name"].as_str().unwrap(), Value::Object(args));
        texts.push(r["result"]["messages"][0]["content"]["text"].as_str().unwrap().to_string());
    }
    let guide = s.call(30, "resources/read", json!({ "uri": "emulabzip://guide" }));
    texts.push(guide["result"]["contents"][0]["text"].as_str().unwrap().to_string());
    // Names that look like tools: lowercase words joined by underscores. Prompt argument names are not tools.
    let not_tools: HashSet<&str> = ["crash_id", "session_id", "url_or_host", "video_seconds"].into();
    for text in &texts {
        for word in text
            .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .filter(|w| w.contains('_'))
        {
            let word = word.trim_matches('_');
            if not_tools.contains(word) || word.is_empty() {
                continue;
            }
            assert!(offered.contains(word), "a prompt or the guide names `{word}`, which is not a tool");
        }
    }
}

#[test]
fn resources_are_listed_and_read_without_a_device() {
    let mut s = Server::start_session();
    let list = s.call(2, "resources/list", json!({}));
    let uris: Vec<&str> =
        list["result"]["resources"].as_array().unwrap().iter().map(|r| r["uri"].as_str().unwrap()).collect();
    for uri in [
        "emulabzip://guide",
        "emulabzip://devices",
        "emulabzip://sessions",
        "emulabzip://flows",
        "emulabzip://rules",
        "emulabzip://network/flows",
    ] {
        assert!(uris.contains(&uri), "{uri} is missing");
    }
    let templates = s.call(3, "resources/templates/list", json!({}));
    assert!(templates["result"]["resourceTemplates"].as_array().unwrap().len() >= 4);
    for (i, uri) in [
        "emulabzip://guide",
        "emulabzip://sessions",
        "emulabzip://flows",
        "emulabzip://states",
        "emulabzip://links/history",
        "emulabzip://rules",
        "emulabzip://network/flows",
        "emulabzip://devices",
    ]
    .iter()
    .enumerate()
    {
        let r = s.call(10 + i as u32, "resources/read", json!({ "uri": uri }));
        assert!(r["result"]["contents"][0]["text"].is_string(), "{uri}: {r}");
    }
}

#[test]
fn a_resource_address_can_never_name_a_path() {
    let mut s = Server::start_session();
    for (i, uri) in [
        "emulabzip://sessions/../../etc/passwd",
        "emulabzip://sessions/..",
        "emulabzip://flows/../x",
        "emulabzip://network/flows/abc",
        "emulabzip://network/flows/999999",
        "emulabzip://nothing",
        "file:///etc/passwd",
    ]
    .iter()
    .enumerate()
    {
        let r = s.call(2 + i as u32, "resources/read", json!({ "uri": uri }));
        assert!(r["error"].is_object(), "{uri} should be refused: {r}");
    }
}
