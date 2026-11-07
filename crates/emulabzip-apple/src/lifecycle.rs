//! Making, starting, stopping, erasing and deleting iOS simulators, with `simctl`.

use std::time::Duration;

use emulabzip_core::{Choice, CreateDevice, DeviceOptions, Error, Result};
use serde::Deserialize;

use crate::simctl;

/// How long a started simulator has to finish booting.
const BOOT_TIMEOUT: Duration = Duration::from_secs(240);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Types {
    #[serde(default)]
    devicetypes: Vec<Named>,
    #[serde(default)]
    runtimes: Vec<Runtime>,
}

#[derive(Debug, Deserialize)]
struct Named {
    identifier: String,
    name: String,
    #[serde(default, rename = "productFamily")]
    family: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Runtime {
    identifier: String,
    name: String,
    #[serde(default)]
    is_available: bool,
}

/// What a simulator can be made from: the iPhone and iPad models, and the iOS runtimes that are installed.
pub fn parse_options(device_types: &[u8], runtimes: &[u8]) -> Result<DeviceOptions> {
    let bad = |e: serde_json::Error| Error::parse("simulator options", e.to_string());
    let types: Types = serde_json::from_slice(device_types).map_err(bad)?;
    let runs: Types = serde_json::from_slice(runtimes).map_err(bad)?;
    let models = types
        .devicetypes
        .into_iter()
        .filter(|t| matches!(t.family.as_str(), "iPhone" | "iPad"))
        .map(|t| Choice { id: t.identifier, label: t.name, installed: true })
        .collect();
    let mut images: Vec<Choice> = runs
        .runtimes
        .into_iter()
        .filter(|r| r.identifier.contains(".iOS-"))
        .map(|r| Choice { id: r.identifier, label: r.name, installed: r.is_available })
        .collect();
    images.dedup_by(|a, b| a.id == b.id);
    Ok(DeviceOptions { images, models })
}

pub async fn options() -> Result<DeviceOptions> {
    let (types, runtimes) =
        tokio::join!(simctl::run(["list", "devicetypes", "-j"]), simctl::run(["list", "runtimes", "-j"]));
    parse_options(&types?, &runtimes?)
}

/// The UDID `simctl create` prints.
pub fn parse_udid(out: &str) -> Option<String> {
    let line = out.lines().map(str::trim).find(|l| !l.is_empty())?;
    (line.len() == 36 && line.chars().all(|c| c.is_ascii_hexdigit() || c == '-')).then(|| line.to_string())
}

/// Makes a simulator and returns its UDID. It is not started.
pub async fn create(request: &CreateDevice) -> Result<String> {
    let out =
        simctl::run_text(["create", request.name.as_str(), request.model.as_str(), request.image.as_str()]).await?;
    parse_udid(&out)
        .ok_or_else(|| Error::failed(format!("simctl made a simulator but did not say which: {}", out.trim())))
}

/// Starts a simulator and waits until it has booted. One that is booted already is left as it is.
pub async fn boot(udid: &str) -> Result<()> {
    match simctl::run(["boot", udid]).await {
        Ok(_) => {}
        Err(Error::Simctl(m)) if m.contains("current state: Booted") || m.contains("already") => {}
        Err(e) => return Err(e),
    }
    tokio::time::timeout(BOOT_TIMEOUT, simctl::run(["bootstatus", udid, "-b"])).await.map_err(|_| {
        Error::failed(format!("The simulator did not finish booting in {} seconds.", BOOT_TIMEOUT.as_secs()))
    })??;
    Ok(())
}

/// Stops a simulator. One that is stopped already is not an error.
pub async fn shutdown(udid: &str) -> Result<()> {
    match simctl::run(["shutdown", udid]).await {
        Ok(_) => Ok(()),
        Err(Error::Simctl(m)) if m.contains("current state: Shutdown") => Ok(()),
        Err(e) => Err(e),
    }
}

/// Erases a simulator's contents and settings. It has to be shut down.
pub async fn erase(udid: &str) -> Result<()> {
    simctl::run(["erase", udid]).await.map(|_| ()).map_err(running_hint)
}

pub async fn delete(udid: &str) -> Result<()> {
    simctl::run(["delete", udid]).await.map(|_| ()).map_err(running_hint)
}

/// Refuses a simulator that is not shut down: `simctl delete` would take a running one away under its apps.
pub fn require_stopped(sim: &crate::devices::SimDevice) -> Result<()> {
    if sim.state == "Shutdown" {
        Ok(())
    } else {
        Err(Error::failed(format!("{} is {}. Shut it down first.", sim.name, sim.state.to_lowercase())))
    }
}

fn running_hint(e: Error) -> Error {
    match e {
        Error::Simctl(m) if m.contains("Booted") || m.contains("current state") => {
            Error::failed("The simulator is running. Shut it down first.")
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_models_and_runtimes() {
        let types = br#"{"devicetypes":[
            {"identifier":"com.apple.CoreSimulator.SimDeviceType.iPhone-17","name":"iPhone 17","productFamily":"iPhone"},
            {"identifier":"com.apple.CoreSimulator.SimDeviceType.iPad-Pro","name":"iPad Pro","productFamily":"iPad"},
            {"identifier":"com.apple.CoreSimulator.SimDeviceType.Apple-Watch-10","name":"Apple Watch","productFamily":"Apple Watch"}]}"#;
        let runtimes = br#"{"runtimes":[
            {"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-26-2","name":"iOS 26.2","isAvailable":true},
            {"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-26-2","name":"iOS 26.2","isAvailable":true},
            {"identifier":"com.apple.CoreSimulator.SimRuntime.watchOS-12-0","name":"watchOS 12.0","isAvailable":true},
            {"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-18-0","name":"iOS 18.0","isAvailable":false}]}"#;
        let o = parse_options(types, runtimes).unwrap();
        assert_eq!(o.models.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(), ["iPhone 17", "iPad Pro"]);
        assert_eq!(
            o.images.iter().map(|i| (i.label.as_str(), i.installed)).collect::<Vec<_>>(),
            [("iOS 26.2", true), ("iOS 18.0", false)]
        );
        assert!(parse_options(b"nope", runtimes).is_err());
    }

    #[test]
    fn reads_the_udid_of_a_new_simulator() {
        assert_eq!(
            parse_udid("F8CE84C1-04D1-4E33-B52B-02B59E8A09C5\n").as_deref(),
            Some("F8CE84C1-04D1-4E33-B52B-02B59E8A09C5")
        );
        assert_eq!(parse_udid("Invalid runtime"), None);
        assert_eq!(parse_udid(""), None);
    }

    #[test]
    fn only_a_simulator_that_is_shut_down_can_be_erased_or_deleted() {
        let sim = |state: &str| crate::devices::SimDevice {
            udid: "u".into(),
            name: "iPhone 17".into(),
            state: state.into(),
            is_available: true,
            device_type_identifier: None,
            data_path: None,
            data_path_size: None,
            log_path: None,
            last_booted_at: None,
            runtime: None,
        };
        assert!(require_stopped(&sim("Shutdown")).is_ok());
        for state in ["Booted", "Booting", "Shutting Down"] {
            let e = require_stopped(&sim(state)).unwrap_err().to_string();
            assert!(e.contains("Shut it down first") && e.contains("iPhone 17"), "{e}");
        }
    }

    #[test]
    fn a_running_simulator_is_told_to_shut_down_first() {
        let e = running_hint(Error::Simctl("Unable to erase contents and settings in current state: Booted".into()));
        assert!(e.to_string().contains("Shut it down first"));
        assert!(matches!(running_hint(Error::Simctl("Invalid device".into())), Error::Simctl(_)));
    }
}
