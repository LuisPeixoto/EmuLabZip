//! Sending an Android device's traffic to the proxy, and trusting its certificate.
//!
//! The device reaches the proxy on this computer through `adb reverse`, so `127.0.0.1:<port>` on
//! the device is the proxy. That works for emulators, USB devices and wireless debugging alike,
//! without touching the network.

use std::time::Duration;

use emulabzip_adb::AdbClient;
use emulabzip_core::{ANDROID_SECURITY_CONFIG, CaInstall, CaKind, Error, NetworkStatus, Result};

/// The device-side path the certificate is copied to before it is installed.
const DEVICE_PEM: &str = "/data/local/tmp/emulabzip-ca.pem";
const DOWNLOAD_CRT: &str = "/sdcard/Download/emulabzip-ca.crt";
/// Present in the store directory once our copy of it is mounted.
const MARKER: &str = ".emulabzip";

/// Reads `settings get global http_proxy`: `127.0.0.1:9090`, `:0` and `null` mean off.
pub fn parse_proxy(output: &str) -> Option<String> {
    let value = output.trim();
    (!value.is_empty() && value != "null" && value != ":0" && !value.ends_with(":0")).then(|| value.to_string())
}

/// The port of a `127.0.0.1:<port>` proxy, which is how this app points a device at itself.
pub fn local_port(proxy: &str) -> Option<u16> {
    proxy.strip_prefix("127.0.0.1:")?.parse().ok()
}

pub async fn status(adb: &AdbClient, serial: &str) -> Result<NetworkStatus> {
    let proxy = parse_proxy(&adb.shell(serial, "settings get global http_proxy").await?);
    let connected = proxy.as_deref().and_then(local_port).is_some();
    let note = match &proxy {
        Some(p) if !connected => Some(format!("The device already uses the proxy {p}. Connecting replaces it.")),
        _ => None,
    };
    Ok(NetworkStatus { proxy, connected, note })
}

pub async fn connect(adb: &AdbClient, serial: &str, port: u16) -> Result<NetworkStatus> {
    let spec = format!("tcp:{port}");
    adb.reverse(serial, &spec, &spec).await?;
    adb.shell(serial, &format!("settings put global http_proxy 127.0.0.1:{port}")).await?;
    status(adb, serial).await
}

pub async fn disconnect(adb: &AdbClient, serial: &str) -> Result<NetworkStatus> {
    let current = parse_proxy(&adb.shell(serial, "settings get global http_proxy").await?);
    adb.shell(serial, "settings put global http_proxy :0").await?;
    if let Some(port) = current.as_deref().and_then(local_port) {
        // Only our own forward: tools such as Metro keep theirs.
        let _ = adb.reverse_remove(serial, &format!("tcp:{port}")).await;
    }
    status(adb, serial).await
}

// ---------------------------------------------------------------------------------------------
// Certificates
// ---------------------------------------------------------------------------------------------

/// Whether the message of `adb root` means adbd is now running as root.
pub fn root_granted(message: &str) -> bool {
    message.contains("restarting adbd as root") || message.contains("already running as root")
}

/// The script that adds the certificate to the system store of a rooted device.
///
/// `/system` cannot be written on most images, so a copy of the store with our certificate is put
/// in a tmpfs that replaces the directory. Since Android 14 apps read the store from the
/// conscrypt APEX instead, so the copy is also bind-mounted over it, in the namespaces of the
/// zygotes and of the apps they have already started. It lasts until the device reboots.
///
/// Running it again must not stack another set of mounts, which slows the device down until it
/// stops answering. A marker file in the tmpfs tells that it is already in place; every namespace
/// shares that one filesystem, so adding or replacing the file there is enough.
pub fn system_install_script(name: &str) -> String {
    format!(
        r#"DIR=/system/etc/security/cacerts
add() {{
  cp {DEVICE_PEM} $DIR/{name}
  chown root:root $DIR/{name}
  chmod 644 $DIR/{name}
  chcon u:object_r:system_file:s0 $DIR/{name}
}}
if [ -e $DIR/{MARKER} ]; then
  add
  rm -f {DEVICE_PEM}
  echo updated
  exit 0
fi
set -e
TMP=/data/local/tmp/emulabzip-certs
rm -rf $TMP && mkdir -p -m 700 $TMP
if [ -d /apex/com.android.conscrypt/cacerts ]; then
  cp /apex/com.android.conscrypt/cacerts/* $TMP/
else
  cp $DIR/* $TMP/
fi
mount -t tmpfs tmpfs $DIR
cp $TMP/* $DIR/
chown root:root $DIR/*
chmod 644 $DIR/*
chcon u:object_r:system_file:s0 $DIR/*
add
touch $DIR/{MARKER}
if [ -d /apex/com.android.conscrypt/cacerts ]; then
  /bin/mount --bind $DIR /apex/com.android.conscrypt/cacerts
  for Z in $(pidof zygote) $(pidof zygote64); do
    timeout 10 nsenter --mount=/proc/$Z/ns/mnt -- /bin/mount --bind $DIR /apex/com.android.conscrypt/cacerts
    for P in $(ps -A -o PID -P $Z | grep -v PID); do
      timeout 10 nsenter --mount=/proc/$P/ns/mnt -- /bin/mount --bind $DIR /apex/com.android.conscrypt/cacerts &
    done
  done
  wait
fi
rm -rf $TMP {DEVICE_PEM}
echo installed
"#
    )
}

/// How long installing may take. The device is busy while it remounts, but never for minutes.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(90);

/// Asks `adbd` for root, as `adb root` does. `false` means this device does not allow it (a
/// production build).
///
/// `adbd` is left running as root afterwards. Restarting it again would drop the connection a
/// second time, and the certificate stays in place until the device reboots either way.
async fn try_root(adb: &AdbClient, serial: &str) -> Result<bool> {
    Ok(root_granted(&adb.root(serial).await?))
}

pub async fn install_ca(adb: &AdbClient, serial: &str, pem: &str, der: &[u8], name: &str) -> Result<CaInstall> {
    if try_root(adb, serial).await? {
        install_system_ca(adb, serial, pem, name).await
    } else {
        install_user_ca(adb, serial, der).await
    }
}

async fn install_system_ca(adb: &AdbClient, serial: &str, pem: &str, name: &str) -> Result<CaInstall> {
    let mut sync = adb.sync(serial).await?;
    let pushed = sync.push(DEVICE_PEM, 0o644, pem.as_bytes()).await;
    sync.close().await;
    pushed?;
    let script = format!("sh -c '{}'", system_install_script(name).replace('\'', r"'\''"));
    let out = tokio::time::timeout(INSTALL_TIMEOUT, adb.shell(serial, &script)).await.map_err(|_| {
        Error::Adb("the device took too long to add the certificate. Restart it and try again.".into())
    })??;
    // `set -e` stops at the first failure and the shell prints why.
    if out.contains("not found")
        || out.contains("Permission denied")
        || out.contains("Read-only")
        || out.contains("Operation not permitted")
    {
        return Err(Error::Adb(format!("could not add the certificate to the system store: {}", out.trim())));
    }
    let present = adb.shell(serial, &format!("ls /system/etc/security/cacerts/{name}")).await?;
    if !present.contains(name) || present.contains("No such file") {
        return Err(Error::Adb(format!(
            "the certificate is not in the system store after installing it. The device said: {}",
            out.trim()
        )));
    }
    Ok(CaInstall {
        kind: CaKind::System,
        done: true,
        message: "The certificate is trusted by every app on this device until it restarts. adbd now runs as root, as \
                  after `adb root`."
            .into(),
        steps: vec![],
        snippet: None,
    })
}

async fn install_user_ca(adb: &AdbClient, serial: &str, der: &[u8]) -> Result<CaInstall> {
    adb.push(serial, DOWNLOAD_CRT, 0o644, der).await?;
    // Android 11 and newer refuse to install a certificate from an intent and open the settings
    // instead; older versions show the installer. Either way the user finishes it by hand.
    let _ = adb.shell(serial, "am start -a android.settings.SECURITY_SETTINGS").await;
    Ok(CaInstall {
        kind: CaKind::User,
        done: false,
        message: "This device does not allow adding certificates to the system store, so the certificate was copied \
                  to Downloads and has to be installed by hand. Chrome and apps that opt in will trust it."
            .into(),
        steps: vec![
            "Open Settings › Security & privacy › More security settings › Encryption & credentials.".into(),
            "Tap Install a certificate › CA certificate, then Install anyway.".into(),
            "Pick emulabzip-ca.crt in Downloads. Android asks you to set a screen lock first if the device has none."
                .into(),
            "Apps only trust user certificates when they allow it. For your own debug builds, add the \
             network_security_config shown below and reference it from the manifest."
                .into(),
        ],
        snippet: Some(ANDROID_SECURITY_CONFIG.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_proxy_setting() {
        assert_eq!(parse_proxy("127.0.0.1:9090\n").as_deref(), Some("127.0.0.1:9090"));
        for off in ["null\n", ":0\n", "", "\n", "10.0.0.2:0"] {
            assert_eq!(parse_proxy(off), None, "{off:?}");
        }
    }

    #[test]
    fn recognises_our_own_proxy() {
        assert_eq!(local_port("127.0.0.1:9090"), Some(9090));
        assert_eq!(local_port("10.0.0.2:8888"), None);
        assert_eq!(local_port("127.0.0.1:nope"), None);
    }

    #[test]
    fn understands_what_adb_root_says() {
        assert!(root_granted("restarting adbd as root"));
        assert!(root_granted("adbd is already running as root"));
        assert!(!root_granted("adbd cannot run as root in production builds"));
    }

    #[test]
    fn the_install_script_names_the_certificate_and_covers_the_apex() {
        let script = system_install_script("deadbeef.0");
        assert!(script.contains("$DIR/deadbeef.0"));
        assert!(script.contains("/apex/com.android.conscrypt/cacerts"));
        assert!(script.contains("nsenter --mount=/proc/$Z/ns/mnt"));
    }

    #[test]
    fn the_install_script_does_not_stack_mounts_or_hang() {
        let script = system_install_script("deadbeef.0");
        // The marker check comes before the first mount, and every nsenter is bounded.
        assert!(script.find(".emulabzip").unwrap() < script.find("mount -t tmpfs").unwrap());
        assert_eq!(script.matches("nsenter").count(), script.matches("timeout 10 nsenter").count());
        assert_eq!(script.matches("mount -t tmpfs").count(), 1);
    }
}
