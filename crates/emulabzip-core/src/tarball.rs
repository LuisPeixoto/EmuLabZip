//! Reading and writing the `tar` files an app's state is kept in, with the checks that make unpacking one safe.
//!
//! An archive can come from a file someone sent, so nothing is unpacked before every entry has been looked at: no
//! absolute paths, no `..`, no hard links or devices, and no symbolic link that points out of the folder.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use tar::EntryType;

use crate::error::{Error, Result};
use crate::state::is_safe_entry;

/// What an archive holds, or what was written or read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub files: u64,
    pub bytes: u64,
}

/// The most entries an archive may hold. A sandbox with more is not one a person can use.
pub const MAX_ENTRIES: u64 = 2_000_000;

fn bad(detail: impl Into<String>) -> Error {
    Error::parse("a state archive", detail)
}

/// Strips the `./` that `tar -C dir .` puts in front of every name.
fn clean(path: &str) -> &str {
    path.trim_start_matches("./")
}

fn link_is_safe(target: &str) -> bool {
    !target.starts_with('/') && !target.split('/').any(|p| p == "..") && !target.contains('\0')
}

/// Packs the contents of `dir` into a tar file at `out`. Top-level names in `skip_top` (caches, say) are left out.
pub fn pack_dir(dir: &Path, out: &Path, skip_top: &[&str]) -> Result<Summary> {
    let mut summary = Summary::default();
    let mut builder = tar::Builder::new(File::create(out)?);
    builder.follow_symlinks(false);
    let mut names: Vec<_> = fs::read_dir(dir)?.collect::<std::io::Result<_>>()?;
    names.sort_by_key(|e| e.file_name());
    for entry in names {
        let name = entry.file_name().to_string_lossy().into_owned();
        if skip_top.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            builder.append_dir_all(&name, &path)?;
            summary.files += count_files(&path);
            summary.bytes += dir_bytes(&path);
        } else if meta.is_file() || meta.file_type().is_symlink() {
            builder.append_path_with_name(&path, &name)?;
            summary.files += 1;
            summary.bytes += meta.len();
        }
    }
    builder.finish()?;
    Ok(summary)
}

fn count_files(dir: &Path) -> u64 {
    walk(dir).filter(|m| m.is_file()).count() as u64
}

fn dir_bytes(dir: &Path) -> u64 {
    walk(dir).filter(|m| m.is_file()).map(|m| m.len()).sum()
}

fn walk(dir: &Path) -> impl Iterator<Item = fs::Metadata> {
    let mut stack = vec![dir.to_path_buf()];
    let mut found = Vec::new();
    while let Some(d) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else { continue };
        for e in read.flatten() {
            let Ok(m) = fs::symlink_metadata(e.path()) else { continue };
            if m.is_dir() {
                stack.push(e.path());
            }
            found.push(m);
        }
    }
    found.into_iter()
}

/// One thing in an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

/// Lists an archive and refuses it when any entry is unsafe. Nothing is written.
pub fn check(archive: &Path) -> Result<Vec<Entry>> {
    let mut tar = tar::Archive::new(File::open(archive)?);
    let mut out = Vec::new();
    for entry in tar.entries().map_err(|e| bad(e.to_string()))? {
        let entry = entry.map_err(|e| bad(e.to_string()))?;
        if out.len() as u64 >= MAX_ENTRIES {
            return Err(bad("too many entries"));
        }
        let raw = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
        let path = clean(&raw).to_string();
        if path.is_empty() || path == "." {
            continue;
        }
        if !is_safe_entry(&path) {
            return Err(bad(format!("`{raw}` is not inside the app's folder")));
        }
        match entry.header().entry_type() {
            EntryType::Regular | EntryType::Continuous | EntryType::GNUSparse => {}
            EntryType::Directory => {}
            EntryType::Symlink => {
                let target = entry.link_name().map_err(|e| bad(e.to_string()))?;
                let target = target.map(|t| t.to_string_lossy().into_owned()).unwrap_or_default();
                if !link_is_safe(&target) {
                    return Err(bad(format!("`{raw}` is a link to `{target}`, outside the app's folder")));
                }
            }
            // GNU long names and pax headers are read by the tar crate itself.
            EntryType::GNULongName | EntryType::GNULongLink | EntryType::XGlobalHeader | EntryType::XHeader => continue,
            other => return Err(bad(format!("`{raw}` is a {other:?}, which a sandbox does not hold"))),
        }
        out.push(Entry {
            path,
            size: entry.header().size().unwrap_or(0),
            is_dir: entry.header().entry_type().is_dir(),
        });
    }
    Ok(out)
}

/// Unpacks an archive into `dest`, after checking all of it.
pub fn unpack(archive: &Path, dest: &Path) -> Result<Summary> {
    check(archive)?;
    fs::create_dir_all(dest)?;
    let mut tar = tar::Archive::new(File::open(archive)?);
    tar.set_preserve_permissions(true);
    tar.set_overwrite(true);
    let mut summary = Summary::default();
    for entry in tar.entries().map_err(|e| bad(e.to_string()))? {
        let mut entry = entry.map_err(|e| bad(e.to_string()))?;
        let size = entry.header().size().unwrap_or(0);
        let is_file = entry.header().entry_type().is_file();
        if !entry.unpack_in(dest).map_err(|e| bad(e.to_string()))? {
            return Err(bad("an entry would have been written outside the app's folder"));
        }
        if is_file {
            summary.files += 1;
            summary.bytes += size;
        }
    }
    Ok(summary)
}

/// Removes everything inside `dir`, but not `dir` itself, and not the top-level names in `keep`.
pub fn empty_dir(dir: &Path, keep: &[&str]) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if keep.contains(&entry.file_name().to_string_lossy().as_ref()) {
            continue;
        }
        let path = entry.path();
        if fs::symlink_metadata(&path)?.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Reads one regular file out of an archive, at most `max` bytes of it. `None` when it is not there.
pub fn read_file(archive: &Path, name: &str, max: usize) -> Result<Option<Vec<u8>>> {
    let mut tar = tar::Archive::new(File::open(archive)?);
    for entry in tar.entries().map_err(|e| bad(e.to_string()))? {
        let entry = entry.map_err(|e| bad(e.to_string()))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let raw = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
        if clean(&raw) != name {
            continue;
        }
        let mut bytes = Vec::new();
        entry.take(max as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > max {
            return Err(bad(format!("`{name}` is bigger than {max} bytes")));
        }
        return Ok(Some(bytes));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-tarball-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample(dir: &Path) {
        fs::create_dir_all(dir.join("databases")).unwrap();
        fs::create_dir_all(dir.join("cache")).unwrap();
        fs::write(dir.join("databases/app.db"), b"SQLite format 3\0").unwrap();
        fs::write(dir.join("cache/junk"), b"x").unwrap();
        fs::write(dir.join("a.txt"), b"hello").unwrap();
    }

    #[test]
    fn a_folder_packs_and_unpacks_to_the_same_files() {
        let dir = scratch("roundtrip");
        sample(&dir.join("src"));
        let archive = dir.join("a.tar");
        let packed = pack_dir(&dir.join("src"), &archive, &["cache"]).unwrap();
        assert_eq!(packed.files, 2);
        let entries = check(&archive).unwrap();
        assert!(entries.iter().any(|e| e.path == "databases/app.db"));
        assert!(!entries.iter().any(|e| e.path.starts_with("cache")), "skipped: {entries:?}");
        let unpacked = unpack(&archive, &dir.join("out")).unwrap();
        assert_eq!(unpacked.files, 2);
        assert_eq!(fs::read(dir.join("out/a.txt")).unwrap(), b"hello");
        assert_eq!(fs::read(dir.join("out/databases/app.db")).unwrap(), b"SQLite format 3\0");
        assert_eq!(read_file(&archive, "a.txt", 100).unwrap().as_deref(), Some(&b"hello"[..]));
        assert_eq!(read_file(&archive, "nope", 100).unwrap(), None);
        assert!(read_file(&archive, "a.txt", 2).is_err(), "bigger than the limit");
        fs::remove_dir_all(dir).ok();
    }

    /// An archive with one entry whose name is written into the header as it is, which a builder would refuse.
    fn archive_with(name: &str, kind: EntryType, link: Option<&str>) -> Vec<u8> {
        let mut header = tar::Header::new_old();
        {
            let old = header.as_old_mut();
            old.name[..name.len()].copy_from_slice(name.as_bytes());
        }
        header.set_size(if kind == EntryType::Regular { 3 } else { 0 });
        header.set_mode(0o644);
        header.set_entry_type(kind);
        if let Some(link) = link {
            header.as_old_mut().linkname[..link.len()].copy_from_slice(link.as_bytes());
        }
        header.set_cksum();
        let mut out = header.as_bytes().to_vec();
        if kind == EntryType::Regular {
            let mut data = b"abc".to_vec();
            data.resize(512, 0);
            out.extend(data);
        }
        out.extend(vec![0u8; 1024]);
        out
    }

    #[test]
    fn an_archive_that_would_escape_is_refused_before_anything_is_written() {
        let dir = scratch("evil");
        for (name, kind, link) in [
            ("../evil", EntryType::Regular, None),
            ("a/../../evil", EntryType::Regular, None),
            ("/etc/passwd", EntryType::Regular, None),
            ("link", EntryType::Symlink, Some("/etc")),
            ("link", EntryType::Symlink, Some("../../x")),
            ("hard", EntryType::Link, Some("a")),
            ("dev", EntryType::Char, None),
        ] {
            let file = dir.join("e.tar");
            fs::write(&file, archive_with(name, kind, link)).unwrap();
            assert!(check(&file).is_err(), "{name} {kind:?} {link:?}");
            assert!(unpack(&file, &dir.join("out")).is_err(), "{name}");
            assert!(!dir.join("evil").exists() && !dir.join("out/evil").exists());
        }
        // A harmless one is accepted, with or without the `./`.
        for name in ["ok.txt", "./ok.txt"] {
            let file = dir.join("ok.tar");
            fs::write(&file, archive_with(name, EntryType::Regular, None)).unwrap();
            assert_eq!(check(&file).unwrap().len(), 1, "{name}");
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_folder_is_emptied_but_keeps_what_it_is_told() {
        let dir = scratch("empty");
        sample(&dir);
        empty_dir(&dir, &["cache"]).unwrap();
        assert!(dir.join("cache/junk").exists());
        assert!(!dir.join("databases").exists() && !dir.join("a.txt").exists());
        assert!(dir.exists(), "the folder itself stays");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn garbage_is_not_an_archive_and_does_not_panic() {
        let dir = scratch("garbage");
        let file = dir.join("g.tar");
        fs::write(&file, vec![0xAB; 2000]).unwrap();
        assert!(check(&file).is_err());
        fs::write(&file, b"").unwrap();
        assert!(check(&file).unwrap().is_empty());
        fs::remove_dir_all(dir).ok();
    }
}
