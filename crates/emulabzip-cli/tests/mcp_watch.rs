//! The watch tools of `emulabzip mcp`: what is refused before any device is looked up, and the empty lists.

mod common;

use common::Server;
use serde_json::{Value, json};

fn call(s: &mut Server, id: u32, name: &str, args: Value) -> Value {
    s.call(id, "tools/call", json!({ "name": name, "arguments": args }))
}

fn text(v: &Value) -> String {
    v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

#[test]
fn a_watch_that_cannot_be_started_is_refused_before_a_device_is_looked_up() {
    let mut s = Server::start_session();
    let refused = |s: &mut Server, id: u32, args: Value| {
        let r = call(s, id, "watch_start", args);
        assert_eq!(r["result"]["isError"], true, "{r}");
        text(&r)
    };
    assert!(refused(&mut s, 2, json!({ "kind": "telepathy" })).contains("is not a kind of watch"));
    assert!(refused(&mut s, 3, json!({ "kind": "react_console" })).contains("needs `app`"));
    assert!(refused(&mut s, 4, json!({ "kind": "logs", "app": "not an app" })).contains("package name"));
    assert!(refused(&mut s, 5, json!({ "kind": "logs", "level": "loud" })).contains("level must be"));
    assert!(refused(&mut s, 6, json!({ "kind": "webview_console", "socket": "a b" })).contains("socket name"));
}

#[test]
fn there_are_no_watches_until_one_is_started_and_unknown_ones_are_said() {
    let mut s = Server::start_session();
    let list = call(&mut s, 2, "watch_list", json!({}));
    assert_eq!(list["result"]["isError"], false, "{list}");
    assert_eq!(serde_json::from_str::<Value>(&text(&list)).unwrap(), json!([]));
    for (id, name) in [(3, "watch_read"), (4, "watch_stop")] {
        let r = call(&mut s, id, name, json!({ "id": 42 }));
        assert_eq!(r["result"]["isError"], true, "{name}: {r}");
        assert!(text(&r).contains("no watch has the id 42"), "{}", text(&r));
    }
}
