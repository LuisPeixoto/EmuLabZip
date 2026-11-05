//! Reads Java heap dumps (`.hprof`, as Android's `am dumpheap` writes them) and finds leaks.
//!
//! [`analyze`] reads the dump, computes the dominator tree and retained sizes, and looks for the
//! objects that should be gone: an `Activity` that was destroyed, a `Fragment` that was removed. Each
//! comes with the shortest chain of references from a garbage collector root that keeps it alive, which
//! is where the fix usually is.

pub mod graph;
pub mod hprof;
pub mod leaks;
pub mod testing;

pub use hprof::{Error, Heap, MAX_INPUT};
pub use leaks::{Analysis, ClassStat, Confidence, Dominator, Leak, Options, Step, Summary, analyze};
