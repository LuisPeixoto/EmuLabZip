//! The high-level client used by the Android backend.

use std::io::ErrorKind;
use std::net::SocketAddr;

use emulabzip_core::{Error, Result, Stream};
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;

use crate::devices::{DeviceEntry, parse_devices};
use crate::protocol::Connection;
use crate::server;
use crate::sync::{self, RemoteEntry, RemoteStat};

/// Talks to the ADB server over TCP. Cheap to clone: every call opens its own socket, the same
/// way the `adb` command does.
#[derive(Debug, Clone)]
pub struct AdbClient {
    addr: SocketAddr,
    autostart: bool,
}

impl Default for AdbClient {
    fn default() -> Self {
        AdbClient::new()
    }
}

impl AdbClient {
    /// A client for the server named by the environment, or `127.0.0.1:5037`. The server is
    /// started with `adb start-server` when it is not running.
    pub fn new() -> Self {
        AdbClient { addr: server::server_addr(), autostart: true }
    }

    /// A client for an explicit address that never tries to start a server. Used by tests.
    pub fn with_addr(addr: SocketAddr) -> Self {
        AdbClient { addr, autostart: false }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    async fn connect(&self) -> Result<Connection> {
        match Connection::open(self.addr).await {
            Ok(c) => Ok(c),
            Err(e) if self.autostart && e.kind() == ErrorKind::ConnectionRefused => {
                server::start_server().await?;
                Connection::open(self.addr).await.map_err(Error::from)
            }
            Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
                Err(Error::Adb(format!("no adb server is listening on {}", self.addr)))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Whether a server is reachable, or can be started.
    pub async fn ping(&self) -> bool {
        self.version().await.is_ok()
    }

    /// The protocol version of the running server (41 for current platform tools).
    pub async fn version(&self) -> Result<u32> {
        let mut c = self.connect().await?;
        c.request("host:version").await?;
        let body = c.read_hex_prefixed().await?;
        let text = String::from_utf8_lossy(&body);
        u32::from_str_radix(text.trim(), 16).map_err(|_| Error::parse("adb version", text))
    }

    pub async fn devices(&self) -> Result<Vec<DeviceEntry>> {
        let mut c = self.connect().await?;
        c.request("host:devices-l").await?;
        let body = c.read_hex_prefixed().await?;
        Ok(parse_devices(&String::from_utf8_lossy(&body)))
    }

    /// Pushes the full device list every time a device appears, disappears or changes state.
    pub async fn track_devices(&self) -> Result<Stream<Vec<DeviceEntry>>> {
        let mut c = self.connect().await?;
        c.request("host:track-devices-l").await?;
        Ok(Stream::spawn(4, |tx| async move {
            while let Ok(body) = c.read_hex_prefixed().await {
                if tx.send(parse_devices(&String::from_utf8_lossy(&body))).await.is_err() {
                    break;
                }
            }
        }))
    }

    /// A connection already switched to the device, ready for a service request.
    async fn transport(&self, serial: &str) -> Result<Connection> {
        let mut c = self.connect().await?;
        c.request(&format!("host:transport:{serial}")).await.map_err(|e| match e {
            Error::Adb(msg) if msg.contains("not found") => Error::DeviceNotFound(serial.to_string()),
            other => other,
        })?;
        Ok(c)
    }

    /// Runs a shell command and returns its output (stdout and stderr together) as text.
    ///
    /// Build `command` with [`crate::quote::command`] when it contains values from outside.
    pub async fn shell(&self, serial: &str, command: &str) -> Result<String> {
        let bytes = self.exec_service(serial, &format!("shell:{command}")).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Runs a command without a shell's line translation, for binary output such as
    /// `screencap -p`.
    pub async fn exec(&self, serial: &str, command: &str) -> Result<Vec<u8>> {
        self.exec_service(serial, &format!("exec:{command}")).await
    }

    async fn exec_service(&self, serial: &str, service: &str) -> Result<Vec<u8>> {
        let mut c = self.transport(serial).await?;
        c.request(service).await?;
        c.read_to_end().await
    }

    /// Starts a long-running shell command (`logcat`, `top`) and hands back the raw socket.
    /// Closing the socket ends the command on the device.
    pub async fn shell_stream(&self, serial: &str, command: &str) -> Result<TcpStream> {
        let mut c = self.transport(serial).await?;
        c.request(&format!("shell:{command}")).await?;
        Ok(c.into_inner())
    }

    /// Opens a file sync session.
    pub async fn sync(&self, serial: &str) -> Result<SyncSession> {
        let mut c = self.transport(serial).await?;
        c.request("sync:").await?;
        Ok(SyncSession { conn: c })
    }

    /// Lists a remote directory in one call.
    pub async fn list(&self, serial: &str, path: &str) -> Result<Vec<RemoteEntry>> {
        let mut s = self.sync(serial).await?;
        let entries = s.list(path).await;
        s.close().await;
        entries
    }

    /// Copies a remote file into `out` in one call.
    pub async fn pull<W: AsyncWrite + Unpin>(&self, serial: &str, path: &str, out: &mut W) -> Result<u64> {
        let mut s = self.sync(serial).await?;
        let n = s.pull(path, out).await;
        s.close().await;
        n
    }
}

/// An open `sync:` connection. Several operations can share it.
#[derive(Debug)]
pub struct SyncSession {
    conn: Connection,
}

impl SyncSession {
    pub async fn list(&mut self, path: &str) -> Result<Vec<RemoteEntry>> {
        sync::list(self.conn.stream(), path).await
    }

    pub async fn stat(&mut self, path: &str) -> Result<RemoteStat> {
        sync::stat(self.conn.stream(), path).await
    }

    pub async fn pull<W: AsyncWrite + Unpin>(&mut self, path: &str, out: &mut W) -> Result<u64> {
        sync::pull(self.conn.stream(), path, out).await
    }

    pub async fn close(mut self) {
        let _ = sync::quit(self.conn.stream()).await;
    }
}
