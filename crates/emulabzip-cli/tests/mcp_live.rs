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
    // Two example plugins, and only one of them enabled, in the server's own data folder.
    let cli = |args: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_emulabzip"))
            .args(args)
            .env("EMULABZIP_HOME", Server::home())
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    let examples = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/plugins");
    cli(&["plugin", "install", &format!("{examples}/top-processes")]);
    cli(&["plugin", "install", &format!("{examples}/battery-temperature")]);
    cli(&["plugin", "enable", "top-processes"]);
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

    let listed: Value = serde_json::from_str(&text(&call(&mut s, "list_plugins", json!({})))).unwrap();
    let enabled: Vec<(&str, bool)> = listed["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap(), p["enabled"].as_bool().unwrap()))
        .collect();
    assert_eq!(enabled, [("battery-temperature", false), ("top-processes", true)]);
    let ran: Value = serde_json::from_str(&text(&call(
        &mut s,
        "run_plugin",
        json!({ "name": "top-processes", "app": "com.android.systemui" }),
    )))
    .unwrap();
    assert_eq!(ran["events"][0]["type"], "table");
    assert!(ran["events"][0]["rows"].as_array().unwrap().len() >= 5);
    let blocked = call(&mut s, "run_plugin", json!({ "name": "battery-temperature" }));
    assert_eq!(blocked["result"]["isError"], true, "a plugin the user did not enable is refused");
    assert!(blocked["result"]["content"][0]["text"].as_str().unwrap().contains("is not enabled"));
    let metrics: Value =
        serde_json::from_str(&text(&call(&mut s, "run_plugin", json!({ "name": "top-processes" })))).unwrap();
    assert_eq!(metrics["outcome"], "Finished(Some(0))");

    let refused = call(&mut s, "open_url", json!({ "url": "file:///etc/passwd" }));
    assert_eq!(refused["result"]["isError"], true, "a link that reads local files is refused");
}
