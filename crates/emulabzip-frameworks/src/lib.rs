//! Looking inside apps built with Flutter and React Native.
//!
//! Both frameworks draw their own UI, so the platform's view tree says little about them. Each has a debugging
//! protocol that speaks JSON over a WebSocket: Flutter's Dart VM service (JSON-RPC 2.0) and the Chrome DevTools
//! protocol that Hermes exposes through Metro. [`rpc::Rpc`] is the small client both use.

pub mod flutter;
pub mod react_native;
pub mod rpc;

pub use rpc::Rpc;
