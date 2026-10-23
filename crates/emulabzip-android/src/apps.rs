//! Installed packages and the processes that run them.

use std::collections::HashMap;

use emulabzip_core::App;

use crate::info::SEPARATOR;

/// One shell call that lists user packages, system packages and running processes.
pub fn command() -> String {
    format!(
        "pm list packages -3; echo {SEPARATOR}; pm list packages -s; echo {SEPARATOR}; ps -A -o PID,NAME 2>/dev/null || ps"
    )
}

/// `package:com.example.app` lines from `pm list packages`.
pub fn parse_packages(text: &str) -> Vec<&str> {
    text.lines().filter_map(|l| l.trim().strip_prefix("package:")).filter(|p| !p.is_empty()).collect()
}

/// Process name to pid, from `ps -A -o PID,NAME` or the older `ps` with a `USER PID … NAME` layout.
pub fn parse_processes(text: &str) -> HashMap<&str, u32> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else { return HashMap::new() };
    let pid_col = header.split_whitespace().position(|c| c == "PID").unwrap_or(0);
    lines
        .filter_map(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            let pid = cols.get(pid_col)?.parse().ok()?;
            Some((*cols.last()?, pid))
        })
        .collect()
}

/// Combines the outputs of [`command`] into apps, user apps first, each group sorted by id.
pub fn build_apps(output: &str) -> Vec<App> {
    let mut parts = output.split(SEPARATOR);
    let user = parse_packages(parts.next().unwrap_or(""));
    let system = parse_packages(parts.next().unwrap_or(""));
    let processes = parse_processes(parts.next().unwrap_or("").trim_start());

    let mut apps: Vec<App> = user
        .iter()
        .map(|id| (id, true))
        .chain(system.iter().map(|id| (id, false)))
        .map(|(id, is_user)| App {
            id: id.to_string(),
            name: id.to_string(),
            version: None,
            pid: processes.get(id).copied(),
            user: is_user,
        })
        .collect();
    apps.sort_by(|a, b| b.user.cmp(&a.user).then_with(|| a.id.cmp(&b.id)));
    apps.dedup_by(|a, b| a.id == b.id);
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_packages() {
        assert_eq!(parse_packages("package:com.b\npackage:com.a\n\n"), vec!["com.b", "com.a"]);
    }

    #[test]
    fn parses_modern_and_legacy_ps() {
        let modern = parse_processes("  PID NAME\n    1 init\n 4321 com.example.app\n 4400 com.example.app:remote\n");
        assert_eq!(modern.get("com.example.app"), Some(&4321));
        assert_eq!(modern.get("com.example.app:remote"), Some(&4400));

        let legacy = parse_processes(
            "USER     PID   PPID  VSIZE  RSS     WCHAN    PC        NAME\nu0_a54    1234  89    1000   200   ffffffff 00000000 S com.example.app\n",
        );
        assert_eq!(legacy.get("com.example.app"), Some(&1234));
    }

    #[test]
    fn builds_the_app_list() {
        let out = format!(
            "package:com.example.app\n{SEPARATOR}\npackage:com.android.settings\npackage:android\n{SEPARATOR}\n  PID NAME\n 4321 com.example.app\n  900 com.android.settings\n"
        );
        let apps = build_apps(&out);
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0].id, "com.example.app");
        assert!(apps[0].user);
        assert_eq!(apps[0].pid, Some(4321));
        assert_eq!(apps[1].id, "android");
        assert_eq!(apps[1].pid, None);
        assert_eq!(apps[2].pid, Some(900));
    }
}
