//! Perfetto's `trace_processor_shell`: downloaded once, then run as a local server that answers SQL
//! about a trace.
//!
//! EmuLabZip does not ship the binary. It fetches the official release the first time a trace is
//! opened, checks it against a SHA-256 pinned here, and keeps it in its data folder.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use emulabzip_core::{Error, Result, data_dir};
use prost::Message;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::process::{Child, Command};

/// The Perfetto release that is downloaded.
pub const VERSION: &str = "v58.2";

/// One platform's archive in the release, with the SHA-256 GitHub publishes for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    pub archive: &'static str,
    pub sha256: &'static str,
    /// The download size in bytes, for telling the user before downloading.
    pub size: u64,
}

const ASSETS: &[(&str, &str, Asset)] = &[
    (
        "macos",
        "x86_64",
        Asset {
            archive: "mac-amd64.zip",
            sha256: "1ec01f5de30fbf3b4e91c2b96508e3f537f0f57d1d1ce4bdf0892c98c4de039c",
            size: 13_160_464,
        },
    ),
    (
        "macos",
        "aarch64",
        Asset {
            archive: "mac-arm64.zip",
            sha256: "9dbd484a32c9833c95cf58ca5ef86d2c9ad4767149054ad4eaa52bdf027d1507",
            size: 11_794_235,
        },
    ),
    (
        "linux",
        "x86_64",
        Asset {
            archive: "linux-amd64.zip",
            sha256: "32c739f71b2d39721afd294c0b038f2499a44a71b5b6bcdd85b83faca0b240b9",
            size: 14_123_443,
        },
    ),
    (
        "linux",
        "aarch64",
        Asset {
            archive: "linux-arm64.zip",
            sha256: "a82bf4111a340a7ea8577bcfd62e014e8e81b9e6a35a3190f5415fb800051ab0",
            size: 13_378_397,
        },
    ),
    (
        "windows",
        "x86_64",
        Asset {
            archive: "windows-amd64.zip",
            sha256: "5a00dbb990b1aa422c818169259ffaf0671e2ff276a51857f30185d8bf9d1409",
            size: 80_941_801,
        },
    ),
];

/// The archive for this computer.
pub fn asset() -> Result<Asset> {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    ASSETS
        .iter()
        .find(|(o, a, _)| *o == os && *a == arch)
        .map(|(_, _, asset)| *asset)
        .ok_or(Error::Unsupported("Analyzing Perfetto traces on this operating system"))
}

pub fn download_url(asset: &Asset) -> String {
    format!("https://github.com/google/perfetto/releases/download/{VERSION}/{}", asset.archive)
}

fn binary_name() -> &'static str {
    if cfg!(windows) { "trace_processor_shell.exe" } else { "trace_processor_shell" }
}

fn install_dir() -> PathBuf {
    data_dir().join("tools").join(format!("perfetto-{VERSION}"))
}

/// Where the processor is installed, if it is.
pub fn installed() -> Option<PathBuf> {
    let dir = install_dir();
    let binary = dir.join(binary_name());
    (binary.is_file() && dir.join(".verified").is_file()).then_some(binary)
}

/// Reports `(downloaded, total)` bytes.
pub type Progress = Arc<dyn Fn(u64, Option<u64>) + Send + Sync>;

fn failed(what: &str, e: impl std::fmt::Display) -> Error {
    Error::failed(format!("{what}: {e}"))
}

/// Downloads `url` into `dest`, returning the SHA-256 of what was written.
fn fetch(url: &str, dest: &Path, progress: &Progress) -> Result<String> {
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(900))).build().into();
    let mut response =
        agent.get(url).call().map_err(|e| failed("Could not download the Perfetto trace processor", e))?;
    let total = response.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    let mut reader = response.body_mut().as_reader();
    let mut file = std::fs::File::create(dest)?;
    let (mut hasher, mut buf, mut done) = (Sha256::new(), vec![0u8; 64 * 1024], 0u64);
    loop {
        let n = reader.read(&mut buf).map_err(|e| failed("The download was interrupted", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        done += n as u64;
        progress(done, total);
    }
    file.flush()?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks the archive against `expected` and extracts the processor into `dir`.
fn verify_and_extract(archive: &Path, actual: &str, expected: &str, dir: &Path) -> Result<PathBuf> {
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(Error::failed(format!(
            "The downloaded file does not match the expected checksum (got {actual}), so it was not used."
        )));
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)
        .map_err(|e| failed("The download is not a valid archive", e))?;
    let name = binary_name();
    let index = (0..zip.len())
        .find(|&i| {
            zip.name_for_index(i)
                .is_some_and(|n| n.as_ref().is_ok_and(|n| n == name || n.ends_with(&format!("/{name}"))))
        })
        .ok_or_else(|| Error::failed(format!("{name} is not in the downloaded archive")))?;
    std::fs::create_dir_all(dir)?;
    let target = dir.join(name);
    let partial = dir.join(format!("{name}.partial"));
    let mut entry = zip.by_index(index).map_err(|e| failed("Could not read the archive", e))?;
    std::io::copy(&mut entry, &mut std::fs::File::create(&partial)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&partial, &target)?;
    std::fs::write(dir.join(".verified"), expected)?;
    Ok(target)
}

/// Makes sure the processor is installed, downloading it when it is not.
pub async fn ensure_installed(progress: Progress) -> Result<PathBuf> {
    if let Some(path) = installed() {
        return Ok(path);
    }
    let asset = asset()?;
    let dir = install_dir();
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&dir)?;
        let archive = dir.join(format!("{}.download", asset.archive));
        let result = fetch(&download_url(&asset), &archive, &progress)
            .and_then(|sha| verify_and_extract(&archive, &sha, asset.sha256, &dir));
        // Whatever happened, the archive is no longer needed.
        let _ = std::fs::remove_file(&archive);
        result
    })
    .await
    .map_err(|e| failed("The download stopped", e))?
}

// ---------------------------------------------------------------------------------------------
// The RPC protocol: protos/perfetto/trace_processor/trace_processor.proto
// ---------------------------------------------------------------------------------------------

#[derive(Clone, PartialEq, Message)]
struct QueryArgs {
    #[prost(string, optional, tag = "1")]
    sql_query: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct QueryResult {
    #[prost(string, repeated, tag = "1")]
    column_names: Vec<String>,
    #[prost(string, optional, tag = "2")]
    error: Option<String>,
    #[prost(message, repeated, tag = "3")]
    batch: Vec<CellsBatch>,
}

#[derive(Clone, PartialEq, Message)]
struct CellsBatch {
    /// 1 null, 2 integer, 3 float, 4 string, 5 blob.
    #[prost(int32, repeated, packed = "true", tag = "1")]
    cells: Vec<i32>,
    #[prost(int64, repeated, packed = "true", tag = "2")]
    varint_cells: Vec<i64>,
    #[prost(double, repeated, packed = "true", tag = "3")]
    float64_cells: Vec<f64>,
    #[prost(bytes = "vec", repeated, tag = "4")]
    blob_cells: Vec<Vec<u8>>,
    /// Every string of the batch, each one followed by a NUL byte.
    #[prost(string, optional, tag = "5")]
    string_cells: Option<String>,
}

/// One value of a result. Blobs are shown by their size.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Cell {
    Null(()),
    Int(i64),
    Float(f64),
    Text(String),
}

impl Cell {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Cell::Int(i) => Some(*i),
            Cell::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Cell::Int(i) => Some(*i as f64),
            Cell::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Cell::Text(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    /// More rows came back than were kept.
    pub truncated: bool,
}

fn decode_result(bytes: &[u8], limit: usize) -> Result<Rows> {
    let result = QueryResult::decode(bytes)
        .map_err(|e| failed("The trace processor sent an answer that could not be read", e))?;
    if let Some(error) = result.error.filter(|e| !e.is_empty()) {
        return Err(Error::failed(error));
    }
    let width = result.column_names.len();
    let mut flat: Vec<Cell> = Vec::new();
    for batch in &result.batch {
        let (mut ints, mut floats, mut blobs) =
            (batch.varint_cells.iter(), batch.float64_cells.iter(), batch.blob_cells.iter());
        let mut strings = batch.string_cells.as_deref().unwrap_or("").split('\0');
        for kind in &batch.cells {
            flat.push(match kind {
                2 => Cell::Int(*ints.next().ok_or_else(|| failed("Truncated answer", "missing integer"))?),
                3 => Cell::Float(*floats.next().ok_or_else(|| failed("Truncated answer", "missing number"))?),
                4 => Cell::Text(strings.next().ok_or_else(|| failed("Truncated answer", "missing text"))?.to_string()),
                5 => Cell::Text(format!("<{} bytes>", blobs.next().map_or(0, Vec::len))),
                _ => Cell::Null(()),
            });
        }
    }
    if width == 0 || flat.is_empty() {
        return Ok(Rows { columns: result.column_names, rows: Vec::new(), truncated: false });
    }
    let truncated = flat.len() / width > limit;
    let rows: Vec<Vec<Cell>> = flat.chunks_exact(width).take(limit).map(<[Cell]>::to_vec).collect();
    Ok(Rows { columns: result.column_names, rows, truncated })
}

// ---------------------------------------------------------------------------------------------
// The process
// ---------------------------------------------------------------------------------------------

/// A running `trace_processor_shell` with one trace loaded. It stops when this is dropped.
pub struct Processor {
    child: Child,
    port: u16,
    agent: ureq::Agent,
}

/// The most rows a query returns to the caller.
pub const ROW_LIMIT: usize = 5_000;

impl Processor {
    /// Starts the processor on a free port of the loopback interface and waits until it has
    /// loaded `trace`.
    pub async fn start(binary: &Path, trace: &Path) -> Result<Processor> {
        let port = {
            let probe = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
            probe.local_addr()?.port()
        };
        let child = Command::new(binary)
            .args(["--httpd", "--http-ip-address", "127.0.0.1", "--http-port", &port.to_string()])
            .arg(trace)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| failed("Could not start the Perfetto trace processor", e))?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut processor = Processor { child, port, agent };

        // The server answers once the trace is loaded.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = processor.child.try_wait()? {
                return Err(Error::failed(format!(
                    "The Perfetto trace processor stopped ({status}). The file may not be a Perfetto trace."
                )));
            }
            let (agent, url) = (processor.agent.clone(), format!("http://127.0.0.1:{port}/status"));
            let ready = tokio::task::spawn_blocking(move || agent.get(&url).call().is_ok_and(|r| r.status() == 200))
                .await
                .unwrap_or(false);
            if ready {
                return Ok(processor);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::failed("The Perfetto trace processor took too long to load the trace."));
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    /// Runs SQL (PerfettoSQL) and returns up to [`ROW_LIMIT`] rows.
    pub async fn query(&self, sql: &str) -> Result<Rows> {
        let body = QueryArgs { sql_query: Some(sql.to_string()) }.encode_to_vec();
        let (agent, url) = (self.agent.clone(), format!("http://127.0.0.1:{}/query", self.port));
        let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let mut response = agent
                .post(&url)
                .content_type("application/x-protobuf")
                .send(&body[..])
                .map_err(|e| failed("The trace processor did not answer", e))?;
            response
                .body_mut()
                .with_config()
                .limit(512 * 1024 * 1024)
                .read_to_vec()
                .map_err(|e| failed("The trace processor's answer could not be read", e))
        })
        .await
        .map_err(|e| failed("The query stopped", e))??;
        decode_result(&bytes, ROW_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn decodes_every_kind_of_cell_from_a_real_answer() {
        let rows = decode_result(include_bytes!("../../tests/fixtures/query-result.bin"), 100).unwrap();
        assert_eq!(rows.columns, ["a", "b", "c", "d", "e"]);
        assert_eq!(
            rows.rows,
            vec![
                vec![
                    Cell::Int(1),
                    Cell::Float(2.5),
                    Cell::Text("hé".into()),
                    Cell::Null(()),
                    Cell::Text("<2 bytes>".into())
                ],
                vec![
                    Cell::Int(7),
                    Cell::Float(0.5),
                    Cell::Text("x".into()),
                    Cell::Null(()),
                    Cell::Text("<0 bytes>".into())
                ],
            ]
        );
        assert!(!rows.truncated);
    }

    #[test]
    fn keeps_only_the_rows_it_was_asked_for() {
        let rows = decode_result(include_bytes!("../../tests/fixtures/query-result.bin"), 1).unwrap();
        assert_eq!((rows.rows.len(), rows.truncated), (1, true));
    }

    #[test]
    fn reports_an_sql_error_with_the_message_from_the_processor() {
        let err = decode_result(include_bytes!("../../tests/fixtures/query-error.bin"), 100).unwrap_err();
        assert!(err.to_string().contains("syntax error near 'nothing'"), "{err}");
    }

    #[test]
    fn rejects_an_answer_that_is_not_protobuf() {
        assert!(decode_result(b"\xff\xff\xff", 10).is_err());
    }

    #[test]
    fn cells_serialize_as_plain_json_values() {
        let json = serde_json::to_string(&vec![Cell::Null(()), Cell::Int(3), Cell::Float(0.5), Cell::Text("x".into())])
            .unwrap();
        assert_eq!(json, r#"[null,3,0.5,"x"]"#);
    }

    #[test]
    fn pins_a_checksum_for_every_supported_platform() {
        for (os, arch, asset) in ASSETS {
            assert_eq!(asset.sha256.len(), 64, "{os}/{arch}");
            assert!(asset.sha256.chars().all(|c| c.is_ascii_hexdigit()), "{os}/{arch}");
            assert!(asset.archive.ends_with(".zip"));
        }
        assert!(download_url(&ASSETS[0].2).starts_with("https://github.com/google/perfetto/releases/download/v"));
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-trace-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn zip_with(path: &Path, entry: &str, contents: &[u8]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        w.start_file(entry, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(contents).unwrap();
        w.finish().unwrap();
    }

    #[test]
    fn extracts_the_processor_only_when_the_checksum_matches() {
        let dir = scratch("extract");
        let archive = dir.join("a.zip");
        zip_with(&archive, &format!("mac-amd64/{}", binary_name()), b"#!/bin/sh\necho hi\n");
        let sha = hex(&Sha256::digest(std::fs::read(&archive).unwrap()));

        let wrong = verify_and_extract(&archive, &"0".repeat(64), &sha, &dir.join("out-wrong"));
        assert!(wrong.unwrap_err().to_string().contains("does not match the expected checksum"));
        assert!(!dir.join("out-wrong").exists(), "nothing is extracted from a file that fails the check");

        let path = verify_and_extract(&archive, &sha, &sha, &dir.join("out")).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"#!/bin/sh\necho hi\n");
        assert!(dir.join("out/.verified").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o111, 0o111, "it must be executable");
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn complains_when_the_archive_has_no_processor() {
        let dir = scratch("empty");
        let archive = dir.join("a.zip");
        zip_with(&archive, "mac-amd64/traceconv", b"x");
        let sha = hex(&Sha256::digest(std::fs::read(&archive).unwrap()));
        let err = verify_and_extract(&archive, &sha, &sha, &dir.join("out")).unwrap_err();
        assert!(err.to_string().contains("is not in the downloaded archive"), "{err}");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn downloads_a_file_and_reports_its_checksum_and_progress() {
        let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let served = body.clone();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut req = [0u8; 1024];
            let _ = std::io::Read::read(&mut s, &mut req);
            write!(s, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", served.len()).unwrap();
            s.write_all(&served).unwrap();
        });
        let dir = scratch("fetch");
        let last = Arc::new(std::sync::Mutex::new((0u64, None)));
        let seen = last.clone();
        let progress: Progress = Arc::new(move |done, total| *seen.lock().unwrap() = (done, total));
        let sha = fetch(&format!("http://127.0.0.1:{port}/x.zip"), &dir.join("x.zip"), &progress).unwrap();
        assert_eq!(sha, hex(&Sha256::digest(&body)));
        assert_eq!(std::fs::read(dir.join("x.zip")).unwrap(), body);
        assert_eq!(*last.lock().unwrap(), (200_000, Some(200_000)));
        std::fs::remove_dir_all(dir).ok();
    }
}
