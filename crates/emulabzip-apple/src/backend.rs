//! The iOS Simulator implementation of [`Backend`].

use async_trait::async_trait;
use emulabzip_core::{
    App, Backend, CaInstall, Device, Error, FileEntry, HeapDump, InfoSection, LogEntry, LogOptions, MemoryReport,
    NetworkStatus, PerfOptions, PerfSample, Platform, Result, StorageRoot, Stream, TraceFile, TraceRequest,
};
use tokio::process::Command;

use crate::devices::{self, SimDevice};
use crate::info::{self, Extras};
use crate::{apps, memory, network, oslog, perf, simctl, storage, trace};

/// iOS simulators, through `xcrun simctl`. Only available on macOS.
#[derive(Debug, Clone)]
pub struct AppleBackend {
    /// Where the Mac's proxy settings are saved while the simulator's traffic is captured.
    state_dir: std::path::PathBuf,
}

impl Default for AppleBackend {
    fn default() -> Self {
        AppleBackend::new()
    }
}

impl AppleBackend {
    pub fn new() -> Self {
        AppleBackend { state_dir: emulabzip_core::data_dir() }
    }

    /// Keeps the saved proxy settings in `dir` instead of the default folder. Used by tests.
    pub fn with_state_dir(dir: impl Into<std::path::PathBuf>) -> Self {
        AppleBackend { state_dir: dir.into() }
    }

    /// Puts the Mac's proxy back the way it was if EmuLabZip left it pointing at itself, for
    /// example after being killed. Returns whether it had to. Call it at startup and at exit.
    pub async fn restore_system_proxy(&self) -> Result<bool> {
        network::restore_leftover(&self.state_dir).await
    }

    /// Every available iOS simulator, including the ones that are shut down.
    pub async fn simulators(&self) -> Result<Vec<SimDevice>> {
        devices::parse(&simctl::run(["list", "devices", "-j"]).await?)
    }

    async fn simulator(&self, udid: &str) -> Result<SimDevice> {
        self.simulators()
            .await?
            .into_iter()
            .find(|d| d.udid == udid)
            .ok_or_else(|| Error::DeviceNotFound(udid.to_string()))
    }

    async fn root_path(&self, udid: &str, app: &str, root: &str) -> Result<std::path::PathBuf> {
        storage::roots(&self.listapps(udid).await?, app)?
            .into_iter()
            .find(|(r, _)| r.id == root)
            .map(|(_, p)| p)
            .ok_or_else(|| Error::invalid("storage root", root))
    }

    /// The installed apps as JSON, keyed by bundle id.
    pub async fn listapps(&self, udid: &str) -> Result<serde_json::Value> {
        simctl::plist_to_json(&simctl::run(["listapps", udid]).await?).await
    }
}

async fn host_output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).kill_on_drop(true).output().await.ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    out.status.success().then(|| text.trim().to_string()).filter(|t| !t.is_empty())
}

#[async_trait]
impl Backend for AppleBackend {
    fn platform(&self) -> Platform {
        Platform::Ios
    }

    async fn is_available(&self) -> bool {
        simctl::is_available().await
    }

    /// Booted simulators only, to match what `adb` reports for Android.
    async fn devices(&self) -> Result<Vec<Device>> {
        Ok(self.simulators().await?.iter().filter(|d| d.state == "Booted").map(SimDevice::to_device).collect())
    }

    async fn info(&self, id: &str) -> Result<Vec<InfoSection>> {
        let sim = self.simulator(id).await?;
        let (runtimes, locale, languages, macos, xcode) = tokio::join!(
            simctl::run(["list", "runtimes", "-j"]),
            simctl::run_text(["spawn", id, "defaults", "read", "-g", "AppleLocale"]),
            simctl::run_text(["spawn", id, "defaults", "read", "-g", "AppleLanguages"]),
            host_output("sw_vers", &["-productVersion"]),
            host_output("xcrun", &["xcodebuild", "-version"]),
        );
        let extras = Extras {
            runtimes: runtimes.ok().and_then(|r| serde_json::from_slice(&r).ok()),
            locale: locale.ok().map(|l| l.trim().to_string()),
            languages: languages.ok(),
            macos,
            xcode: xcode.and_then(|x| x.lines().next().map(str::to_string)),
        };
        Ok(info::build_sections(&sim, &extras))
    }

    async fn apps(&self, id: &str) -> Result<Vec<App>> {
        let (list, running) = tokio::join!(self.listapps(id), simctl::run_text(["spawn", id, "launchctl", "list"]));
        let running = running.map(|r| apps::parse_running(&r)).unwrap_or_default();
        Ok(apps::build_apps(&list?, &running))
    }

    async fn logs(&self, id: &str, options: LogOptions) -> Result<Stream<LogEntry>> {
        oslog::stream(id, options)
    }

    async fn screenshot(&self, id: &str) -> Result<Vec<u8>> {
        // `simctl io screenshot -` writes a file named `-` on recent Xcode versions instead of
        // using stdout, so go through a temporary file.
        let path = std::env::temp_dir().join(format!("emulabzip-{}-{}.png", std::process::id(), id));
        let result =
            simctl::run(["io".as_ref(), id.as_ref(), "screenshot".as_ref(), "--type=png".as_ref(), path.as_os_str()])
                .await;
        let png = tokio::fs::read(&path).await;
        let _ = tokio::fs::remove_file(&path).await;
        result?;
        let png = png?;
        if png.starts_with(b"\x89PNG") { Ok(png) } else { Err(Error::Simctl("the screenshot was empty".into())) }
    }

    async fn perf(&self, id: &str, options: PerfOptions) -> Result<Stream<PerfSample>> {
        Ok(perf::stream(id.to_string(), options))
    }

    async fn memory(&self, id: &str, app: &str) -> Result<MemoryReport> {
        memory::report(id, app).await
    }

    async fn heap_dump(&self, id: &str, app: &str) -> Result<HeapDump> {
        let stamp = host_output("date", &["+%Y%m%d-%H%M%S"]).await.unwrap_or_else(|| "snapshot".into());
        memory::memgraph(id, app, &stamp).await
    }

    async fn storage_roots(&self, id: &str, app: &str) -> Result<Vec<StorageRoot>> {
        Ok(storage::roots(&self.listapps(id).await?, app)?.into_iter().map(|(r, _)| r).collect())
    }

    async fn list_files(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<FileEntry>> {
        storage::list(&self.root_path(id, app, root).await?, path).await
    }

    async fn read_file(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<u8>> {
        storage::read(&self.root_path(id, app, root).await?, path).await
    }

    async fn trace_templates(&self, _id: &str) -> Result<Vec<String>> {
        trace::templates().await
    }

    async fn record_trace(
        &self,
        id: &str,
        request: &TraceRequest,
        out_dir: &std::path::Path,
        stop: tokio::sync::watch::Receiver<bool>,
    ) -> Result<TraceFile> {
        trace::record(id, request, out_dir, stop).await
    }

    async fn network_status(&self, _id: &str) -> Result<NetworkStatus> {
        network::status().await
    }

    async fn connect_network(&self, _id: &str, port: u16) -> Result<NetworkStatus> {
        network::connect(&self.state_dir, port).await
    }

    async fn disconnect_network(&self, _id: &str) -> Result<NetworkStatus> {
        network::disconnect(&self.state_dir).await
    }

    async fn install_ca(&self, id: &str, pem: &str, _der: &[u8]) -> Result<CaInstall> {
        network::install_ca(id, pem).await
    }
}
