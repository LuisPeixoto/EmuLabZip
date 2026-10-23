//! Runs the client against a tiny in-process ADB server that speaks the host protocol.

use emulabzip_adb::AdbClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn read_request(s: &mut TcpStream) -> String {
    let mut len = [0u8; 4];
    s.read_exact(&mut len).await.unwrap();
    let len = usize::from_str_radix(std::str::from_utf8(&len).unwrap(), 16).unwrap();
    let mut buf = vec![0u8; len];
    s.read_exact(&mut buf).await.unwrap();
    String::from_utf8(buf).unwrap()
}

async fn okay_with(s: &mut TcpStream, body: &str) {
    s.write_all(format!("OKAY{:04x}{body}", body.len()).as_bytes()).await.unwrap();
}

/// Answers one connection per request, the way the real server does.
async fn serve(listener: TcpListener) {
    loop {
        let (mut s, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            let req = read_request(&mut s).await;
            match req.as_str() {
                "host:version" => okay_with(&mut s, "0029").await,
                "host:devices-l" => {
                    okay_with(&mut s, "emulator-5554 device product:sdk model:Pixel_9 device:emu64a transport_id:1\n")
                        .await
                }
                "host:transport:emulator-5554" => {
                    s.write_all(b"OKAY").await.unwrap();
                    let service = read_request(&mut s).await;
                    s.write_all(b"OKAY").await.unwrap();
                    let out: &[u8] = match service.as_str() {
                        "shell:getprop ro.build.version.sdk" => b"35\n",
                        "exec:screencap -p" => b"\x89PNG\r\n\x1a\n",
                        _ => b"/system/bin/sh: unknown: not found\n",
                    };
                    s.write_all(out).await.unwrap();
                }
                other if other.starts_with("host:transport:") => {
                    let msg = format!("device '{}' not found", &other["host:transport:".len()..]);
                    s.write_all(format!("FAIL{:04x}{msg}", msg.len()).as_bytes()).await.unwrap();
                }
                _ => {
                    let msg = "unknown host service";
                    s.write_all(format!("FAIL{:04x}{msg}", msg.len()).as_bytes()).await.unwrap();
                }
            }
        });
    }
}

async fn client() -> AdbClient {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener));
    AdbClient::with_addr(addr)
}

#[tokio::test]
async fn reads_the_server_version() {
    assert_eq!(client().await.version().await.unwrap(), 41);
}

#[tokio::test]
async fn lists_devices() {
    let devices = client().await.devices().await.unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].serial, "emulator-5554");
    assert_eq!(devices[0].model.as_deref(), Some("Pixel 9"));
}

#[tokio::test]
async fn runs_shell_and_exec_commands() {
    let c = client().await;
    assert_eq!(c.shell("emulator-5554", "getprop ro.build.version.sdk").await.unwrap(), "35\n");
    assert!(c.exec("emulator-5554", "screencap -p").await.unwrap().starts_with(b"\x89PNG"));
}

#[tokio::test]
async fn unknown_devices_are_reported_as_not_found() {
    let err = client().await.shell("emulator-9999", "true").await.unwrap_err();
    assert_eq!(err.to_string(), "device `emulator-9999` is not connected");
}

#[tokio::test]
async fn a_missing_server_is_a_clear_error() {
    // Bind and drop to get a port that is very likely closed.
    let addr = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap();
    let err = AdbClient::with_addr(addr).version().await.unwrap_err();
    assert!(err.to_string().contains("no adb server"), "{err}");
}
