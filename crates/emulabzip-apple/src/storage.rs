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

/// Where a file that may not exist yet goes inside `root`. The deepest folder that does exist has to be inside the root
/// once links are followed, so a symbolic link cannot carry a write out of it.
pub fn resolve_for_write(root: &Path, rel: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    if rel.is_empty() {
        return Err(Error::invalid("file", rel));
    }
    let full = root.join(rel);
    let mut existing = full.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(|| Error::invalid("path", rel))?;
    }
    if !existing.canonicalize()?.starts_with(&root) {
        return Err(Error::invalid("path", rel));
    }
    // The file itself must not be a link that leaves the root either.
    if let Ok(meta) = std::fs::symlink_metadata(&full)
        && meta.file_type().is_symlink()
        && !full.canonicalize().map(|c| c.starts_with(&root)).unwrap_or(false)
    {
        return Err(Error::invalid("path", rel));
    }
    Ok(full)
}

pub async fn write(root: &Path, rel: &str, bytes: &[u8]) -> Result<()> {
    let path = resolve_for_write(root, rel)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, bytes).await?;
    Ok(())
}

pub async fn remove(root: &Path, rel: &str) -> Result<()> {
    let path = resolve_for_write(root, rel)?;
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
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
    fn a_write_may_make_new_folders_but_not_leave_the_root() {
        let root = temp_root("write");
        assert!(resolve_for_write(&root, "Documents/new/deeper/file.db").is_ok());
        assert!(resolve_for_write(&root, "../escape").is_err());
        assert!(resolve_for_write(&root, "").is_err());
        #[cfg(unix)]
        {
            let outside = std::env::temp_dir().join(format!("emulabzip-outside-{}", std::process::id()));
            std::fs::create_dir_all(&outside).unwrap();
            std::os::unix::fs::symlink(&outside, root.join("Escape")).unwrap();
            assert!(resolve_for_write(&root, "Escape/file").is_err(), "a link out of the root is refused");
            std::os::unix::fs::symlink(outside.join("target"), root.join("Documents/ln")).unwrap();
            assert!(resolve_for_write(&root, "Documents/ln").is_err(), "so is a file that is such a link");
            std::fs::remove_dir_all(outside).ok();
        }
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn a_file_is_written_and_removed_inside_the_root() {
        let root = temp_root("wr");
        write(&root, "Documents/sub/a.txt", b"one").await.unwrap();
        assert_eq!(std::fs::read(root.join("Documents/sub/a.txt")).unwrap(), b"one");
        write(&root, "Documents/sub/a.txt", b"two").await.unwrap();
        assert_eq!(std::fs::read(root.join("Documents/sub/a.txt")).unwrap(), b"two");
        remove(&root, "Documents/sub/a.txt").await.unwrap();
        assert!(!root.join("Documents/sub/a.txt").exists());
        remove(&root, "Documents/sub/a.txt").await.unwrap();
        std::fs::remove_dir_all(root).ok();
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
