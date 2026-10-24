//! Android collectors for EmuLabZip.
//!
//! Everything here uses public interfaces only: the ADB host protocol, `getprop`, `dumpsys`,
//! `pm`, `ps`, `logcat` and `screencap`. That is the same data Android Studio's profilers read,
//! without pushing any agent onto the device.

pub mod apps;
pub mod backend;
pub mod cert;
pub mod crashes;
pub mod heapdump;
pub mod info;
pub mod logcat;
pub mod meminfo;
pub mod network;
pub mod perf;
pub mod props;
pub mod storage;
pub mod trace;

pub use backend::AndroidBackend;
