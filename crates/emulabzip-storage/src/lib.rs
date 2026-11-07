//! Viewers for the files apps keep: SQLite databases, SharedPreferences, property lists, and a
//! detector that picks the right one.

pub mod detect;
pub mod edit;
pub mod plist;
pub mod prefs;
pub mod schema;
pub mod snapshot;
pub mod sqlite;

pub use detect::{FileKind, detect, image_mime};
