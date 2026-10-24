//! Starting a new plugin from a template.

use std::path::{Path, PathBuf};

use emulabzip_core::{Error, Result};

use crate::manifest::{is_valid_name, plugins_dir};

const RUN: &str = r#"#!/usr/bin/env python3
"""A plugin for EmuLabZip. Print one JSON object per line; EmuLabZip draws them.

The device and the app come in the environment: EMULABZIP_DEVICE, EMULABZIP_PLATFORM
("android" or "ios"), EMULABZIP_APP (empty when none is picked) and EMULABZIP_ADB.
"""
import json
import os
import subprocess

device = os.environ["EMULABZIP_DEVICE"]
platform = os.environ["EMULABZIP_PLATFORM"]


def emit(**event):
    print(json.dumps(event), flush=True)


if platform == "android":
    adb = os.environ.get("EMULABZIP_ADB", "adb")
    uptime = subprocess.run([adb, "-s", device, "shell", "cat", "/proc/uptime"], capture_output=True, text=True, check=True).stdout
    emit(type="metric", name="uptime", value=float(uptime.split()[0]), unit="s")
    emit(type="text", title="Device", body=f"{device} has been up for {uptime.split()[0]} seconds")
else:
    emit(type="text", title="Simulator", body=f"Hello from {device}")
"#;

pub fn manifest(name: &str) -> String {
    format!(
        r#"{{
  "protocol": 1,
  "name": "{name}",
  "version": "0.1.0",
  "description": "Describe what {name} shows",
  "platforms": [],
  "command": ["python3", "run.py"],
  "mode": "once"
}}
"#
    )
}

/// Creates `<root>/<name>` with a manifest and a program that prints a metric and some text.
pub fn scaffold_in(root: &Path, name: &str) -> Result<PathBuf> {
    if !is_valid_name(name) {
        return Err(Error::invalid("plugin name", name));
    }
    let dir = root.join(name);
    if dir.exists() {
        return Err(Error::failed(format!("{} already exists", dir.display())));
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("plugin.json"), manifest(name))?;
    std::fs::write(dir.join("run.py"), RUN)?;
    Ok(dir)
}

pub fn scaffold(name: &str) -> Result<PathBuf> {
    scaffold_in(&plugins_dir(), name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::load;

    #[test]
    fn creates_a_plugin_that_loads() {
        let root = std::env::temp_dir().join(format!("emulabzip-scaffold-{}", std::process::id()));
        let dir = scaffold_in(&root, "uptime").unwrap();
        let plugin = load(&dir).unwrap();
        assert_eq!(plugin.manifest.command, ["python3", "run.py"]);
        assert!(dir.join("run.py").is_file());
        assert!(scaffold_in(&root, "uptime").is_err(), "an existing plugin is never overwritten");
        assert!(scaffold_in(&root, "../escape").is_err());
        assert!(!root.join("../escape").exists());
        std::fs::remove_dir_all(root).ok();
    }
}
