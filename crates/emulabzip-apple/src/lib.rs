//! iOS Simulator collectors for EmuLabZip.
//!
//! Instruments talks to simulators through private frameworks (DTX and `DTServiceHub`). EmuLabZip
//! sticks to the documented command line tools instead (`xcrun simctl`, `log stream`, `plutil`),
//! which are stable across Xcode releases. On Windows and Linux the backend reports itself as
//! unavailable.

pub mod apps;
pub mod backend;
pub mod devices;
pub mod info;
pub mod memory;
pub mod oslog;
pub mod perf;
pub mod simctl;
pub mod storage;

pub use backend::AppleBackend;
