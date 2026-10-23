//! Viewers for the files apps keep: SQLite databases, SharedPreferences, property lists, and a
//! detector that picks the right one.

pub mod detect;
pub mod plist;
pub mod prefs;
pub mod sqlite;

pub use detect::{FileKind, detect, image_mime};
