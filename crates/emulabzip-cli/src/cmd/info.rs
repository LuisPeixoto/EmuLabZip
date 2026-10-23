//! `emulabzip info`

use anyhow::Result;
use emulabzip_core::Lab;

use crate::output;

pub async fn run(lab: &Lab, device: Option<&str>, json: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let sections = lab.info(&device.target()).await?;
    if json {
        let map: serde_json::Map<String, serde_json::Value> = sections
            .iter()
            .map(|s| {
                let items = s.items.iter().map(|(k, v)| (k.clone(), v.clone().into())).collect();
                (s.title.clone(), serde_json::Value::Object(items))
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&map)?);
        return Ok(());
    }
    let width = sections.iter().flat_map(|s| &s.items).map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            println!();
        }
        println!("{}", output::heading(&section.title));
        for (k, v) in &section.items {
            println!("  {}  {v}", output::dim(&format!("{k:<width$}")));
        }
    }
    Ok(())
}
