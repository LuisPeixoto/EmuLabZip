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

/// Keeps this process's standard handles from being inherited by the programs it starts.
///
/// On Windows a child inherits every inheritable handle, not only the ones it is given as its own
/// standard streams. The adb server that `adb start-server` leaves behind would then keep the pipe
/// this process writes to open for as long as it runs, and whoever reads that pipe (a script, a
/// test, another program) would never see the end of it.
#[cfg(windows)]
fn hide_std_handles() {
    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};

    for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: plain Win32 calls on handles the process owns; a failure only means the handle
        // stays inheritable, as it was.
        unsafe {
            let handle = GetStdHandle(id);
            if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

#[cfg(not(windows))]
fn hide_std_handles() {}

/// How long `adb start-server` may take.
const START_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Runs `adb start-server` and waits for it to finish.
///
/// The server stays running after the command returns, and it inherits the command's output. If
/// that output were a pipe, waiting for the pipe to close would wait for the server to end (it
/// never does on Windows), so the output goes to a file instead.
pub async fn start_server() -> Result<()> {
    let adb = find_adb().ok_or(Error::ToolMissing(
        "adb",
        "Install the Android SDK platform tools, or set ANDROID_HOME or EMULABZIP_ADB.",
    ))?;
    tracing::info!(adb = %adb.display(), "starting the adb server");
    hide_std_handles();
    let log = env::temp_dir().join(format!("emulabzip-adb-start-{}.log", std::process::id()));
    let file = std::fs::File::create(&log)?;
    let status = tokio::time::timeout(
        START_TIMEOUT,
        tokio::process::Command::new(&adb)
            .arg("start-server")
            .stdin(std::process::Stdio::null())
            .stdout(file.try_clone()?)
            .stderr(file)
            .kill_on_drop(true)
            .status(),
    )
    .await
    .map_err(|_| Error::Adb("`adb start-server` did not finish in 30 seconds".into()))??;
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    if status.success() { Ok(()) } else { Err(Error::Adb(format!("`adb start-server` failed: {}", said.trim()))) }
}
