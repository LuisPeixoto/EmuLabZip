//! Tools that change or destroy data: not offered by default, and with `--allow-writes` only after the person confirms.

mod common;

use common::Server;
use serde_json::{Value, json};

const GATED: &[&str] = &[
    "restore_state",
    "wipe_device",
    "delete_device",
    "load_emulator_snapshot",
    "delete_emulator_snapshot",
    "write_file",
    "webview_eval",
    "delete_session",
    "remove_flow",
    "remove_mapping",
    "forget_link",
    "visual_update",
];

fn call(s: &mut Server, id: u32, name: &str, args: Value) -> Value {
    s.call(id, "tools/call", json!({ "name": name, "arguments": args }))
}

fn text(v: &Value) -> String {
    v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn names(s: &mut Server) -> Vec<String> {
    let tools = s.call(90, "tools/list", json!({}));
    tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn nothing_destructive_is_offered_unless_writes_are_allowed() {
    let mut s = Server::start_session();
    let offered = names(&mut s);
    for tool in GATED {
        assert!(!offered.iter().any(|n| n == tool), "{tool} is offered by default");
    }
    let r = call(&mut s, 2, "wipe_device", json!({ "device": "x" }));
    assert!(r["error"].is_object() || r["result"]["isError"] == true, "{r}");
}

#[test]
fn with_writes_allowed_every_gated_tool_is_offered_marked_destructive_and_nothing_human_only() {
    let mut s = Server::start_session_with(&["--allow-writes"], Some("accept"));
    let tools = s.call(2, "tools/list", json!({}));
    let list = tools["result"]["tools"].as_array().unwrap();
    for tool in GATED {
        let t = list.iter().find(|t| t["name"] == *tool).unwrap_or_else(|| panic!("{tool} is not offered"));
        assert_eq!(t["annotations"]["destructiveHint"], true, "{tool}");
        assert_eq!(t["annotations"]["readOnlyHint"], false, "{tool}");
    }
    for tool in list {
        let name = tool["name"].as_str().unwrap();
        if !GATED.contains(&name) {
            assert_ne!(tool["annotations"]["destructiveHint"], true, "{name} is destructive but not gated");
        }
    }
    for never in ["approve_script", "enable_plugin", "ci"] {
        assert!(!list.iter().any(|t| t["name"] == never), "{never} must stay with the person");
    }
}

#[test]
fn a_declined_confirmation_changes_nothing() {
    let mut s = Server::start_session_with(&["--allow-writes"], Some("decline"));
    let dir = Server::home().join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("keepme.ezsession");
    std::fs::write(&file, b"x").unwrap();
    let r = call(&mut s, 2, "delete_session", json!({ "id": "keepme" }));
    assert_eq!(r["result"]["isError"], true, "{r}");
    assert!(text(&r).contains("did not confirm"), "{r}");
    assert!(file.exists());
    assert_eq!(s.asked.len(), 1);
    assert!(s.asked[0].contains("keepme"), "{:?}", s.asked);
}

#[test]
fn an_accepted_confirmation_runs_the_change() {
    let mut s = Server::start_session_with(&["--allow-writes"], Some("accept"));
    let dir = Server::home().join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("dropme.ezsession");
    std::fs::write(&file, b"x").unwrap();
    let r = call(&mut s, 2, "delete_session", json!({ "id": "dropme" }));
    assert_eq!(r["result"]["isError"], false, "{r}");
    assert!(!file.exists());
}

#[test]
fn a_client_that_cannot_confirm_is_refused_unless_no_confirm_is_given() {
    let dir = Server::home().join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("quiet.ezsession");
    std::fs::write(&file, b"x").unwrap();
    let mut s = Server::start_session_with(&["--allow-writes"], None);
    let r = call(&mut s, 2, "delete_session", json!({ "id": "quiet" }));
    assert_eq!(r["result"]["isError"], true, "{r}");
    assert!(text(&r).contains("cannot ask"), "{r}");
    assert!(file.exists());
    let mut s = Server::start_session_with(&["--allow-writes", "--no-confirm"], None);
    let r = call(&mut s, 2, "delete_session", json!({ "id": "quiet" }));
    assert_eq!(r["result"]["isError"], false, "{r}");
    assert!(!file.exists());
}

#[test]
fn bad_arguments_are_refused_before_the_person_is_asked() {
    let mut s = Server::start_session_with(&["--allow-writes"], Some("accept"));
    for (tool, args) in [
        ("delete_session", json!({ "id": "../x" })),
        ("write_file", json!({ "app": "a b", "root": "data", "path": "x", "text": "" })),
        ("remove_mapping", json!({ "app": "a b", "version": "1" })),
        ("visual_update", json!({ "name": "../x" })),
        ("load_emulator_snapshot", json!({ "name": "a b" })),
    ] {
        let r = call(&mut s, 2, tool, args);
        assert_eq!(r["result"]["isError"], true, "{tool}: {r}");
    }
    assert!(s.asked.is_empty(), "the person was asked about something invalid: {:?}", s.asked);
}
