//! Shared building blocks for EmuLabZip.
//!
//! This crate has no platform code. It defines the data model ([`Device`], [`LogEntry`], …), the
//! [`Backend`] trait that the Android and iOS collectors implement, and [`Lab`], which puts every
//! backend behind one handle for the CLI and the desktop app.

pub mod backend;
pub mod buffer;
pub mod device;
pub mod error;
pub mod log;
pub mod stream;

pub use backend::{Backend, Lab, LogOptions};
pub use buffer::RingBuffer;
pub use device::{App, Device, DeviceKind, DeviceState, InfoSection, Platform, Target};
pub use error::{Error, Result};
pub use log::{LogEntry, LogFilter, LogLevel, LogMatcher};
pub use stream::Stream;
