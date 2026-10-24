//! Changing what an Android device reports or does: dark mode, the status bar, permissions, the
//! battery, a notification, a link, and (on emulators) the location.
//!
//! Everything goes through `cmd`, `settings`, `pm`, `am` and `dumpsys`, which the shell user may
//! run, and through the emulator console for the location. Nothing is installed on the device.

use std::time::Duration;

use emulabzip_adb::{AdbClient, command};
use emulabzip_core::{Appearance, Control, ControlInfo, Error, Grant, PermissionState, Result, StatusBar};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const DEMO: &str = "com.android.systemui.demo";

/// The emulator's own default position, which clearing the location goes back to (the Googleplex).
const DEFAULT_LAT: &str = "37.422";
const DEFAULT_LON: &str = "-122.084";

/// The console port of `emulator-5554`.
pub fn console_port(serial: &str) -> Option<u16> {
    serial.strip_prefix("emulator-")?.parse().ok()
}

fn demo(args: &[&str]) -> String {
    let mut all = vec!["am", "broadcast", "-a", DEMO, "-e", "command"];
    all.extend_from_slice(args);
    command(all)
}

/// `9:41` as the `0941` that the demo clock wants.
pub fn hhmm(time: &str) -> Option<String> {
    let (h, m) = time.split_once(':')?;
    let (h, m): (u8, u8) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then(|| format!("{h:02}{m:02}"))
}

/// The shell commands that apply a control, in order. The location is not among them: it goes
/// through the emulator console.
pub fn commands_for(control: &Control) -> Result<Vec<String>> {
    Ok(match control {
        Control::Appearance { appearance } => {
            let mode = if *appearance == Appearance::Dark { "yes" } else { "no" };
            vec![command(["cmd", "uimode", "night", mode])]
        }
        Control::StatusBar { status_bar } => status_bar_commands(status_bar)?,
        Control::ClearStatusBar => vec![demo(&["exit"])],
        Control::Permission { app, permission, grant } => {
            let verb = if *grant == Grant::Grant { "grant" } else { "revoke" };
            vec![command(["pm", verb, app, permission])]
        }
        Control::Notification { title, body, .. } => {
            vec![command(["cmd", "notification", "post", "-S", "bigtext", "-t", title, "emulabzip", body])]
        }
        Control::OpenUrl { url } => vec![command(["am", "start", "-a", "android.intent.action.VIEW", "-d", url])],
        Control::Battery { level, charging } => {
            let mut v = vec![command(["dumpsys", "battery", "set", "level", &level.to_string()])];
            v.push(if *charging {
                command(["dumpsys", "battery", "set", "ac", "1"])
            } else {
                command(["dumpsys", "battery", "unplug"])
            });
            v
        }
        Control::ResetBattery => vec![command(["dumpsys", "battery", "reset"])],
        Control::Location { .. } | Control::ClearLocation => Vec::new(),
    })
}

fn status_bar_commands(sb: &StatusBar) -> Result<Vec<String>> {
    let mut v = vec![command(["settings", "put", "global", "sysui_demo_allowed", "1"]), demo(&["enter"])];
    if let Some(time) = &sb.time {
        let hhmm = hhmm(time).ok_or_else(|| Error::invalid("time", time.clone()))?;
        v.push(demo(&["clock", "-e", "hhmm", &hhmm]));
    }
    if let Some(level) = sb.battery {
        v.push(demo(&["battery", "-e", "level", &level.to_string(), "-e", "plugged", "false"]));
    }
    match sb.wifi {
        Some(true) => v.push(demo(&["network", "-e", "wifi", "show", "-e", "level", "4", "-e", "fully", "true"])),
        Some(false) => v.push(demo(&["network", "-e", "wifi", "hide"])),
        None => {}
    }
    match sb.cellular {
        Some(true) => v.push(demo(&["network", "-e", "mobile", "show", "-e", "datatype", "none", "-e", "level", "4"])),
        Some(false) => v.push(demo(&["network", "-e", "mobile", "hide"])),
        None => {}
    }
    // The point of a clean bar is to have nothing else in it.
    v.push(demo(&["notifications", "-e", "visible", "false"]));
    Ok(v)
}

/// What the shell printed when a command failed, if it did.
pub fn failure(output: &str) -> Option<String> {
    let lines: Vec<&str> = output.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let bad = lines.iter().position(|l| {
        l.starts_with("Exception occurred")
            || l.starts_with("Error:")
            || l.starts_with("Error ")
            || l.contains("Unknown command")
            || l.contains("Operation not allowed")
    })?;
    // The reason is on the next line for exceptions, and on the same line for the rest.
    let reason = if lines[bad].starts_with("Exception occurred") {
        lines.get(bad + 1).copied().unwrap_or(lines[bad])
    } else {
        lines[bad]
    };
    Some(reason.trim_start_matches("java.lang.IllegalArgumentException: ").trim_start_matches("Error: ").to_string())
}

pub fn parse_night(output: &str) -> Option<Appearance> {
    match output.lines().find_map(|l| l.trim().strip_prefix("Night mode: "))?.trim() {
        "yes" => Some(Appearance::Dark),
        "no" => Some(Appearance::Light),
        _ => None,
    }
}

/// The runtime permissions of an app, from `dumpsys package <app>`.
pub fn parse_runtime_permissions(dumpsys: &str) -> Vec<PermissionState> {
    let mut found: Vec<PermissionState> = Vec::new();
    let mut inside = false;
    let mut indent = 0;
    for line in dumpsys.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("runtime permissions:") {
            inside = true;
            indent = line.len() - trimmed.len();
            continue;
        }
        if !inside {
            continue;
        }
        // The section ends at the first line that is not indented deeper than its heading.
        if line.len() - trimmed.len() <= indent {
            inside = false;
            continue;
        }
        if let Some((name, rest)) = trimmed.split_once(": granted=") {
            let granted = rest.starts_with("true");
            if !found.iter().any(|p| p.name == name) {
                found.push(PermissionState { name: name.to_string(), granted });
            }
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

pub async fn info(adb: &AdbClient, serial: &str, app: Option<&str>) -> Result<ControlInfo> {
    let emulator = console_port(serial).is_some();
    let mut supported: Vec<&str> =
        vec!["appearance", "statusBar", "clearStatusBar", "notification", "openUrl", "battery", "resetBattery"];
    if app.is_some() {
        supported.push("permission");
    }
    let mut notes = Vec::new();
    if emulator {
        supported.extend(["location", "clearLocation"]);
    } else {
        notes.push("Location can only be changed on emulators.".to_string());
    }
    let appearance = adb.shell(serial, "cmd uimode night 2>&1").await.ok().and_then(|o| parse_night(&o));
    if appearance.is_none() {
        supported.retain(|s| *s != "appearance");
        notes.push("This Android version has no `cmd uimode night` (Android 10 and newer).".to_string());
    }
    let permissions = match app {
        Some(app) => parse_runtime_permissions(&adb.shell(serial, &command(["dumpsys", "package", app])).await?),
        None => Vec::new(),
    };
    Ok(ControlInfo { appearance, supported: supported.into_iter().map(String::from).collect(), permissions, notes })
}

fn auth_token() -> Result<String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| Error::failed("the home folder is unknown"))?;
    let path = std::path::Path::new(&home).join(".emulator_console_auth_token");
    std::fs::read_to_string(&path).map(|t| t.trim().to_string()).map_err(|_| {
        Error::failed(format!(
            "The emulator console token was not found at {}. Start the emulator once from this account.",
            path.display()
        ))
    })
}

/// Sends commands to the emulator console and waits for each answer.
async fn console(port: u16, commands: &[String]) -> Result<()> {
    let work = async {
        let stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .map_err(|e| Error::failed(format!("Could not reach the emulator console on port {port}: {e}")))?;
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        // The banner ends with OK.
        wait_ok(&mut lines).await?;
        let token = auth_token()?;
        write.write_all(format!("auth {token}\n").as_bytes()).await?;
        wait_ok(&mut lines).await?;
        for c in commands {
            write.write_all(format!("{c}\n").as_bytes()).await?;
            wait_ok(&mut lines).await?;
        }
        let _ = write.write_all(b"quit\n").await;
        Ok(())
    };
    tokio::time::timeout(Duration::from_secs(8), work)
        .await
        .map_err(|_| Error::failed("The emulator console did not answer in time."))?
}

async fn wait_ok(lines: &mut tokio::io::Lines<BufReader<tokio::net::tcp::OwnedReadHalf>>) -> Result<()> {
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line == "OK" {
            return Ok(());
        }
        if let Some(reason) = line.strip_prefix("KO") {
            return Err(Error::failed(format!(
                "The emulator console refused: {}",
                reason.trim_start_matches(':').trim()
            )));
        }
    }
    Err(Error::failed("The emulator console closed the connection."))
}

/// Runs `cmd` as the shell user. A notification is posted as the package of whoever runs
/// `cmd notification`, and when `adbd` is root that is `root`, which has no package.
pub fn as_shell_user(cmd: &str) -> String {
    command(["su", "2000", "sh", "-c", cmd])
}

pub async fn apply(adb: &AdbClient, serial: &str, control: &Control) -> Result<()> {
    match control {
        Control::Location { lat, lon } => set_location(serial, lat, lon).await,
        Control::ClearLocation => set_location(serial, DEFAULT_LAT, DEFAULT_LON).await,
        _ => {
            let rooted =
                matches!(control, Control::Notification { .. }) && adb.shell(serial, "id -u").await?.trim() == "0";
            for cmd in commands_for(control)? {
                let cmd = if rooted { as_shell_user(&cmd) } else { cmd };
                let out = adb.shell(serial, &format!("{cmd} 2>&1")).await?;
                if let Some(reason) = failure(&out) {
                    return Err(Error::Adb(reason));
                }
            }
            Ok(())
        }
    }
}

async fn set_location(serial: &str, lat: &str, lon: &str) -> Result<()> {
    let port = console_port(serial).ok_or(Error::Unsupported("Changing the location of a physical device"))?;
    // The console takes longitude first.
    console(port, &[format!("geo fix {} {}", lon.trim(), lat.trim())]).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use emulabzip_core::StatusBar;

    #[test]
    fn finds_the_console_port_of_an_emulator() {
        assert_eq!(console_port("emulator-5560"), Some(5560));
        assert_eq!(console_port("R58M123ABC"), None);
        assert_eq!(console_port("emulator-x"), None);
    }

    #[test]
    fn formats_the_demo_clock() {
        assert_eq!(hhmm("9:41").as_deref(), Some("0941"));
        assert_eq!(hhmm("23:05").as_deref(), Some("2305"));
        assert_eq!((hhmm("24:00"), hhmm("9:61"), hhmm("941")), (None, None, None));
    }

    #[test]
    fn builds_the_commands_for_each_control() {
        let c = |control: Control| commands_for(&control).unwrap();
        assert_eq!(c(Control::Appearance { appearance: Appearance::Dark }), ["cmd uimode night yes"]);
        assert_eq!(c(Control::ClearStatusBar), [format!("am broadcast -a {DEMO} -e command exit")]);
        assert_eq!(
            c(Control::Permission {
                app: "a.b".into(),
                permission: "android.permission.CAMERA".into(),
                grant: Grant::Revoke
            }),
            ["pm revoke a.b android.permission.CAMERA"]
        );
        assert_eq!(c(Control::ResetBattery), ["dumpsys battery reset"]);
        assert_eq!(
            c(Control::Battery { level: 15, charging: false }),
            ["dumpsys battery set level 15", "dumpsys battery unplug"]
        );
        assert_eq!(c(Control::Battery { level: 80, charging: true })[1], "dumpsys battery set ac 1");
        assert!(c(Control::Location { lat: "1".into(), lon: "2".into() }).is_empty());
    }

    #[test]
    fn runs_a_command_as_the_shell_user() {
        assert_eq!(
            as_shell_user("cmd notification post -t T tag body"),
            "su 2000 sh -c 'cmd notification post -t T tag body'"
        );
        assert!(as_shell_user("echo 'hi'").starts_with("su 2000 sh -c 'echo "), "single quotes inside are escaped");
    }

    #[test]
    fn quotes_what_comes_from_outside() {
        let cmds = commands_for(&Control::Notification {
            app: "a.b".into(),
            title: "it's $(rm -rf /)".into(),
            body: "two words; more".into(),
            payload: None,
        })
        .unwrap();
        let line = &cmds[0];
        assert!(line.starts_with("cmd notification post -S bigtext -t "));
        // Every special character sits inside single quotes.
        assert!(line.contains("'it'\\''s $(rm -rf /)'") && line.contains("'two words; more'"), "{line}");
        let open = commands_for(&Control::OpenUrl { url: "https://x.test/?a=1&b=2".into() }).unwrap();
        assert!(open[0].ends_with("-d 'https://x.test/?a=1&b=2'"), "{}", open[0]);
    }

    #[test]
    fn the_status_bar_enters_demo_mode_and_shows_what_was_asked() {
        let cmds = commands_for(&Control::StatusBar {
            status_bar: StatusBar {
                time: Some("9:41".into()),
                battery: Some(100),
                wifi: Some(true),
                cellular: Some(false),
            },
        })
        .unwrap();
        assert_eq!(cmds[0], "settings put global sysui_demo_allowed 1");
        assert!(cmds[1].ends_with("-e command enter"));
        assert!(cmds.iter().any(|c| c.ends_with("clock -e hhmm 0941")));
        assert!(cmds.iter().any(|c| c.contains("battery -e level 100 -e plugged false")));
        assert!(cmds.iter().any(|c| c.contains("-e wifi show")));
        assert!(cmds.iter().any(|c| c.contains("-e mobile hide")));
        assert!(cmds.last().unwrap().ends_with("notifications -e visible false"));
        let bare = commands_for(&Control::StatusBar { status_bar: StatusBar::default() }).unwrap();
        assert_eq!(bare.len(), 3, "enter demo mode and hide notifications");
        assert!(
            commands_for(&Control::StatusBar {
                status_bar: StatusBar { time: Some("99:99".into()), ..Default::default() }
            })
            .is_err()
        );
    }

    #[test]
    fn reads_the_failure_the_shell_printed() {
        let grant = "\nException occurred while executing 'grant':\njava.lang.IllegalArgumentException: Unknown permission: not.a.permission\n\tat com.android.server.pm.Foo.bar(Foo.java:1)\n";
        assert_eq!(failure(grant).as_deref(), Some("Unknown permission: not.a.permission"));
        assert_eq!(failure("Starting: Intent { act=android.intent.action.VIEW }\nError: Activity not started, unable to resolve Intent {...}\n").as_deref(), Some("Activity not started, unable to resolve Intent {...}"));
        assert!(failure("Broadcast completed: result=0").is_none() && failure("").is_none());
    }

    #[test]
    fn reads_dark_mode() {
        assert_eq!(parse_night("Night mode: yes\n"), Some(Appearance::Dark));
        assert_eq!(parse_night("Night mode: no\n"), Some(Appearance::Light));
        assert_eq!(parse_night("Night mode: auto\n"), None);
        assert_eq!(parse_night("cmd: Can't find service: uimode"), None);
    }

    #[test]
    fn lists_the_runtime_permissions_of_an_app() {
        let dump = "    User 0: ceDataInode=1\n      gids=[3003]\n      runtime permissions:\n        android.permission.READ_SMS: granted=true, flags=[ GRANTED_BY_DEFAULT]\n        android.permission.ACCESS_FINE_LOCATION: granted=false, flags=[ USER_SENSITIVE_WHEN_GRANTED]\n        android.permission.READ_SMS: granted=true, flags=[]\n      disabledComponents:\n        a.b.C: granted=true\n";
        let p = parse_runtime_permissions(dump);
        assert_eq!(
            p.iter().map(|p| (p.name.as_str(), p.granted)).collect::<Vec<_>>(),
            [("android.permission.ACCESS_FINE_LOCATION", false), ("android.permission.READ_SMS", true)]
        );
        assert!(parse_runtime_permissions("no permissions here").is_empty());
    }
}
