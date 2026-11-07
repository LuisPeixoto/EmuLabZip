//! The streams added in format 2 (clock, touches, slow frames, video, screenshot times), recorded from a fake device,
//! and a format 1 file that must still open. No device or daemon is involved.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use emulabzip_core::{
    App, Backend, Device, DeviceClock, DeviceKind, DeviceState, InfoSection, InputEvent, InputKind, Lab, LogEntry,
    LogOptions, Platform, Result, SlowFrame, Stream, VideoSegment,
};
use emulabzip_session::{Options, Recorder, Session};

struct Fake;

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
        Ok(Stream::spawn(1, |_tx| async move {
            std::future::pending::<()>().await;
        }))
    }

    async fn screenshot(&self, _: &str) -> Result<Vec<u8>> {
        Ok(b"\x89PNG fake".to_vec())
    }

    async fn clock(&self, _: &str) -> Result<DeviceClock> {
        Ok(DeviceClock { offset_ms: 1500, tz_minutes: -180, boot_epoch_ms: 1_000 })
    }

    async fn input(&self, _: &str) -> Result<Stream<InputEvent>> {
        Ok(Stream::spawn(4, |tx| async move {
            let tap =
                InputEvent { t_ms: 5, kind: InputKind::Tap, x: 100, y: 200, x2: 100, y2: 200, duration_ms: 60 };
            let _ = tx.send(tap).await;
            std::future::pending::<()>().await;
        }))
    }

    async fn slow_frames(&self, _: &str, _: &str) -> Result<Stream<SlowFrame>> {
        Ok(Stream::spawn(4, |tx| async move {
            let _ = tx.send(SlowFrame { t_ms: 7, duration_ms: 48.5 }).await;
            std::future::pending::<()>().await;
        }))
    }

    async fn record_video(
        &self,
        _: &str,
        out_dir: &std::path::Path,
        _segment_secs: u32,
        mut stop: tokio::sync::watch::Receiver<bool>,
        segments: tokio::sync::mpsc::UnboundedSender<VideoSegment>,
    ) -> Result<()> {
        std::fs::write(out_dir.join("0001.mp4"), b"\0\0\0 ftypmp42 fake video")?;
        let _ = segments.send(VideoSegment { file: "0001.mp4".into(), start_ms: 10, end_ms: 20 });
        while !*stop.borrow() {
            if stop.changed().await.is_err() {
                break;
            }
        }
        Ok(())
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
    let dir = std::env::temp_dir().join(format!("emulabzip-streams-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn the_new_streams_are_recorded_and_read_back() {
    let dir = scratch("v2");
    let out = dir.join("run.ezsession");
    let options = Options {
        app: Some("com.example.app".into()),
        screens_every: Some(Duration::from_secs(1)),
        video: Some(30),
        taps: true,
        ..Default::default()
    };
    let recorder = Recorder::start(Lab::new().with(Fake), &device(), out.clone(), options).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1400)).await;
    let stats = recorder.stats();
    assert_eq!((stats.taps, stats.slow_frames, stats.video_pieces), (1, 1, 1), "{stats:?}");
    recorder.finish().await.unwrap();

    let session = Session::open(&out).unwrap();
    assert_eq!(session.meta.version, 2);
    assert_eq!(session.meta.clock, Some(DeviceClock { offset_ms: 1500, tz_minutes: -180, boot_epoch_ms: 1_000 }));
    assert_eq!(session.input.len(), 1);
    assert_eq!(session.input[0].kind, InputKind::Tap);
    assert_eq!(session.slow_frames, vec![SlowFrame { t_ms: 7, duration_ms: 48.5 }]);
    assert_eq!(session.video.len(), 1);
    assert!(session.video_file("0001.mp4").unwrap().starts_with(b"\0\0\0 ftyp"));
    assert!(session.video_file("../session.json").is_err());
    // The index is not a screenshot, and every screenshot has a time inside the recording.
    assert!(session.screens.iter().all(|n| n.ends_with(".png")));
    assert_eq!(session.screen_times.len(), session.screens.len());
    assert!(session.screen_times.iter().all(|s| s.t_ms >= session.meta.started_ms));
    let extracted = session.extract_video("0001.mp4", &dir.join("out")).unwrap();
    assert!(extracted.is_file());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn video_is_stored_without_compression() {
    let dir = scratch("stored");
    let out = dir.join("run.ezsession");
    let options = Options { video: Some(30), ..Default::default() };
    let recorder = Recorder::start(Lab::new().with(Fake), &device(), out.clone(), options).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    recorder.finish().await.unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
    let entry = zip.by_name("video/0001.mp4").unwrap();
    assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_version_1_session_still_opens() {
    let dir = scratch("v1");
    let folder = dir.join("old.ezsession.partial");
    std::fs::create_dir_all(folder.join("crashes")).unwrap();
    std::fs::write(
        folder.join("session.json"),
        r#"{"version":1,"tool":"EmuLabZip 0.9.0","device":{"id":"x","name":"Old","platform":"android","osVersion":null,"model":null},
            "app":null,"startedMs":1000,"endedMs":2000,"network":false,"screensEverySecs":null,"logsDropped":0}"#,
    )
    .unwrap();
    let session = Session::open(&folder).unwrap();
    assert_eq!(session.meta.version, 1);
    assert!(session.meta.clock.is_none() && session.input.is_empty() && session.video.is_empty());
    std::fs::remove_dir_all(dir).ok();
}
