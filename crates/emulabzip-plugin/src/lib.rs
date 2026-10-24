//! Plugins: your own collectors for EmuLabZip.
//!
//! A plugin is a folder with a `plugin.json` and a program. EmuLabZip starts the program for a
//! device (never through a shell), and the program prints one JSON object per line on its standard
//! output: a metric, a table, some text, a log line or an error. EmuLabZip draws them. There is no
//! plugin code inside the app, so a plugin cannot break the interface, and the app never runs one
//! that the user did not enable.
//!
//! See `docs/plugins.md` for the protocol.

pub mod approval;
pub mod event;
pub mod manifest;
pub mod runner;
pub mod scaffold;

pub use approval::Approvals;
pub use event::{Event, parse_line};
pub use manifest::{Manifest, Mode, Plugin, Problem, discover, plugins_dir};
pub use runner::{Context, Outcome, run};

/// The protocol version a manifest must state.
pub const PROTOCOL: u32 = 1;
