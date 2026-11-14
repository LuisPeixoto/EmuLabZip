//! Asking the iOS Simulator about an app's claim on a domain: the Associated Domains entitlement.

use emulabzip_core::Result;
use emulabzip_core::links::{Facts, parse_entitlements};

use crate::simctl;

pub async fn facts(id: &str, app: Option<&str>) -> Result<Facts> {
    let mut facts = Facts::default();
    let Some(app) = app else {
        facts.notes.push("Give the app (its bundle id) to read its entitlements.".into());
        return Ok(facts);
    };
    let container = match simctl::run_text(["get_app_container", id, app, "app"]).await {
        Ok(path) => path.trim().to_string(),
        Err(e) => {
            facts.notes.push(format!("{app} is not installed on the simulator ({e})."));
            return Ok(facts);
        }
    };
    // `codesign` writes the entitlements of a signed bundle as a property list. A simulator build that is not
    // signed has none, and that is said instead of guessed.
    let output = tokio::process::Command::new("codesign")
        .args(["-d", "--entitlements", ":-", &container])
        .stdin(std::process::Stdio::null())
        .output()
        .await;
    match output {
        Ok(o) if !o.stdout.is_empty() => {
            facts.entitlements = Some(parse_entitlements(&String::from_utf8_lossy(&o.stdout)));
        }
        Ok(_) => facts.notes.push("The app carries no entitlements: it is not signed with any.".into()),
        Err(e) => facts.notes.push(format!("`codesign` could not run: {e}")),
    }
    Ok(facts)
}
