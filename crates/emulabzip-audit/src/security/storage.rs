//! What the app keeps on the device: preferences and databases in plain files.

use emulabzip_core::Severity;

use super::finding;
use crate::finding::{Area, Finding};
use crate::patterns::{is_empty_secret, redact, scan, sensitive_name};

/// One key and value of a preferences, DataStore, MMKV or property list file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub file: String,
    pub key: String,
    pub value: String,
}

/// One column of one table of a SQLite database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub file: String,
    pub table: String,
    pub column: String,
}

/// Values that only sit under a secret-sounding name: a date (`2026-11-07T20:54:21Z`), or binary data a viewer shows as `<…>`.
fn is_not_a_secret(value: &str) -> bool {
    let v = value.trim();
    let date = v.len() >= 10 && v.as_bytes()[..4].iter().all(u8::is_ascii_digit) && &v[4..5] == "-" && &v[7..8] == "-";
    date || (v.starts_with('<') && v.ends_with('>'))
}

pub fn analyze(stored: &[Stored], columns: &[Column]) -> Vec<Finding> {
    let mut out = Vec::new();
    for s in stored {
        let hits = scan(&s.value);
        let location = Some(format!("{} · {}", s.file, s.key));
        for hit in &hits {
            let mut x = finding(
                format!("{}-in-storage", hit.kind.name),
                hit.kind.severity,
                Area::Storage,
                format!("`{}` holds {} in a plain file.", s.key, hit.kind.what),
                "Keep secrets in the platform's secure store (the Android Keystore, with EncryptedSharedPreferences or EncryptedFile; the iOS Keychain), and do not store personal data you do not need.",
            );
            x.evidence = Some(hit.redacted.clone());
            x.location = location.clone();
            out.push(x);
        }
        if hits.is_empty() && sensitive_name(&s.key) && !is_empty_secret(&s.value) && !is_not_a_secret(&s.value) {
            let mut x = finding(
                "secret-in-storage",
                Severity::Warning,
                Area::Storage,
                format!("`{}` is named like a secret and has a value, kept in a plain file.", s.key),
                "Keep secrets in the platform's secure store (the Android Keystore, with EncryptedSharedPreferences or EncryptedFile; the iOS Keychain).",
            );
            x.evidence = Some(redact(&s.value));
            x.location = location;
            out.push(x);
        }
    }
    for c in columns {
        if sensitive_name(&c.column) {
            let mut x = finding(
                "sensitive-column",
                Severity::Warning,
                Area::Storage,
                format!(
                    "The column `{}.{}` is named like a secret and the database is not encrypted.",
                    c.table, c.column
                ),
                "Encrypt the database (SQLCipher) or the value, or do not store it.",
            );
            x.location = Some(format!("{} · {}.{}", c.file, c.table, c.column));
            out.push(x);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(key: &str, value: &str) -> Stored {
        Stored { file: "shared_prefs/auth.xml".into(), key: key.into(), value: value.into() }
    }

    #[test]
    fn secrets_and_personal_data_in_preferences_are_found() {
        let found = analyze(
            &[
                stored("theme", "dark"),
                stored("session_token", "abcdef123456"),
                stored("refresh_token", ""),
                stored("email", "ana@example.com"),
                stored("auth", "Bearer abcdef0123456789abcdef"),
            ],
            &[],
        );
        let rules: Vec<_> = found.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(rules, ["secret-in-storage", "email-in-storage", "bearer-token-in-storage"]);
        assert_eq!(found[0].location.as_deref(), Some("shared_prefs/auth.xml · session_token"));
        assert!(!found[0].evidence.as_deref().unwrap().contains("123456"));
    }

    #[test]
    fn dates_and_binary_data_under_a_secret_sounding_name_are_left_alone() {
        let found = analyze(
            &[
                stored("PasswordWarningLastUpdate", "2026-11-07T20:54:21Z"),
                stored("PasswordBag/0", "<data: 35 bytes>"),
                stored("password", "hunter2hunter2"),
            ],
            &[],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].location.as_deref(), Some("shared_prefs/auth.xml · password"));
    }

    #[test]
    fn columns_named_like_secrets_are_found() {
        let cols = [
            Column { file: "databases/app.db".into(), table: "users".into(), column: "password_hash".into() },
            Column { file: "databases/app.db".into(), table: "users".into(), column: "name".into() },
        ];
        let found = analyze(&[], &cols);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, "sensitive-column");
    }
}
