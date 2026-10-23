//! Browsing a simulator app's containers. They are ordinary folders on the Mac, so they are read
//! with the host file system.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use emulabzip_core::files::{MAX_READ_BYTES, join, sort_entries};
use emulabzip_core::{Error, FileEntry, Result, StorageRoot};
use serde_json::Value;

use crate::apps::file_url_to_path;

/// The containers of one app, from `simctl listapps`: its data container, its bundle and its
/// app group containers.
pub fn roots(listapps: &Value, bundle: &str) -> Result<Vec<(StorageRoot, PathBuf)>> {
    let info = listapps.get(bundle).ok_or_else(|| Error::NotFound(format!("{bundle} is not installed")))?;
    let mut out = Vec::new();
    let mut push = |id: String, label: String, path: String| {
        let p = PathBuf::from(&path);
        let unavailable = (!p.is_dir()).then(|| "The folder does not exist yet.".to_string());
        out.push((StorageRoot { id, label, location: path, unavailable }, p));
    };
    if let Some(url) = info.get("DataContainer").and_then(Value::as_str) {
        push("data".into(), "Data container".into(), file_url_to_path(url));
    }
    if let Some(path) = info.get("Path").and_then(Value::as_str) {
        push("bundle".into(), "App bundle".into(), path.trim_end_matches('/').to_string());
    }
    if let Some(groups) = info.get("GroupContainers").and_then(Value::as_object) {
        for (id, url) in groups {
            if let Some(url) = url.as_str() {
                push(format!("group:{id}"), id.clone(), file_url_to_path(url));
            }
        }
    }
    Ok(out)
}

/// Resolves `rel` inside `root`, refusing anything that ends up outside it (for example through
/// a symbolic link).
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let full = if rel.is_empty() { root.clone() } else { root.join(rel) }.canonicalize()?;
    if full.starts_with(&root) { Ok(full) } else { Err(Error::invalid("path", rel)) }
}

pub async fn list(root: &Path, rel: &str) -> Result<Vec<FileEntry>> {
    let dir = resolve(root, rel)?;
    let mut entries = Vec::new();
    let mut read = tokio::fs::read_dir(&dir).await?;
    while let Some(entry) = read.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = tokio::fs::metadata(entry.path()).await else { continue };
        entries.push(FileEntry {
            path: join(rel, &name),
            name,
            is_dir: meta.is_dir(),
            size: if meta.is_dir() { 0 } else { meta.len() },
            modified: meta.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
        });
    }
    sort_entries(&mut entries);
    Ok(entries)
}

pub async fn read(root: &Path, rel: &str) -> Result<Vec<u8>> {
    let path = resolve(root, rel)?;
    let meta = tokio::fs::metadata(&path).await?;
    if meta.is_dir() {
        return Err(Error::invalid("file", rel));
    }
    if meta.len() > MAX_READ_BYTES {
        return Err(Error::Unsupported("Reading files larger than 256 MB"));
    }
    Ok(tokio::fs::read(&path).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-storage-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Documents")).unwrap();
        std::fs::write(dir.join("Documents/notes.txt"), b"hello").unwrap();
        std::fs::write(dir.join("Library.plist"), b"x").unwrap();
        dir
    }

    #[test]
    fn finds_the_containers() {
        let list = json!({"com.example": {
            "DataContainer": "file:///tmp/a%20b/",
            "Path": "/Applications/Example.app",
            "GroupContainers": {"group.com.example": "file:///tmp/group/"}
        }});
        let found = roots(&list, "com.example").unwrap();
        let ids: Vec<_> = found.iter().map(|(r, _)| r.id.as_str()).collect();
        assert_eq!(ids, ["data", "bundle", "group:group.com.example"]);
        assert_eq!(found[0].0.location, "/tmp/a b");
        assert!(roots(&list, "com.other").is_err());
    }

    #[tokio::test]
    async fn lists_and_reads_inside_the_root() {
        let root = temp_root("list");
        let entries = list(&root, "").await.unwrap();
        assert_eq!(entries[0].name, "Documents");
        assert!(entries[0].is_dir);
        assert_eq!(list(&root, "Documents").await.unwrap()[0].path, "Documents/notes.txt");
        assert_eq!(read(&root, "Documents/notes.txt").await.unwrap(), b"hello");
        assert!(read(&root, "Documents").await.is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn refuses_paths_outside_the_root() {
        let root = temp_root("escape");
        assert!(resolve(&root, "..").is_err());
        std::fs::remove_dir_all(root).ok();
    }
}
