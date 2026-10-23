//! Running `xcrun simctl`.
//!
//! Arguments are passed straight to the process, never through a shell, so device names and
//! bundle ids cannot inject commands.

use std::ffi::OsStr;
use std::process::Stdio;

use emulabzip_core::{Error, Result};
use tokio::process::{Child, Command};

/// Whether `xcrun simctl` can be used on this host.
pub async fn is_available() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    Command::new("xcrun")
        .args(["--find", "simctl"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}

fn command<I, S>(args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl").args(args).stdin(Stdio::null()).kill_on_drop(true);
    cmd
}

/// Runs `simctl` and returns its standard output.
pub async fn run<I, S>(args: I) -> Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = command(args).output().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            Error::ToolMissing("xcrun", "Install Xcode and run `xcode-select --install` to use the iOS Simulator.")
        }
        _ => e.into(),
    })?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::Simctl(stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("command failed").trim().to_string()))
    }
}

/// Runs `simctl` and returns its standard output as text.
pub async fn run_text<I, S>(args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(String::from_utf8_lossy(&run(args).await?).into_owned())
}

/// Starts a long-running `simctl` command with its standard output piped. The child is killed
/// when it is dropped.
pub fn spawn<I, S>(args: I) -> Result<Child>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(command(args).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?)
}

/// Converts an old-style (OpenStep) property list, which `simctl listapps` prints, into JSON
/// with `plutil`.
pub async fn plist_to_json(plist: &[u8]) -> Result<serde_json::Value> {
    use tokio::io::AsyncWriteExt;

    let mut child = Command::new("plutil")
        .args(["-convert", "json", "-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let input = plist.to_vec();
    let writer = tokio::spawn(async move {
        let _ = stdin.write_all(&input).await;
    });
    let output = child.wait_with_output().await?;
    let _ = writer.await;
    if !output.status.success() {
        return Err(Error::parse("property list", String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| Error::parse("property list", e.to_string()))
}
