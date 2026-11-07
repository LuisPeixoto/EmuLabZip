//! Guessing what a file is, so the right viewer opens.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    Sqlite,
    /// Android `shared_prefs/*.xml`.
    SharedPrefs,
    /// Binary or XML property list.
    Plist,
    Json,
    /// A Preferences DataStore (`*.preferences_pb`).
    DataStore,
    /// A Proto DataStore: protobuf under `datastore/`.
    ProtoDataStore,
    /// An MMKV data file (under `mmkv/`).
    Mmkv,
    /// A Realm file.
    Realm,
    /// React Native's AsyncStorage on iOS: a `manifest.json` in `RCTAsyncLocalStorage_V1`.
    AsyncStorage,
    Image,
    Text,
    Binary,
}

const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

/// What a file is, from its bytes and its name. Give the path relative to the root where there is one: the folder says
/// what `mmkv/default` and `datastore/x.pb` are.
pub fn detect(name: &str, bytes: &[u8]) -> FileKind {
    let lower = name.to_ascii_lowercase();
    if bytes.starts_with(SQLITE_MAGIC) {
        return FileKind::Sqlite;
    }
    // A Realm file has its mnemonic after the two top references.
    if bytes.len() >= 24 && &bytes[16..20] == b"T-DB" {
        return FileKind::Realm;
    }
    if lower.ends_with(".preferences_pb") {
        return FileKind::DataStore;
    }
    if lower.contains("/datastore/") && lower.ends_with(".pb")
        || lower.starts_with("datastore/") && lower.ends_with(".pb")
    {
        return FileKind::ProtoDataStore;
    }
    if (lower.contains("/mmkv/") || lower.starts_with("mmkv/")) && !lower.ends_with(".crc") && !bytes.is_empty() {
        return FileKind::Mmkv;
    }
    if lower.contains("rctasynclocalstorage") && lower.ends_with("manifest.json") {
        return FileKind::AsyncStorage;
    }
    if bytes.starts_with(b"bplist00") {
        return FileKind::Plist;
    }
    if image_mime(bytes).is_some() {
        return FileKind::Image;
    }
    let Ok(text) = std::str::from_utf8(&bytes[..bytes.len().min(64 * 1024)]).or_else(|e| {
        // A multi-byte character may be cut at the sample boundary.
        std::str::from_utf8(&bytes[..e.valid_up_to()]).map_err(|_| e)
    }) else {
        return FileKind::Binary;
    };
    if bytes.len() > 64 * 1024 && text.len() < 60 * 1024 {
        return FileKind::Binary;
    }
    let head = text.trim_start();
    if lower.ends_with(".plist")
        || head.starts_with("<?xml") && head.contains("<!DOCTYPE plist")
        || head.starts_with("<plist")
    {
        return FileKind::Plist;
    }
    if head.starts_with("<?xml") && head.contains("<map") || lower.contains("shared_prefs/") && lower.ends_with(".xml")
    {
        return FileKind::SharedPrefs;
    }
    if lower.ends_with(".json")
        || (head.starts_with('{') || head.starts_with('[')) && serde_json::from_str::<serde_json::Value>(text).is_ok()
    {
        return FileKind::Json;
    }
    if text.chars().take(4096).any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return FileKind::Binary;
    }
    FileKind::Text
}

/// The MIME type of common image formats, from their magic bytes.
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    Some(match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_magic_bytes() {
        assert_eq!(detect("x", b"SQLite format 3\0rest"), FileKind::Sqlite);
        assert_eq!(detect("x", b"bplist00..."), FileKind::Plist);
        assert_eq!(detect("x", &[0x89, b'P', b'N', b'G', 0, 0]), FileKind::Image);
        assert_eq!(image_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
    }

    #[test]
    fn detects_text_formats() {
        assert_eq!(detect("prefs.xml", b"<?xml version='1.0'?>\n<map>\n</map>"), FileKind::SharedPrefs);
        assert_eq!(
            detect("Info.plist", b"<?xml version=\"1.0\"?><!DOCTYPE plist PUBLIC \"\"><plist></plist>"),
            FileKind::Plist
        );
        assert_eq!(detect("data", b"{\"a\": 1}"), FileKind::Json);
        assert_eq!(detect("notes.txt", b"hello\nworld"), FileKind::Text);
        assert_eq!(detect("blob", &[0, 1, 2, 3, 255]), FileKind::Binary);
    }

    #[test]
    fn detects_what_apps_keep_by_name_and_place() {
        assert_eq!(detect("files/datastore/demo.preferences_pb", &[0x0a, 0x00]), FileKind::DataStore);
        assert_eq!(detect("files/datastore/state.pb", &[0x08, 0x01]), FileKind::ProtoDataStore);
        assert_eq!(detect("datastore/state.pb", &[0x08, 0x01]), FileKind::ProtoDataStore);
        assert_eq!(detect("files/mmkv/mmkv.default", &[1, 0, 0, 0, 1]), FileKind::Mmkv);
        assert_eq!(detect("files/mmkv/mmkv.default.crc", &[1, 2, 3]), FileKind::Binary);
        assert_eq!(detect("Library/RCTAsyncLocalStorage_V1/manifest.json", b"{}"), FileKind::AsyncStorage);
        let mut realm = vec![0u8; 64];
        realm[16..20].copy_from_slice(b"T-DB");
        assert_eq!(detect("default.realm", &realm), FileKind::Realm);
        // A file called pb elsewhere is not one of them.
        assert_eq!(detect("notes/other.pb", &[0, 1, 2]), FileKind::Binary);
    }
}
