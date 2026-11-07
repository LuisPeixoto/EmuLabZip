//! Loading a recording to replay: a HAR file, or the network of a session.

use std::path::Path;

use emulabzip_core::{Error, Result};
use emulabzip_net::ReplaySet;

use crate::format::{EXTENSION, bad};
use crate::reader::Session;

/// The biggest HAR file read. A bigger one is not one the proxy wrote.
const MAX_HAR: u64 = 512 * 1024 * 1024;

/// Reads `path` as a recording: a session (`.ezsession`, or the folder of one) or a HAR file.
///
/// `passthrough` lets requests that are not in the recording reach the server; `timing` makes each answer wait as long
/// as the server did.
pub fn load(path: &Path, passthrough: bool, timing: bool) -> Result<ReplaySet> {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let is_session = path.is_dir() || path.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION));
    let har = if is_session {
        let session = Session::open(path)?;
        session.har.ok_or_else(|| {
            Error::failed(format!(
                "{name} has no recorded network to replay. Record a session with --network, or give a HAR file."
            ))
        })?
    } else {
        let meta =
            std::fs::metadata(path).map_err(|e| Error::failed(format!("could not open {}: {e}", path.display())))?;
        if meta.len() > MAX_HAR {
            return Err(bad("HAR file", format!("{name} is too large")));
        }
        let bytes = std::fs::read(path)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| bad("HAR file", format!("{name} is not a JSON document ({e}). Give a HAR or a session.")))?
    };
    let set = ReplaySet::from_har(&har, &name).map_err(Error::failed)?;
    if set.is_empty() {
        return Err(Error::failed(format!(
            "{name} has no request that can be replayed (failed requests, WebSockets and gRPC calls are left out)."
        )));
    }
    Ok(set.passthrough(passthrough).with_timing(timing))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-replay-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const HAR: &str = r#"{"log":{"entries":[{"startedDateTime":"2026-11-07T12:00:00.000Z","time":10,
        "request":{"method":"GET","url":"https://a.test/x"},"response":{"status":200,"headers":[],"content":{"text":"hi"}}}]}}"#;

    #[test]
    fn a_har_file_is_a_recording() {
        let dir = scratch("har");
        let file = dir.join("capture.har");
        std::fs::write(&file, HAR).unwrap();
        let set = load(&file, false, false).unwrap();
        assert_eq!((set.len(), set.source()), (1, "capture.har"));
        assert!(set.is_strict());
        assert!(!load(&file, true, false).unwrap().is_strict());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn what_cannot_be_replayed_is_explained() {
        let dir = scratch("bad");
        let missing = dir.join("nope.har");
        assert!(load(&missing, false, false).unwrap_err().to_string().contains("could not open"));
        let junk = dir.join("junk.har");
        std::fs::write(&junk, "not json").unwrap();
        assert!(load(&junk, false, false).unwrap_err().to_string().contains("not a JSON"));
        let empty = dir.join("empty.har");
        std::fs::write(&empty, r#"{"log":{"entries":[]}}"#).unwrap();
        assert!(load(&empty, false, false).unwrap_err().to_string().contains("no request that can be replayed"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_session_without_network_says_how_to_record_one() {
        let dir = scratch("nonet");
        let folder = dir.join("run.ezsession.partial");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("session.json"),
            r#"{"version":2,"tool":"t","device":{"id":"x","name":"X","platform":"android","osVersion":null,"model":null},
               "app":null,"startedMs":1,"endedMs":2,"network":false,"screensEverySecs":null,"logsDropped":0}"#,
        )
        .unwrap();
        let error = load(&folder, false, false).unwrap_err().to_string();
        assert!(error.contains("--network"), "{error}");
        std::fs::remove_dir_all(dir).ok();
    }
}
