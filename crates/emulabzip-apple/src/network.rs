//! Routing the iOS Simulator's traffic to the proxy, and trusting its certificate.
//!
//! A simulator uses the network of the Mac it runs on, so the only way to capture its traffic
//! without changing the app is to point the Mac's own HTTP and HTTPS proxy at EmuLabZip. That
//! affects every app on the Mac while it is on, so the settings it replaces are saved first and
//! put back when the switch is turned off, when EmuLabZip quits, and on the next start if it
//! was killed in between.

use std::path::{Path, PathBuf};

use emulabzip_core::{CaInstall, CaKind, Error, NetworkStatus, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::simctl;

/// The file, next to the CA, that remembers the settings to restore.
pub const STATE_FILE: &str = "macos-proxy.json";

const HOST: &str = "127.0.0.1";

/// One proxy setting of a network service, as `networksetup -getwebproxy` reports it. A disabled
/// proxy can still hold a server and port, and they are kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    pub enabled: bool,
    pub server: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceProxy {
    pub service: String,
    pub web: Setting,
    pub secure: Setting,
}

/// The enabled network services. The first line of the output is a banner, and disabled
/// services start with an asterisk.
pub fn parse_services(output: &str) -> Vec<String> {
    output.lines().skip(1).filter(|l| !l.trim().is_empty() && !l.starts_with('*')).map(str::to_string).collect()
}

/// Reads `networksetup -getwebproxy` or `-getsecurewebproxy`:
///
/// ```text
/// Enabled: Yes
/// Server: 127.0.0.1
/// Port: 9090
/// Authenticated Proxy Enabled: 0
/// ```
pub fn parse_setting(output: &str) -> Setting {
    let field = |name: &str| output.lines().find_map(|l| l.strip_prefix(name)?.strip_prefix(':')).map(str::trim);
    Setting {
        enabled: field("Enabled").is_some_and(|v| v.eq_ignore_ascii_case("yes")),
        server: field("Server").unwrap_or_default().to_string(),
        port: field("Port").and_then(|p| p.parse().ok()).unwrap_or(0),
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

/// The `networksetup` calls that point every service at the proxy.
pub fn connect_commands(services: &[String], port: u16) -> Vec<Vec<String>> {
    let port = port.to_string();
    services
        .iter()
        .flat_map(|s| [strings(&["-setwebproxy", s, HOST, &port]), strings(&["-setsecurewebproxy", s, HOST, &port])])
        .collect()
}

/// The calls that put the saved settings back. Setting a server turns the proxy on, so a proxy
/// that was off is switched off again afterwards.
pub fn restore_commands(saved: &[ServiceProxy]) -> Vec<Vec<String>> {
    let mut cmds = Vec::new();
    for s in saved {
        for (set, state, setting) in [
            ("-setwebproxy", "-setwebproxystate", &s.web),
            ("-setsecurewebproxy", "-setsecurewebproxystate", &s.secure),
        ] {
            if !setting.server.is_empty() && setting.port != 0 {
                cmds.push(vec![set.to_string(), s.service.clone(), setting.server.clone(), setting.port.to_string()]);
            }
            cmds.push(vec![
                state.to_string(),
                s.service.clone(),
                if setting.enabled { "on" } else { "off" }.to_string(),
            ]);
        }
    }
    cmds
}

/// The effective proxy from `scutil --proxy`, which merges every service.
pub fn parse_scutil(output: &str) -> Option<String> {
    let get = |key: &str| {
        output.lines().find_map(|l| {
            let (k, v) = l.trim().split_once(" : ")?;
            (k == key).then(|| v.trim().to_string())
        })
    };
    let (host, port) = if get("HTTPSEnable").as_deref() == Some("1") {
        (get("HTTPSProxy")?, get("HTTPSPort")?)
    } else if get("HTTPEnable").as_deref() == Some("1") {
        (get("HTTPProxy")?, get("HTTPPort")?)
    } else {
        return None;
    };
    Some(format!("{host}:{port}"))
}

const NOTE_OFF: &str = "The simulator uses this Mac's network, so connecting sets the Mac's own HTTP and HTTPS proxy.";
const NOTE_ON: &str = "All HTTP and HTTPS traffic from this Mac goes through EmuLabZip while this is on. It is turned off \
                       again when you disconnect or quit.";

fn unsupported() -> Error {
    Error::Unsupported("Capturing the iOS Simulator's traffic outside macOS")
}

async fn run(program: &str, args: &[String]) -> Result<String> {
    let out = Command::new(program).args(args).kill_on_drop(true).output().await?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let message =
            [stderr.trim(), stdout.trim()].into_iter().find(|m| !m.is_empty()).unwrap_or("the command failed");
        Err(Error::Simctl(format!("{program}: {message}")))
    }
}

async fn services() -> Result<Vec<String>> {
    Ok(parse_services(&run("networksetup", &strings(&["-listallnetworkservices"])).await?))
}

async fn read_all(services: &[String]) -> Result<Vec<ServiceProxy>> {
    let mut all = Vec::new();
    for service in services {
        let web = parse_setting(&run("networksetup", &["-getwebproxy".to_string(), service.clone()]).await?);
        let secure = parse_setting(&run("networksetup", &["-getsecurewebproxy".to_string(), service.clone()]).await?);
        all.push(ServiceProxy { service: service.clone(), web, secure });
    }
    Ok(all)
}

/// Runs the commands as the user, and if the system refuses, once more with an administrator
/// prompt for the whole batch.
async fn apply(commands: &[Vec<String>]) -> Result<()> {
    let mut direct = Ok(());
    for cmd in commands {
        if let Err(e) = run("networksetup", cmd).await {
            direct = Err(e);
            break;
        }
    }
    if direct.is_ok() {
        return Ok(());
    }
    let script = commands
        .iter()
        .map(|c| {
            let quoted: Vec<String> = c.iter().map(|a| format!("'{}'", a.replace('\'', r"'\''"))).collect();
            format!("/usr/sbin/networksetup {}", quoted.join(" "))
        })
        .collect::<Vec<_>>()
        .join(" && ");
    let apple = format!(
        "do shell script \"{}\" with administrator privileges with prompt \"EmuLabZip wants to change your proxy settings.\"",
        script.replace('\\', "\\\\").replace('"', "\\\"")
    );
    run("osascript", &["-e".to_string(), apple]).await.map(|_| ())
}

pub async fn status() -> Result<NetworkStatus> {
    if !cfg!(target_os = "macos") {
        return Err(unsupported());
    }
    let proxy = parse_scutil(&run("scutil", &strings(&["--proxy"])).await?);
    let connected = proxy.as_deref().is_some_and(|p| p.starts_with(HOST));
    let note = Some(if connected { NOTE_ON } else { NOTE_OFF }.to_string());
    Ok(NetworkStatus { proxy, connected, note })
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join(STATE_FILE)
}

pub async fn connect(state_dir: &Path, port: u16) -> Result<NetworkStatus> {
    if !cfg!(target_os = "macos") {
        return Err(unsupported());
    }
    let services = services().await?;
    let current = read_all(&services).await?;
    // Connecting twice must not save our own settings as the ones to restore.
    let ours = current.iter().any(|s| s.web.enabled && s.web.server == HOST);
    let path = state_path(state_dir);
    if !(ours && path.exists()) {
        tokio::fs::create_dir_all(state_dir).await?;
        let json = serde_json::to_vec_pretty(&current).map_err(|e| Error::parse("proxy settings", e.to_string()))?;
        tokio::fs::write(&path, json).await?;
    }
    apply(&connect_commands(&services, port)).await?;
    status().await
}

/// Puts the Mac's proxy settings back. Without a saved copy, it only switches off proxies that
/// point at this computer.
pub async fn disconnect(state_dir: &Path) -> Result<NetworkStatus> {
    if !cfg!(target_os = "macos") {
        return Err(unsupported());
    }
    let path = state_path(state_dir);
    let saved: Option<Vec<ServiceProxy>> =
        tokio::fs::read(&path).await.ok().and_then(|b| serde_json::from_slice(&b).ok());
    match saved {
        Some(saved) => apply(&restore_commands(&saved)).await?,
        None => {
            let current = read_all(&services().await?).await?;
            let ours: Vec<ServiceProxy> = current
                .into_iter()
                .map(|mut s| {
                    s.web.enabled = false;
                    s.secure.enabled = false;
                    s
                })
                .filter(|s| s.web.server == HOST || s.secure.server == HOST)
                .collect();
            let cmds: Vec<Vec<String>> = ours
                .iter()
                .flat_map(|s| {
                    [
                        strings(&["-setwebproxystate", &s.service, "off"]),
                        strings(&["-setsecurewebproxystate", &s.service, "off"]),
                    ]
                })
                .collect();
            if !cmds.is_empty() {
                apply(&cmds).await?;
            }
        }
    }
    let _ = tokio::fs::remove_file(&path).await;
    status().await
}

/// Undoes a connection that was left behind, for example when EmuLabZip was killed. Returns
/// whether there was anything to undo.
pub async fn restore_leftover(state_dir: &Path) -> Result<bool> {
    if !cfg!(target_os = "macos") || !state_path(state_dir).exists() {
        return Ok(false);
    }
    disconnect(state_dir).await?;
    Ok(true)
}

/// Adds the certificate to the simulator's keychain as a trusted root.
pub async fn install_ca(udid: &str, pem: &str) -> Result<CaInstall> {
    let file = std::env::temp_dir().join(format!("emulabzip-ca-{}.pem", std::process::id()));
    tokio::fs::write(&file, pem).await?;
    let result =
        simctl::run([std::ffi::OsStr::new("keychain"), udid.as_ref(), "add-root-cert".as_ref(), file.as_os_str()])
            .await;
    let _ = tokio::fs::remove_file(&file).await;
    result?;
    Ok(CaInstall {
        kind: CaKind::Simulator,
        done: true,
        message: "The certificate is trusted by this simulator. Restart apps that were already running.".into(),
        steps: vec![],
        snippet: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEB_OFF: &str = "Enabled: No\nServer: 127.0.0.1\nPort: 9393\nAuthenticated Proxy Enabled: 0\n";
    const WEB_ON: &str = "Enabled: Yes\nServer: proxy.corp\nPort: 8080\nAuthenticated Proxy Enabled: 0\n";

    #[test]
    fn lists_enabled_services_only() {
        let out = "An asterisk (*) denotes that a network service is disabled.\nWi-Fi\n*Thunderbolt Bridge\nEthernet\n";
        assert_eq!(parse_services(out), ["Wi-Fi", "Ethernet"]);
    }

    #[test]
    fn reads_a_proxy_setting_even_when_it_is_off() {
        assert_eq!(parse_setting(WEB_OFF), Setting { enabled: false, server: "127.0.0.1".into(), port: 9393 });
        assert_eq!(parse_setting(WEB_ON), Setting { enabled: true, server: "proxy.corp".into(), port: 8080 });
        assert_eq!(parse_setting(""), Setting::default());
    }

    #[test]
    fn builds_the_commands_to_connect() {
        let cmds = connect_commands(&["Wi-Fi".to_string()], 9090);
        assert_eq!(cmds[0], ["-setwebproxy", "Wi-Fi", "127.0.0.1", "9090"]);
        assert_eq!(cmds[1], ["-setsecurewebproxy", "Wi-Fi", "127.0.0.1", "9090"]);
    }

    #[test]
    fn restores_an_enabled_proxy_and_a_disabled_one_with_its_leftovers() {
        let saved = vec![
            ServiceProxy { service: "Wi-Fi".into(), web: parse_setting(WEB_ON), secure: parse_setting(WEB_ON) },
            ServiceProxy { service: "Ethernet".into(), web: parse_setting(WEB_OFF), secure: Setting::default() },
        ];
        let cmds = restore_commands(&saved);
        assert!(cmds.contains(&strings(&["-setwebproxy", "Wi-Fi", "proxy.corp", "8080"])));
        assert!(cmds.contains(&strings(&["-setwebproxystate", "Wi-Fi", "on"])));
        // Setting a server turns the proxy on, so the one that was off is switched off again.
        assert!(cmds.contains(&strings(&["-setwebproxy", "Ethernet", "127.0.0.1", "9393"])));
        assert!(cmds.contains(&strings(&["-setwebproxystate", "Ethernet", "off"])));
        // A service that never had a proxy gets no server, only the state.
        assert!(!cmds.iter().any(|c| c[0] == "-setsecurewebproxy" && c[1] == "Ethernet"));
        assert!(cmds.contains(&strings(&["-setsecurewebproxystate", "Ethernet", "off"])));
    }

    #[test]
    fn saved_settings_survive_a_round_trip() {
        let saved =
            vec![ServiceProxy { service: "Wi-Fi".into(), web: parse_setting(WEB_ON), secure: Setting::default() }];
        let json = serde_json::to_vec(&saved).unwrap();
        assert_eq!(serde_json::from_slice::<Vec<ServiceProxy>>(&json).unwrap(), saved);
    }

    #[test]
    fn reads_the_effective_proxy() {
        let on = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 9090\n  HTTPProxy : 127.0.0.1\n  HTTPSEnable : 1\n  HTTPSPort : 9090\n  HTTPSProxy : 127.0.0.1\n}";
        assert_eq!(parse_scutil(on).as_deref(), Some("127.0.0.1:9090"));
        assert_eq!(parse_scutil("<dictionary> {\n  HTTPEnable : 0\n  HTTPSEnable : 0\n}"), None);
    }
}
