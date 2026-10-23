//! Framing for the ADB host protocol.
//!
//! Every request is a four digit hexadecimal length followed by the payload, for example
//! `000chost:version`. The server answers `OKAY`, or `FAIL` followed by a length-prefixed message.
//! After a successful `host:transport:<serial>` the same socket talks to the device, and after
//! `shell:` or `exec:` it carries the raw output of the command.

use std::net::SocketAddr;
use std::time::Duration;

use emulabzip_core::{Error, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// One socket to the ADB server.
#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub async fn open(addr: SocketAddr) -> std::io::Result<Self> {
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await.map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out connecting to the adb server")
        })??;
        stream.set_nodelay(true)?;
        Ok(Connection { stream })
    }

    /// Sends a request and waits for `OKAY`.
    pub async fn request(&mut self, payload: &str) -> Result<()> {
        tracing::trace!(payload, "adb request");
        self.stream.write_all(encode(payload)?.as_bytes()).await?;
        self.read_status().await
    }

    async fn read_status(&mut self) -> Result<()> {
        let mut status = [0u8; 4];
        self.stream.read_exact(&mut status).await?;
        match &status {
            b"OKAY" => Ok(()),
            b"FAIL" => {
                let message = self.read_hex_prefixed().await?;
                Err(Error::Adb(String::from_utf8_lossy(&message).into_owned()))
            }
            other => Err(Error::Adb(format!("unexpected reply `{}`", String::from_utf8_lossy(other)))),
        }
    }

    /// Reads a payload preceded by a four digit hexadecimal length.
    pub async fn read_hex_prefixed(&mut self) -> Result<Vec<u8>> {
        let mut len = [0u8; 4];
        self.stream.read_exact(&mut len).await?;
        let len = parse_hex_len(&len)?;
        let mut buf = vec![0u8; len];
        self.stream.read_exact(&mut buf).await?;
        Ok(buf)
    }

    /// Reads until the server closes the socket.
    pub async fn read_to_end(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.stream.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    pub fn into_inner(self) -> TcpStream {
        self.stream
    }

    pub fn stream(&mut self) -> &mut TcpStream {
        &mut self.stream
    }
}

/// Encodes a request with its length prefix.
pub fn encode(payload: &str) -> Result<String> {
    if payload.len() > 0xffff {
        return Err(Error::invalid("adb request", format!("{} bytes long", payload.len())));
    }
    Ok(format!("{:04x}{payload}", payload.len()))
}

pub fn parse_hex_len(bytes: &[u8; 4]) -> Result<usize> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| usize::from_str_radix(s, 16).ok())
        .ok_or_else(|| Error::Adb(format!("bad length prefix `{}`", String::from_utf8_lossy(bytes))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_with_a_hex_length() {
        assert_eq!(encode("host:version").unwrap(), "000chost:version");
        assert_eq!(encode("").unwrap(), "0000");
    }

    #[test]
    fn parses_hex_lengths() {
        assert_eq!(parse_hex_len(b"001f").unwrap(), 31);
        assert_eq!(parse_hex_len(b"FFFF").unwrap(), 65535);
        assert!(parse_hex_len(b"zz00").is_err());
    }

    #[test]
    fn rejects_oversized_requests() {
        assert!(encode(&"x".repeat(70_000)).is_err());
    }
}
