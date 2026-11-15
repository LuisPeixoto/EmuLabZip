//! Recording sessions and reading app data over MCP: what is refused before a device is looked up, and the empty states.

mod common;

use common::Server;
use serde_json::{Value, json};

fn call(s: &mut Server, id: u32, name: &str, args: Value) -> Value {
    s.call(id, "tools/call", json!({ "name": name, "arguments": args }))
}

fn text(v: &Value) -> String {
    v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn refused(s: &mut Server, id: u32, name: &str, args: Value) -> String {
    let r = call(s, id, name, args);
    assert_eq!(r["result"]["isError"], true, "{name} should be refused: {r}");
    text(&r)
}

#[test]
fn a_recording_that_cannot_start_is_refused_before_a_device_is_looked_up() {
    let mut s = Server::start_session();
    assert!(refused(&mut s, 2, "session_start", json!({ "name": "../x" })).contains("not a session name"));
    assert!(refused(&mut s, 3, "session_start", json!({ "name": ".hidden" })).contains("not a session name"));
    assert!(refused(&mut s, 4, "session_start", json!({ "app": "not an app" })).contains("package name"));
    assert!(refused(&mut s, 5, "session_start", json!({ "video_seconds": 500 })).contains("between 5 and 60"));
    // A name already taken.
    let dir = Server::home().join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("taken.ezsession"), b"x").unwrap();
    assert!(refused(&mut s, 6, "session_start", json!({ "name": "taken" })).contains("already exists"));
}

#[test]
fn nothing_records_until_asked_and_unknown_sessions_are_said() {
    let mut s = Server::start_session();
    assert!(text(&call(&mut s, 2, "session_status", json!({}))).contains("No session is recording"));
    assert!(refused(&mut s, 3, "session_stop", json!({})).contains("No session is recording"));
    assert!(
        refused(&mut s, 4, "session_compare", json!({ "baseline": "../a", "run": "b" })).contains("not a session name")
    );
    assert!(refused(&mut s, 5, "session_compare", json!({ "baseline": "nope", "run": "nada" })).contains("no session"));
}

#[test]
fn storage_arguments_are_checked_before_a_device_is_looked_up() {
    let mut s = Server::start_session();
    assert!(refused(&mut s, 2, "storage_roots", json!({ "app": "not an app" })).contains("package name"));
    assert!(
        refused(&mut s, 3, "read_database", json!({ "app": "a b", "root": "data", "path": "x.db" }))
            .contains("package name")
    );
    assert!(
        refused(&mut s, 4, "read_preferences", json!({ "app": "a b", "root": "data", "path": "x.xml" }))
            .contains("package name")
    );
}
