//! Asking an Android device about a link: who opens it, and what it decided about an app's claim on the domain.

use emulabzip_adb::{AdbClient, command};
use emulabzip_core::Result;
use emulabzip_core::links::{Facts, parse_get_app_links, parse_resolve_activity};

/// The shell command that says which activity a link resolves to.
pub fn resolve_command(url: &str) -> String {
    command([
        "cmd",
        "package",
        "resolve-activity",
        "--brief",
        "-a",
        "android.intent.action.VIEW",
        "-c",
        "android.intent.category.BROWSABLE",
        "-d",
        url,
    ])
}

pub async fn facts(adb: &AdbClient, serial: &str, app: Option<&str>, url: &str) -> Result<Facts> {
    let mut facts = Facts::default();
    let out = adb.shell(serial, &format!("{} 2>&1", resolve_command(url))).await?;
    facts.resolved = parse_resolve_activity(&out);
    if let Some(app) = app {
        let out = adb.shell(serial, &format!("{} 2>&1", command(["pm", "get-app-links", app]))).await?;
        facts.app_links = parse_get_app_links(&out);
        if facts.app_links.is_none() {
            facts.notes.push(
                "`pm get-app-links` gave nothing for this app: it needs Android 12 or later, and the app must be installed."
                    .into(),
            );
        }
    }
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_is_quoted_for_the_shell() {
        let c = resolve_command("https://a.example/x?y=1&z=2");
        assert!(c.starts_with("cmd package resolve-activity --brief -a android.intent.action.VIEW"), "{c}");
        assert!(c.ends_with("-d 'https://a.example/x?y=1&z=2'"), "{c}");
    }
}
