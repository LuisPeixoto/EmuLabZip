//! A client for `emulabzip mcp`, shared by the MCP tests.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

pub struct Server {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    lines: mpsc::Receiver<String>,
}

impl Server {
    pub fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_emulabzip"))
            .arg("mcp")
            .env("EMULABZIP_HOME", std::env::temp_dir().join(format!("emulabzip-mcp-{}", std::process::id())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let out = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Server { child, stdin, lines }
    }

    pub fn call(&mut self, id: u32, method: &str, params: Value) -> Value {
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.stdin, "{request}").unwrap();
        loop {
            let line = self.lines.recv_timeout(Duration::from_secs(60)).expect("the server answers");
            let value: Value = serde_json::from_str(&line).expect("every line of stdout is JSON");
            if value["id"] == id {
                return value;
            }
        }
    }

    pub fn notify(&mut self, method: &str) {
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "method": method })).unwrap();
    }

    pub fn start_session() -> Server {
        let mut s = Server::start();
        let hello = s.call(1, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }));
        assert!(hello["result"]["capabilities"]["tools"].is_object(), "{hello}");
        assert!(hello["result"]["instructions"].as_str().unwrap().contains("list_devices"));
        s.notify("notifications/initialized");
        s
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
