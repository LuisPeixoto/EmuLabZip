//! Making sense of recordings: flame graphs from CPU samples, and (soon) Perfetto traces.

pub mod flame;
pub mod library;
pub mod perfetto;
pub mod simpleperf;
pub mod xctrace;

pub use flame::{FlameBuilder, Hotspot, Node, Origin, folded, hotspots};
