//! WebViews that can be inspected: the DevTools sockets an Android device listens on.
//!
//! A debuggable WebView (`WebView.setWebContentsDebuggingEnabled(true)`) and Chrome each open an abstract unix socket
//! that speaks the Chrome DevTools protocol, named `webview_devtools_remote_<pid>` and `chrome_devtools_remote`.

use serde::{Deserialize, Serialize};

/// One DevTools socket on a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevtoolsSocket {
    /// The abstract socket name, without the `@` that `/proc/net/unix` shows.
    pub socket: String,
    /// `webview` for an app's WebView, `chrome` for Chrome, `other` for any other browser engine.
    pub kind: String,
    /// The process that owns the socket, when the name says it.
    pub pid: Option<u32>,
    /// The app that process runs, when it is known.
    pub app: Option<String>,
}
