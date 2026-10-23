//! The contract between platform collectors and their consumers (the CLI and the desktop app).

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::device::{App, Device, InfoSection, Platform, Target};
use crate::error::{Error, Result};
use crate::files::{FileEntry, StorageRoot};
use crate::log::LogEntry;
use crate::memory::{HeapDump, MemoryReport};
use crate::network::{CaInstall, NetworkStatus};
use crate::perf::{PerfOptions, PerfSample};
use crate::stream::Stream;
use crate::trace::{TraceFile, TraceRequest};

/// How a log stream should start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogOptions {
    /// How many recent lines to replay before following new ones. `0` starts from now.
    pub tail: u32,
    /// Include verbose and debug messages. The iOS unified log is very chatty at this level.
    pub verbose: bool,
}

impl Default for LogOptions {
    fn default() -> Self {
        LogOptions { tail: 500, verbose: true }
    }
}

/// A source of devices for one platform: `adb` for Android, `simctl` for the iOS Simulator.
///
/// Methods take the device id (adb serial or simulator UDID). Features a platform does not have
/// return [`Error::Unsupported`], so callers can hide the matching UI.
#[async_trait]
pub trait Backend: Send + Sync {
    fn platform(&self) -> Platform;

    /// Whether the host has the tooling this backend needs (`adb`, `xcrun`).
    async fn is_available(&self) -> bool;

    /// Devices that are currently running or attached.
    async fn devices(&self) -> Result<Vec<Device>>;

    /// Facts about the device, grouped for display.
    async fn info(&self, id: &str) -> Result<Vec<InfoSection>>;

    /// Installed apps, with their process id when running.
    async fn apps(&self, id: &str) -> Result<Vec<App>>;

    /// Follows the device log until the returned stream is dropped.
    async fn logs(&self, id: &str, options: LogOptions) -> Result<Stream<LogEntry>>;

    /// A PNG screenshot of the device screen.
    async fn screenshot(&self, id: &str) -> Result<Vec<u8>>;

    /// Samples CPU, memory, frames and battery until the returned stream is dropped.
    async fn perf(&self, _id: &str, _options: PerfOptions) -> Result<Stream<PerfSample>> {
        Err(Error::Unsupported("Performance sampling"))
    }

    /// A breakdown of the memory used by a running app.
    async fn memory(&self, _id: &str, _app: &str) -> Result<MemoryReport> {
        Err(Error::Unsupported("Memory reports"))
    }

    /// A heap snapshot of a running app (`.hprof` on Android, `.memgraph` on iOS).
    async fn heap_dump(&self, _id: &str, _app: &str) -> Result<HeapDump> {
        Err(Error::Unsupported("Heap dumps"))
    }

    /// The places where an app keeps files.
    async fn storage_roots(&self, _id: &str, _app: &str) -> Result<Vec<StorageRoot>> {
        Err(Error::Unsupported("Storage browsing"))
    }

    /// Lists a directory, given relative to a storage root.
    async fn list_files(&self, _id: &str, _app: &str, _root: &str, _path: &str) -> Result<Vec<FileEntry>> {
        Err(Error::Unsupported("Storage browsing"))
    }

    /// Reads a file, given relative to a storage root.
    async fn read_file(&self, _id: &str, _app: &str, _root: &str, _path: &str) -> Result<Vec<u8>> {
        Err(Error::Unsupported("Storage browsing"))
    }

    /// Whether the device sends its traffic to a proxy.
    async fn network_status(&self, _id: &str) -> Result<NetworkStatus> {
        Err(Error::Unsupported("Network capture"))
    }

    /// Sends the device's HTTP(S) traffic to the proxy listening on `127.0.0.1:port` of this
    /// computer.
    async fn connect_network(&self, _id: &str, _port: u16) -> Result<NetworkStatus> {
        Err(Error::Unsupported("Network capture"))
    }

    /// Undoes [`Backend::connect_network`].
    async fn disconnect_network(&self, _id: &str) -> Result<NetworkStatus> {
        Err(Error::Unsupported("Network capture"))
    }

    /// Makes the device trust the proxy's certificate authority, given as PEM and DER.
    async fn install_ca(&self, _id: &str, _pem: &str, _der: &[u8]) -> Result<CaInstall> {
        Err(Error::Unsupported("Certificate installation"))
    }

    /// The Instruments templates that can be recorded on this platform. Empty when there are none.
    async fn trace_templates(&self, _id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Records a trace and saves it in `out_dir`. It stops early when `stop` turns `true`, keeping
    /// what was recorded so far.
    async fn record_trace(
        &self,
        _id: &str,
        _request: &TraceRequest,
        _out_dir: &Path,
        _stop: tokio::sync::watch::Receiver<bool>,
    ) -> Result<TraceFile> {
        Err(Error::Unsupported("Recording traces"))
    }
}

/// Every available backend behind one handle.
#[derive(Clone, Default)]
pub struct Lab {
    backends: Vec<Arc<dyn Backend>>,
}

impl Lab {
    pub fn new() -> Self {
        Lab::default()
    }

    pub fn with(mut self, backend: impl Backend + 'static) -> Self {
        self.backends.push(Arc::new(backend));
        self
    }

    pub fn backends(&self) -> &[Arc<dyn Backend>] {
        &self.backends
    }

    pub fn backend(&self, platform: Platform) -> Result<Arc<dyn Backend>> {
        self.backends.iter().find(|b| b.platform() == platform).cloned().ok_or(Error::Unsupported(match platform {
            Platform::Android => "Android",
            Platform::Ios => "The iOS Simulator",
        }))
    }

    /// Devices from every backend. A backend that fails (for example because `adb` is missing)
    /// is reported in the second list instead of hiding the devices of the others.
    pub async fn devices(&self) -> (Vec<Device>, Vec<(Platform, Error)>) {
        let mut devices = Vec::new();
        let mut errors = Vec::new();
        for backend in &self.backends {
            if !backend.is_available().await {
                continue;
            }
            match backend.devices().await {
                Ok(mut list) => devices.append(&mut list),
                Err(e) => errors.push((backend.platform(), e)),
            }
        }
        devices.sort_by(|a, b| (a.platform as u8, &a.name).cmp(&(b.platform as u8, &b.name)));
        (devices, errors)
    }

    /// Finds a device by exact id, then by case-insensitive name. With no query, returns the only
    /// connected device.
    pub async fn resolve(&self, query: Option<&str>) -> Result<Device> {
        let (devices, _) = self.devices().await;
        match query {
            None => match devices.len() {
                1 => Ok(devices.into_iter().next().expect("one device")),
                0 => Err(Error::NotFound("no running emulator or simulator was found".into())),
                n => Err(Error::NotFound(format!("{n} devices are running; pick one with --device"))),
            },
            Some(q) => {
                let lower = q.to_lowercase();
                devices
                    .iter()
                    .find(|d| d.id == q)
                    .or_else(|| devices.iter().find(|d| d.name.to_lowercase() == lower))
                    .or_else(|| devices.iter().find(|d| d.name.to_lowercase().contains(&lower)))
                    .cloned()
                    .ok_or_else(|| Error::DeviceNotFound(q.to_string()))
            }
        }
    }

    pub async fn info(&self, target: &Target) -> Result<Vec<InfoSection>> {
        self.backend(target.platform)?.info(&target.id).await
    }

    pub async fn apps(&self, target: &Target) -> Result<Vec<App>> {
        self.backend(target.platform)?.apps(&target.id).await
    }

    pub async fn logs(&self, target: &Target, options: LogOptions) -> Result<Stream<LogEntry>> {
        self.backend(target.platform)?.logs(&target.id, options).await
    }

    pub async fn screenshot(&self, target: &Target) -> Result<Vec<u8>> {
        self.backend(target.platform)?.screenshot(&target.id).await
    }

    pub async fn perf(&self, target: &Target, options: PerfOptions) -> Result<Stream<PerfSample>> {
        self.backend(target.platform)?.perf(&target.id, options).await
    }

    pub async fn memory(&self, target: &Target, app: &str) -> Result<MemoryReport> {
        self.backend(target.platform)?.memory(&target.id, app).await
    }

    pub async fn heap_dump(&self, target: &Target, app: &str) -> Result<HeapDump> {
        self.backend(target.platform)?.heap_dump(&target.id, app).await
    }

    pub async fn storage_roots(&self, target: &Target, app: &str) -> Result<Vec<StorageRoot>> {
        self.backend(target.platform)?.storage_roots(&target.id, app).await
    }

    pub async fn network_status(&self, target: &Target) -> Result<NetworkStatus> {
        self.backend(target.platform)?.network_status(&target.id).await
    }

    pub async fn connect_network(&self, target: &Target, port: u16) -> Result<NetworkStatus> {
        self.backend(target.platform)?.connect_network(&target.id, port).await
    }

    pub async fn disconnect_network(&self, target: &Target) -> Result<NetworkStatus> {
        self.backend(target.platform)?.disconnect_network(&target.id).await
    }

    pub async fn install_ca(&self, target: &Target, pem: &str, der: &[u8]) -> Result<CaInstall> {
        self.backend(target.platform)?.install_ca(&target.id, pem, der).await
    }

    pub async fn trace_templates(&self, target: &Target) -> Result<Vec<String>> {
        self.backend(target.platform)?.trace_templates(&target.id).await
    }

    pub async fn record_trace(
        &self,
        target: &Target,
        request: &TraceRequest,
        out_dir: &Path,
        stop: tokio::sync::watch::Receiver<bool>,
    ) -> Result<TraceFile> {
        crate::trace::check_duration(request.duration_s())
            .map_err(|_| Error::invalid("duration", request.duration_s().to_string()))?;
        self.backend(target.platform)?.record_trace(&target.id, request, out_dir, stop).await
    }

    pub async fn list_files(&self, target: &Target, app: &str, root: &str, path: &str) -> Result<Vec<FileEntry>> {
        if !crate::files::is_safe_relative(path) {
            return Err(Error::invalid("path", path));
        }
        self.backend(target.platform)?.list_files(&target.id, app, root, path).await
    }

    pub async fn read_file(&self, target: &Target, app: &str, root: &str, path: &str) -> Result<Vec<u8>> {
        if !crate::files::is_safe_relative(path) {
            return Err(Error::invalid("path", path));
        }
        self.backend(target.platform)?.read_file(&target.id, app, root, path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{DeviceKind, DeviceState};

    struct Fake(Platform, Vec<&'static str>);

    #[async_trait]
    impl Backend for Fake {
        fn platform(&self) -> Platform {
            self.0
        }
        async fn is_available(&self) -> bool {
            true
        }
        async fn devices(&self) -> Result<Vec<Device>> {
            Ok(self
                .1
                .iter()
                .map(|name| Device {
                    id: format!("id-{name}"),
                    name: name.to_string(),
                    platform: self.0,
                    kind: DeviceKind::Emulator,
                    state: DeviceState::Online,
                    os_version: None,
                    api_level: None,
                    model: None,
                    abi: None,
                })
                .collect())
        }
        async fn info(&self, _: &str) -> Result<Vec<InfoSection>> {
            Ok(vec![])
        }
        async fn apps(&self, _: &str) -> Result<Vec<App>> {
            Ok(vec![])
        }
        async fn logs(&self, _: &str, _: LogOptions) -> Result<Stream<LogEntry>> {
            Err(Error::Unsupported("logs"))
        }
        async fn screenshot(&self, _: &str) -> Result<Vec<u8>> {
            Err(Error::Unsupported("screenshots"))
        }
    }

    #[tokio::test]
    async fn merges_devices_android_first() {
        let lab =
            Lab::new().with(Fake(Platform::Ios, vec!["iPhone 17"])).with(Fake(Platform::Android, vec!["Pixel_9"]));
        let (devices, errors) = lab.devices().await;
        assert!(errors.is_empty());
        assert_eq!(devices.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), vec!["Pixel_9", "iPhone 17"]);
    }

    #[tokio::test]
    async fn resolves_by_id_name_or_fragment() {
        let lab = Lab::new().with(Fake(Platform::Android, vec!["Pixel_9", "Galaxy_S24"]));
        assert_eq!(lab.resolve(Some("id-Pixel_9")).await.unwrap().name, "Pixel_9");
        assert_eq!(lab.resolve(Some("galaxy_s24")).await.unwrap().name, "Galaxy_S24");
        assert_eq!(lab.resolve(Some("galaxy")).await.unwrap().name, "Galaxy_S24");
        assert!(lab.resolve(None).await.is_err());
        assert!(lab.resolve(Some("nexus")).await.is_err());
    }

    #[tokio::test]
    async fn optional_features_default_to_unsupported() {
        let lab = Lab::new().with(Fake(Platform::Android, vec![]));
        let t = Target { platform: Platform::Android, id: "x".into() };
        assert!(matches!(lab.memory(&t, "app").await.unwrap_err(), Error::Unsupported(_)));
        assert!(matches!(lab.storage_roots(&t, "app").await.unwrap_err(), Error::Unsupported(_)));
        assert!(matches!(lab.connect_network(&t, 9090).await.unwrap_err(), Error::Unsupported(_)));
        assert!(matches!(lab.install_ca(&t, "", &[]).await.unwrap_err(), Error::Unsupported(_)));
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let request = TraceRequest::CpuProfile { app: None, duration_s: 5, system_wide: true };
        let err = lab.record_trace(&t, &request, Path::new("/tmp"), rx).await.unwrap_err();
        assert!(matches!(err, Error::Unsupported(_)));
        assert!(lab.trace_templates(&t).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_paths_that_escape_the_root() {
        let lab = Lab::new().with(Fake(Platform::Android, vec![]));
        let t = Target { platform: Platform::Android, id: "x".into() };
        assert!(matches!(lab.read_file(&t, "app", "data", "../../etc").await.unwrap_err(), Error::Invalid { .. }));
    }

    #[tokio::test]
    async fn missing_backend_is_unsupported() {
        let lab = Lab::new().with(Fake(Platform::Android, vec![]));
        let err = lab.screenshot(&Target { platform: Platform::Ios, id: "x".into() }).await.unwrap_err();
        assert!(matches!(err, Error::Unsupported(_)));
    }
}
