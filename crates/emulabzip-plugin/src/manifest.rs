//! `plugin.json` and finding the plugins on this computer.

use std::path::{Path, PathBuf};

use emulabzip_core::{Platform, data_dir};
use serde::{Deserialize, Serialize};

use crate::PROTOCOL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// The program prints its results and exits.
    Once,
    /// The program keeps running and prints results as it goes, until it is stopped.
    Stream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// The protocol version the plugin speaks. Only 1 exists.
    pub protocol: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// `android`, `ios`, or both. Empty means both.
    #[serde(default)]
    pub platforms: Vec<Platform>,
    /// The program and its arguments. The program is looked up in the plugin's folder first, then
    /// on the PATH.
    pub command: Vec<String>,
    pub mode: Mode,
    /// Seconds a `once` plugin may run before it is stopped (default 60).
    #[serde(default)]
    pub timeout_s: Option<u64>,
}

/// A plugin found on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub manifest: Manifest,
    pub dir: PathBuf,
    /// A fingerprint of `plugin.json`. Approval is tied to it: edit the file and it lapses.
    pub fingerprint: String,
}

/// A folder that looked like a plugin but could not be used, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub dir: PathBuf,
    pub message: String,
}

/// Where plugins live: `plugins/` in the data folder.
pub fn plugins_dir() -> PathBuf {
    data_dir().join("plugins")
}

/// Letters, digits, `-` and `_`, starting with a letter or digit. A name is a folder name too.
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && name.len() <= 48
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let manifest: Manifest = serde_json::from_str(text).map_err(|e| format!("plugin.json is not valid: {e}"))?;
        manifest.check()?;
        Ok(manifest)
    }

    fn check(&self) -> Result<(), String> {
        if self.protocol != PROTOCOL {
            return Err(format!(
                "it speaks protocol {}, and this EmuLabZip understands protocol {PROTOCOL}",
                self.protocol
            ));
        }
        if !is_valid_name(&self.name) {
            return Err(format!("`{}` is not a name: use letters, digits, - and _", self.name));
        }
        if self.command.first().is_none_or(|c| c.trim().is_empty()) {
            return Err("`command` must name a program".into());
        }
        Ok(())
    }

    pub fn supports(&self, platform: Platform) -> bool {
        self.platforms.is_empty() || self.platforms.contains(&platform)
    }
}

fn fingerprint(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Reads one plugin folder.
pub fn load(dir: &Path) -> Result<Plugin, String> {
    let text = std::fs::read_to_string(dir.join("plugin.json")).map_err(|e| format!("cannot read plugin.json: {e}"))?;
    let manifest = Manifest::parse(&text)?;
    let folder = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if folder != manifest.name {
        return Err(format!("the folder is called `{folder}` but the plugin is called `{}`", manifest.name));
    }
    Ok(Plugin { fingerprint: fingerprint(&text), manifest, dir: dir.to_path_buf() })
}

/// The plugins in `root`, by name, and the folders that could not be read.
pub fn discover_in(root: &Path) -> (Vec<Plugin>, Vec<Problem>) {
    let (mut plugins, mut problems) = (Vec::new(), Vec::new());
    let Ok(read) = std::fs::read_dir(root) else { return (plugins, problems) };
    for entry in read.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || dir.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.')) {
            continue;
        }
        match load(&dir) {
            Ok(p) => plugins.push(p),
            Err(message) => problems.push(Problem { dir, message }),
        }
    }
    plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    problems.sort_by(|a, b| a.dir.cmp(&b.dir));
    (plugins, problems)
}

pub fn discover() -> (Vec<Plugin>, Vec<Problem>) {
    discover_in(&plugins_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{ "protocol": 1, "name": "battery-temp", "version": "0.1.0", "description": "x", "platforms": ["android"], "command": ["./run.sh", "--fast"], "mode": "stream" }"#;

    #[test]
    fn reads_a_manifest() {
        let m = Manifest::parse(GOOD).unwrap();
        assert_eq!((m.name.as_str(), m.mode, m.command.len()), ("battery-temp", Mode::Stream, 2));
        assert!(m.supports(Platform::Android) && !m.supports(Platform::Ios));
        let both = Manifest::parse(&GOOD.replace(r#""platforms": ["android"], "#, "")).unwrap();
        assert!(both.supports(Platform::Ios), "no platforms means all of them");
    }

    #[test]
    fn rejects_what_cannot_be_run() {
        for (text, why) in [
            (GOOD.replace("\"protocol\": 1", "\"protocol\": 2"), "protocol"),
            (GOOD.replace("battery-temp", "../evil"), "not a name"),
            (GOOD.replace("[\"./run.sh\", \"--fast\"]", "[]"), "command"),
            (GOOD.replace("[\"./run.sh\", \"--fast\"]", "[\" \"]"), "command"),
            ("{".to_string(), "not valid"),
        ] {
            let err = Manifest::parse(&text).unwrap_err();
            assert!(err.contains(why), "{err}");
        }
    }

    #[test]
    fn names_are_safe_folder_names() {
        for good in ["a", "battery-temp", "Top_Procs2"] {
            assert!(is_valid_name(good), "{good}");
        }
        for bad in ["", "-a", "a b", "a/b", "..", "a.b", &"x".repeat(49)] {
            assert!(!is_valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn finds_plugins_and_says_what_is_wrong_with_the_rest() {
        let root = std::env::temp_dir().join(format!("emulabzip-plugins-{}", std::process::id()));
        for (folder, text) in
            [("battery-temp", GOOD.to_string()), ("wrong-name", GOOD.to_string()), ("broken", "{".to_string())]
        {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            std::fs::write(root.join(folder).join("plugin.json"), text).unwrap();
        }
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        let (plugins, problems) = discover_in(&root);
        assert_eq!(plugins.iter().map(|p| p.manifest.name.as_str()).collect::<Vec<_>>(), ["battery-temp"]);
        let messages: Vec<&str> = problems.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(problems.len(), 3, "{messages:?}");
        assert!(messages.iter().any(|m| m.contains("is called `wrong-name`")));
        assert!(messages.iter().any(|m| m.contains("cannot read plugin.json")));
        assert_eq!(plugins[0].fingerprint.len(), 64);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn the_fingerprint_changes_with_the_file() {
        assert_ne!(fingerprint(GOOD), fingerprint(&GOOD.replace("0.1.0", "0.1.1")));
        assert_eq!(fingerprint(GOOD), fingerprint(GOOD));
    }
}
