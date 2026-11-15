//! A client for `emulabzip mcp`, shared by the MCP tests.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

pub struct Server {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    lines: mpsc::Receiver<String>,
    /// How this client answers a request to confirm: `accept` or `decline`. None means it cannot be asked.
    answer: Option<&'static str>,
    /// What the server asked the person to confirm.
    pub asked: Vec<String>,
}

impl Server {
    /// The data folder the server runs with.
    pub fn home() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("emulabzip-mcp-{}", std::process::id()))
    }

    #[allow(dead_code)]
    pub fn start() -> Server {
        Server::start_with(&[])
    }

    pub fn start_with(args: &[&str]) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_emulabzip"))
            .arg("mcp")
            .args(args)
            .env("EMULABZIP_HOME", Server::home())
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
        Server { child, stdin: Some(stdin), lines, answer: None, asked: Vec::new() }
    }

    pub fn call(&mut self, id: u32, method: &str, params: Value) -> Value {
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.stdin.as_mut().unwrap(), "{request}").unwrap();
        loop {
            let line = self.lines.recv_timeout(Duration::from_secs(60)).expect("the server answers");
            let value: Value = serde_json::from_str(&line).expect("every line of stdout is JSON");
            if value["method"] == "elicitation/create" {
                self.asked.push(value["params"]["message"].as_str().unwrap_or_default().to_string());
                let result = match self.answer {
                    Some("accept") => json!({ "action": "accept", "content": { "confirm": true } }),
                    _ => json!({ "action": "decline" }),
                };
                writeln!(
                    self.stdin.as_mut().unwrap(),
                    "{}",
                    json!({ "jsonrpc": "2.0", "id": value["id"], "result": result })
                )
                .unwrap();
                continue;
            }
            if value["id"] == id && value.get("method").is_none() {
                return value;
            }
        }
    }

    /// Closes standard input and waits for the server to finish, the way a client that disconnects does.
    #[allow(dead_code)]
    pub fn finish(mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }

    pub fn notify(&mut self, method: &str) {
        writeln!(self.stdin.as_mut().unwrap(), "{}", json!({ "jsonrpc": "2.0", "method": method })).unwrap();
    }

    #[allow(dead_code)]
    pub fn start_session() -> Server {
        Server::start_session_with(&[], None)
    }

    /// A session with the server started with `args`. `answer` makes the client able to confirm: `accept` or `decline`.
    pub fn start_session_with(args: &[&str], answer: Option<&'static str>) -> Server {
        let mut s = Server::start_with(args);
        s.answer = answer;
        let capabilities = if answer.is_some() { json!({ "elicitation": { "form": {} } }) } else { json!({}) };
        let hello = s.call(1, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": capabilities, "clientInfo": { "name": "test", "version": "0" } }));
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
