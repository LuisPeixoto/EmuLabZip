//! Finding the WebViews and browsers a device lets a debugger into.

use emulabzip_core::DevtoolsSocket;

use crate::apps::parse_processes;
use crate::info::SEPARATOR;

/// One shell call: the unix sockets, then the processes, so a pid can be named.
pub fn command() -> String {
    format!("cat /proc/net/unix; echo {SEPARATOR}; ps -A -o PID,NAME 2>/dev/null || ps")
}

/// The DevTools sockets in `/proc/net/unix`, with the app of each one that a pid names.
pub fn parse(output: &str) -> Vec<DevtoolsSocket> {
    let mut parts = output.split(SEPARATOR);
    let unix = parts.next().unwrap_or("");
    let processes = parse_processes(parts.next().unwrap_or("").trim_start());
    let mut sockets: Vec<DevtoolsSocket> = Vec::new();
    for line in unix.lines() {
        // The path is the last column, and an abstract socket's starts with `@`.
        let Some(name) = line.split_whitespace().last().and_then(|p| p.strip_prefix('@')) else { continue };
        let Some(kind) = kind_of(name) else { continue };
        if sockets.iter().any(|s| s.socket == name) {
            continue;
        }
        let pid = name.rsplit_once('_').and_then(|(_, tail)| tail.parse::<u32>().ok());
        let app = pid.and_then(|pid| processes.iter().find(|(_, p)| **p == pid).map(|(n, _)| (*n).to_string()));
        sockets.push(DevtoolsSocket { socket: name.to_string(), kind: kind.to_string(), pid, app });
    }
    // WebViews first (they are the point), then by name so the order does not jump around.
    sockets.sort_by(|a, b| (a.kind != "webview", &a.socket).cmp(&(b.kind != "webview", &b.socket)));
    sockets
}

/// What owns a DevTools socket of this name, or `None` when it is not one.
fn kind_of(name: &str) -> Option<&'static str> {
    let stem = match name.rsplit_once('_') {
        Some((stem, tail)) if tail.chars().all(|c| c.is_ascii_digit()) && !tail.is_empty() => stem,
        _ => name,
    };
    if !stem.ends_with("devtools_remote") {
        return None;
    }
    Some(if stem.starts_with("webview_") {
        "webview"
    } else if stem.starts_with("chrome_") {
        "chrome"
    } else {
        "other"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIX: &str = "Num       RefCount Protocol Flags    Type St Inode Path
0000000000000000: 00000002 00000000 00010000 0001 01 20111 @webview_devtools_remote_4321
0000000000000000: 00000002 00000000 00010000 0001 01 20112 @chrome_devtools_remote
0000000000000000: 00000002 00000000 00010000 0001 01 20113 /dev/socket/zygote
0000000000000000: 00000002 00000000 00010000 0001 01 20114 @com.example.web_devtools_remote
0000000000000000: 00000003 00000000 00000000 0001 03 20115
0000000000000000: 00000002 00000000 00010000 0001 01 20116 @webview_devtools_remote_4321
0000000000000000: 00000002 00000000 00010000 0001 01 20117 @some_other_socket_9
";

    #[test]
    fn finds_webview_and_browser_sockets_and_names_the_app() {
        let output = format!("{UNIX}{SEPARATOR}\n  PID NAME\n    1 init\n 4321 com.example.shop\n");
        let found = parse(&output);
        assert_eq!(found.len(), 3, "{found:?}");
        assert_eq!(found[0].socket, "webview_devtools_remote_4321");
        assert_eq!(found[0].kind, "webview");
        assert_eq!(found[0].pid, Some(4321));
        assert_eq!(found[0].app.as_deref(), Some("com.example.shop"));
        let chrome = found.iter().find(|s| s.kind == "chrome").unwrap();
        assert_eq!((chrome.pid, chrome.app.as_deref()), (None, None));
        assert!(found.iter().any(|s| s.kind == "other" && s.socket == "com.example.web_devtools_remote"));
    }

    #[test]
    fn a_device_with_no_such_sockets_gives_an_empty_list() {
        assert!(parse("").is_empty());
        assert!(parse(&format!("Num\n{SEPARATOR}\n")).is_empty());
    }

    #[test]
    fn only_devtools_names_count() {
        assert_eq!(kind_of("webview_devtools_remote_12"), Some("webview"));
        assert_eq!(kind_of("chrome_devtools_remote_9"), Some("chrome"));
        assert_eq!(kind_of("devtools_remote"), Some("other"));
        assert_eq!(kind_of("remote_devtools"), None);
        assert_eq!(kind_of("webview_zygote_socket"), None);
    }
}
