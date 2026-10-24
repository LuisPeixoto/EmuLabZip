//! Talks to `emulabzip mcp` the way an assistant does: JSON-RPC lines over its standard input.

mod common;

use common::Server;
use serde_json::{Value, json};

#[test]
fn lists_every_tool_with_a_description_and_a_schema() {
    let mut s = Server::start_session();
    let tools = s.call(2, "tools/list", json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "list_devices",
        "device_info",
        "list_apps",
        "read_logs",
        "screenshot",
        "perf_snapshot",
        "memory_report",
        "list_files",
        "read_file",
        "list_crashes",
        "get_crash",
        "list_traces",
        "trace_summary",
        "trace_query",
        "record_trace",
        "control_info",
        "control_location",
        "control_appearance",
        "control_status_bar",
        "control_permission",
        "control_notification",
        "open_url",
        "control_battery",
    ] {
        assert!(names.contains(&expected), "{expected} is missing from {names:?}");
    }
    for t in tools {
        assert!(!t["description"].as_str().unwrap_or_default().is_empty(), "{} has no description", t["name"]);
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
    }
    let logs = tools.iter().find(|t| t["name"] == "read_logs").unwrap();
    assert!(logs["inputSchema"]["properties"]["grep"]["description"].as_str().unwrap().contains("case-insensitive"));
}

#[test]
fn answers_without_a_device() {
    let mut s = Server::start_session();
    let devices = s.call(2, "tools/call", json!({ "name": "list_devices", "arguments": {} }));
    assert_eq!(devices["result"]["isError"], false, "{devices}");
    let text = devices["result"]["content"][0]["text"].as_str().unwrap();
    assert!(serde_json::from_str::<Value>(text).unwrap()["devices"].is_array());

    let traces = s.call(3, "tools/call", json!({ "name": "list_traces", "arguments": {} }));
    assert_eq!(traces["result"]["isError"], false);
    assert_eq!(traces["result"]["content"][0]["text"], "[]");
}

#[test]
fn mistakes_come_back_as_tool_errors_the_assistant_can_read() {
    let mut s = Server::start_session();
    let bad_mode = s.call(2, "tools/call", json!({ "name": "control_appearance", "arguments": { "mode": "purple" } }));
    assert_eq!(bad_mode["result"]["isError"], true, "{bad_mode}");
    let missing =
        s.call(3, "tools/call", json!({ "name": "trace_summary", "arguments": { "id": "20260101-000000-aaaa" } }));
    assert_eq!(missing["result"]["isError"], true);
    assert!(missing["result"]["content"][0]["text"].as_str().unwrap().contains("recording"));
    let traversal = s.call(4, "tools/call", json!({ "name": "trace_summary", "arguments": { "id": "../../etc" } }));
    assert_eq!(traversal["result"]["isError"], true);
    let plugin = s.call(6, "tools/call", json!({ "name": "run_plugin", "arguments": { "name": "does-not-exist" } }));
    assert_eq!(plugin["result"]["isError"], true);
    assert!(plugin["result"]["content"][0]["text"].as_str().unwrap().contains("no plugin called"));
    let unknown = s.call(5, "tools/call", json!({ "name": "no_such_tool", "arguments": {} }));
    assert!(unknown["error"].is_object() || unknown["result"]["isError"] == true, "{unknown}");
}
