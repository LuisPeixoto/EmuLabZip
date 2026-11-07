//! `emulabzip avd`, `emulabzip sim` and `emulabzip snapshot`: make, start, stop, wipe and delete emulators and
//! simulators, and keep an emulator's own snapshots.

use anyhow::{Result, bail};
use emulabzip_core::{BootOptions, CreateDevice, Device, DeviceState, Lab, Platform, SnapshotAction, is_device_name};

use super::state::need_yes;
use crate::output::{self, cell};

/// A device by id or name, running or not.
async fn find(lab: &Lab, platform: Platform, query: &str) -> Result<Device> {
    let (devices, _) = lab.all_devices().await;
    let found: Vec<Device> = devices
        .into_iter()
        .filter(|d| d.platform == platform && (d.id == query || d.name == query || d.id == format!("avd:{query}")))
        .collect();
    match found.len() {
        0 => bail!("no {platform} device is called {query}. `emulabzip devices --all` lists them."),
        1 => Ok(found.into_iter().next().unwrap()),
        _ => bail!("{} devices are called {query}; use the id.", found.len()),
    }
}

pub async fn list(lab: &Lab, platform: Platform, json: bool) -> Result<()> {
    let (devices, errors) = lab.all_devices().await;
    let devices: Vec<Device> = devices.into_iter().filter(|d| d.platform == platform).collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
        return Ok(());
    }
    for (p, e) in errors.iter().filter(|(p, _)| *p == platform) {
        eprintln!("{} {e}", output::dim(&format!("{p}:")));
    }
    if devices.is_empty() {
        println!("No {platform} devices.");
        return Ok(());
    }
    let rows: Vec<_> = devices.iter().map(output::device_row).collect();
    print!("{}", output::table(&["PLATFORM", "NAME", "OS", "STATE", "ID"], &rows));
    Ok(())
}

pub async fn options(lab: &Lab, platform: Platform, json: bool) -> Result<()> {
    let options = lab.device_options(platform).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&options)?);
        return Ok(());
    }
    let (images, models) = if platform == Platform::Android { ("IMAGES", "DEVICES") } else { ("RUNTIMES", "TYPES") };
    let rows = |list: &[emulabzip_core::Choice]| -> Vec<Vec<(String, String)>> {
        list.iter()
            .map(|c| {
                vec![cell(c.id.clone()), cell(c.label.clone()), cell(if c.installed { "" } else { "not installed" })]
            })
            .collect()
    };
    println!("{}", output::heading(images));
    print!("{}", output::table(&["ID", "NAME", ""], &rows(&options.images)));
    println!("\n{}", output::heading(models));
    print!("{}", output::table(&["ID", "NAME", ""], &rows(&options.models)));
    Ok(())
}

pub async fn create(lab: &Lab, platform: Platform, name: &str, image: &str, model: &str) -> Result<()> {
    let request = CreateDevice { name: name.into(), image: image.into(), model: model.into() };
    if !request.is_valid() {
        bail!("the name, image and model must be plain identifiers (letters, digits, . _ - ; :)");
    }
    let device = lab.create_device(platform, &request).await?;
    println!("Created {} ({})", device.name, device.id);
    Ok(())
}

pub async fn boot(lab: &Lab, platform: Platform, query: &str, options: BootOptions, yes: bool) -> Result<()> {
    if options.wipe {
        need_yes(yes, "Starting from factory state wipes the device's data and")?;
    }
    let device = find(lab, platform, query).await?;
    if device.state == DeviceState::Online {
        bail!("{} is running already.", device.name);
    }
    let started = lab.boot_device(&device.target(), &options).await?;
    println!("{} is running as {}", started.name, started.id);
    Ok(())
}

pub async fn shutdown(lab: &Lab, platform: Platform, query: &str) -> Result<()> {
    let device = find(lab, platform, query).await?;
    lab.shutdown_device(&device.target()).await?;
    println!("Shut down {}", device.name);
    Ok(())
}

pub async fn wipe(lab: &Lab, platform: Platform, query: &str, yes: bool) -> Result<()> {
    need_yes(yes, "Wiping a device throws away everything on it and")?;
    let device = find(lab, platform, query).await?;
    lab.wipe_device(&device.target(), true).await?;
    println!("Wiped {}", device.name);
    Ok(())
}

pub async fn delete(lab: &Lab, platform: Platform, query: &str, yes: bool) -> Result<()> {
    need_yes(yes, "Deleting a device removes it and its data for good and")?;
    let device = find(lab, platform, query).await?;
    lab.delete_device(&device.target(), true).await?;
    println!("Deleted {}", device.name);
    Ok(())
}

pub async fn snapshots(lab: &Lab, device: Option<&str>, json: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let list = lab.emulator_snapshots(&device.target()).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    if list.is_empty() {
        println!("{} has no snapshots.", device.name);
        return Ok(());
    }
    let rows: Vec<_> =
        list.iter().map(|s| vec![cell(s.name.clone()), cell(s.size.clone()), cell(s.date.clone())]).collect();
    print!("{}", output::table(&["NAME", "SIZE", "DATE"], &rows));
    Ok(())
}

pub async fn snapshot(lab: &Lab, device: Option<&str>, action: SnapshotAction, name: &str, yes: bool) -> Result<()> {
    if !is_device_name(name) {
        bail!("a snapshot name is letters, digits, . _ and -");
    }
    if action.is_destructive() {
        need_yes(yes, &format!("{} throws away what the emulator is doing now and", action.label()))?;
    }
    let device = lab.resolve(device).await?;
    lab.emulator_snapshot(&device.target(), action, name, true).await?;
    println!("{}: {name}", action.label());
    Ok(())
}
