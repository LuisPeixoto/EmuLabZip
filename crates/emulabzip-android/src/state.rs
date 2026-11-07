//! Saving and restoring an app's data, and writing into its storage.
//!
//! The private data directory is reached with `run-as`, so this works for debuggable apps only, like browsing it does.
//! Data goes out through `exec:` (which keeps binary bytes intact) as a `tar` stream, and comes back through a file in
//! `/data/local/tmp`, which `run-as` can read, unpacked as the app, so the files keep belonging to it.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use emulabzip_adb::{AdbClient, command, is_package_name, quote};
use emulabzip_core::tarball;
use emulabzip_core::{DATA_TAR, EXTERNAL_TAR, Error, Result, StateFiles};

use crate::storage::{DATA, EXTERNAL, external_base, run_as_error};

/// What is left out of a saved sandbox: caches the app rebuilds, and its native libraries (a link into the install).
const SKIP: [&str; 3] = ["cache", "code_cache", "lib"];

fn check(app: &str) -> Result<()> {
    if is_package_name(app) { Ok(()) } else { Err(Error::invalid("package name", app)) }
}

fn data_dir(app: &str) -> String {
    format!("/data/data/{app}")
}

/// A name for a file on the device that no other run shares.
fn scratch_name() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("/data/local/tmp/emulabzip-state-{nanos:x}.tar")
}

pub async fn stop(adb: &AdbClient, serial: &str, app: &str) -> Result<()> {
    check(app)?;
    adb.shell(serial, &command(["am", "force-stop", app])).await?;
    Ok(())
}

/// `1.2.0 (42)` from `dumpsys package`, or the part of it that is there.
pub fn parse_version(dumpsys: &str) -> Option<String> {
    let find = |key: &str| {
        dumpsys.lines().find_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix(key)?;
            Some(rest.split_whitespace().next()?.to_string())
        })
    };
    match (find("versionName="), find("versionCode=")) {
        (Some(name), Some(code)) => Some(format!("{name} ({code})")),
        (Some(name), None) => Some(name),
        (None, Some(code)) => Some(code),
        _ => None,
    }
}

async fn version(adb: &AdbClient, serial: &str, app: &str) -> Option<String> {
    let out = adb.shell(serial, &command(["dumpsys", "package", app])).await.ok()?;
    parse_version(&out)
}

/// The app's data, checked: `run-as` answers with a message when it may not look inside.
async fn require_debuggable(adb: &AdbClient, serial: &str, app: &str) -> Result<()> {
    let id = adb.shell(serial, &command(["run-as", app, "id"])).await?;
    match run_as_error(&id) {
        Some(e) => Err(match e {
            Error::Adb(m) if m.contains("debuggable") => Error::failed(
                "Only debuggable apps can be saved: Android keeps the data of the others out of reach. Install a debug build of the app.",
            ),
            other => other,
        }),
        None => Ok(()),
    }
}

pub async fn save(
    adb: &AdbClient,
    serial: &str,
    app: &str,
    out_dir: &Path,
    include_external: bool,
) -> Result<StateFiles> {
    check(app)?;
    require_debuggable(adb, serial, app).await?;
    // The app is stopped so a database is not written to while it is read.
    stop(adb, serial, app).await?;
    std::fs::create_dir_all(out_dir)?;

    let excludes: Vec<String> = SKIP.iter().map(|n| format!("--exclude=./{n}")).collect();
    let mut args =
        vec!["run-as".to_string(), app.to_string(), "tar".into(), "-cf".into(), "-".into(), "-C".into(), data_dir(app)];
    args.extend(excludes);
    args.push(".".into());
    let bytes = adb.exec(serial, &format!("{} 2>/dev/null", command(args))).await?;
    let data = out_dir.join(DATA_TAR);
    std::fs::write(&data, &bytes)?;
    // The stream carries no exit code, so the archive is read to see that it is one.
    let entries = tarball::check(&data).map_err(|_| {
        Error::failed("The device did not return the app's data as an archive. Is the app debuggable, and installed?")
    })?;
    if entries.is_empty() {
        return Err(Error::failed("The app has no data to save."));
    }

    let mut external = None;
    if include_external {
        let base = external_base(app);
        let mut sync = adb.sync(serial).await?;
        let exists = sync.stat(&base).await.map(|s| s.exists()).unwrap_or(false);
        sync.close().await;
        if exists {
            let bytes = adb.exec(serial, &format!("tar -cf - -C {} . 2>/dev/null", quote(&base))).await?;
            let path = out_dir.join(EXTERNAL_TAR);
            std::fs::write(&path, &bytes)?;
            if tarball::check(&path).map(|e| e.len() > 1).unwrap_or(false) {
                external = Some(path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(StateFiles { data, external, app_version: version(adb, serial, app).await })
}

/// The script that empties the sandbox (but for what a save leaves out) and unpacks the archive into it.
fn restore_script(app: &str, archive: &str) -> String {
    let mut keep = String::new();
    for name in SKIP {
        keep.push_str(&format!(" ! -name {name}"));
    }
    format!(
        "cd {dir} && find . -mindepth 1 -maxdepth 1{keep} -exec rm -rf {{}} + ; tar -xf {archive} -C {dir}; echo emulabzip-exit=$?",
        dir = quote(&data_dir(app)),
        archive = quote(archive),
    )
}

pub async fn restore(adb: &AdbClient, serial: &str, app: &str, data: &Path, external: Option<&Path>) -> Result<()> {
    check(app)?;
    // The archive is looked at here, before the device is touched.
    tarball::check(data)?;
    if let Some(external) = external {
        tarball::check(external)?;
    }
    require_debuggable(adb, serial, app).await?;
    stop(adb, serial, app).await?;

    let remote = scratch_name();
    adb.push(serial, &remote, 0o644, &std::fs::read(data)?).await?;
    let out = adb.shell(serial, &command(["run-as", app, "sh", "-c", &restore_script(app, &remote)])).await;
    let _ = adb.shell(serial, &command(["rm", "-f", &remote])).await;
    let out = out?;
    if let Some(e) = run_as_error(&out) {
        return Err(e);
    }
    if !out.lines().any(|l| l.trim() == "emulabzip-exit=0") {
        return Err(Error::failed(format!(
            "The device could not unpack the app's data: {}",
            out.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no answer")
        )));
    }

    if let Some(external) = external {
        let base = external_base(app);
        let remote = scratch_name();
        adb.push(serial, &remote, 0o644, &std::fs::read(external)?).await?;
        let script = format!(
            "mkdir -p {b} && find {b} -mindepth 1 -maxdepth 1 -exec rm -rf {{}} + ; tar -xf {a} -C {b}; echo emulabzip-exit=$?",
            b = quote(&base),
            a = quote(&remote)
        );
        let out = adb.shell(serial, &script).await;
        let _ = adb.shell(serial, &command(["rm", "-f", &remote])).await;
        if !out?.lines().any(|l| l.trim() == "emulabzip-exit=0") {
            return Err(Error::failed("The device could not unpack the app's files on shared storage."));
        }
    }
    Ok(())
}

/// Where a path inside a root lives on the device.
fn full_path(app: &str, root: &str, path: &str) -> Result<String> {
    match root {
        DATA => Ok(format!("{}/{path}", data_dir(app))),
        EXTERNAL => Ok(format!("{}/{path}", external_base(app))),
        other => Err(Error::invalid("storage root", other)),
    }
}

/// The script that puts a pushed file where it belongs, as the app: the contents are copied over the file, so it keeps
/// its owner and permissions, and its folders are made if they are missing.
fn write_script(target: &str, scratch: &str) -> String {
    let t = quote(target);
    format!("mkdir -p \"$(dirname {t})\" && cat {s} > {t}; echo emulabzip-exit=$?", s = quote(scratch))
}

pub async fn write(adb: &AdbClient, serial: &str, app: &str, root: &str, path: &str, bytes: &[u8]) -> Result<()> {
    check(app)?;
    let target = full_path(app, root, path)?;
    let remote = scratch_name();
    adb.push(serial, &remote, 0o644, bytes).await?;
    let script = write_script(&target, &remote);
    let out = if root == DATA {
        adb.shell(serial, &command(["run-as", app, "sh", "-c", &script])).await
    } else {
        adb.shell(serial, &script).await
    };
    let _ = adb.shell(serial, &command(["rm", "-f", &remote])).await;
    let out = out?;
    if let Some(e) = run_as_error(&out) {
        return Err(e);
    }
    if out.lines().any(|l| l.trim() == "emulabzip-exit=0") {
        Ok(())
    } else {
        Err(Error::failed(format!("The device could not write {path}: {}", out.trim())))
    }
}

pub async fn remove(adb: &AdbClient, serial: &str, app: &str, root: &str, path: &str) -> Result<()> {
    check(app)?;
    let target = full_path(app, root, path)?;
    let out = if root == DATA {
        adb.shell(serial, &command(["run-as", app, "rm", "-f", "--", &target])).await?
    } else {
        adb.shell(serial, &command(["rm", "-f", "--", &target])).await?
    };
    match run_as_error(&out) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_from_dumpsys() {
        let out = "Packages:\n  Package [dev.x] (abc):\n    versionCode=42 minSdk=24 targetSdk=35\n    versionName=1.2.0\n    other=1\n";
        assert_eq!(parse_version(out).as_deref(), Some("1.2.0 (42)"));
        assert_eq!(parse_version("versionCode=7 minSdk=1").as_deref(), Some("7"));
        assert_eq!(parse_version("versionName=2.0").as_deref(), Some("2.0"));
        assert_eq!(parse_version("nothing"), None);
    }

    #[test]
    fn the_restore_script_empties_the_sandbox_but_keeps_what_a_save_leaves_out() {
        let s = restore_script("dev.x", "/data/local/tmp/a.tar");
        assert!(s.starts_with("cd /data/data/dev.x && find . -mindepth 1 -maxdepth 1"));
        for name in SKIP {
            assert!(s.contains(&format!("! -name {name}")), "{s}");
        }
        assert!(s.contains("tar -xf /data/local/tmp/a.tar -C /data/data/dev.x") && s.contains("emulabzip-exit=$?"));
    }

    #[test]
    fn a_write_copies_over_the_file_and_makes_its_folders() {
        let s = write_script("/data/data/dev.x/databases/my db", "/data/local/tmp/s.tar");
        assert!(s.contains("mkdir -p \"$(dirname '/data/data/dev.x/databases/my db')\""), "{s}");
        assert!(s.contains("cat /data/local/tmp/s.tar > '/data/data/dev.x/databases/my db'"), "{s}");
    }

    #[test]
    fn roots_map_to_their_folders_and_others_are_refused() {
        assert_eq!(full_path("dev.x", "data", "a/b").unwrap(), "/data/data/dev.x/a/b");
        assert_eq!(full_path("dev.x", "external", "a").unwrap(), "/sdcard/Android/data/dev.x/a");
        assert!(full_path("dev.x", "bundle", "a").is_err());
    }

    #[test]
    fn scratch_names_do_not_repeat() {
        assert_ne!(scratch_name(), scratch_name());
        assert!(scratch_name().starts_with("/data/local/tmp/emulabzip-state-"));
    }
}
