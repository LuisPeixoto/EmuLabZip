//! Summarizes real traces, one per preset. Opt-in, because it needs recordings and the processor:
//!
//! ```bash
//! EMULABZIP_PROCESSOR=…/trace_processor_shell EMULABZIP_TRACES=<dir with cpu, frames and memory .pftrace> \
//!   cargo test -p emulabzip-trace --test summary_live -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use emulabzip_trace::perfetto::{processor::Processor, summary::summarize};

async fn summary(name: &str) -> emulabzip_trace::perfetto::summary::Summary {
    let binary = PathBuf::from(std::env::var("EMULABZIP_PROCESSOR").expect("EMULABZIP_PROCESSOR"));
    let dir = PathBuf::from(std::env::var("EMULABZIP_TRACES").expect("EMULABZIP_TRACES"));
    let processor = Processor::start(&binary, &dir.join(format!("{name}.pftrace"))).await.unwrap();
    let s = summarize(&processor).await.unwrap();
    println!("{name}: {s:#?}");
    s
}

#[tokio::test]
#[ignore = "needs a processor binary and recorded traces"]
async fn summarizes_a_cpu_trace() {
    let s = summary("cpu").await;
    let cpu = s.cpu.expect("sched data");
    assert!(cpu.cpus > 0 && cpu.busy_fraction > 0.0 && !cpu.by_process.is_empty() && !cpu.by_thread.is_empty());
    assert!(s.duration_s > 1.0);
}

#[tokio::test]
#[ignore = "needs a processor binary and recorded traces"]
async fn summarizes_a_frames_trace() {
    let f = summary("frames").await.frames.expect("frame timeline data");
    assert!(f.total > 0 && f.p50_ms > 0.0 && f.p99_ms >= f.p50_ms);
}

#[tokio::test]
#[ignore = "needs a processor binary and recorded traces"]
async fn summarizes_a_memory_trace() {
    let m = summary("memory").await.memory.expect("memory data");
    assert!(!m.system.is_empty() || !m.processes.is_empty());
}
