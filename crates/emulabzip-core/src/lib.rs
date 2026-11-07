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
pub mod energy;
pub mod error;
pub mod events;
pub mod files;
pub mod frameworks;
pub mod layout;
pub mod lifecycle;
pub mod log;
pub mod memory;
pub mod network;
pub mod paths;
pub mod perf;
pub mod startup;
pub mod state;
pub mod stream;
pub mod tarball;
pub mod trace;

pub use a11y::{Audit, Finding, Severity};
pub use backend::{Backend, DeviceIo, DeviceStream, Lab, LogOptions, is_log_needle};
pub use buffer::RingBuffer;
pub use compare::{DiffRow, diff};
pub use control::{Appearance, Control, ControlInfo, Grant, PermissionState, StatusBar};
pub use crash::{
    CrashKind, CrashReport, CrashSummary, Frame, FrameOrigin, Hint, JsFrame, Language, ThreadTrace, js_thread,
    parse_js_frame,
};
pub use device::{App, Device, DeviceKind, DeviceState, InfoSection, Platform, Target};
pub use energy::{EnergyComponent, EnergyReport, Wakelock};
pub use error::{Error, Result};
pub use events::{DeviceClock, InputEvent, InputKind, MAX_SEGMENT_SECS, SlowFrame, VideoSegment};
pub use files::{FileEntry, StorageRoot};
pub use frameworks::Framework;
pub use layout::{Bounds, LayoutSnapshot, Screen, UiNode};
pub use lifecycle::{
    BootOptions, Choice, CreateDevice, DeviceOptions, EmulatorSnapshot, SnapshotAction, avd_name, is_device_name,
    is_device_ref, is_image_id,
};
pub use log::{LogEntry, LogFilter, LogLevel, LogMatcher};
pub use memory::{CategoryDelta, HeapDump, LeakGroup, MemoryCategory, MemoryReport, NativeLeaks};
pub use network::{ANDROID_SECURITY_CONFIG, CaInstall, CaKind, NetworkStatus};
pub use paths::data_dir;
pub use perf::{PerfOptions, PerfSample};
pub use startup::{StartKind, StartupReport, StartupRun};
pub use state::{DATA_TAR, EXTERNAL_TAR, MAX_WRITE_BYTES, StateFiles, is_safe_entry};
pub use stream::Stream;
pub use trace::{PerfettoPreset, TraceFile, TraceFormat, TraceRequest};
