//! The mapping files, source maps and `.dSYM` folders kept on this computer, for each app and version.
//!
//! ```text
//! mappings/com.example.app/1.2.0__42_/
//!     meta.json             the app and the version as they were given
//!     mapping.txt           ProGuard or R8
//!     index.android.bundle.map
//!     Crashy.dSYM/          a folder
//! ```
//!
//! A crash report names the app and the version it came from, so the files that belong to it are found without asking.
//! The same version of two apps never share a folder, and neither do two versions of one app.

use std::path::{Path, PathBuf};

use emulabzip_core::{Error, Result, data_dir};
use serde::{Deserialize, Serialize};

const META: &str = "meta.json";
/// The most a copied file or folder may hold. Mappings of big apps are tens of megabytes; this is only a stop for a
/// wrong choice, such as a whole disk.
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// What a stored file is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// ProGuard or R8 `mapping.txt`: Java and Kotlin stacks.
    Mapping,
    /// A source map: JavaScript stacks of React Native.
    SourceMap,
    /// A `.dSYM` folder: native iOS stacks.
    Dsym,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Mapping => "mapping.txt",
            Kind::SourceMap => "source map",
            Kind::Dsym => "dSYM",
        }
    }

    pub fn parse(text: &str) -> Option<Kind> {
        match text.to_ascii_lowercase().as_str() {
            "mapping" | "mapping.txt" | "proguard" | "r8" => Some(Kind::Mapping),
            "sourcemap" | "source-map" | "map" => Some(Kind::SourceMap),
            "dsym" => Some(Kind::Dsym),
            _ => None,
        }
    }
}

/// A file kept for an app and version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub app: String,
    pub version: String,
    pub kind: Kind,
    /// The name inside the version's folder.
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    app: String,
    version: String,
}

/// The store, rooted at a folder (`mappings/` in the data folder by default).
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Default for Store {
    fn default() -> Self {
        Store { root: data_dir().join("mappings") }
    }
}

/// A folder name for a version: `1.2.0 (42)` is `1.2.0__42_`. Other characters become `_`, so no version can name a path.
pub fn folder_name(version: &str) -> String {
    version.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-') { c } else { '_' }).collect()
}

fn check(app: &str, version: &str) -> Result<()> {
    if !emulabzip_core::control::is_app_id(app) {
        return Err(Error::invalid("app", app));
    }
    let v = version.trim();
    if v.is_empty() || v.len() > 80 || v.chars().any(char::is_control) || v == "." || v == ".." {
        return Err(Error::invalid("version", version));
    }
    Ok(())
}

fn size_of(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => m.len(),
        Ok(_) => std::fs::read_dir(path).map(|d| d.flatten().map(|e| size_of(&e.path())).sum()).unwrap_or(0),
        Err(_) => 0,
    }
}

fn copy_dir(from: &Path, to: &Path, budget: &mut u64) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            copy_dir(&src, &dst, budget)?;
        } else {
            let len = entry.metadata()?.len();
            *budget = budget.checked_sub(len).ok_or_else(|| Error::failed("That is too big to keep: 2 GB at most."))?;
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// What a file or folder is, from its name and what it holds.
pub fn detect(path: &Path) -> Result<Kind> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if path.is_dir() {
        if name.to_ascii_lowercase().ends_with(".dsym") && crate::dsym::binary_of(path).is_some() {
            return Ok(Kind::Dsym);
        }
        return Err(Error::failed(format!("{name} is a folder, but not a .dSYM with a binary inside.")));
    }
    let head = {
        use std::io::Read;
        let mut buf = vec![0u8; 64 * 1024];
        let n = std::fs::File::open(path)
            .map_err(|e| Error::failed(format!("Could not open {}: {e}", path.display())))?
            .read(&mut buf)?;
        String::from_utf8_lossy(&buf[..n]).into_owned()
    };
    if head.trim_start().starts_with('{') && head.contains("\"mappings\"") {
        return Ok(Kind::SourceMap);
    }
    // A mapping has lines like `com.example.Foo -> a.b:` and members under them.
    let looks_like_mapping = head.lines().filter(|l| l.contains(" -> ")).take(3).count() >= 1
        && (name == "mapping.txt" || head.lines().any(|l| l.trim_end().ends_with(':') && l.contains(" -> ")));
    if looks_like_mapping {
        return Ok(Kind::Mapping);
    }
    Err(Error::failed(format!(
        "{name} is not a mapping.txt, a source map or a .dSYM. A source map is JSON with `mappings`; a mapping.txt lists `original -> renamed:` lines."
    )))
}

impl Store {
    /// A store in another folder, for tests.
    pub fn at(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn dir(&self, app: &str, version: &str) -> PathBuf {
        self.root.join(app).join(folder_name(version.trim()))
    }

    /// Keeps a copy of `source` for `app` at `version`. What it is gets worked out from the file itself, or `kind` says.
    pub fn add(&self, app: &str, version: &str, source: &Path, kind: Option<Kind>) -> Result<Stored> {
        check(app, version)?;
        let kind = match kind {
            Some(k) => k,
            None => detect(source)?,
        };
        let version = version.trim();
        if size_of(source) > MAX_BYTES {
            return Err(Error::failed("That is too big to keep: 2 GB at most."));
        }
        let original = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let name = match kind {
            Kind::Mapping => "mapping.txt".to_string(),
            _ => folder_name(&original),
        };
        if name.is_empty() || name == "." || name == ".." || name == META {
            return Err(Error::invalid("file name", original));
        }
        let dir = self.dir(app, version);
        std::fs::create_dir_all(&dir)?;
        let meta = Meta { app: app.into(), version: version.into() };
        std::fs::write(dir.join(META), serde_json::to_vec_pretty(&meta).map_err(|e| Error::failed(e.to_string()))?)?;
        let target = dir.join(&name);
        if target.is_dir() {
            std::fs::remove_dir_all(&target)?;
        }
        if source.is_dir() {
            let mut budget = MAX_BYTES;
            copy_dir(source, &target, &mut budget)?;
        } else {
            std::fs::copy(source, &target)?;
        }
        Ok(Stored { app: app.into(), version: version.into(), kind, name, size_bytes: size_of(&target), path: target })
    }

    /// Everything kept, or only what belongs to `app`.
    pub fn list(&self, app: Option<&str>) -> Vec<Stored> {
        let mut out = Vec::new();
        let apps: Vec<PathBuf> = match app {
            Some(a) if emulabzip_core::control::is_app_id(a) => vec![self.root.join(a)],
            Some(_) => return out,
            None => std::fs::read_dir(&self.root).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default(),
        };
        for app_dir in apps {
            let Ok(versions) = std::fs::read_dir(&app_dir) else { continue };
            for version_dir in versions.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
                let Some(meta) =
                    std::fs::read(version_dir.join(META)).ok().and_then(|b| serde_json::from_slice::<Meta>(&b).ok())
                else {
                    continue;
                };
                let Ok(files) = std::fs::read_dir(&version_dir) else { continue };
                for file in files.flatten() {
                    let path = file.path();
                    let name = file.file_name().to_string_lossy().to_string();
                    if name == META {
                        continue;
                    }
                    let kind = match detect(&path) {
                        Ok(k) => k,
                        Err(_) => continue,
                    };
                    out.push(Stored {
                        app: meta.app.clone(),
                        version: meta.version.clone(),
                        kind,
                        name,
                        size_bytes: size_of(&path),
                        path,
                    });
                }
            }
        }
        out.sort_by(|a, b| (&a.app, &a.version, &a.name).cmp(&(&b.app, &b.version, &b.name)));
        out
    }

    /// What belongs to this version of the app. Without a version, nothing is assumed: a mapping of the wrong build
    /// reads worse than none.
    pub fn find(&self, app: &str, version: Option<&str>) -> Vec<Stored> {
        let Some(version) = version else { return Vec::new() };
        self.list(Some(app)).into_iter().filter(|s| s.version == version.trim()).collect()
    }

    /// Takes away what is kept for a version: everything, or only one kind. Returns how many files went.
    pub fn remove(&self, app: &str, version: &str, kind: Option<Kind>) -> Result<usize> {
        check(app, version)?;
        let dir = self.dir(app, version);
        let mut removed = 0;
        for item in self.find(app, Some(version)) {
            if kind.is_some_and(|k| k != item.kind) {
                continue;
            }
            if item.path.is_dir() {
                std::fs::remove_dir_all(&item.path)?;
            } else {
                std::fs::remove_file(&item.path)?;
            }
            removed += 1;
        }
        // Leave no empty folders behind.
        let empty = std::fs::read_dir(&dir).map(|d| d.flatten().all(|e| e.file_name() == META)).unwrap_or(false);
        if empty {
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::remove_dir(self.root.join(app));
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("emulabzip-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn mapping(dir: &Path) -> PathBuf {
        let p = dir.join("mapping.txt");
        std::fs::write(&p, "# compiler: R8\ncom.example.Foo -> a.b:\n    1:1:void run():10:10 -> a\n").unwrap();
        p
    }

    fn dsym(dir: &Path) -> PathBuf {
        let d = dir.join("Crashy.dSYM");
        std::fs::create_dir_all(d.join("Contents/Resources/DWARF")).unwrap();
        std::fs::write(d.join("Contents/Resources/DWARF/Crashy"), b"binary").unwrap();
        d
    }

    fn map(dir: &Path) -> PathBuf {
        let p = dir.join("index.android.bundle.map");
        std::fs::write(&p, r#"{"version":3,"sources":["App.tsx"],"names":[],"mappings":"AAAA"}"#).unwrap();
        p
    }

    #[test]
    fn keeps_the_three_kinds_per_app_and_version() {
        let dir = scratch("kinds");
        let store = Store::at(dir.join("store"));
        let a = store.add("com.example.app", "1.2.0 (42)", &mapping(&dir), None).unwrap();
        assert_eq!((a.kind, a.name.as_str()), (Kind::Mapping, "mapping.txt"));
        let b = store.add("com.example.app", "1.2.0 (42)", &map(&dir), None).unwrap();
        assert_eq!(b.kind, Kind::SourceMap);
        let c = store.add("com.example.app", "1.2.0 (42)", &dsym(&dir), None).unwrap();
        assert_eq!((c.kind, c.name.as_str()), (Kind::Dsym, "Crashy.dSYM"));
        assert!(c.path.join("Contents/Resources/DWARF/Crashy").is_file());
        assert!(dir.join("store/com.example.app/1.2.0__42_").is_dir());

        let found = store.find("com.example.app", Some("1.2.0 (42)"));
        assert_eq!(found.len(), 3);
        assert!(store.find("com.example.app", Some("1.3.0")).is_empty(), "another version has nothing");
        assert!(store.find("com.example.app", None).is_empty());
        assert!(store.find("com.other", Some("1.2.0 (42)")).is_empty(), "another app has nothing");
        assert_eq!(store.list(None).len(), 3);
        assert_eq!(store.list(Some("com.example.app")).len(), 3);
        assert!(store.list(Some("../x")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn adding_again_replaces_and_removing_cleans_up() {
        let dir = scratch("replace");
        let store = Store::at(dir.join("store"));
        store.add("com.a.b", "1", &mapping(&dir), None).unwrap();
        store.add("com.a.b", "1", &mapping(&dir), None).unwrap();
        store.add("com.a.b", "1", &dsym(&dir), None).unwrap();
        store.add("com.a.b", "1", &dsym(&dir), None).unwrap();
        assert_eq!(store.list(None).len(), 2);
        assert_eq!(store.remove("com.a.b", "1", Some(Kind::Dsym)).unwrap(), 1);
        assert_eq!(store.list(None).len(), 1);
        assert_eq!(store.remove("com.a.b", "1", None).unwrap(), 1);
        assert!(store.list(None).is_empty());
        assert!(!dir.join("store/com.a.b").exists(), "no empty folders are left");
        assert_eq!(store.remove("com.a.b", "1", None).unwrap(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_what_is_not_a_mapping_and_names_that_cannot_be_trusted() {
        let dir = scratch("refuse");
        let store = Store::at(dir.join("store"));
        let text = dir.join("notes.txt");
        std::fs::write(&text, "just some words\n").unwrap();
        assert!(store.add("com.a.b", "1", &text, None).unwrap_err().to_string().contains("not a mapping.txt"));
        assert!(store.add("com.a.b", "1", &dir, None).is_err(), "a folder that is not a dSYM");
        assert!(store.add("not an app", "1", &mapping(&dir), None).is_err());
        for bad in ["", "   ", "..", ".", "a\nb"] {
            assert!(store.add("com.a.b", bad, &mapping(&dir), None).is_err(), "{bad:?}");
        }
        assert!(store.add("com.a.b", "1", &dir.join("missing"), None).is_err());
        // Whatever the version says, it stays inside the app's folder.
        let a = store.add("com.a.b", "../../etc", &mapping(&dir), None).unwrap();
        assert!(a.path.starts_with(dir.join("store/com.a.b")), "{:?}", a.path);
        assert_eq!(folder_name("../x y"), ".._x_y");
        // A kind given by the user wins over the guess.
        assert_eq!(store.add("com.a.b", "2", &text, Some(Kind::Mapping)).unwrap().kind, Kind::Mapping);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn kinds_can_be_named_on_the_command_line() {
        assert_eq!(Kind::parse("dSYM"), Some(Kind::Dsym));
        assert_eq!(Kind::parse("Mapping.txt"), Some(Kind::Mapping));
        assert_eq!(Kind::parse("map"), Some(Kind::SourceMap));
        assert_eq!(Kind::parse("zip"), None);
    }
}
