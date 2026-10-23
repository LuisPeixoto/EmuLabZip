//! Downloads the real Perfetto trace processor and queries a real trace. Needs the network, so it is
//! opt-in: `cargo test -p emulabzip-trace --test processor_live -- --ignored --nocapture`.
//!
//! The trace is whatever `EMULABZIP_TRACE` points to (a `.pftrace` recorded by EmuLabZip or Perfetto).

use std::sync::Arc;

use emulabzip_trace::perfetto::processor::{Cell, Processor, ensure_installed};

#[tokio::test]
#[ignore = "downloads the Perfetto trace processor"]
async fn downloads_verifies_and_queries() {
    let home = std::env::temp_dir().join(format!("emulabzip-live-processor-{}", std::process::id()));
    // SAFETY: the only test in this binary, so nothing else reads the environment meanwhile.
    unsafe { std::env::set_var("EMULABZIP_HOME", &home) };
    let trace = std::env::var("EMULABZIP_TRACE").expect("set EMULABZIP_TRACE to a .pftrace file");

    let last = Arc::new(std::sync::Mutex::new((0u64, None)));
    let seen = last.clone();
    let binary = ensure_installed(Arc::new(move |done, total| *seen.lock().unwrap() = (done, total))).await.unwrap();
    println!("installed at {}; last progress {:?}", binary.display(), last.lock().unwrap());
    assert!(binary.is_file());
    // The second call finds it and does not download again.
    *last.lock().unwrap() = (0, None);
    assert_eq!(ensure_installed(Arc::new(|_, _| panic!("downloaded twice"))).await.unwrap(), binary);

    let processor = Processor::start(&binary, std::path::Path::new(&trace)).await.unwrap();
    let rows = processor
        .query("select count(*) as n, (trace_end() - trace_start()) / 1e9 as seconds from sched")
        .await
        .unwrap();
    println!("{rows:?}");
    assert_eq!(rows.columns, ["n", "seconds"]);
    assert!(matches!(rows.rows[0][0], Cell::Int(_)));

    let error = processor.query("select nope from nothing").await.unwrap_err();
    assert!(error.to_string().contains("syntax error"), "{error}");
    drop(processor);
    std::fs::remove_dir_all(home).ok();
}
