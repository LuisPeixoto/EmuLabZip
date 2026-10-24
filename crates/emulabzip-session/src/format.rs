//! What a session holds, and how it is laid out on disk.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use emulabzip_core::{Device, Error, Platform, Result};
use serde::{Deserialize, Serialize};

/// The version of the layout. Readers refuse a newer one.
pub const VERSION: u32 = 1;
/// The extension of a finished session.
pub const EXTENSION: &str = "ezsession";

pub const META: &str = "session.json";
pub const LOGS: &str = "logs.jsonl";
pub const PERF: &str = "perf.jsonl";
pub const NETWORK: &str = "network.har";
pub const CRASHES: &str = "crashes";
pub const SCREENS: &str = "screens";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMeta {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub os_version: Option<String>,
    pub model: Option<String>,
}

impl From<&Device> for DeviceMeta {
    fn from(d: &Device) -> Self {
        DeviceMeta {
            id: d.id.clone(),
            name: d.name.clone(),
            platform: d.platform,
            os_version: d.os_version.clone(),
            model: d.model.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub version: u32,
    /// The EmuLabZip that wrote it.
    pub tool: String,
    pub device: DeviceMeta,
    /// The app the recording was about, when one was named.
    pub app: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub started_ms: u64,
    /// Not set while recording, so a session that was cut short can be told apart.
    pub ended_ms: Option<u64>,
    /// Whether the requests of the device went through the proxy.
    pub network: bool,
    pub screens_every_secs: Option<u64>,
    /// Log lines past the limit that were not kept.
    #[serde(default)]
    pub logs_dropped: u64,
}

impl Meta {
    pub fn duration_ms(&self) -> Option<u64> {
        self.ended_ms.map(|end| end.saturating_sub(self.started_ms))
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub(crate) fn bad(what: &'static str, detail: impl Into<String>) -> Error {
    Error::parse(what, detail)
}

/// Appends one JSON value per line, and flushes on request, so little is lost if the run is killed.
pub(crate) struct JsonlWriter {
    out: BufWriter<File>,
}

impl JsonlWriter {
    pub fn create(path: &Path) -> Result<Self> {
        Ok(JsonlWriter { out: BufWriter::new(File::create(path)?) })
    }

    pub fn write(&mut self, value: &impl Serialize) -> Result<()> {
        serde_json::to_writer(&mut self.out, value).map_err(|e| bad("a session line", e.to_string()))?;
        self.out.write_all(b"\n")?;
        Ok(())
    }

    pub fn flush(&mut self) {
        let _ = self.out.flush();
    }
}

/// The folder a session is recorded into: `run.ezsession` is recorded in `run.ezsession.partial`.
pub fn work_dir(out: &Path) -> PathBuf {
    let mut name = out.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".partial");
    out.with_file_name(name)
}

/// Zips the folder into `out` and removes the folder. The file is written next to its final name and
/// moved into place, so a half-written file never has the real name.
pub fn pack(dir: &Path, out: &Path) -> Result<()> {
    let tmp = out.with_extension("ezsession.tmp");
    {
        let mut zip = zip::ZipWriter::new(BufWriter::new(File::create(&tmp)?));
        add_dir(&mut zip, dir, dir)?;
        zip.finish().map_err(|e| bad("the session archive", e.to_string()))?.flush()?;
    }
    fs::rename(&tmp, out)?;
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn add_dir(zip: &mut zip::ZipWriter<BufWriter<File>>, root: &Path, dir: &Path) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            add_dir(zip, root, &path)?;
            continue;
        }
        let name = path
            .strip_prefix(root)
            .map_err(|e| bad("a session path", e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        // Screenshots are compressed already.
        let method =
            if name.ends_with(".png") { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        zip.start_file(name, zip::write::SimpleFileOptions::default().compression_method(method))
            .map_err(|e| bad("the session archive", e.to_string()))?;
        std::io::copy(&mut File::open(&path)?, zip)?;
    }
    Ok(())
}
