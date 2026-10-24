//! The MCP tools against a real emulator. Needs the crash app to have failed on it (see
//! `tools/crash-app`): `EMULABZIP_DEVICE=emulator-5560 cargo test -p emulabzip-cli --test mcp_live -- --ignored`.

mod common;

use common::Server;
use serde_json::{Value, json};

fn text(response: &Value) -> String {
    assert_eq!(response["result"]["isError"], false, "{response}");
    response["result"]["content"][0]["text"].as_str().unwrap().to_string()
}

#[test]
#[ignore = "needs a running emulator where the crash app has failed"]
fn reads_and_controls_a_real_emulator() {
    let device = std::env::var("EMULABZIP_DEVICE").expect("EMULABZIP_DEVICE");
    let mut s = Server::start_session();
    let mut id = 10;
    let mut call = |s: &mut Server, name: &str, mut args: Value| {
        id += 1;
        args["device"] = json!(device);
        s.call(id, "tools/call", json!({ "name": name, "arguments": args }))
    };

    let apps = text(&call(&mut s, "list_apps", json!({ "all": true })));
    assert!(apps.contains("dev.emulabzip.crashy"), "the crash app is installed");

    let crashes: Value = serde_json::from_str(&text(&call(
        &mut s,
        "list_crashes",
        json!({ "app": "dev.emulabzip.crashy", "kind": "native" }),
    )))
    .unwrap();
    let first = crashes[0]["id"].as_str().expect("a native crash is listed");
    let report: Value = serde_json::from_str(&text(&call(&mut s, "get_crash", json!({ "id": first })))).unwrap();
    assert_eq!(report["summary"]["kind"], "nativeCrash");
    assert_eq!(report["raw"], "", "the raw text is only sent when asked for");
    assert!(report["threads"][0]["frames"].as_array().unwrap().len() > 10);

    // The screenshot comes back as an image, not as text.
    let shot = call(&mut s, "screenshot", json!({}));
    assert_eq!(shot["result"]["content"][0]["type"], "image", "{shot}");
    assert_eq!(shot["result"]["content"][0]["mimeType"], "image/png");

    // A control changes the device, and the info tool sees it.
    text(&call(&mut s, "control_appearance", json!({ "mode": "dark" })));
    let info: Value =
        serde_json::from_str(&text(&call(&mut s, "control_info", json!({ "app": "dev.emulabzip.crashy" })))).unwrap();
    assert_eq!(info["appearance"], "dark");
    text(&call(&mut s, "control_appearance", json!({ "mode": "light" })));

    let logs = text(&call(&mut s, "read_logs", json!({ "lines": 20 })));
    assert!(!logs.is_empty());
    let perf: Value = serde_json::from_str(&text(&call(&mut s, "perf_snapshot", json!({ "samples": 2 })))).unwrap();
    assert_eq!(perf.as_array().unwrap().len(), 2);

    let refused = call(&mut s, "open_url", json!({ "url": "file:///etc/passwd" }));
    assert_eq!(refused["result"]["isError"], true, "a link that reads local files is refused");
}
