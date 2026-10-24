//! Shared building blocks for EmuLabZip.
//!
//! This crate has no platform code. It defines the data model ([`Device`], [`LogEntry`], …), the
//! [`Backend`] trait that the Android and iOS collectors implement, and [`Lab`], which puts every
//! backend behind one handle for the CLI and the desktop app.

pub mod a11y;
pub mod backend;
pub mod buffer;
pub mod compare;
pub mod control;
pub mod crash;
pub mod device;
pub mod error;
pub mod files;
pub mod layout;
pub mod log;
pub mod memory;
pub mod network;
pub mod paths;
pub mod perf;
pub mod stream;
pub mod trace;

pub use a11y::{Audit, Finding, Severity};
pub use backend::{Backend, Lab, LogOptions};
pub use buffer::RingBuffer;
pub use compare::{DiffRow, diff};
pub use control::{Appearance, Control, ControlInfo, Grant, PermissionState, StatusBar};
pub use crash::{CrashKind, CrashReport, CrashSummary, Frame, FrameOrigin, Hint, ThreadTrace};
pub use device::{App, Device, DeviceKind, DeviceState, InfoSection, Platform, Target};
pub use error::{Error, Result};
pub use files::{FileEntry, StorageRoot};
pub use layout::{Bounds, LayoutSnapshot, Screen, UiNode};
pub use log::{LogEntry, LogFilter, LogLevel, LogMatcher};
pub use memory::{CategoryDelta, HeapDump, MemoryCategory, MemoryReport};
pub use network::{ANDROID_SECURITY_CONFIG, CaInstall, CaKind, NetworkStatus};
pub use paths::data_dir;
pub use perf::{PerfOptions, PerfSample};
pub use stream::Stream;
pub use trace::{PerfettoPreset, TraceFile, TraceFormat, TraceRequest};
