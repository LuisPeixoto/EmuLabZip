//! Saving and restoring a simulator app's data container, and stopping the app.
//!
//! The container is an ordinary folder on the Mac, so it is packed and unpacked with the host file system.

use std::path::{Path, PathBuf};

use emulabzip_core::tarball;
use emulabzip_core::{DATA_TAR, Error, Result, StateFiles};

use crate::simctl;

/// What is left out of a saved container: scratch space the app rebuilds.
const SKIP: [&str; 1] = ["tmp"];

/// Stops the app. An app that is not running is not an error.
pub async fn stop(udid: &str, bundle: &str) -> Result<()> {
    match simctl::run(["terminate", udid, bundle]).await {
        Ok(_) => Ok(()),
        Err(Error::Simctl(m))
            if m.to_lowercase().contains("found nothing") || m.to_lowercase().contains("not running") =>
        {
            Ok(())
        }
        // `simctl terminate` also fails when the process was gone already.
        Err(Error::Simctl(_)) => Ok(()),
        Err(e) => Err(e),
    }
}

pub async fn save(container: PathBuf, out_dir: &Path, app_version: Option<String>) -> Result<StateFiles> {
    if !container.is_dir() {
        return Err(Error::failed("The app has no data container yet. Open it once, then save."));
    }
    std::fs::create_dir_all(out_dir)?;
    let data = out_dir.join(DATA_TAR);
    let out = data.clone();
    tokio::task::spawn_blocking(move || tarball::pack_dir(&container, &out, &SKIP))
        .await
        .map_err(|e| Error::failed(e.to_string()))??;
    Ok(StateFiles { data, external: None, app_version })
}

/// Replaces the container's contents with the archive. The archive is checked first, and the folder itself stays, so the
/// app keeps its container.
pub async fn restore(container: PathBuf, data: &Path) -> Result<()> {
    let data = data.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        tarball::check(&data)?;
        if !container.is_dir() {
            return Err(Error::failed("The app has no data container to restore into. Open it once, then restore."));
        }
        tarball::empty_dir(&container, &SKIP)?;
        tarball::unpack(&data, &container)?;
        Ok(())
    })
    .await
    .map_err(|e| Error::failed(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-state-apple-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn a_container_is_saved_changed_and_restored() {
        let dir = scratch("roundtrip");
        let container = dir.join("c");
        std::fs::create_dir_all(container.join("Documents")).unwrap();
        std::fs::create_dir_all(container.join("tmp")).unwrap();
        std::fs::write(container.join("Documents/a.txt"), b"one").unwrap();
        std::fs::write(container.join("tmp/junk"), b"x").unwrap();

        let saved = save(container.clone(), &dir.join("out"), Some("1.0 (1)".into())).await.unwrap();
        assert_eq!(saved.app_version.as_deref(), Some("1.0 (1)"));
        assert!(saved.external.is_none());

        std::fs::write(container.join("Documents/a.txt"), b"two").unwrap();
        std::fs::write(container.join("Documents/new.txt"), b"extra").unwrap();
        restore(container.clone(), &saved.data).await.unwrap();
        assert_eq!(std::fs::read(container.join("Documents/a.txt")).unwrap(), b"one");
        assert!(!container.join("Documents/new.txt").exists(), "what was added is gone");
        assert!(container.join("tmp/junk").exists(), "scratch space is not touched");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn a_missing_container_is_explained() {
        let dir = scratch("missing");
        let err = save(dir.join("nope"), &dir.join("out"), None).await.unwrap_err().to_string();
        assert!(err.contains("no data container"), "{err}");
        std::fs::remove_dir_all(dir).ok();
    }
}
