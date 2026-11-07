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
        "measure_startup",
        "measure_energy",
        "analyze_heap",
        "detect_frameworks",
        "flutter_widget_tree",
        "flutter_frames",
        "flutter_memory",
        "react_tree",
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
        "get_layout",
        "audit_accessibility",
        "list_sessions",
        "session_summary",
        "session_timeline",
        "create_bug_report",
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

/// A finished session in the server's own sessions folder.
fn plant_session(name: &str) {
    let dir = Server::home().join("sessions").join(format!("{name}.ezsession.partial"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("session.json"),
        r#"{"version":1,"tool":"EmuLabZip test","device":{"id":"emulator-5554","name":"Test Pixel","platform":"android","osVersion":"14","model":null},"app":"com.example.app","startedMs":1000,"endedMs":9000,"network":false,"screensEverySecs":null}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("logs.jsonl"),
        r#"{"seq":1,"time":"t","pid":1,"tid":1,"level":"error","tag":"Net","message":"timeout after 30 s","subsystem":null}"#,
    )
    .unwrap();
    let file = Server::home().join("sessions").join(format!("{name}.ezsession"));
    emulabzip_session::format::pack(&dir, &file).unwrap();
}

#[test]
fn sessions_are_listed_and_summarized_from_the_sessions_folder_only() {
    plant_session("run-one");
    let mut s = Server::start_session();
    let list = s.call(2, "tools/call", json!({ "name": "list_sessions", "arguments": {} }));
    assert_eq!(list["result"]["isError"], false, "{list}");
    let entries: Value = serde_json::from_str(list["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(entries.as_array().unwrap().iter().any(|e| e["id"] == "run-one" && e["partial"] == false), "{entries}");

    let summary = s.call(3, "tools/call", json!({ "name": "session_summary", "arguments": { "id": "run-one" } }));
    assert_eq!(summary["result"]["isError"], false, "{summary}");
    let v: Value = serde_json::from_str(summary["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!((v["device"]["name"].as_str(), v["logs"]["errors"].as_u64()), (Some("Test Pixel"), Some(1)));
    assert_eq!(v["topErrors"][0]["message"], "Net: timeout after # s");

    for bad in ["../../etc/passwd", "a/b", "", "nothing-by-this-name"] {
        let r = s.call(4, "tools/call", json!({ "name": "session_summary", "arguments": { "id": bad } }));
        assert_eq!(r["result"]["isError"], true, "{bad}: {r}");
    }
}

#[test]
fn a_bug_report_is_written_to_the_reports_folder() {
    plant_session("report-me");
    let mut s = Server::start_session();
    let r = s.call(
        2,
        "tools/call",
        json!({ "name": "create_bug_report", "arguments": { "session": "report-me", "title": "Login <fails>", "issue": "github:a/b" } }),
    );
    assert_eq!(r["result"]["isError"], false, "{r}");
    let v: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let path = std::path::PathBuf::from(v["path"].as_str().unwrap());
    assert!(path.starts_with(Server::home().join("reports")), "{path:?}");
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.contains("Login &lt;fails&gt;") && html.contains("timeout after 30 s"), "{html}");
    assert!(v["issueUrl"].as_str().unwrap().starts_with("https://github.com/a/b/issues/new?title=Login%20%3Cfails%3E"));
    assert!(v["markdown"].as_str().unwrap().contains("emulabzip-report.html"));
}

#[test]
fn a_bug_report_checks_its_arguments_before_asking_for_a_device() {
    let mut s = Server::start_session();
    for (args, word) in [
        (json!({ "issue": "nope" }), "no tracker named"),
        (json!({ "issue": "github:a" }), "github:owner/repo"),
        (json!({ "session": "x", "app": "a.b" }), "app"),
        (json!({ "session": "../../etc" }), "not a session name"),
        (json!({ "session": "no-such-session" }), "no session called"),
    ] {
        let r = s.call(2, "tools/call", json!({ "name": "create_bug_report", "arguments": args }));
        assert_eq!(r["result"]["isError"], true, "{args}: {r}");
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains(word), "{args}: {text}");
    }
}

fn plant_timeline_session(name: &str) {
    let dir = Server::home().join("sessions").join(format!("{name}.ezsession.partial"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("session.json"),
        r#"{"version":2,"tool":"EmuLabZip test","device":{"id":"emulator-5554","name":"Test Pixel","platform":"android","osVersion":"14","model":null},"app":"com.example.app","startedMs":1000,"endedMs":9000,"network":false,"screensEverySecs":null,"logsDropped":0,"clock":{"offsetMs":0,"tzMinutes":0,"bootEpochMs":0}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("input.jsonl"),
        "{\"tMs\":3000,\"kind\":\"tap\",\"x\":10,\"y\":20,\"x2\":10,\"y2\":20,\"durationMs\":80}\n{\"tMs\":5000,\"kind\":\"swipe\",\"x\":1,\"y\":2,\"x2\":300,\"y2\":2,\"durationMs\":200}\n",
    )
    .unwrap();
    std::fs::write(dir.join("frames.jsonl"), "{\"tMs\":4000,\"durationMs\":120.0}\n").unwrap();
    let file = Server::home().join("sessions").join(format!("{name}.ezsession"));
    emulabzip_session::format::pack(&dir, &file).unwrap();
}

#[test]
fn a_timeline_is_read_from_a_recorded_session() {
    plant_timeline_session("timeline-me");
    let mut s = Server::start_session();
    let r = s.call(2, "tools/call", json!({ "name": "session_timeline", "arguments": { "id": "timeline-me" } }));
    assert_eq!(r["result"]["isError"], false, "{r}");
    let v: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let times: Vec<u64> = v["events"].as_array().unwrap().iter().map(|e| e["tMs"].as_u64().unwrap()).collect();
    assert_eq!(times, vec![2000, 3000, 4000], "oldest first, from the start of the recording: {v}");
    assert_eq!((v["eventsTotal"].as_u64(), v["truncated"].as_bool()), (Some(3), Some(false)));
    assert_eq!(v["events"][0]["lane"], "taps");

    // Lanes and a limit narrow it down.
    let r = s.call(
        3,
        "tools/call",
        json!({ "name": "session_timeline", "arguments": { "id": "timeline-me", "lanes": ["frames", "taps"], "limit": 2 } }),
    );
    let v: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        (v["events"].as_array().unwrap().len(), v["eventsTotal"].as_u64(), v["truncated"].as_bool()),
        (2, Some(3), Some(true))
    );
    let r = s.call(
        4,
        "tools/call",
        json!({ "name": "session_timeline", "arguments": { "id": "timeline-me", "lanes": ["frames"] } }),
    );
    let v: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(v["events"].as_array().unwrap().len(), 1);
}

#[test]
fn a_timeline_checks_its_arguments_before_opening_anything() {
    let mut s = Server::start_session();
    for (args, word) in [
        (json!({ "id": "no-such-session", "lanes": ["nope"] }), "not a lane"),
        (json!({ "id": "../../etc" }), "not a session name"),
        (json!({ "id": "no-such-session" }), "no session called"),
    ] {
        let r = s.call(2, "tools/call", json!({ "name": "session_timeline", "arguments": args }));
        assert_eq!(r["result"]["isError"], true, "{args}: {r}");
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains(word), "{args}: {r}");
    }
}
