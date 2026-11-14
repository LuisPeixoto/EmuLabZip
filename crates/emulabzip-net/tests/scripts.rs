//! The example scripts in tools/rule-scripts, built for WebAssembly, run through the real runtime.

use emulabzip_net::script::{Call, Script, Verdict};

const ADD_HEADER: &[u8] = include_bytes!("../../../tools/rule-scripts/dist/add-header.wasm");
const REDACT: &[u8] = include_bytes!("../../../tools/rule-scripts/dist/redact-json.wasm");
const FLAKY: &[u8] = include_bytes!("../../../tools/rule-scripts/dist/flaky.wasm");

fn headers(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn add_header_adds_a_header_to_what_it_is_shown() {
    let script = Script::from_bytes("add-header.wasm", ADD_HEADER).unwrap();
    assert!(script.on_request && script.on_response);
    let sent = headers(&[("accept", "*/*")]);
    let out = script
        .response(&Call { method: "GET", url: "https://a.test/x", headers: &sent, body: b"", status: Some(200) })
        .unwrap();
    assert_eq!(out.verdict, Verdict::Continue);
    let list = out.headers.unwrap();
    assert_eq!(list[0], ("accept".to_string(), "*/*".to_string()));
    assert_eq!(list[1], ("x-emulabzip-script".to_string(), "add-header: response".to_string()));
}

#[test]
fn redact_json_hides_sensitive_fields_and_leaves_the_rest() {
    let script = Script::from_bytes("redact-json.wasm", REDACT).unwrap();
    assert!(!script.on_request && script.on_response);
    let body = br#"{"user":{"id":7,"email":"ana@example.com","profile":[{"token":"abc"}]},"name":"Ana"}"#;
    let sent = headers(&[("content-type", "application/json")]);
    let out = script
        .response(&Call { method: "GET", url: "https://a.test/me", headers: &sent, body, status: Some(200) })
        .unwrap();
    let changed: serde_json::Value = serde_json::from_slice(&out.body.expect("the body was changed")).unwrap();
    assert_eq!(changed["user"]["email"], "***");
    assert_eq!(changed["user"]["profile"][0]["token"], "***");
    assert_eq!((changed["user"]["id"].as_i64(), changed["name"].as_str()), (Some(7), Some("Ana")));
    // Nothing sensitive, or not JSON: left alone.
    let plain = script
        .response(&Call {
            method: "GET",
            url: "https://a.test/x",
            headers: &sent,
            body: br#"{"a":1}"#,
            status: Some(200),
        })
        .unwrap();
    assert_eq!(plain.body, None);
    let html = headers(&[("content-type", "text/html")]);
    let page = script
        .response(&Call {
            method: "GET",
            url: "https://a.test/x",
            headers: &html,
            body: br#"{"email":"x"}"#,
            status: Some(200),
        })
        .unwrap();
    assert_eq!(page.body, None);
}

#[test]
fn flaky_fails_the_same_addresses_every_time() {
    let script = Script::from_bytes("flaky.wasm", FLAKY).unwrap();
    let mut verdicts = Vec::new();
    for i in 0..30 {
        let url = format!("https://a.test/item/{i}");
        let ask = || script.request(&Call { method: "GET", url: &url, headers: &[], body: b"", status: None }).unwrap();
        let (first, second) = (ask(), ask());
        assert_eq!(
            first.verdict, second.verdict,
            "{url}: a script keeps nothing, so the same address gives the same answer"
        );
        if first.verdict == Verdict::Replace {
            assert_eq!(first.status, Some(503));
            assert!(first.message.as_deref().unwrap().contains("flaky"));
        }
        verdicts.push(first.verdict);
    }
    let failing = verdicts.iter().filter(|v| **v == Verdict::Replace).count();
    assert!((4..=16).contains(&failing), "about a third fail: {failing} of 30");
}
