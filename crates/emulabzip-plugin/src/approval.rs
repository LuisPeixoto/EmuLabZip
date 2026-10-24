//! Which plugins the user has allowed to run.
//!
//! A plugin is a program, so it never runs by itself: it has to be enabled first, and enabling is
//! tied to the exact `plugin.json` that was shown. Change the file and the approval lapses.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::manifest::{Plugin, plugins_dir};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approvals {
    /// Plugin name to the fingerprint of the `plugin.json` that was approved.
    approved: BTreeMap<String, String>,
    #[serde(skip)]
    path: PathBuf,
}

const FILE: &str = ".approved.json";

impl Approvals {
    pub fn load() -> Approvals {
        Approvals::load_in(&plugins_dir())
    }

    /// Reads the approvals kept in `root`. A missing or damaged file means nothing is approved.
    pub fn load_in(root: &Path) -> Approvals {
        let path = root.join(FILE);
        let mut approvals: Approvals =
            std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        approvals.path = path;
        approvals
    }

    pub fn is_approved(&self, plugin: &Plugin) -> bool {
        self.approved.get(&plugin.manifest.name).is_some_and(|f| *f == plugin.fingerprint)
    }

    pub fn approve(&mut self, plugin: &Plugin) -> std::io::Result<()> {
        self.approved.insert(plugin.manifest.name.clone(), plugin.fingerprint.clone());
        self.save()
    }

    pub fn revoke(&mut self, name: &str) -> std::io::Result<()> {
        self.approved.remove(name);
        self.save()
    }

    fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = self.path.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
        std::fs::rename(temp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Manifest, Mode};

    fn plugin(fingerprint: &str) -> Plugin {
        Plugin {
            manifest: Manifest {
                protocol: 1,
                name: "demo".into(),
                version: "1".into(),
                description: String::new(),
                platforms: vec![],
                command: vec!["x".into()],
                mode: Mode::Once,
                timeout_s: None,
            },
            dir: PathBuf::from("/tmp/demo"),
            fingerprint: fingerprint.into(),
        }
    }

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("emulabzip-approvals-{name}-{}", std::process::id()))
    }

    #[test]
    fn nothing_is_approved_to_begin_with() {
        let a = Approvals::load_in(&root("none"));
        assert!(!a.is_approved(&plugin("abc")));
    }

    #[test]
    fn an_approval_is_kept_and_tied_to_the_file() {
        let dir = root("keep");
        let mut a = Approvals::load_in(&dir);
        a.approve(&plugin("abc")).unwrap();
        let again = Approvals::load_in(&dir);
        assert!(again.is_approved(&plugin("abc")));
        assert!(!again.is_approved(&plugin("changed")), "editing plugin.json withdraws the approval");

        let mut again = again;
        again.revoke("demo").unwrap();
        assert!(!Approvals::load_in(&dir).is_approved(&plugin("abc")));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_damaged_file_approves_nothing() {
        let dir = root("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), "not json").unwrap();
        assert!(!Approvals::load_in(&dir).is_approved(&plugin("abc")));
        std::fs::remove_dir_all(dir).ok();
    }
}
