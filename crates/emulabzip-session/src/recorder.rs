//! Following a device and writing a session.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use emulabzip_core::{Device, Error, Lab, LogOptions, PerfOptions, Result, Target};
use emulabzip_net::{FlowStore, to_har};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::format::{
    CRASHES, DeviceMeta, JsonlWriter, LOGS, META, Meta, NETWORK, PERF, SCREENS, VERSION, bad, now_ms, pack, work_dir,
};

/// How often the device is asked for new crashes.
const CRASH_POLL: Duration = Duration::from_secs(5);
/// How often what has been written is pushed to disk.
const FLUSH: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Options {
    /// The app the recording is about; samples are taken for it and crashes are limited to it.
    pub app: Option<String>,
    /// Take a screenshot this often.
    pub screens_every: Option<Duration>,
    pub perf_interval_ms: u32,
    /// Log lines kept; later ones are counted but not written.
    pub max_log_lines: u64,
    /// The requests to save as a HAR file at the end. The caller runs the proxy that fills it.
    pub flows: Option<Arc<FlowStore>>,
}

impl Default for Options {
    fn default() -> Self {
        Options { app: None, screens_every: None, perf_interval_ms: 1000, max_log_lines: 500_000, flows: None }
    }
}

/// What has been recorded so far, for a progress line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub logs: u64,
    pub samples: u64,
    pub crashes: u64,
    pub screens: u64,
}

#[derive(Default)]
struct Counters {
    logs: AtomicU64,
    dropped: AtomicU64,
    samples: AtomicU64,
    crashes: AtomicU64,
    screens: AtomicU64,
}

/// A recording in progress. Call [`Recorder::finish`] to write the file.
pub struct Recorder {
    dir: PathBuf,
    out: PathBuf,
    meta: Meta,
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
    counters: Arc<Counters>,
    flows: Option<Arc<FlowStore>>,
    /// Ids of crashes seen while recording.
    new_crashes: Arc<std::sync::Mutex<Vec<String>>>,
}

impl Recorder {
    /// Starts following `device`. Files go to `<out>.partial` until [`Recorder::finish`].
    pub async fn start(lab: Lab, device: &Device, out: PathBuf, options: Options) -> Result<Recorder> {
        let dir = work_dir(&out);
        if dir.exists() {
            return Err(Error::failed(format!(
                "{} is left over from a recording that did not finish. Open it with `emulabzip session show`, or delete it.",
                dir.display()
            )));
        }
        fs::create_dir_all(dir.join(CRASHES))?;
        if options.screens_every.is_some() {
            fs::create_dir_all(dir.join(SCREENS))?;
        }
        let target = device.target();
        let meta = Meta {
            version: VERSION,
            tool: format!("EmuLabZip {}", env!("CARGO_PKG_VERSION")),
            device: DeviceMeta::from(device),
            app: options.app.clone(),
            started_ms: now_ms(),
            ended_ms: None,
            network: options.flows.is_some(),
            screens_every_secs: options.screens_every.map(|d| d.as_secs()),
            logs_dropped: 0,
        };
        write_meta(&dir, &meta)?;

        let (stop, rx) = watch::channel(false);
        let counters = Arc::new(Counters::default());
        let new_crashes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut tasks = Vec::new();

        // The streams are opened before this returns, so a failure to start is the caller's to see.
        let logs = lab.logs(&target, LogOptions { tail: 0, verbose: true }).await?;
        tasks.push(tokio::spawn(follow_logs(
            logs,
            dir.join(LOGS),
            rx.clone(),
            counters.clone(),
            options.max_log_lines,
        )));

        match lab.perf(&target, PerfOptions { app: options.app.clone(), interval_ms: options.perf_interval_ms }).await {
            Ok(samples) => tasks.push(tokio::spawn(follow_perf(samples, dir.join(PERF), rx.clone(), counters.clone()))),
            Err(e) => tracing::info!("not sampling performance: {e}"),
        }

        let seen: HashSet<String> = lab
            .crashes(&target, options.app.as_deref())
            .await
            .map(|l| l.into_iter().map(|c| c.id).collect())
            .unwrap_or_default();
        tasks.push(tokio::spawn(follow_crashes(
            lab.clone(),
            target.clone(),
            options.app.clone(),
            seen,
            dir.join(CRASHES),
            rx.clone(),
            counters.clone(),
            new_crashes.clone(),
        )));

        if let Some(every) = options.screens_every {
            tasks.push(tokio::spawn(follow_screens(lab, target, every, dir.join(SCREENS), rx, counters.clone())));
        }

        Ok(Recorder { dir, out, meta, stop, tasks, counters, flows: options.flows, new_crashes })
    }

    pub fn stats(&self) -> Stats {
        let c = &self.counters;
        Stats {
            logs: c.logs.load(Ordering::Relaxed),
            samples: c.samples.load(Ordering::Relaxed),
            crashes: c.crashes.load(Ordering::Relaxed),
            screens: c.screens.load(Ordering::Relaxed),
        }
    }

    /// Ids of the crashes that happened since the recording started.
    pub fn new_crashes(&self) -> Vec<String> {
        self.new_crashes.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Stops following, writes the end of the session and packs it into its file. Returns the path.
    pub async fn finish(mut self) -> Result<PathBuf> {
        // One last look for crashes happens inside the poller when it is told to stop.
        let _ = self.stop.send(true);
        for task in self.tasks.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(20), task).await;
        }
        if let Some(flows) = &self.flows {
            let har = to_har(&flows.all());
            fs::write(
                self.dir.join(NETWORK),
                serde_json::to_vec_pretty(&har).map_err(|e| bad("the requests", e.to_string()))?,
            )?;
        }
        self.meta.ended_ms = Some(now_ms());
        self.meta.logs_dropped = self.counters.dropped.load(Ordering::Relaxed);
        write_meta(&self.dir, &self.meta)?;
        let (dir, out) = (self.dir.clone(), self.out.clone());
        tokio::task::spawn_blocking(move || pack(&dir, &out)).await.map_err(|e| Error::failed(e.to_string()))??;
        Ok(self.out.clone())
    }
}

fn write_meta(dir: &std::path::Path, meta: &Meta) -> Result<()> {
    fs::write(dir.join(META), serde_json::to_vec_pretty(meta).map_err(|e| bad("session.json", e.to_string()))?)?;
    Ok(())
}

async fn stopped(rx: &mut watch::Receiver<bool>) {
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            return;
        }
    }
}

async fn follow_logs(
    mut stream: emulabzip_core::Stream<emulabzip_core::LogEntry>,
    path: PathBuf,
    mut stop: watch::Receiver<bool>,
    counters: Arc<Counters>,
    max: u64,
) {
    let Ok(mut out) = JsonlWriter::create(&path) else { return };
    let mut tick = tokio::time::interval(FLUSH);
    loop {
        tokio::select! {
            entry = stream.next() => match entry {
                Some(entry) => {
                    if counters.logs.load(Ordering::Relaxed) < max {
                        if out.write(&entry).is_ok() {
                            counters.logs.fetch_add(1, Ordering::Relaxed);
                        }
                    } else {
                        counters.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
                None => break,
            },
            _ = tick.tick() => out.flush(),
            _ = stopped(&mut stop) => break,
        }
    }
    out.flush();
}

async fn follow_perf(
    mut stream: emulabzip_core::Stream<emulabzip_core::PerfSample>,
    path: PathBuf,
    mut stop: watch::Receiver<bool>,
    counters: Arc<Counters>,
) {
    let Ok(mut out) = JsonlWriter::create(&path) else { return };
    loop {
        tokio::select! {
            sample = stream.next() => match sample {
                Some(sample) => {
                    if out.write(&sample).is_ok() {
                        counters.samples.fetch_add(1, Ordering::Relaxed);
                        out.flush();
                    }
                }
                None => break,
            },
            _ = stopped(&mut stop) => break,
        }
    }
    out.flush();
}

#[allow(clippy::too_many_arguments)]
async fn follow_crashes(
    lab: Lab,
    target: Target,
    app: Option<String>,
    mut seen: HashSet<String>,
    dir: PathBuf,
    mut stop: watch::Receiver<bool>,
    counters: Arc<Counters>,
    found: Arc<std::sync::Mutex<Vec<String>>>,
) {
    let mut tick = tokio::time::interval(CRASH_POLL);
    tick.tick().await;
    loop {
        let last = tokio::select! {
            _ = tick.tick() => false,
            _ = stopped(&mut stop) => true,
        };
        if let Ok(list) = lab.crashes(&target, app.as_deref()).await {
            for summary in
                list.into_iter().filter(|c| emulabzip_core::crash::is_valid_id(&c.id) && seen.insert(c.id.clone()))
            {
                let report = lab.crash(&target, &summary.id).await;
                let name = format!("{}.json", summary.id);
                let json = match report {
                    Ok(report) => serde_json::to_vec_pretty(&report),
                    // A report that cannot be fetched is still worth knowing about.
                    Err(_) => serde_json::to_vec_pretty(&emulabzip_core::CrashReport {
                        summary: summary.clone(),
                        details: Vec::new(),
                        threads: Vec::new(),
                        hints: Vec::new(),
                        raw: String::new(),
                    }),
                };
                if let Ok(json) = json
                    && fs::write(dir.join(name), json).is_ok()
                {
                    counters.crashes.fetch_add(1, Ordering::Relaxed);
                    found.lock().unwrap_or_else(|e| e.into_inner()).push(summary.id);
                }
            }
        }
        if last {
            break;
        }
    }
}

async fn follow_screens(
    lab: Lab,
    target: Target,
    every: Duration,
    dir: PathBuf,
    mut stop: watch::Receiver<bool>,
    counters: Arc<Counters>,
) {
    let mut tick = tokio::time::interval(every.max(Duration::from_secs(1)));
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = stopped(&mut stop) => break,
        }
        if let Ok(png) = lab.screenshot(&target).await {
            let n = counters.screens.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = fs::write(dir.join(format!("{n:04}.png")), png);
        }
    }
}
