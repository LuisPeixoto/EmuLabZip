//! Changing what apps keep: SQLite rows, SharedPreferences and property lists.
//!
//! Everything here works on bytes or on a copy of a file, never on a device: the caller pulls the file, edits it here,
//! and writes it back through `Lab::write_file`, with the app stopped. Nothing is changed unless the edit is exact: one
//! row for one key, a value of the type that was asked for, a path that exists.

pub mod device;
pub mod plist;
pub mod prefs;
pub mod sqlite;
