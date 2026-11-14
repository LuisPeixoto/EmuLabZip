//! The network tools of `emulabzip mcp`, with no device: the proxy runs on this computer and a local server answers.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use std::sync::Mutex;

use common::Server;
use serde_json::{Value, json};

/// The tests share one data folder, and so one rules file: they take turns.
static TURN: Mutex<()> = Mutex::new(());

/// A server that answers `{"ok": true}` to everything, and returns its port.
fn origin() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let body = r#"{"ok": true}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    port
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// What a device would send through the proxy.
fn through(proxy: u16, origin: u16, path: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", proxy)).unwrap();
    write!(s, "GET http://127.0.0.1:{origin}{path} HTTP/1.1\r\nHost: 127.0.0.1:{origin}\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

fn text(v: &Value) -> &str {
    v["result"]["content"][0]["text"].as_str().unwrap_or_else(|| panic!("no text in {v}"))
}

fn call(s: &mut Server, id: u32, name: &str, args: Value) -> Value {
    s.call(id, "tools/call", json!({ "name": name, "arguments": args }))
}

fn ok(s: &mut Server, id: u32, name: &str, args: Value) -> Value {
    let r = call(s, id, name, args);
    assert_eq!(r["result"]["isError"], false, "{name}: {r}");
    serde_json::from_str(text(&r)).unwrap_or(Value::Null)
}

fn refused(s: &mut Server, id: u32, name: &str, args: Value) -> String {
    let r = call(s, id, name, args);
    assert_eq!(r["result"]["isError"], true, "{name} should be refused: {r}");
    text(&r).to_string()
}

#[test]
fn the_proxy_runs_between_calls_and_what_it_saw_is_there_to_read() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Server::start_session();
    let origin = origin();
    assert_eq!(ok(&mut s, 2, "net_status", json!({}))["running"], false);

    // A mock, so one address is answered by a rule and the other by the server.
    ok(
        &mut s,
        3,
        "net_rules_set",
        json!({ "rules": [{ "name": "teapot", "matcher": { "path": "/mocked" },
            "action": { "type": "mock", "status": 418, "headers": [["content-type", "text/plain"]], "body": "short and stout" } }] }),
    );
    assert_eq!(ok(&mut s, 4, "net_rules_get", json!({}))[0]["name"], "teapot");

    let port = free_port();
    let started = ok(&mut s, 5, "net_start", json!({ "port": port }));
    assert_eq!((started["running"].clone(), started["port"].clone()), (json!(true), json!(port)));
    assert!(through(port, origin, "/hello").contains("\"ok\": true"));
    assert!(through(port, origin, "/mocked").contains("short and stout"));

    // The next call, on the same session, reads what the last one made.
    let flows = ok(&mut s, 6, "net_flows", json!({}));
    assert_eq!(flows["captured"], 2, "{flows}");
    let rows = flows["flows"].as_array().unwrap();
    assert!(rows.iter().any(|f| f["status"] == 418 && f["rule"] == "teapot"), "{rows:?}");
    let only_ok = ok(&mut s, 7, "net_flows", json!({ "status": "2xx", "path": "/hello" }));
    assert_eq!(only_ok["shown"], 1);
    let errors = ok(&mut s, 8, "net_flows", json!({ "status": "4xx" }));
    assert_eq!(errors["shown"], 1);
    // What is new since the last read.
    let next = flows["nextSinceId"].as_u64().unwrap();
    assert_eq!(ok(&mut s, 9, "net_flows", json!({ "since_id": next }))["shown"], 0);
    through(port, origin, "/again");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(ok(&mut s, 10, "net_flows", json!({ "since_id": next }))["shown"], 1);

    // One request in full, with its body decoded and pretty-printed.
    let id = only_ok["flows"][0]["id"].as_u64().unwrap();
    let flow = ok(&mut s, 11, "net_flow", json!({ "id": id }));
    assert_eq!(flow["status"], 200);
    assert!(flow["responseBody"]["text"].as_str().unwrap().contains("\"ok\": true"), "{flow}");
    assert!(refused(&mut s, 12, "net_flow", json!({ "id": 99999 })).contains("net_flows"));

    // Conditions, and a HAR file in the exports folder only.
    let slow = ok(&mut s, 13, "net_conditions", json!({ "preset": "slow-3g" }));
    assert!(
        slow["summary"].as_str().unwrap().to_lowercase().contains("3g")
            || slow["conditions"]["latencyMs"].as_u64().unwrap() > 0
    );
    assert!(refused(&mut s, 14, "net_conditions", json!({ "preset": "warp-speed" })).contains("not a preset"));
    ok(&mut s, 15, "net_conditions", json!({ "off": true }));
    let har = ok(&mut s, 16, "net_export_har", json!({ "name": "run" }));
    let path = std::path::PathBuf::from(har["path"].as_str().unwrap());
    assert!(path.starts_with(Server::home().join("exports")) && path.is_file(), "{path:?}");
    refused(&mut s, 17, "net_export_har", json!({ "name": "../escape" }));
    refused(&mut s, 18, "net_export_har", json!({ "name": "a/b" }));

    // Stopping keeps what was seen, until it is cleared.
    assert_eq!(ok(&mut s, 19, "net_stop", json!({}))["running"], false);
    assert!(ok(&mut s, 20, "net_flows", json!({}))["captured"].as_u64().unwrap() >= 3);
    call(&mut s, 21, "net_clear", json!({}));
    assert_eq!(ok(&mut s, 22, "net_flows", json!({}))["captured"], 0);
}

#[test]
fn rules_that_would_read_or_run_something_the_assistant_was_not_given_are_refused() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Server::start_session();
    let set = |s: &mut Server, id: u32, rule: Value| refused(s, id, "net_rules_set", json!({ "rules": [rule] }));
    let rule = |action: Value| json!({ "name": "r", "matcher": {}, "action": action });
    // A file outside the mocks folder cannot be served to the app, because the app's answer can be read back.
    std::fs::create_dir_all(Server::home().join("mocks")).unwrap();
    let outside = std::env::temp_dir().join(format!("emulabzip-secret-{}", std::process::id()));
    std::fs::write(&outside, "secret").unwrap();
    let why = set(&mut s, 2, rule(json!({ "type": "mapLocal", "path": outside })));
    assert!(why.contains("only files inside"), "{why}");
    let why = set(&mut s, 3, rule(json!({ "type": "mapLocal", "path": "/no/such/file" })));
    assert!(why.contains("does not exist"), "{why}");
    // A script nobody approved does not get into a rule.
    let why = set(&mut s, 4, rule(json!({ "type": "script", "path": "/no/such/script.wasm" })));
    assert!(!why.is_empty());
    // An OpenAPI file has to be a file with the right extension.
    assert!(set(&mut s, 5, rule(json!({ "type": "validate", "spec": "/etc/passwd" }))).contains("not a .yaml"));
    // Garbage is said, with the rule's place.
    assert!(set(&mut s, 6, json!({ "nonsense": true })).contains("rule 1"));
    // A file inside the folder is fine.
    let inside = Server::home().join("mocks").join("hello.txt");
    std::fs::write(&inside, "hi").unwrap();
    ok(&mut s, 7, "net_rules_set", json!({ "rules": [rule(json!({ "type": "mapLocal", "path": inside }))] }));
    ok(&mut s, 8, "net_rules_set", json!({ "rules": [] }));
    std::fs::remove_file(outside).ok();
}

#[test]
fn network_arguments_are_checked_before_anything_starts() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Server::start_session();
    assert!(refused(&mut s, 2, "net_start", json!({ "port": 0 })).contains("port 0"));
    assert!(refused(&mut s, 3, "net_replay", json!({ "session": "../x" })).contains("not a session name"));
    assert!(refused(&mut s, 4, "net_replay", json!({ "session": "a", "har": "b" })).contains("not both"));
    assert!(refused(&mut s, 5, "net_resend", json!({ "id": 1 })).contains("net_flows"));
    assert!(refused(&mut s, 6, "net_resume", json!({ "id": 1 })).contains("no longer held"));
    assert!(refused(&mut s, 7, "openapi_check", json!({ "spec": "relative.yaml" })).contains("full path"));
    assert_eq!(ok(&mut s, 8, "net_status", json!({}))["running"], false);
}
