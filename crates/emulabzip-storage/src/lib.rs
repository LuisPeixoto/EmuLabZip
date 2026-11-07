//! Viewers for the files apps keep: SQLite databases, SharedPreferences, property lists, and a
//! detector that picks the right one.

pub mod asyncstorage;
pub mod coredata;
pub mod datastore;
pub mod detect;
pub mod edit;
pub mod mmkv;
pub mod plist;
pub mod prefs;
pub mod proto;
pub mod schema;
pub mod snapshot;
pub mod sqlite;

pub use detect::{FileKind, detect, image_mime};
