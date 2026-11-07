//! Android Virtual Devices: the ones that exist on this computer, making, starting, stopping, wiping and removing them,
//! and the snapshots an emulator keeps of itself.
//!
//! AVDs are listed from the files the SDK writes (`~/.android/avd/<name>.ini` and the `config.ini` in its folder), so
//! no Java is started to show them. Making and removing one runs `avdmanager`; starting one runs `emulator`, which is left
//! running on its own when this program ends, as an emulator started from a terminal would be.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use emulabzip_adb::AdbClient;
use emulabzip_core::{
    BootOptions, Choice, CreateDevice, Device, DeviceKind, DeviceOptions, DeviceState, EmulatorSnapshot, Error,
    Platform, Result,
};
use tokio::process::Command;

use crate::controls::console_lines;

/// How long a started emulator has to finish booting.
const BOOT_TIMEOUT: Duration = Duration::from_secs(240);

/// Where the SDK keeps AVDs: `ANDROID_AVD_HOME`, or `~/.android/avd`.
pub fn avd_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("ANDROID_AVD_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(home));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(Path::new(&home).join(".android").join("avd"))
}

/// `key=value` lines, as the SDK writes them.
pub fn parse_ini(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

fn get<'a>(ini: &'a [(String, String)], key: &str) -> Option<&'a str> {
    ini.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// What the SDK knows about one AVD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avd {
    pub name: String,
    pub dir: PathBuf,
    pub api: Option<u32>,
    pub abi: Option<String>,
    pub device: Option<String>,
    pub tag: Option<String>,
}

/// The Android API level in `system-images/android-35/google_apis/x86_64/`, or `android-35`, or `android-36.1`.
pub fn api_of(path: &str) -> Option<u32> {
    let after = path.split("android-").nth(1)?;
    after.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

/// Reads the AVDs in a folder.
pub fn list_in(home: &Path) -> Vec<Avd> {
    let Ok(read) = std::fs::read_dir(home) else { return Vec::new() };
    let mut out: Vec<Avd> = read
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            let name = file.strip_suffix(".ini")?.to_string();
            if !emulabzip_core::is_device_name(&name) {
                return None;
            }
            let pointer = parse_ini(&std::fs::read_to_string(e.path()).ok()?);
            let dir = get(&pointer, "path").map(PathBuf::from).unwrap_or_else(|| home.join(format!("{name}.avd")));
            let config = parse_ini(&std::fs::read_to_string(dir.join("config.ini")).ok()?);
            Some(Avd {
                api: get(&config, "image.sysdir.1")
                    .and_then(api_of)
                    .or_else(|| get(&config, "target").and_then(api_of)),
                abi: get(&config, "abi.type").map(str::to_string),
                device: get(&config, "hw.device.name").map(str::to_string),
                tag: get(&config, "tag.id").map(str::to_string),
                name,
                dir,
            })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

pub fn list() -> Vec<Avd> {
    avd_home().map(|h| list_in(&h)).unwrap_or_default()
}

/// An AVD that is not running, as a device.
pub fn as_device(avd: &Avd) -> Device {
    Device {
        id: format!("avd:{}", avd.name),
        name: avd.name.clone(),
        platform: Platform::Android,
        kind: DeviceKind::Emulator,
        state: DeviceState::Shutdown,
        os_version: None,
        api_level: avd.api,
        model: avd.device.clone(),
        abi: avd.abi.clone(),
    }
}

/// A tool of the SDK: `emulator`, or `avdmanager`.
pub fn sdk_tool(name: &str) -> Result<PathBuf> {
    let exe = if cfg!(windows) { if name == "avdmanager" { "avdmanager.bat" } else { "emulator.exe" } } else { name };
    let candidates: &[&str] = if name == "avdmanager" {
        &["cmdline-tools/latest/bin", "cmdline-tools/bin", "tools/bin"]
    } else {
        &["emulator"]
    };
    for root in emulabzip_adb::server::sdk_roots() {
        for dir in candidates {
            let path = root.join(dir).join(exe);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err(Error::ToolMissing(
        if name == "avdmanager" { "avdmanager" } else { "the Android emulator" },
        "Install the Android SDK command-line tools and the emulator, or set ANDROID_HOME.",
    ))
}

fn sdk_env(cmd: &mut Command) {
    if std::env::var_os("ANDROID_HOME").is_none()
        && let Some(root) = emulabzip_adb::server::sdk_roots().into_iter().find(|r| r.is_dir())
    {
        cmd.env("ANDROID_HOME", &root).env("ANDROID_SDK_ROOT", root);
    }
    // avdmanager needs a JDK; Android Studio's is used when JAVA_HOME is not set.
    if std::env::var_os("JAVA_HOME").is_none() {
        let studio = Path::new("/Applications/Android Studio.app/Contents/jbr/Contents/Home");
        if studio.join("bin/java").is_file() {
            cmd.env("JAVA_HOME", studio);
        }
    }
}

/// System images on disk, as `system-images;android-35;google_apis;x86_64`.
pub fn system_images_in(sdk: &Path) -> Vec<Choice> {
    let mut out = Vec::new();
    let Ok(apis) = std::fs::read_dir(sdk.join("system-images")) else { return out };
    for api in apis.flatten() {
        let api_name = api.file_name().to_string_lossy().into_owned();
        let Ok(tags) = std::fs::read_dir(api.path()) else { continue };
        for tag in tags.flatten() {
            let tag_name = tag.file_name().to_string_lossy().into_owned();
            let Ok(abis) = std::fs::read_dir(tag.path()) else { continue };
            for abi in abis.flatten() {
                let abi_name = abi.file_name().to_string_lossy().into_owned();
                let id = format!("system-images;{api_name};{tag_name};{abi_name}");
                if !emulabzip_core::is_image_id(&id) {
                    continue;
                }
                out.push(Choice {
                    label: format!(
                        "{} · {} · {abi_name}",
                        api_name.replace("android-", "Android "),
                        tag_name.replace('_', " ")
                    ),
                    id,
                    installed: true,
                });
            }
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

/// The device definitions `avdmanager` knows, from `avdmanager list device -c`.
pub fn parse_device_ids(text: &str) -> Vec<Choice> {
    text.lines()
        .map(str::trim)
        .filter(|l| emulabzip_core::is_image_id(l))
        .map(|l| Choice { id: l.to_string(), label: l.replace('_', " "), installed: true })
        .collect()
}

pub async fn options() -> Result<DeviceOptions> {
    let images = emulabzip_adb::server::sdk_roots()
        .into_iter()
        .find(|r| r.join("system-images").is_dir())
        .map(|r| system_images_in(&r))
        .unwrap_or_default();
    let tool = sdk_tool("avdmanager")?;
    let mut cmd = Command::new(tool);
    cmd.args(["list", "device", "-c"]).stdin(Stdio::null()).kill_on_drop(true);
    sdk_env(&mut cmd);
    let out = tokio::time::timeout(Duration::from_secs(60), cmd.output())
        .await
        .map_err(|_| Error::failed("avdmanager did not answer in time."))??;
    if !out.status.success() {
        return Err(Error::failed(format!(
            "avdmanager failed: {}",
            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")
        )));
    }
    Ok(DeviceOptions { images, models: parse_device_ids(&String::from_utf8_lossy(&out.stdout)) })
}

/// Makes an AVD. One that exists is not replaced.
pub async fn create(request: &CreateDevice) -> Result<Device> {
    if list().iter().any(|a| a.name == request.name) {
        return Err(Error::failed(format!("There is an emulator called {} already.", request.name)));
    }
    let tool = sdk_tool("avdmanager")?;
    let mut cmd = Command::new(tool);
    cmd.args(["create", "avd", "-n", &request.name, "-k", &request.image, "-d", &request.model])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    sdk_env(&mut cmd);
    let mut child = cmd.spawn()?;
    // avdmanager asks whether to make a custom hardware profile.
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(b"no\n").await;
    }
    let out = tokio::time::timeout(Duration::from_secs(120), child.wait_with_output())
        .await
        .map_err(|_| Error::failed("avdmanager did not finish in time."))??;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said.lines().find(|l| !l.trim().is_empty()).unwrap_or("it gave no reason");
        return Err(Error::failed(format!("avdmanager could not make the emulator: {said}")));
    }
    list()
        .iter()
        .find(|a| a.name == request.name)
        .map(as_device)
        .ok_or_else(|| Error::failed("The emulator was made, but it is not listed."))
}

pub async fn delete(name: &str) -> Result<()> {
    let tool = sdk_tool("avdmanager")?;
    let mut cmd = Command::new(tool);
    cmd.args(["delete", "avd", "-n", name]).stdin(Stdio::null()).kill_on_drop(true);
    sdk_env(&mut cmd);
    let out = tokio::time::timeout(Duration::from_secs(60), cmd.output())
        .await
        .map_err(|_| Error::failed("avdmanager did not finish in time."))??;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::failed(format!(
            "avdmanager could not delete {name}: {}",
            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")
        )))
    }
}

/// Puts an AVD that is not running back to factory state: its data, its cache and its snapshots go.
pub fn wipe(avd: &Avd) -> Result<()> {
    let Ok(read) = std::fs::read_dir(&avd.dir) else {
        return Err(Error::NotFound(format!("the folder of {} is missing", avd.name)));
    };
    for e in read.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let doomed = name.starts_with("userdata-qemu.img")
            || name.starts_with("cache.img")
            || name == "snapshots"
            || name == "snapshot.img";
        if !doomed {
            continue;
        }
        let path = e.path();
        if path.is_dir() { std::fs::remove_dir_all(&path)? } else { std::fs::remove_file(&path)? }
    }
    Ok(())
}

/// Whether a running device's name is the AVD's. Android writes an AVD's name into a property of the running emulator
/// with its underscores turned into spaces (`Pixel_9` runs as `Pixel 9`).
pub fn same_avd(running: &str, avd: &str) -> bool {
    running.replace('_', " ") == avd.replace('_', " ")
}

/// The emulator console's port, from a serial: `emulator-5554` is 5554.
pub fn console_port(serial: &str) -> Option<u16> {
    serial.strip_prefix("emulator-")?.parse().ok()
}

/// Starts an emulator and waits until it has booted. Returns its serial.
pub async fn boot(adb: &AdbClient, name: &str, options: &BootOptions) -> Result<String> {
    if !list().iter().any(|a| a.name == name) {
        return Err(Error::NotFound(format!("there is no emulator called {name}")));
    }
    let before: Vec<String> =
        adb.devices().await.map(|d| d.into_iter().map(|e| e.serial).collect()).unwrap_or_default();
    let tool = sdk_tool("emulator")?;
    let mut args: Vec<String> = vec!["-avd".into(), name.into(), "-no-audio".into(), "-no-boot-anim".into()];
    if options.headless {
        args.push("-no-window".into());
    }
    if options.cold {
        args.push("-no-snapshot-load".into());
    }
    if options.wipe {
        args.push("-wipe-data".into());
    }
    let log = emulabzip_core::data_dir().join("emulator-logs");
    std::fs::create_dir_all(&log)?;
    let log_file = std::fs::File::create(log.join(format!("{name}.log")))?;
    let mut cmd = std::process::Command::new(tool);
    cmd.args(&args).stdin(Stdio::null()).stdout(log_file.try_clone()?).stderr(log_file);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group, so it is not stopped with this program.
        cmd.process_group(0);
    }
    sdk_env_std(&mut cmd);
    cmd.spawn().map_err(|e| Error::failed(format!("could not start the emulator: {e}")))?;

    let wait = async {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let Ok(list) = adb.devices().await else { continue };
            for entry in list
                .into_iter()
                .filter(|d| d.serial.starts_with("emulator-") && !before.contains(&d.serial) && d.is_online())
            {
                let done = adb.shell(&entry.serial, "getprop sys.boot_completed").await.is_ok_and(|o| o.trim() == "1");
                if done {
                    return entry.serial;
                }
            }
        }
    };
    tokio::time::timeout(BOOT_TIMEOUT, wait).await.map_err(|_| {
        Error::failed(format!(
            "{name} did not finish booting in {} seconds. Its log is in {}.",
            BOOT_TIMEOUT.as_secs(),
            log.join(format!("{name}.log")).display()
        ))
    })
}

fn sdk_env_std(cmd: &mut std::process::Command) {
    if std::env::var_os("ANDROID_HOME").is_none()
        && let Some(root) = emulabzip_adb::server::sdk_roots().into_iter().find(|r| r.is_dir())
    {
        cmd.env("ANDROID_HOME", &root).env("ANDROID_SDK_ROOT", root);
    }
}

/// Stops an emulator through its console.
pub async fn shutdown(adb: &AdbClient, serial: &str) -> Result<()> {
    let port = console_port(serial).ok_or(Error::Unsupported("Shutting down a physical device"))?;
    let _ = console_lines(port, &["kill".to_string()]).await;
    // It is gone when adb no longer lists it.
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !adb.devices().await.is_ok_and(|d| d.iter().any(|e| e.serial == serial)) {
            return Ok(());
        }
    }
    Err(Error::failed("The emulator did not shut down in time."))
}

/// The snapshots in the output of `avd snapshot list`:
///
/// ```text
/// List of snapshots present on all disks:
/// ID        TAG                 VM SIZE                DATE       VM CLOCK
/// --        default_boot          1.4G 2026-11-07 15:00:00   00:01:03.123
/// ```
pub fn parse_snapshots(lines: &[String]) -> Vec<EmulatorSnapshot> {
    lines
        .iter()
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            // An id (`--` or a number), a name, a size, a date and a time, and a clock.
            if parts.len() < 6 || !(parts[0] == "--" || parts[0].chars().all(|c| c.is_ascii_digit())) {
                return None;
            }
            Some(EmulatorSnapshot {
                name: parts[1].to_string(),
                size: parts[2].to_string(),
                date: format!("{} {}", parts[3], parts[4]),
            })
        })
        .collect()
}

pub async fn snapshots(serial: &str) -> Result<Vec<EmulatorSnapshot>> {
    let port = console_port(serial).ok_or(Error::Unsupported("Snapshots of a physical device"))?;
    let lines = console_lines(port, &["avd snapshot list".to_string()]).await?;
    Ok(parse_snapshots(&lines))
}

pub async fn snapshot(serial: &str, action: emulabzip_core::SnapshotAction, name: &str) -> Result<()> {
    use emulabzip_core::SnapshotAction::*;
    let port = console_port(serial).ok_or(Error::Unsupported("Snapshots of a physical device"))?;
    let verb = match action {
        Save => "save",
        Load => "load",
        Delete => "del",
    };
    console_lines(port, &[format!("avd snapshot {verb} {name}")]).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-avd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_the_avds_the_sdk_wrote() {
        let h = home("list");
        for (name, api, device) in [("Pixel_9", "android-35", "pixel_9"), ("elz_test", "android-36.1", "pixel_6")] {
            let dir = h.join(format!("{name}.avd"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                h.join(format!("{name}.ini")),
                format!("avd.ini.encoding=UTF-8\npath={}\ntarget={api}\n", dir.display()),
            )
            .unwrap();
            std::fs::write(
                dir.join("config.ini"),
                format!("abi.type=x86_64\nhw.device.name={device}\nimage.sysdir.1=system-images/{api}/google_apis_playstore/x86_64/\ntag.id=google_apis_playstore\n"),
            )
            .unwrap();
        }
        // Not AVDs: a stray file, a name the tools would not take, an ini whose folder is gone.
        std::fs::write(h.join("notes.txt"), "x").unwrap();
        std::fs::write(h.join("bad name.ini"), "path=/nowhere\n").unwrap();
        std::fs::write(h.join("ghost.ini"), "path=/nowhere\n").unwrap();
        let avds = list_in(&h);
        assert_eq!(avds.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["elz_test", "Pixel_9"]);
        assert_eq!(
            (avds[1].api, avds[1].abi.as_deref(), avds[1].device.as_deref()),
            (Some(35), Some("x86_64"), Some("pixel_9"))
        );
        assert_eq!(avds[0].api, Some(36), "36.1 is API 36");
        let d = as_device(&avds[1]);
        assert_eq!((d.id.as_str(), d.state, d.api_level), ("avd:Pixel_9", DeviceState::Shutdown, Some(35)));
        std::fs::remove_dir_all(h).ok();
    }

    #[test]
    fn api_levels_come_from_paths() {
        assert_eq!(api_of("system-images/android-35/google_apis/x86_64/"), Some(35));
        assert_eq!(api_of("android-36.1"), Some(36));
        assert_eq!(api_of("nothing"), None);
    }

    #[test]
    fn finds_the_system_images_on_disk() {
        let sdk = home("images");
        std::fs::create_dir_all(sdk.join("system-images/android-35/google_apis_playstore/x86_64")).unwrap();
        std::fs::create_dir_all(sdk.join("system-images/android-34/default/arm64-v8a")).unwrap();
        let images = system_images_in(&sdk);
        assert_eq!(images[0].id, "system-images;android-35;google_apis_playstore;x86_64");
        assert_eq!(images[0].label, "Android 35 · google apis playstore · x86_64");
        assert_eq!(images.len(), 2);
        assert!(images.iter().all(|i| i.installed));
        std::fs::remove_dir_all(sdk).ok();
    }

    #[test]
    fn reads_the_device_definitions() {
        let ids = parse_device_ids("pixel_6\npixel_9_pro\n\nWarning: something\n  tv_1080p  \n");
        assert_eq!(ids.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["pixel_6", "pixel_9_pro", "tv_1080p"]);
    }

    #[test]
    fn reads_the_emulators_snapshot_list() {
        let lines: Vec<String> = [
            "List of snapshots present on all disks:",
            "ID        TAG                 VM SIZE                DATE       VM CLOCK",
            "--        default_boot          1.4G 2026-11-07 15:00:00   00:01:03.123",
            "2         before_login        800.2M 2026-11-07 15:10:00   00:02:00.000",
            "OK",
        ]
        .map(String::from)
        .to_vec();
        let list = parse_snapshots(&lines);
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[0].name.as_str(), list[0].size.as_str(), list[0].date.as_str()),
            ("default_boot", "1.4G", "2026-11-07 15:00:00")
        );
        assert_eq!(list[1].name, "before_login");
        assert!(parse_snapshots(&["There is no snapshot available.".to_string()]).is_empty());
    }

    #[test]
    fn a_wipe_removes_data_and_snapshots_but_keeps_the_definition() {
        let h = home("wipe");
        let dir = h.join("x.avd");
        std::fs::create_dir_all(dir.join("snapshots/default_boot")).unwrap();
        for f in [
            "config.ini",
            "userdata-qemu.img",
            "userdata-qemu.img.qcow2",
            "cache.img",
            "sdcard.img",
            "hardware-qemu.ini",
        ] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        wipe(&Avd { name: "x".into(), dir: dir.clone(), api: None, abi: None, device: None, tag: None }).unwrap();
        for gone in ["userdata-qemu.img", "userdata-qemu.img.qcow2", "cache.img", "snapshots"] {
            assert!(!dir.join(gone).exists(), "{gone}");
        }
        for kept in ["config.ini", "sdcard.img", "hardware-qemu.ini"] {
            assert!(dir.join(kept).exists(), "{kept}");
        }
        std::fs::remove_dir_all(h).ok();
    }

    #[test]
    fn a_running_emulator_is_matched_to_its_avd_though_android_turns_underscores_into_spaces() {
        assert!(same_avd("Pixel 9", "Pixel_9"));
        assert!(same_avd("Pixel_9", "Pixel_9"));
        assert!(same_avd("elz life 1", "elz_life_1"));
        assert!(!same_avd("Pixel 9", "Pixel_5"));
    }

    #[test]
    fn ports_come_from_serials() {
        assert_eq!(console_port("emulator-5554"), Some(5554));
        assert_eq!(console_port("R5CT1234"), None);
    }
}
