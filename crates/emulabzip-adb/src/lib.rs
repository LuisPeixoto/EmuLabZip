//! An async client for the ADB host protocol.
//!
//! EmuLabZip talks to the ADB server (`127.0.0.1:5037`) directly instead of spawning `adb` for
//! every call. That keeps device tracking and log streaming cheap, and works with any recent
//! platform-tools release. Only the parts EmuLabZip needs are implemented: device listing and
//! tracking, `shell:`, `exec:` and the `sync:` sub-protocol for listing and pulling files.

pub mod client;
pub mod devices;
pub mod protocol;
pub mod quote;
pub mod server;
pub mod sync;

pub use client::{AdbClient, SyncSession};
pub use devices::DeviceEntry;
pub use quote::{command, is_package_name, quote};
pub use sync::{RemoteEntry, RemoteStat};
