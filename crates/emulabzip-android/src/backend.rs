//! The Android implementation of [`Backend`], on top of the ADB host protocol.

use async_trait::async_trait;
use emulabzip_adb::{AdbClient, DeviceEntry};
use emulabzip_core::{
    App, Backend, Device, DeviceKind, DeviceState, Error, FileEntry, HeapDump, InfoSection, LogEntry, LogOptions,
    MemoryReport, NetworkStatus, PerfOptions, PerfSample, Platform, Result, StorageRoot, Stream, TraceFile,
    TraceRequest,
};
use emulabzip_core::{CaInstall, Control, ControlInfo, CrashReport, CrashSummary, LayoutSnapshot};
use tokio::task::JoinSet;

use crate::props::Props;
use crate::{apps, cert, controls, crashes, heapdump, info, layout, logcat, meminfo, network, perf, storage, trace};

/// Android emulators and devices, through the local ADB server.
#[derive(Debug, Clone, Default)]
pub struct AndroidBackend {
    adb: AdbClient,
}

impl AndroidBackend {
    pub fn new() -> Self {
        AndroidBackend { adb: AdbClient::new() }
    }

    pub fn with_client(adb: AdbClient) -> Self {
        AndroidBackend { adb }
    }

    pub fn adb(&self) -> &AdbClient {
        &self.adb
    }

    /// Turns an adb device entry into a [`Device`], reading its properties when it is online.
    async fn describe(adb: AdbClient, entry: DeviceEntry) -> Device {
        let props = if entry.is_online() {
            adb.shell(&entry.serial, "getprop").await.map(|out| Props::parse(&out)).unwrap_or_default()
        } else {
            Props::default()
        };
        device_from(&entry, &props)
    }
}

/// Combines what `adb devices -l` and `getprop` say about a device.
pub fn device_from(entry: &DeviceEntry, props: &Props) -> Device {
    let state = match entry.state.as_str() {
        "device" if !props.is_empty() && !props.is_booted() => DeviceState::Booting,
        "device" => DeviceState::Online,
        "unauthorized" | "authorizing" => DeviceState::Unauthorized,
        s if s.starts_with("no permissions") => DeviceState::Unauthorized,
        _ => DeviceState::Offline,
    };
    let kind = if entry.is_emulator() || props.is_emulator() { DeviceKind::Emulator } else { DeviceKind::Physical };
    let name = props
        .avd_name()
        .map(|n| n.replace('_', " "))
        .or_else(|| props.get("ro.product.model").map(str::to_string))
        .or_else(|| entry.model.clone())
        .unwrap_or_else(|| entry.serial.clone());
    Device {
        id: entry.serial.clone(),
        name,
        platform: Platform::Android,
        kind,
        state,
        os_version: props.get("ro.build.version.release").map(str::to_string),
        api_level: props.api_level(),
        model: props.get("ro.product.model").map(str::to_string).or_else(|| entry.model.clone()),
        abi: props.get("ro.product.cpu.abi").map(str::to_string),
    }
}

#[async_trait]
impl Backend for AndroidBackend {
    fn platform(&self) -> Platform {
        Platform::Android
    }

    async fn is_available(&self) -> bool {
        self.adb.ping().await
    }

    async fn devices(&self) -> Result<Vec<Device>> {
        let entries = self.adb.devices().await?;
        let mut set = JoinSet::new();
        for (i, entry) in entries.into_iter().enumerate() {
            let adb = self.adb.clone();
            set.spawn(async move { (i, AndroidBackend::describe(adb, entry).await) });
        }
        let mut devices: Vec<(usize, Device)> = set.join_all().await;
        devices.sort_by_key(|(i, _)| *i);
        Ok(devices.into_iter().map(|(_, d)| d).collect())
    }

    async fn info(&self, id: &str) -> Result<Vec<InfoSection>> {
        let out = self.adb.shell(id, &info::command()).await?;
        Ok(info::build_sections(id, &info::RawInfo::split(&out)))
    }

    async fn apps(&self, id: &str) -> Result<Vec<App>> {
        let out = self.adb.shell(id, &apps::command()).await?;
        Ok(apps::build_apps(&out))
    }

    async fn logs(&self, id: &str, options: LogOptions) -> Result<Stream<LogEntry>> {
        let socket = self.adb.shell_stream(id, &logcat::command(options)).await?;
        Ok(logcat::stream(socket))
    }

    async fn screenshot(&self, id: &str) -> Result<Vec<u8>> {
        let png = self.adb.exec(id, "screencap -p").await?;
        if png.starts_with(b"\x89PNG") {
            Ok(png)
        } else {
            Err(Error::Adb(format!("screencap failed: {}", String::from_utf8_lossy(&png).trim())))
        }
    }

    async fn layout(&self, id: &str) -> Result<LayoutSnapshot> {
        layout::snapshot(&self.adb, id).await
    }

    async fn perf(&self, id: &str, options: PerfOptions) -> Result<Stream<PerfSample>> {
        if let Some(app) = &options.app
            && !emulabzip_adb::is_package_name(app)
        {
            return Err(Error::invalid("package name", app.clone()));
        }
        Ok(perf::stream(self.adb.clone(), id.to_string(), options))
    }

    async fn memory(&self, id: &str, app: &str) -> Result<MemoryReport> {
        if !emulabzip_adb::is_package_name(app) {
            return Err(Error::invalid("package name", app));
        }
        let out = self.adb.shell(id, &emulabzip_adb::command(["dumpsys", "meminfo", app])).await?;
        meminfo::parse(app, &out)
    }

    async fn heap_dump(&self, id: &str, app: &str) -> Result<HeapDump> {
        let stamp = self.adb.shell(id, "date +%Y%m%d-%H%M%S").await?;
        heapdump::dump(&self.adb, id, app, stamp.trim()).await
    }

    async fn storage_roots(&self, id: &str, app: &str) -> Result<Vec<StorageRoot>> {
        storage::roots(&self.adb, id, app).await
    }

    async fn list_files(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<FileEntry>> {
        storage::list(&self.adb, id, app, root, path).await
    }

    async fn read_file(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<u8>> {
        storage::read(&self.adb, id, app, root, path).await
    }

    async fn record_trace(
        &self,
        id: &str,
        request: &TraceRequest,
        out_dir: &std::path::Path,
        stop: tokio::sync::watch::Receiver<bool>,
    ) -> Result<TraceFile> {
        trace::record(&self.adb, id, request, out_dir, stop).await
    }

    async fn crashes(&self, id: &str, app: Option<&str>) -> Result<Vec<CrashSummary>> {
        crashes::list(&self.adb, id, app).await
    }

    async fn crash(&self, id: &str, crash_id: &str) -> Result<CrashReport> {
        crashes::get(&self.adb, id, crash_id).await
    }

    async fn controls(&self, id: &str, app: Option<&str>) -> Result<ControlInfo> {
        controls::info(&self.adb, id, app).await
    }

    async fn control(&self, id: &str, control: &Control) -> Result<()> {
        controls::apply(&self.adb, id, control).await
    }

    async fn network_status(&self, id: &str) -> Result<NetworkStatus> {
        network::status(&self.adb, id).await
    }

    async fn connect_network(&self, id: &str, port: u16) -> Result<NetworkStatus> {
        network::connect(&self.adb, id, port).await
    }

    async fn disconnect_network(&self, id: &str) -> Result<NetworkStatus> {
        network::disconnect(&self.adb, id).await
    }

    async fn install_ca(&self, id: &str, pem: &str, der: &[u8]) -> Result<CaInstall> {
        let name = format!("{}.0", cert::subject_hash_old(der)?);
        network::install_ca(&self.adb, id, pem, der, &name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(serial: &str, state: &str) -> DeviceEntry {
        DeviceEntry {
            serial: serial.into(),
            state: state.into(),
            product: None,
            model: Some("sdk gphone64 arm64".into()),
            device: None,
            transport_id: None,
        }
    }

    #[test]
    fn names_emulators_after_their_avd() {
        let props = Props::parse(
            "[ro.boot.qemu.avd_name]: [Pixel_9]\n[sys.boot_completed]: [1]\n[ro.build.version.release]: [15]\n[ro.build.version.sdk]: [35]\n",
        );
        let d = device_from(&entry("emulator-5554", "device"), &props);
        assert_eq!(d.name, "Pixel 9");
        assert_eq!(d.kind, DeviceKind::Emulator);
        assert_eq!(d.state, DeviceState::Online);
        assert_eq!(d.os_label(), "Android 15 (API 35)");
    }

    #[test]
    fn reports_booting_devices() {
        let props = Props::parse("[ro.product.model]: [Pixel 8]\n[sys.boot_completed]: []\n");
        let d = device_from(&entry("37121FDJH00", "device"), &props);
        assert_eq!(d.state, DeviceState::Booting);
        assert_eq!(d.kind, DeviceKind::Physical);
        assert_eq!(d.name, "Pixel 8");
    }

    #[test]
    fn maps_unreachable_states() {
        assert_eq!(device_from(&entry("x", "unauthorized"), &Props::default()).state, DeviceState::Unauthorized);
        assert_eq!(device_from(&entry("x", "offline"), &Props::default()).state, DeviceState::Offline);
        assert_eq!(device_from(&entry("x", "offline"), &Props::default()).name, "sdk gphone64 arm64");
    }
}
