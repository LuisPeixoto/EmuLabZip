//! Quoting for commands that run in the device shell.

/// Quotes one argument for `/system/bin/sh` so it is passed through literally.
///
/// Values that only contain safe characters are left as they are, which keeps commands readable
/// in debug logs.
pub fn quote(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '=' | '@' | '+' | ','));
    if safe { arg.to_string() } else { format!("'{}'", arg.replace('\'', r"'\''")) }
}

/// Joins a program and its arguments into one shell command line.
pub fn command<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().map(|a| quote(a.as_ref())).collect::<Vec<_>>().join(" ")
}

/// Whether `name` looks like an Android package name (`com.example.app`). Used to reject values
/// before they reach commands such as `run-as`.
pub fn is_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.split('.').all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_safe_words_alone() {
        assert_eq!(quote("dumpsys"), "dumpsys");
        assert_eq!(quote("/data/local/tmp/a.txt"), "/data/local/tmp/a.txt");
    }

    #[test]
    fn quotes_spaces_and_metacharacters() {
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("x; rm -rf /"), "'x; rm -rf /'");
        assert_eq!(quote("$(id)"), "'$(id)'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn escapes_single_quotes() {
        assert_eq!(quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn builds_command_lines() {
        assert_eq!(command(["ls", "-la", "/sdcard/My Files"]), "ls -la '/sdcard/My Files'");
    }

    #[test]
    fn validates_package_names() {
        assert!(is_package_name("com.example.app"));
        assert!(is_package_name("dev.emulabzip_demo.v2"));
        assert!(!is_package_name("com.example;id"));
        assert!(!is_package_name("1com.example"));
        assert!(!is_package_name("com..example"));
        assert!(!is_package_name(""));
    }
}
