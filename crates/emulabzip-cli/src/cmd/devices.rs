//! `emulabzip devices`

use anyhow::Result;
use emulabzip_core::Lab;

use crate::output;

pub async fn run(lab: &Lab, json: bool) -> Result<()> {
    let (devices, errors) = lab.devices().await;
    if json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
        return Ok(());
    }
    for (platform, e) in &errors {
        eprintln!("{} {e}", output::dim(&format!("{platform}:")));
    }
    if devices.is_empty() {
        println!("No running emulators or simulators.");
        println!("{}", output::dim("Start an Android emulator or boot an iOS simulator, then run this again."));
        return Ok(());
    }
    let rows: Vec<_> = devices.iter().map(output::device_row).collect();
    print!("{}", output::table(&["PLATFORM", "NAME", "OS", "STATE", "ID"], &rows));
    Ok(())
}
