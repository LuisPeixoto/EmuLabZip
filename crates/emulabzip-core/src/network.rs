//! Routing a device's traffic through the EmuLabZip proxy, and trusting its certificate.

use serde::{Deserialize, Serialize};

/// Whether a device sends its traffic to the proxy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    /// The device's HTTP proxy points at the given address, for example `127.0.0.1:9090`.
    pub proxy: Option<String>,
    /// The proxy setting is ours, so disconnecting will undo something we did.
    pub connected: bool,
    /// What reaching the proxy means on this platform, shown next to the switch.
    pub note: Option<String>,
}

/// How the certificate authority ended up on a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaKind {
    /// Trusted by every app, because it lives in the system store.
    System,
    /// Trusted by apps that opt in to user certificates (browsers, debug builds).
    User,
    /// Added to the iOS Simulator's keychain.
    Simulator,
}

/// The outcome of installing the certificate, and what is left for the user to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaInstall {
    pub kind: CaKind,
    /// The certificate is already trusted and nothing else is needed.
    pub done: bool,
    pub message: String,
    /// Steps for the user, in order. Empty when `done`.
    pub steps: Vec<String>,
    /// A `network_security_config.xml` that lets a debug build trust user certificates.
    pub snippet: Option<String>,
}

/// The `network_security_config` that makes an Android debug build trust user-installed
/// certificates. Release builds are unaffected.
pub const ANDROID_SECURITY_CONFIG: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <debug-overrides>
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />
        </trust-anchors>
    </debug-overrides>
</network-security-config>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_in_camel_case() {
        let json = serde_json::to_value(CaInstall {
            kind: CaKind::User,
            done: false,
            message: "x".into(),
            steps: vec![],
            snippet: None,
        })
        .unwrap();
        assert_eq!(json["kind"], "user");
        assert!(json["snippet"].is_null());
    }

    #[test]
    fn the_snippet_is_valid_xml_with_a_debug_override() {
        assert!(ANDROID_SECURITY_CONFIG.contains("<debug-overrides>"));
        assert!(ANDROID_SECURITY_CONFIG.contains(r#"src="user""#));
    }
}
