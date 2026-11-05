//! The energy an app used, from `dumpsys batterystats`.
//!
//! Android keeps a power model per app. The device has to count as unplugged for it to count at all, so
//! a measurement starts by pretending to unplug the battery and resetting the statistics, and ends by
//! reading them and plugging the battery back in. That last step always runs.

use emulabzip_adb::{AdbClient, command, is_package_name};
use emulabzip_core::{EnergyComponent, EnergyReport, Error, Result, Wakelock};

/// `1h 2m 3s 450ms` as milliseconds.
pub fn parse_duration_ms(text: &str) -> u64 {
    let mut total = 0f64;
    for token in text.split_whitespace() {
        let digits: String = token.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let unit = &token[digits.len()..];
        let Ok(n) = digits.parse::<f64>() else { continue };
        total += match unit {
            "d" => n * 86_400_000.0,
            "h" => n * 3_600_000.0,
            "m" => n * 60_000.0,
            "s" => n * 1000.0,
            "ms" => n,
            _ => 0.0,
        };
    }
    total as u64
}

/// How `dumpsys batterystats` names a user id: `10146` is `u0a146`.
pub fn uid_label(uid: u32) -> String {
    if uid >= 10_000 { format!("u{}a{}", uid / 100_000, uid % 100_000 - 10_000) } else { uid.to_string() }
}

/// `12.3KB`, `4MB`, `800B`.
fn parse_bytes(text: &str) -> u64 {
    let t = text.trim();
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let n: f64 = digits.parse().unwrap_or(0.0);
    let factor = match t[digits.len()..].trim().to_ascii_uppercase().as_str() {
        "KB" => 1024.0,
        "MB" => 1024.0 * 1024.0,
        "GB" => 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    (n * factor) as u64
}

/// `(3 times)` or `(3 times) max=…` as 3.
fn times(text: &str) -> u64 {
    text.split_once('(').and_then(|(_, r)| r.split_once(" times")).and_then(|(n, _)| n.trim().parse().ok()).unwrap_or(0)
}

/// Reads one app's share of the report. `uid` is the app's numeric user id.
pub fn parse(app: &str, uid: u32, text: &str, seconds: u64) -> Result<EnergyReport> {
    let label = uid_label(uid);
    let mut report = EnergyReport {
        app: app.to_string(),
        seconds,
        estimated_mah: None,
        components: Vec::new(),
        cpu_ms: 0,
        wakelocks: Vec::new(),
        alarms: 0,
        jobs: 0,
        network_rx_bytes: 0,
        network_tx_bytes: 0,
        notes: Vec::new(),
    };

    // The model: `UID u0a146: 0.000276 fg: … ( cpu=0.000276 cpu:fg=… wifi=… )`
    let prefix = format!("UID {label}:");
    let mut found = false;
    if let Some(line) = text.lines().map(str::trim).find(|l| l.starts_with(&prefix)) {
        found = true;
        let rest = line[prefix.len()..].trim();
        report.estimated_mah = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        if let Some((_, inside)) = rest.split_once('(') {
            let inside = inside.split(')').next().unwrap_or("");
            for token in inside.split_whitespace() {
                if let Some((name, value)) = token.split_once('=')
                    && !name.contains(':')
                    && let Ok(mah) = value.parse::<f64>()
                {
                    report.components.push(EnergyComponent { name: name.to_string(), mah });
                }
            }
        }
    }

    // The app's own section: `  u0a146:` up to the next unindented line or the next user id.
    let header = format!("  {label}:");
    let mut inside = false;
    for line in text.lines() {
        if line == header {
            inside = true;
            found = true;
            continue;
        }
        if !inside {
            continue;
        }
        if !line.starts_with("    ") {
            break;
        }
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Total cpu time:") {
            // `u=3s 549ms s=4s 35ms`
            let user = rest.split_once("u=").map(|(_, r)| r.split(" s=").next().unwrap_or("")).unwrap_or("");
            let system = rest.split_once(" s=").map(|(_, r)| r).unwrap_or("");
            report.cpu_ms = parse_duration_ms(user) + parse_duration_ms(system);
        } else if let Some(rest) = t.strip_prefix("Wake lock ") {
            let (tag, details) = rest
                .split_once(": ")
                .or_else(|| rest.split_once(" realtime").map(|(a, _)| (a, "")))
                .unwrap_or((rest, ""));
            let ms = parse_duration_ms(details.split(['(', 'p']).next().unwrap_or(""));
            if tag.contains("*alarm*") {
                report.alarms += times(details).max(1);
            }
            report.wakelocks.push(Wakelock { tag: tag.trim().to_string(), ms, count: times(details).max(1) });
        } else if let Some(rest) = t.strip_prefix("Job ") {
            report.jobs += times(rest).max(1);
        } else if let Some(rest) = t.strip_prefix("Mobile network:").or_else(|| t.strip_prefix("Wi-Fi network:")) {
            let (rx, tx) = rest.split_once(" received,").unwrap_or((rest, ""));
            report.network_rx_bytes += parse_bytes(rx);
            report.network_tx_bytes += parse_bytes(tx.trim().split(" sent").next().unwrap_or(""));
        }
    }

    if !found {
        return Err(Error::NotFound(format!("the device recorded no use of energy by {app} (is it running?)")));
    }
    Ok(report.tidy())
}

/// The numeric user id of an app, from `pm list packages -U`.
pub fn parse_uid(out: &str, app: &str) -> Option<u32> {
    out.lines().find_map(|l| {
        let l = l.trim();
        let (pkg, uid) = l.strip_prefix("package:")?.split_once(" uid:")?;
        (pkg == app).then(|| uid.split(',').next()?.trim().parse().ok()).flatten()
    })
}

/// Starts counting: the battery is made to look unplugged, and the statistics start from zero.
pub async fn begin(adb: &AdbClient, serial: &str) -> Result<()> {
    adb.shell(serial, &command(["dumpsys", "battery", "unplug"])).await?;
    if let Err(e) = adb.shell(serial, &command(["dumpsys", "batterystats", "--reset"])).await {
        restore(adb, serial).await;
        return Err(e);
    }
    Ok(())
}

/// Puts the battery back as it was. Safe to call at any time, and never fails.
pub async fn restore(adb: &AdbClient, serial: &str) {
    let _ = adb.shell(serial, &command(["dumpsys", "battery", "reset"])).await;
}

/// Reads what `app` used since [`begin`], and restores the battery whatever happens.
pub async fn end(adb: &AdbClient, serial: &str, app: &str, seconds: u64) -> Result<EnergyReport> {
    let result = read(adb, serial, app, seconds).await;
    restore(adb, serial).await;
    result
}

async fn read(adb: &AdbClient, serial: &str, app: &str, seconds: u64) -> Result<EnergyReport> {
    if !is_package_name(app) {
        return Err(Error::invalid("package name", app));
    }
    let uid = parse_uid(&adb.shell(serial, &command(["pm", "list", "packages", "-U", app])).await?, app)
        .ok_or_else(|| Error::NotFound(format!("{app} is not installed")))?;
    let text = adb.shell(serial, &command(["dumpsys", "batterystats", "--charged", app])).await?;
    let mut report = parse(app, uid, &text, seconds)?;
    if report.estimated_mah.is_none() {
        report.notes.push(
            "The system gave no estimate for this app: it used too little in this time, or it was not running.".into(),
        );
    }
    if serial.starts_with("emulator-") {
        report.notes.push("An emulator has no battery: Android computes these numbers from a model, so compare runs with each other, not with a phone.".into());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME: &str = include_str!("../tests/fixtures/energy/batterystats-chrome.txt");

    #[test]
    fn reads_the_energy_of_a_real_app() {
        let r = parse("com.android.chrome", 10146, CHROME, 30).unwrap();
        assert_eq!(r.estimated_mah, Some(0.000276));
        assert_eq!(r.components.len(), 1, "{:?}", r.components);
        assert_eq!((r.components[0].name.as_str(), r.components[0].mah), ("cpu", 0.000276));
        // 3 s 549 ms of user time plus 4 s 35 ms of system time.
        assert_eq!(r.cpu_ms, 7584);
        assert!(r.wakelocks.iter().any(|w| w.tag == "*launch*"), "{:?}", r.wakelocks);
    }

    #[test]
    fn an_app_the_device_never_saw_is_explained() {
        let err = parse("com.example.ghost", 10999, CHROME, 30).unwrap_err();
        assert!(err.to_string().contains("no use of energy"), "{err}");
    }

    #[test]
    fn durations_and_user_ids() {
        assert_eq!(parse_duration_ms("3s 549ms"), 3549);
        assert_eq!(parse_duration_ms("1h 2m 3s 4ms"), 3_723_004);
        assert_eq!(parse_duration_ms("26s 97ms "), 26_097);
        assert_eq!(parse_duration_ms(""), 0);
        assert_eq!(uid_label(10146), "u0a146");
        assert_eq!(uid_label(1000), "1000");
        assert_eq!(uid_label(1_010_146), "u10a146");
    }

    #[test]
    fn finds_an_apps_user_id() {
        let out = "package:com.android.chrome uid:10146\npackage:com.android.chromecast uid:10150\n";
        assert_eq!(parse_uid(out, "com.android.chrome"), Some(10146));
        assert_eq!(parse_uid(out, "com.android"), None);
    }

    #[test]
    fn wake_locks_jobs_and_traffic_are_counted() {
        let text = "  Estimated power use (mAh):\n    UID u0a9: 0.5 ( cpu=0.3 wifi=0.2 )\n\n  u0a9:\n    Wake lock sync: 2s 500ms partial (3 times) max=900ms\n    Wake lock *alarm* realtime\n    Job com.x/.Sync: 1s realtime (4 times)\n    Mobile network: 1.5KB received, 2KB sent (packets 3 received, 2 sent)\n    Wi-Fi network: 1MB received, 512B sent (packets 9 received, 8 sent)\n    Total cpu time: u=1s s=500ms \n\nAfter\n";
        let r = parse("com.x", 10009, text, 10).unwrap();
        assert_eq!(r.estimated_mah, Some(0.5));
        assert_eq!(r.components.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["cpu", "wifi"]);
        assert_eq!((r.cpu_ms, r.jobs, r.alarms), (1500, 4, 1));
        assert_eq!(r.wakelocks.iter().find(|w| w.tag == "sync").map(|w| (w.ms, w.count)), Some((2500, 3)));
        assert_eq!(r.network_rx_bytes, 1536 + 1024 * 1024);
        assert_eq!(r.network_tx_bytes, 2048 + 512);
    }
}
