//! Records a fake device, writes the session, and reads it back. No device or daemon is involved.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use emulabzip_core::{
    App, Backend, CrashKind, CrashReport, CrashSummary, Device, DeviceKind, DeviceState, Error, InfoSection, Lab,
    LogEntry, LogLevel, LogOptions, PerfOptions, PerfSample, Platform, Result, Stream,
};
use emulabzip_net::{Flow, FlowKind, FlowState, FlowStore};
use emulabzip_session::{Options, Recorder, Session, markdown, summarize};

/// A device that logs and samples by itself and starts crashing after the second look.
struct Fake {
    crash_lists: Arc<AtomicUsize>,
}

fn crash() -> CrashSummary {
    CrashSummary {
        id: "crash-1".into(),
        kind: CrashKind::Crash,
        app: Some("com.example.app".into()),
        process: "com.example.app".into(),
        pid: Some(42),
        time_ms: Some(1),
        title: "java.lang.IllegalStateException: boom".into(),
    }
}

#[async_trait]
impl Backend for Fake {
    fn platform(&self) -> Platform {
        Platform::Android
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn devices(&self) -> Result<Vec<Device>> {
        Ok(vec![device()])
    }

    async fn info(&self, _: &str) -> Result<Vec<InfoSection>> {
        Ok(Vec::new())
    }

    async fn apps(&self, _: &str) -> Result<Vec<App>> {
        Ok(Vec::new())
    }

    async fn logs(&self, _: &str, _: LogOptions) -> Result<Stream<LogEntry>> {
        Ok(Stream::spawn(64, |tx| async move {
            for seq in 0.. {
                let level = if seq % 3 == 0 { LogLevel::Error } else { LogLevel::Info };
                let entry = LogEntry {
                    seq,
                    time: "10-23 14:02:11.123".into(),
                    pid: 42,
                    tid: 42,
                    level,
                    tag: "Net".into(),
                    message: format!("request {seq} failed after {} ms", 100 + seq),
                    subsystem: None,
                };
                if tx.send(entry).await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }))
    }

    async fn screenshot(&self, _: &str) -> Result<Vec<u8>> {
        Ok(b"\x89PNG fake".to_vec())
    }

    async fn perf(&self, _: &str, _: PerfOptions) -> Result<Stream<PerfSample>> {
        Ok(Stream::spawn(8, |tx| async move {
            for i in 0..u64::MAX {
                let sample = PerfSample {
                    t_ms: i * 100,
                    app_cpu: Some(10.0 + i as f32),
                    app_memory_kb: Some(100_000 + i * 1000),
                    ..Default::default()
                };
                if tx.send(sample).await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }))
    }

    async fn crashes(&self, _: &str, _: Option<&str>) -> Result<Vec<CrashSummary>> {
        // The first look is the start of the recording: nothing yet.
        Ok(if self.crash_lists.fetch_add(1, Ordering::SeqCst) == 0 { Vec::new() } else { vec![crash()] })
    }

    async fn crash(&self, _: &str, id: &str) -> Result<CrashReport> {
        if id != "crash-1" {
            return Err(Error::NotFound(id.into()));
        }
        Ok(CrashReport {
            summary: crash(),
            details: vec![("Signal".into(), "none".into())],
            threads: Vec::new(),
            hints: Vec::new(),
            raw: "raw report".into(),
        })
    }
}

fn device() -> Device {
    Device {
        id: "fake-1".into(),
        name: "Fake Pixel".into(),
        platform: Platform::Android,
        kind: DeviceKind::Emulator,
        state: DeviceState::Online,
        os_version: Some("14".into()),
        api_level: Some(34),
        model: Some("Pixel".into()),
        abi: None,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("emulabzip-session-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn lab() -> Lab {
    Lab::new().with(Fake { crash_lists: Arc::new(AtomicUsize::new(0)) })
}

fn flows() -> Arc<FlowStore> {
    let store = FlowStore::new(100);
    for (path, status, ms) in [("/fast", 200u16, 20u64), ("/slow", 200, 900), ("/broken", 500, 50)] {
        let mut f = Flow::new(FlowKind::Http, "GET", "https", "api.example.com", 443, path);
        f.status = Some(status);
        f.state = FlowState::Completed;
        f.timing.total_ms = Some(ms);
        f.timing.ttfb_ms = Some(ms / 2);
        store.begin(f);
    }
    store
}

#[tokio::test]
async fn a_recording_is_written_to_one_file_and_read_back() {
    let dir = scratch("roundtrip");
    let out = dir.join("run.ezsession");
    let options = Options {
        app: Some("com.example.app".into()),
        screens_every: Some(Duration::from_secs(1)),
        perf_interval_ms: 250,
        flows: Some(flows()),
        ..Default::default()
    };
    let recorder = Recorder::start(lab(), &device(), out.clone(), options).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(recorder.stats().logs > 10 && recorder.stats().samples >= 5, "{:?}", recorder.stats());
    let written = recorder.finish().await.unwrap();
    assert_eq!(written, out);
    assert!(out.is_file());
    assert!(!dir.join("run.ezsession.partial").exists(), "the working folder is gone");

    let session = Session::open(&out).unwrap();
    assert!(!session.partial);
    assert_eq!(
        (session.meta.device.name.as_str(), session.meta.app.as_deref()),
        ("Fake Pixel", Some("com.example.app"))
    );
    assert!(session.meta.ended_ms.unwrap() >= session.meta.started_ms);
    assert!(session.logs.len() > 10);
    assert!(session.perf.len() >= 5);
    // The crash appeared after the start, and was found when the recording stopped.
    assert_eq!(session.crashes.len(), 1);
    assert_eq!(session.crashes[0].raw, "raw report");
    assert!(!session.screens.is_empty());
    assert!(session.screen(&session.screens[0]).unwrap().starts_with(b"\x89PNG"));
    assert!(session.screen("../session.json").is_err());
    assert_eq!(session.har.as_ref().unwrap()["log"]["entries"].as_array().unwrap().len(), 3);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn the_summary_counts_errors_crashes_requests_and_samples() {
    let dir = scratch("summary");
    let out = dir.join("run.ezsession");
    let options = Options {
        app: Some("com.example.app".into()),
        perf_interval_ms: 250,
        flows: Some(flows()),
        ..Default::default()
    };
    let recorder = Recorder::start(lab(), &device(), out.clone(), options).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    recorder.finish().await.unwrap();

    let summary = summarize(&Session::open(&out).unwrap());
    assert!(summary.logs.errors >= 3 && summary.logs.total > summary.logs.errors);
    // The numbers differ in every line; they are counted as one error.
    assert_eq!(summary.top_errors.len(), 1, "{:?}", summary.top_errors);
    assert_eq!(summary.top_errors[0].message, "Net: request # failed after # ms");
    assert_eq!(summary.crashes.len(), 1);
    let net = summary.network.as_ref().unwrap();
    assert_eq!((net.requests, net.failed), (3, 1));
    assert_eq!(net.slowest[0].url, "https://api.example.com/slow");
    let perf = summary.perf.as_ref().unwrap();
    assert!(perf.cpu_peak.unwrap() >= perf.cpu_avg.unwrap());

    let report = markdown(&summary);
    assert!(report.contains("## EmuLabZip session") && report.contains("Fake Pixel"), "{report}");
    assert!(report.contains("IllegalStateException") && report.contains("/slow"), "{report}");
    assert!(!report.contains("cut short"));
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_recording_that_never_finished_can_still_be_opened() {
    let dir = scratch("partial");
    let out = dir.join("run.ezsession");
    let recorder = Recorder::start(lab(), &device(), out.clone(), Options::default()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1300)).await;
    // The program is killed: nothing is finished, nothing is packed.
    drop(recorder);
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(!out.exists());
    let session = Session::open(&dir.join("run.ezsession.partial")).unwrap();
    assert!(session.partial);
    assert!(session.meta.ended_ms.is_none());
    assert!(session.logs.len() > 5, "the logs written so far are there");
    assert!(markdown(&summarize(&session)).contains("cut short"));

    // A new recording will not overwrite it.
    let err = Recorder::start(lab(), &device(), out, Options::default()).await.err().expect("refused").to_string();
    assert!(err.contains("left over"), "{err}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn what_is_not_a_session_is_explained() {
    let dir = scratch("bad");
    let not_zip = dir.join("notes.ezsession");
    std::fs::write(&not_zip, "hello").unwrap();
    assert!(Session::open(&not_zip).unwrap_err().to_string().contains("not a session"));
    assert!(Session::open(&dir.join("missing.ezsession")).is_err());

    // A folder with a session.json from the future.
    let newer = dir.join("newer");
    std::fs::create_dir_all(&newer).unwrap();
    std::fs::write(
        newer.join("session.json"),
        r#"{"version":99,"tool":"x","device":{"id":"a","name":"n","platform":"android","osVersion":null,"model":null},"app":null,"startedMs":0,"endedMs":1,"network":false,"screensEverySecs":null}"#,
    )
    .unwrap();
    assert!(Session::open(&newer).unwrap_err().to_string().contains("newer EmuLabZip"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_half_written_last_line_is_skipped() {
    let dir = scratch("torn");
    std::fs::write(
        dir.join("session.json"),
        r#"{"version":1,"tool":"x","device":{"id":"a","name":"n","platform":"android","osVersion":null,"model":null},"app":null,"startedMs":0,"endedMs":null,"network":false,"screensEverySecs":null}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("logs.jsonl"),
        concat!(
            r#"{"seq":1,"time":"t","pid":1,"tid":1,"level":"error","tag":"A","message":"one","subsystem":null}"#,
            "\n",
            r#"{"seq":2,"time":"t","pid":1,"ti"#
        ),
    )
    .unwrap();
    let session = Session::open(&dir).unwrap();
    assert_eq!(session.logs.len(), 1);
    std::fs::remove_dir_all(dir).ok();
}
