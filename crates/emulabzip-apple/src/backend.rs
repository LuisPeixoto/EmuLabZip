//! The iOS Simulator implementation of [`Backend`].

use async_trait::async_trait;
use emulabzip_core::{App, Backend, Device, Error, InfoSection, LogEntry, LogOptions, Platform, Result, Stream};
use tokio::process::Command;

use crate::devices::{self, SimDevice};
use crate::info::{self, Extras};
use crate::{apps, oslog, simctl};

/// iOS simulators, through `xcrun simctl`. Only available on macOS.
#[derive(Debug, Clone, Default)]
pub struct AppleBackend;

impl AppleBackend {
    pub fn new() -> Self {
        AppleBackend
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
}
