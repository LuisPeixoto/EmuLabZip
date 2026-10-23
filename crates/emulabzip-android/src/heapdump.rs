//! Heap dumps with `am dumpheap`.

use std::time::Duration;

use emulabzip_adb::{AdbClient, command, is_package_name};
use emulabzip_core::{Error, HeapDump, Result};

/// Where the dump is written on the device. `/data/local/tmp` is writable by the shell user, and
/// the file is opened by the system server on the app's behalf.
fn remote_path(stamp: &str) -> String {
    format!("/data/local/tmp/emulabzip-{stamp}.hprof")
}

/// Asks the app to dump its Java heap, waits for the file to be complete, pulls it and removes it
/// from the device. Works for debuggable apps, or any app on an `eng`/`userdebug` image.
pub async fn dump(adb: &AdbClient, serial: &str, app: &str, stamp: &str) -> Result<HeapDump> {
    if !is_package_name(app) {
        return Err(Error::invalid("package name", app));
    }
    let path = remote_path(stamp);
    let out = adb.shell(serial, &command(["am", "dumpheap", app, &path])).await?;
    if let Some(reason) = failure(&out) {
        let _ = adb.shell(serial, &command(["rm", "-f", &path])).await;
        return Err(Error::Adb(format!("{reason}. Heap dumps need a debuggable app.")));
    }

    // `am dumpheap` can return before the file is complete: wait until its size stops changing.
    let mut last = 0u64;
    let mut stable = 0;
    for _ in 0..240 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let size =
            adb.list(serial, "/data/local/tmp").await?.into_iter().find(|e| path.ends_with(&e.name)).map(|e| e.size);
        match size {
            Some(s) if s > 0 && s == last => {
                stable += 1;
                if stable >= 2 {
                    break;
                }
            }
            Some(s) => {
                last = s;
                stable = 0;
            }
            None => {}
        }
    }
    if last == 0 {
        let _ = adb.shell(serial, &command(["rm", "-f", &path])).await;
        return Err(Error::Adb("the heap dump was not written. Is the app debuggable?".into()));
    }

    let mut bytes = Vec::with_capacity(last as usize);
    let pulled = adb.pull(serial, &path, &mut bytes).await;
    let _ = adb.shell(serial, &command(["rm", "-f", &path])).await;
    pulled?;
    Ok(HeapDump { file_name: format!("{app}-{stamp}.hprof"), bytes })
}

/// The reason `am dumpheap` failed, from the exception it printed, if any.
///
/// ```text
/// Exception occurred while executing 'dumpheap':
/// java.lang.SecurityException: Process not debuggable: com.android.systemui
/// ```
fn failure(output: &str) -> Option<String> {
    if !output.contains("Exception") && !output.contains("Error:") {
        return None;
    }
    let line = output
        .lines()
        .map(str::trim)
        .find(|l| (l.contains("Exception:") || l.starts_with("Error:")) && !l.starts_with("Exception occurred"))
        .unwrap_or("am dumpheap failed");
    // Drop the Java class name: `java.lang.SecurityException: Process not debuggable` → the message.
    Some(match line.split_once(": ") {
        Some((class, message)) if class.contains('.') || class == "Error" => message.to_string(),
        _ => line.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_failure_reason() {
        let out = "File: /data/local/tmp/t.hprof\n\nException occurred while executing 'dumpheap':\njava.lang.SecurityException: Process not debuggable: com.android.systemui\n\tat com.android.server.am.ActivityManagerService.enforceDebuggable(ActivityManagerService.java:6155)\n";
        assert_eq!(failure(out).as_deref(), Some("Process not debuggable: com.android.systemui"));
        assert_eq!(failure("File: /data/local/tmp/t.hprof\nWaiting for dump to finish...\n"), None);
    }

    #[test]
    fn writes_to_the_shell_temp_directory() {
        assert_eq!(remote_path("20261023-131500"), "/data/local/tmp/emulabzip-20261023-131500.hprof");
    }
}
