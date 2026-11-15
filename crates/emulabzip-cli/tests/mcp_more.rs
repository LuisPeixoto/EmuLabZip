//! The second batch of tools: what is refused before a device is looked up, and what works with no device.

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
fn arguments_are_checked_before_a_device_is_looked_up() {
    let mut s = Server::start_session();
    assert!(
        refused(&mut s, 2, "symbolicate_stack", json!({ "stack": "x", "app": "a b", "version": "1" }))
            .contains("package name")
    );
    assert!(
        refused(&mut s, 3, "add_mapping", json!({ "app": "com.a.b", "version": "1", "file": "relative.txt" }))
            .contains("full path")
    );
    assert!(refused(&mut s, 4, "react_console", json!({ "app": "a b" })).contains("package name"));
    assert!(
        refused(&mut s, 5, "react_console", json!({ "app": "com.a.b", "seconds": 600 })).contains("between 1 and 60")
    );
    assert!(refused(&mut s, 6, "device_options", json!({ "platform": "windows" })).contains("android or ios"));
    assert!(
        refused(&mut s, 7, "create_device", json!({ "platform": "x", "name": "n", "image": "i", "model": "m" }))
            .contains("platform")
    );
    assert!(!refused(&mut s, 8, "flow_export", json!({ "flow": "nope", "format": "maestro" })).is_empty());
    assert!(refused(&mut s, 9, "i18n_sweep", json!({ "locales": ["not a tag!"] })).contains("language tag"));
    assert!(refused(&mut s, 10, "i18n_sweep", json!({ "text_sizes": [900] })).contains("between 50 and 300"));
    assert!(refused(&mut s, 11, "i18n_sweep", json!({ "app": "a b" })).contains("package name"));
    assert!(refused(&mut s, 12, "memory_diff", json!({ "app": "a b" })).contains("package name"));
    assert!(refused(&mut s, 13, "visual_capture", json!({ "name": "../x" })).contains("baseline name"));
    assert!(!refused(&mut s, 14, "package_budget", json!({ "file": "/etc/passwd" })).is_empty());
}

#[test]
fn flows_can_be_written_and_exported_without_a_device() {
    let mut s = Server::start_session();
    let name = format!("mcp-more-{}", std::process::id());
    let steps = json!([{ "type": "launch" }, { "type": "wait", "ms": 300 }]);
    let made = call(&mut s, 2, "flow_create", json!({ "name": name, "app": "com.example.app", "steps": steps }));
    if made["result"]["isError"] == true {
        // The step shape is the flow format's; say what it wanted so the test can be fixed rather than silenced.
        panic!("flow_create refused: {}", text(&made));
    }
    assert!(refused(&mut s, 3, "flow_create", json!({ "name": name, "steps": [] })).contains("exists"));
    let script = text(&call(&mut s, 4, "flow_export", json!({ "flow": name, "format": "script" })));
    assert!(script.contains("emulabzip"), "{script}");
    assert!(text(&call(&mut s, 5, "flow_show", json!({ "flow": name }))).contains("com.example.app"));
    let _ = call(&mut s, 6, "list_mappings", json!({}));
}
