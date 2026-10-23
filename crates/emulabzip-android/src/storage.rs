//! Browsing an app's files: the private data directory through `run-as` (debuggable apps only)
//! and the app's folder in shared storage through the sync protocol.

use emulabzip_adb::{AdbClient, command, is_package_name, quote};
use emulabzip_core::files::{MAX_READ_BYTES, join, sort_entries};
use emulabzip_core::{Error, FileEntry, Result, StorageRoot};

pub const DATA: &str = "data";
pub const EXTERNAL: &str = "external";

fn external_base(app: &str) -> String {
    format!("/sdcard/Android/data/{app}")
}

fn check(app: &str) -> Result<()> {
    if is_package_name(app) { Ok(()) } else { Err(Error::invalid("package name", app)) }
}

/// `run-as` prints an explanation instead of failing loudly; turn it into an error.
fn run_as_error(out: &str) -> Option<Error> {
    let line = out.lines().find(|l| l.starts_with("run-as:"))?;
    let reason = line.trim_start_matches("run-as:").trim();
    Some(Error::Adb(if reason.contains("not debuggable") {
        "only debuggable apps can be browsed. Install a debug build of the app.".to_string()
    } else {
        reason.to_string()
    }))
}

pub async fn roots(adb: &AdbClient, serial: &str, app: &str) -> Result<Vec<StorageRoot>> {
    check(app)?;
    let id = adb.shell(serial, &command(["run-as", app, "id"])).await?;
    let data_unavailable = run_as_error(&id).map(|e| match e {
        Error::Adb(msg) => capitalize(&msg),
        other => other.to_string(),
    });

    let base = external_base(app);
    let mut sync = adb.sync(serial).await?;
    let external = sync.stat(&base).await;
    sync.close().await;
    let external_unavailable = match external {
        Ok(s) if s.exists() => None,
        _ => Some("The app has no files in shared storage yet.".to_string()),
    };

    Ok(vec![
        StorageRoot {
            id: DATA.into(),
            label: "App data".into(),
            location: format!("/data/data/{app}"),
            unavailable: data_unavailable,
        },
        StorageRoot {
            id: EXTERNAL.into(),
            label: "Shared storage".into(),
            location: base,
            unavailable: external_unavailable,
        },
    ])
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The script that lists a directory inside the app sandbox. It runs as the app, so the glob is
/// expanded with the app's permissions.
fn list_script(path: &str) -> String {
    let dir = if path.is_empty() { ".".to_string() } else { quote(path) };
    format!("cd {dir} && stat -c '%F|%s|%Y|%n' -- * .[!.]* 2>/dev/null; true")
}

/// Parses `stat -c '%F|%s|%Y|%n'` lines.
pub fn parse_stat(dir: &str, out: &str) -> Vec<FileEntry> {
    let mut entries: Vec<FileEntry> = out
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '|');
            let kind = parts.next()?;
            let size = parts.next()?.parse().ok()?;
            let modified = parts.next()?.parse().ok();
            let name = parts.next()?.to_string();
            if name.is_empty() || name == "*" || name == ".[!.]*" {
                return None;
            }
            Some(FileEntry { path: join(dir, &name), name, is_dir: kind == "directory", size, modified })
        })
        .collect();
    sort_entries(&mut entries);
    entries
}

pub async fn list(adb: &AdbClient, serial: &str, app: &str, root: &str, path: &str) -> Result<Vec<FileEntry>> {
    check(app)?;
    match root {
        DATA => {
            let out = adb.shell(serial, &command(["run-as", app, "sh", "-c", &list_script(path)])).await?;
            if let Some(e) = run_as_error(&out) {
                return Err(e);
            }
            Ok(parse_stat(path, &out))
        }
        EXTERNAL => {
            let remote = format!("{}/{}", external_base(app), path.trim_matches('/'));
            let mut entries: Vec<FileEntry> = adb
                .list(serial, remote.trim_end_matches('/'))
                .await?
                .into_iter()
                .map(|e| FileEntry {
                    path: join(path, &e.name),
                    is_dir: e.is_dir(),
                    size: e.size,
                    modified: Some(u64::from(e.mtime)),
                    name: e.name,
                })
                .collect();
            sort_entries(&mut entries);
            Ok(entries)
        }
        other => Err(Error::invalid("storage root", other)),
    }
}

pub async fn read(adb: &AdbClient, serial: &str, app: &str, root: &str, path: &str) -> Result<Vec<u8>> {
    check(app)?;
    if path.is_empty() {
        return Err(Error::invalid("file", path));
    }
    match root {
        DATA => {
            let size = adb.shell(serial, &command(["run-as", app, "stat", "-c", "%s", "--", path])).await?;
            if let Some(e) = run_as_error(&size) {
                return Err(e);
            }
            let size: u64 = size.trim().parse().map_err(|_| Error::NotFound(path.to_string()))?;
            if size > MAX_READ_BYTES {
                return Err(Error::Unsupported("Reading files larger than 256 MB"));
            }
            // `exec:` keeps the bytes intact, unlike `shell:`.
            adb.exec(serial, &command(["run-as", app, "cat", "--", path])).await
        }
        EXTERNAL => {
            let remote = format!("{}/{}", external_base(app), path);
            let mut bytes = Vec::new();
            adb.pull(serial, &remote, &mut bytes).await?;
            Ok(bytes)
        }
        other => Err(Error::invalid("storage root", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stat_output() {
        let out = "regular file|2048|1792773580|app.db\ndirectory|4096|1792773580|shared_prefs\nregular empty file|0|1792773580|a file|with pipe\n";
        let e = parse_stat("databases", out);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].name, "shared_prefs");
        assert!(e[0].is_dir);
        assert_eq!(e[0].path, "databases/shared_prefs");
        assert_eq!(e[1].name, "a file|with pipe");
        assert_eq!(e[2].size, 2048);
        assert_eq!(e[2].modified, Some(1792773580));
    }

    #[test]
    fn ignores_unmatched_globs() {
        assert!(parse_stat("", "directory|4096|0|*\n").is_empty());
    }

    #[test]
    fn builds_a_quoted_list_script() {
        assert_eq!(list_script(""), "cd . && stat -c '%F|%s|%Y|%n' -- * .[!.]* 2>/dev/null; true");
        assert!(list_script("files/My Docs").starts_with("cd 'files/My Docs' &&"));
    }

    #[test]
    fn explains_run_as_failures() {
        let e = run_as_error("run-as: package not debuggable: com.example\n").unwrap();
        assert!(e.to_string().contains("only debuggable apps"));
        assert!(run_as_error("uid=10190(u0_a190)").is_none());
    }
}
