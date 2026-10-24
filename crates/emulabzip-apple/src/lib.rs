//! iOS Simulator collectors for EmuLabZip.
//!
//! Instruments talks to simulators through private frameworks (DTX and `DTServiceHub`). EmuLabZip
//! sticks to the documented command line tools instead (`xcrun simctl`, `log stream`, `plutil`),
//! which are stable across Xcode releases. On Windows and Linux the backend reports itself as
//! unavailable.
//!
//! The one exception is the view tree: `simctl` cannot list views, so [`layout`] reads the
//! accessibility tree Simulator.app exposes to macOS, which needs the Accessibility permission.

pub mod apps;
pub mod backend;
pub mod controls;
pub mod crashes;
pub mod devices;
pub mod info;
pub mod layout;
pub mod memory;
pub mod network;
pub mod oslog;
pub mod perf;
pub mod simctl;
pub mod storage;
pub mod trace;

pub use backend::AppleBackend;
