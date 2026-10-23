//! Finding the ADB server and, when it is not running, the `adb` binary that starts it.

use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use emulabzip_core::{Error, Result};

pub const DEFAULT_PORT: u16 = 5037;

/// The address of the ADB server, honouring the same environment variables as `adb` itself:
/// `ADB_SERVER_SOCKET=tcp:host:port` and `ANDROID_ADB_SERVER_PORT`.
pub fn server_addr() -> SocketAddr {
    if let Ok(socket) = env::var("ADB_SERVER_SOCKET")
        && let Some(addr) = parse_server_socket(&socket)
    {
        return addr;
    }
    let port = env::var("ANDROID_ADB_SERVER_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn parse_server_socket(value: &str) -> Option<SocketAddr> {
    let rest = value.strip_prefix("tcp:")?;
    let (host, port) = rest.rsplit_once(':').unwrap_or(("127.0.0.1", rest));
    let host = if host.is_empty() || host == "localhost" { "127.0.0.1" } else { host };
    format!("{host}:{port}").parse().ok()
}

/// Locates the `adb` executable: `$EMULABZIP_ADB`, the `PATH`, then the usual SDK locations.
pub fn find_adb() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "adb.exe" } else { "adb" };

    if let Some(p) = env::var_os("EMULABZIP_ADB").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    if let Some(path) = env::var_os("PATH")
        && let Some(p) = env::split_paths(&path).map(|dir| dir.join(exe)).find(|p| p.is_file())
    {
        return Some(p);
    }
    sdk_roots().into_iter().map(|root| root.join("platform-tools").join(exe)).find(|p| p.is_file())
}

/// Candidate Android SDK directories, most specific first.
pub fn sdk_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> =
        ["ANDROID_HOME", "ANDROID_SDK_ROOT"].iter().filter_map(env::var_os).map(PathBuf::from).collect();
    if let Some(home) = home_dir() {
        if cfg!(target_os = "macos") {
            roots.push(home.join("Library/Android/sdk"));
        } else if cfg!(windows) {
            if let Some(local) = env::var_os("LOCALAPPDATA") {
                roots.push(Path::new(&local).join("Android").join("Sdk"));
            }
        } else {
            roots.push(home.join("Android/Sdk"));
        }
    }
    roots
}

fn home_dir() -> Option<PathBuf> {
    env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Runs `adb start-server` and waits for it to finish.
pub async fn start_server() -> Result<()> {
    let adb = find_adb().ok_or(Error::ToolMissing(
        "adb",
        "Install the Android SDK platform tools, or set ANDROID_HOME or EMULABZIP_ADB.",
    ))?;
    tracing::info!(adb = %adb.display(), "starting the adb server");
    let output = tokio::process::Command::new(&adb).arg("start-server").output().await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Adb(format!("`adb start-server` failed: {}", String::from_utf8_lossy(&output.stderr).trim())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adb_server_socket() {
        assert_eq!(parse_server_socket("tcp:5038"), Some("127.0.0.1:5038".parse().unwrap()));
        assert_eq!(parse_server_socket("tcp:localhost:5039"), Some("127.0.0.1:5039".parse().unwrap()));
        assert_eq!(parse_server_socket("tcp:10.0.0.2:5037"), Some("10.0.0.2:5037".parse().unwrap()));
        assert_eq!(parse_server_socket("local:/tmp/adb"), None);
    }
}
