//! Audits that look at an app from the outside and say what is wrong: secrets and personal data it leaves lying
//! around, how big a build is, and whether a screen still looks the way it did.
//!
//! Everything here works on text, bytes and files given to it. Nothing talks to a device, so each rule can be tested
//! on a handful of lines.

pub mod collect;
pub mod finding;
pub mod package;
pub mod patterns;
pub mod security;

pub use emulabzip_core::Severity;
pub use finding::{Area, Audit, Finding};
