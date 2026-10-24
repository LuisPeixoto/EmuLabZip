//! Sessions: a recording of what happened on a device while an app was used.
//!
//! A session is a folder while it is being recorded, so a run that is killed still leaves its logs,
//! samples and requests on disk, and a single `.ezsession` file (a zip) once it ends. Everything
//! inside is plain JSON, a HAR file and PNG screenshots, so other tools can read it too.
//!
//! * [`Recorder`] follows a device and writes the session.
//! * [`Session`] reads one back, from the folder or the file.
//! * [`summarize`] turns it into numbers and a readable report.

pub mod format;
pub mod reader;
pub mod recorder;
pub mod summary;

pub use format::{DeviceMeta, EXTENSION, Meta, VERSION};
pub use reader::Session;
pub use recorder::{Options, Recorder, Stats};
pub use summary::{Summary, markdown, summarize};
