//! Opens a trace in the Perfetto UI without uploading it anywhere.
//!
//! The UI is a web page on `ui.perfetto.dev`. It can load a trace from a URL, so we serve the file
//! once on the loopback interface, with the CORS headers the page needs, and hand out the link.
//! The server answers only that one file, and stops after it was sent or after a timeout.

use std::path::{Path, PathBuf};
use std::time::Duration;

use emulabzip_core::{Error, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const ORIGIN: &str = "https://ui.perfetto.dev";

/// How long the UI has to come and get the file.
pub const WAIT: Duration = Duration::from_secs(120);

/// A running one-shot server and the link that makes the UI load from it.
pub struct Handoff {
    pub url: String,
    pub port: u16,
    task: tokio::task::JoinHandle<()>,
}

impl Handoff {
    /// Resolves when the file was served or the wait ran out.
    pub async fn finished(mut self) {
        (&mut self.task).await.ok();
    }
}

impl Drop for Handoff {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Serves `trace` once on an ephemeral loopback port.
pub async fn serve(trace: &Path) -> Result<Handoff> {
    let name = trace
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::invalid("trace file", trace.display().to_string()))?
        .to_string();
    if !trace.is_file() {
        return Err(Error::NotFound(trace.display().to_string()));
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let path = trace.to_path_buf();
    let route = format!("/{}", encode(&name));
    let url = format!("{ORIGIN}/#!/?url=http://127.0.0.1:{port}{route}");
    let task = tokio::spawn(async move {
        let _ = tokio::time::timeout(WAIT, accept_until_served(listener, path, route)).await;
    });
    Ok(Handoff { url, port, task })
}

async fn accept_until_served(listener: TcpListener, path: PathBuf, route: String) {
    while let Ok((stream, _)) = listener.accept().await {
        if handle(stream, &path, &route).await.unwrap_or(false) {
            return;
        }
    }
}

/// Answers one request. True when it was the one that delivered the file.
async fn handle(mut stream: TcpStream, path: &Path, route: &str) -> std::io::Result<bool> {
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < 16 * 1024 {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        head.extend_from_slice(&buf[..n]);
    }
    let head = String::from_utf8_lossy(&head);
    let mut line = head.lines().next().unwrap_or_default().split_whitespace();
    let (method, target) = (line.next().unwrap_or_default(), line.next().unwrap_or_default());
    let cors = format!(
        "Access-Control-Allow-Origin: {ORIGIN}\r\nAccess-Control-Allow-Private-Network: true\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nVary: Origin\r\n"
    );
    match (method, target) {
        ("OPTIONS", _) => {
            stream
                .write_all(
                    format!("HTTP/1.1 204 No Content\r\n{cors}Content-Length: 0\r\nConnection: close\r\n\r\n")
                        .as_bytes(),
                )
                .await?;
            Ok(false)
        }
        ("GET", t) if t == route => {
            let body = tokio::fs::read(path).await?;
            stream
                .write_all(format!("HTTP/1.1 200 OK\r\n{cors}Content-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes())
                .await?;
            stream.write_all(&body).await?;
            stream.shutdown().await.ok();
            Ok(true)
        }
        _ => {
            stream
                .write_all(
                    format!("HTTP/1.1 404 Not Found\r\n{cors}Content-Length: 0\r\nConnection: close\r\n\r\n")
                        .as_bytes(),
                )
                .await?;
            Ok(false)
        }
    }
}

/// Percent-encodes everything but the characters that are safe in a URL path.
fn encode(name: &str) -> String {
    name.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn request(port: u16, raw: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(raw.as_bytes()).await.unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).await.unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    fn trace() -> (tempdir::Dir, PathBuf) {
        let dir = tempdir::Dir::new();
        let path = dir.0.join("my trace.pftrace");
        std::fs::write(&path, b"PERFETTO").unwrap();
        (dir, path)
    }

    /// A scratch directory that cleans up after itself.
    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Dir {
                let p = std::env::temp_dir().join(
                    format!("emulabzip-ui-{}-{:?}", std::process::id(), std::thread::current().id())
                        .replace(['(', ')'], ""),
                );
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).ok();
            }
        }
    }

    #[test]
    fn encodes_names_for_a_url() {
        assert_eq!(encode("my trace (1).pftrace"), "my%20trace%20%281%29.pftrace");
    }

    #[tokio::test]
    async fn serves_the_file_once_with_the_headers_the_ui_needs() {
        let (_dir, path) = trace();
        let handoff = serve(&path).await.unwrap();
        assert!(handoff.url.starts_with("https://ui.perfetto.dev/#!/?url=http://127.0.0.1:"));
        assert!(handoff.url.ends_with("/my%20trace.pftrace"));

        let preflight = request(handoff.port, "OPTIONS /my%20trace.pftrace HTTP/1.1\r\nHost: x\r\n\r\n").await;
        assert!(
            preflight.starts_with("HTTP/1.1 204")
                && preflight.contains("Access-Control-Allow-Origin: https://ui.perfetto.dev")
        );
        assert!(preflight.contains("Access-Control-Allow-Private-Network: true"));

        let other = request(handoff.port, "GET /etc/passwd HTTP/1.1\r\nHost: x\r\n\r\n").await;
        assert!(other.starts_with("HTTP/1.1 404"), "only the trace is served: {other}");

        let got = request(handoff.port, "GET /my%20trace.pftrace HTTP/1.1\r\nHost: x\r\n\r\n").await;
        assert!(got.starts_with("HTTP/1.1 200") && got.ends_with("PERFETTO"));
        assert!(got.contains("Access-Control-Allow-Origin: https://ui.perfetto.dev"));

        tokio::time::timeout(Duration::from_secs(5), handoff.finished()).await.expect("stops after serving the file");
    }

    #[tokio::test]
    async fn refuses_a_file_that_does_not_exist() {
        assert!(serve(Path::new("/nonexistent/x.pftrace")).await.is_err());
    }
}
