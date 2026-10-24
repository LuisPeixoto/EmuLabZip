//! Runs the `emulabzip` binary itself, with a private data folder and no device.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_emulabzip"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn ca_path_creates_the_certificate_with_a_private_key() {
    let home = scratch("ca");
    let out = bin().args(["ca", "path"]).env("EMULABZIP_HOME", &home).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    assert!(path.starts_with(&home), "{path:?}");
    let pem = std::fs::read_to_string(&path).unwrap();
    assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let key = std::fs::metadata(home.join("emulabzip-ca-key.pem")).unwrap();
        assert_eq!(key.permissions().mode() & 0o777, 0o600);
    }

    // The same certificate is reused the next time, so devices keep trusting it.
    let again = bin().args(["ca", "path"]).env("EMULABZIP_HOME", &home).output().unwrap();
    assert_eq!(out.stdout, again.stdout);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), pem);
    std::fs::remove_dir_all(home).ok();
}

/// One Ethernet + IPv4 + TCP packet, with the checksums left at zero.
fn tcp_packet(from_client: bool, seq: u32, payload: &[u8]) -> Vec<u8> {
    let (sport, dport, src, dst) = if from_client {
        (50000u16, 80u16, [10, 0, 2, 15], [93, 184, 216, 34])
    } else {
        (80, 50000, [93, 184, 216, 34], [10, 0, 2, 15])
    };
    let mut p = vec![0u8; 12];
    p.extend_from_slice(&[0x08, 0x00]);
    let total = (20 + 20 + payload.len()) as u16;
    p.extend_from_slice(&[0x45, 0]);
    p.extend_from_slice(&total.to_be_bytes());
    p.extend_from_slice(&[0, 0, 0, 0, 64, 6, 0, 0]);
    p.extend_from_slice(&src);
    p.extend_from_slice(&dst);
    p.extend_from_slice(&sport.to_be_bytes());
    p.extend_from_slice(&dport.to_be_bytes());
    p.extend_from_slice(&seq.to_be_bytes());
    p.extend_from_slice(&[0, 0, 0, 0, 0x50, 0x18, 0xff, 0xff, 0, 0, 0, 0]);
    p.extend_from_slice(payload);
    p
}

/// A classic little-endian capture with Ethernet frames, one microsecond-stamped packet each.
fn pcap(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes());
    out.extend_from_slice(&[2, 0, 4, 0]);
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&65535u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    for (i, data) in packets.iter().enumerate() {
        out.extend_from_slice(&(1_700_000_000u32 + i as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }
    out
}

#[test]
fn net_import_reads_a_capture_and_writes_a_har() {
    let home = scratch("import");
    let request = b"GET /hello HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Type: text/plain\r\n\r\nhello";
    let file = home.join("capture.pcap");
    std::fs::write(&file, pcap(&[tcp_packet(true, 1000, request), tcp_packet(false, 5000, response)])).unwrap();
    let har = home.join("out.har");

    let out = bin()
        .args(["net", "import"])
        .arg(&file)
        .arg("--json")
        .arg("--har")
        .arg(&har)
        .env("EMULABZIP_HOME", &home)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let flow: serde_json::Value = serde_json::from_str(stdout.lines().next().expect("a flow")).unwrap();
    assert_eq!(
        (flow["method"].as_str(), flow["status"].as_u64(), flow["host"].as_str()),
        (Some("GET"), Some(200), Some("example.com"))
    );
    assert_eq!(flow["source"], "pcap");

    let har: serde_json::Value = serde_json::from_slice(&std::fs::read(&har).unwrap()).unwrap();
    assert_eq!(har["log"]["entries"][0]["response"]["content"]["text"], "hello");
    std::fs::remove_dir_all(home).ok();
}

#[test]
fn net_import_explains_a_file_that_is_not_a_capture() {
    let home = scratch("import-bad");
    let file = home.join("notes.txt");
    std::fs::write(&file, "GET / HTTP/1.1").unwrap();
    let out = bin().args(["net", "import"]).arg(&file).env("EMULABZIP_HOME", &home).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a pcap"), "{}", String::from_utf8_lossy(&out.stderr));
    std::fs::remove_dir_all(home).ok();
}

#[test]
fn net_refuses_a_rules_file_it_cannot_read() {
    let home = scratch("rules-bad");
    let file = home.join("rules.json");
    std::fs::write(&file, "{ not json").unwrap();
    // The rules are read before anything else, so no device is looked for.
    let out = bin().args(["net", "--no-connect", "--rules"]).arg(&file).env("EMULABZIP_HOME", &home).output().unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("not valid JSON"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::remove_dir_all(home).ok();
}

/// Capturing needs `kill -INT` to stop the command the way Ctrl+C does.
#[cfg(unix)]
mod capture {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{bin, scratch};

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    /// Serves `hello` to every request, on a thread of its own.
    fn upstream() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 5\r\nconnection: close\r\n\r\nhello",
                );
            }
        });
        port
    }

    #[test]
    fn net_records_a_request_and_writes_a_har() {
        let home = scratch("net");
        let har = home.join("capture.har");
        let (proxy_port, upstream_port) = (free_port(), upstream());

        let child = bin()
            .args(["net", "--no-connect", "--json", "--port", &proxy_port.to_string(), "--har"])
            .arg(&har)
            .env("EMULABZIP_HOME", &home)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        // Wait for the proxy to accept connections.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut client = loop {
            match TcpStream::connect(("127.0.0.1", proxy_port)) {
                Ok(c) => break c,
                Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => panic!("the proxy never started: {e}"),
            }
        };
        write!(
            client,
            "GET http://127.0.0.1:{upstream_port}/hello HTTP/1.1\r\nhost: 127.0.0.1:{upstream_port}\r\nconnection: close\r\n\r\n"
        )
        .unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 200") && reply.ends_with("hello"), "{reply}");
        std::thread::sleep(Duration::from_millis(300));

        // Ctrl+C: the command stops, prints what it saw and writes the HAR.
        Command::new("kill").args(["-INT", &child.id().to_string()]).status().unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

        let stdout = String::from_utf8_lossy(&out.stdout);
        let flow: serde_json::Value = serde_json::from_str(stdout.lines().next().expect("one flow on stdout")).unwrap();
        assert_eq!((flow["method"].as_str(), flow["status"].as_u64()), (Some("GET"), Some(200)));
        assert_eq!(flow["path"], "/hello");

        let har: serde_json::Value = serde_json::from_slice(&std::fs::read(&har).unwrap()).unwrap();
        assert_eq!(har["log"]["entries"][0]["response"]["content"]["text"], "hello");
        assert!(String::from_utf8_lossy(&out.stderr).contains("Saved 1 requests"));
        std::fs::remove_dir_all(home).ok();
    }

    #[test]
    fn net_applies_the_mocks_of_a_rules_file() {
        let home = scratch("mock");
        let rules = home.join("rules.json");
        std::fs::write(
            &rules,
            r#"[{"name":"stub","matcher":{"path":"/hello"},"action":{"type":"mock","status":418,"body":"teapot"}},
                {"name":"hold","action":{"type":"breakpoint","request":true}}]"#,
        )
        .unwrap();
        let (proxy_port, upstream_port) = (free_port(), upstream());
        let child = bin()
            .args(["net", "--no-connect", "--json", "--port", &proxy_port.to_string(), "--rules"])
            .arg(&rules)
            .env("EMULABZIP_HOME", &home)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut client = loop {
            match TcpStream::connect(("127.0.0.1", proxy_port)) {
                Ok(c) => break c,
                Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => panic!("the proxy never started: {e}"),
            }
        };
        write!(client, "GET http://127.0.0.1:{upstream_port}/hello HTTP/1.1\r\nhost: 127.0.0.1:{upstream_port}\r\nconnection: close\r\n\r\n").unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 418") && reply.ends_with("teapot"), "{reply}");
        std::thread::sleep(Duration::from_millis(300));
        Command::new("kill").args(["-INT", &child.id().to_string()]).status().unwrap();
        let out = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("hold") && stderr.contains("needs the app"), "{stderr}");
        let flow: serde_json::Value =
            serde_json::from_str(String::from_utf8_lossy(&out.stdout).lines().next().unwrap()).unwrap();
        assert_eq!((flow["mocked"].as_bool(), flow["rule"].as_str()), (Some(true), Some("stub")));
        std::fs::remove_dir_all(home).ok();
    }
}
