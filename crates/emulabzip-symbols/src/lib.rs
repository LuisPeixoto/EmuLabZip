//! Making crash stacks readable.
//!
//! - [`retrace`]: the names ProGuard and R8 changed, restored from a `mapping.txt`.
//! - [`sourcemap`]: positions in a React Native bundle, looked up in a source map.
//! - [`dsym`]: addresses in an iOS app, looked up in its `.dSYM` with `atos` (on a Mac).
//!
//! Everything here works on text and numbers, so it can be tested with real files and without a device.

pub mod dsym;
pub mod retrace;
pub mod sourcemap;
pub mod store;
