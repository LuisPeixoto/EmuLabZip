//! `emulabzip trace`

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use emulabzip_core::{Lab, PerfettoPreset, TraceFile, TraceFormat, TraceRequest};
use emulabzip_trace::flame::{folded, hotspots};
use emulabzip_trace::library;
use emulabzip_trace::perfetto::processor::{self, Cell, Processor, Rows};
use emulabzip_trace::perfetto::summary::{Summary, summarize};
use emulabzip_trace::perfetto::ui;
use emulabzip_trace::{simpleperf, xctrace};
use tokio::sync::watch;

use crate::output;

/// A scratch folder for one recording, removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch> {
        let dir = std::env::temp_dir().join(format!("emulabzip-trace-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Records until the duration is up or Ctrl+C is pressed, which keeps what was recorded so far.
async fn record(lab: &Lab, device: Option<&str>, request: TraceRequest, dir: &Path) -> Result<TraceFile> {
    let device = lab.resolve(device).await?;
    eprintln!(
        "{}",
        output::dim(&format!("Recording {} s on {}. Press Ctrl+C to stop early.", request.duration_s(), device.name))
    );
    let (stop, stopped) = watch::channel(false);
    let target = device.target();
    let recording = lab.record_trace(&target, &request, dir, stopped);
    tokio::pin!(recording);
    loop {
        tokio::select! {
            file = &mut recording => return Ok(file?),
            _ = tokio::signal::ctrl_c() => {
                eprintln!("{}", output::dim("Stopping…"));
                let _ = stop.send(true);
            }
        }
    }
}

fn stamp() -> String {
    library::new_id()
}

/// Copies a file, or a folder (an Instruments document is one), to `to`.
fn copy_all(from: &Path, to: &Path) -> Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_all(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(from, to).with_context(|| format!("could not write {}", to.display()))?;
    }
    Ok(())
}

pub struct RecordArgs {
    pub device: Option<String>,
    pub preset: PerfettoPreset,
    pub config: Option<PathBuf>,
    pub duration: u32,
    pub app: Option<String>,
    pub output: Option<PathBuf>,
    pub open: bool,
}

pub async fn record_perfetto(lab: &Lab, args: RecordArgs) -> Result<()> {
    let config = match (&args.preset, &args.config) {
        (PerfettoPreset::Custom, Some(path)) => {
            Some(std::fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?)
        }
        (PerfettoPreset::Custom, None) => {
            bail!("--preset custom needs --config FILE with a Perfetto text configuration")
        }
        _ => None,
    };
    let scratch = Scratch::new()?;
    let request = TraceRequest::Perfetto { preset: args.preset, config, duration_s: args.duration, app: args.app };
    let file = record(lab, args.device.as_deref(), request, &scratch.0).await?;
    let out = args.output.unwrap_or_else(|| PathBuf::from(format!("trace-{}.pftrace", stamp())));
    copy_all(&file.path, &out)?;
    println!("Saved {} ({})", out.display(), output::size(std::fs::metadata(&out)?.len()));
    if args.open {
        open_in_perfetto(&out).await?;
    }
    Ok(())
}

pub struct CpuArgs {
    pub device: Option<String>,
    pub app: Option<String>,
    pub system: bool,
    pub duration: u32,
    pub output: Option<PathBuf>,
    pub folded: Option<PathBuf>,
    pub top: usize,
}

pub async fn record_cpu(lab: &Lab, args: CpuArgs) -> Result<()> {
    if args.app.is_none() && !args.system {
        bail!("pick an app with --app, or profile the whole system with --system");
    }
    let scratch = Scratch::new()?;
    let request =
        TraceRequest::CpuProfile { app: args.app.clone(), duration_s: args.duration, system_wide: args.system };
    let file = record(lab, args.device.as_deref(), request, &scratch.0).await?;
    let bytes = std::fs::read(&file.path)?;
    if let Some(out) = &args.output {
        std::fs::write(out, &bytes).with_context(|| format!("could not write {}", out.display()))?;
        println!("Saved {}", out.display());
    }
    report_cpu(&simpleperf::parse(&bytes)?, &args)
}

fn report_cpu(profile: &simpleperf::Profile, args: &CpuArgs) -> Result<()> {
    let processes = profile.processes(0, false);
    let Some(busiest) = processes.first() else { bail!("the profile has no samples") };
    let ns = profile.unit(0) == simpleperf::Unit::Nanoseconds;
    let show = |w: u64| if ns { format!("{:.1} ms", w as f64 / 1e6) } else { format!("{w}") };
    println!("{}", output::heading("Processes by CPU time"));
    for p in processes.iter().take(8) {
        let pid = if p.name.starts_with("pid ") { String::new() } else { output::dim(&format!("pid {}", p.pid)) };
        println!("  {:>10}  {} {pid}", show(p.weight), p.name);
    }
    let flame = profile.flame(&simpleperf::Filter { pid: Some(busiest.pid), ..Default::default() });
    println!("\n{}", output::heading(&format!("Hot functions in {}", busiest.name)));
    for h in hotspots(&flame, args.top) {
        println!("  {:>10}  {}", show(h.own), h.name);
    }
    if let Some(path) = &args.folded {
        std::fs::write(path, folded(&flame)).with_context(|| format!("could not write {}", path.display()))?;
        println!("\nFolded stacks of {} saved to {}", busiest.name, path.display());
    }
    Ok(())
}

pub struct InstrumentsArgs {
    pub device: Option<String>,
    pub template: String,
    pub app: Option<String>,
    pub duration: u32,
    pub output: Option<PathBuf>,
    pub top: usize,
}

pub async fn record_instruments(lab: &Lab, args: InstrumentsArgs) -> Result<()> {
    let scratch = Scratch::new()?;
    let request = TraceRequest::Instruments { template: args.template, app: args.app, duration_s: args.duration };
    let file = record(lab, args.device.as_deref(), request, &scratch.0).await?;
    let out = args.output.unwrap_or_else(|| PathBuf::from(format!("recording-{}.trace", stamp())));
    copy_all(&file.path, &out)?;
    println!("Saved {}", out.display());
    if let Some(xml) = file.extras.iter().find(|p| p.extension().is_some_and(|e| e == "xml")) {
        let profile = xctrace::parse(&std::fs::read_to_string(xml)?)?;
        if let Some(busiest) = profile.processes().first() {
            println!("\n{}", output::heading(&format!("Hot functions in {}", busiest.name)));
            for h in hotspots(&profile.flame(Some(&busiest.name)), args.top) {
                println!("  {:>10}  {}", format!("{:.1} ms", h.own as f64 / 1e6), h.name);
            }
        }
    }
    println!("{}", output::dim("Open the .trace in Instruments for the full timeline."));
    Ok(())
}

/// Makes sure the trace processor is there, downloading it the first time.
async fn processor_binary() -> Result<PathBuf> {
    if let Some(path) = processor::installed() {
        return Ok(path);
    }
    let asset = processor::asset()?;
    eprintln!(
        "{}",
        output::dim(&format!(
            "Downloading the Perfetto trace processor {} ({}), once. It is checked against a pinned SHA-256.",
            processor::VERSION,
            output::size(asset.size)
        ))
    );
    let progress: processor::Progress = Arc::new(|done, total| {
        if let Some(total) = total {
            eprint!("\r  {:>3}%", done * 100 / total.max(1));
        }
    });
    let path = processor::ensure_installed(progress).await?;
    eprintln!("\r  done");
    Ok(path)
}

async fn open(trace: &Path) -> Result<Processor> {
    if !trace.is_file() {
        bail!("{} is not a file", trace.display());
    }
    let binary = processor_binary().await?;
    Ok(Processor::start(&binary, trace).await?)
}

fn cell(c: &Cell) -> String {
    match c {
        Cell::Null(()) => "NULL".into(),
        Cell::Int(i) => i.to_string(),
        Cell::Float(f) => format!("{f:.3}"),
        Cell::Text(t) => t.clone(),
    }
}

fn print_table(rows: &Rows) {
    let widths: Vec<usize> = (0..rows.columns.len())
        .map(|i| {
            rows.rows
                .iter()
                .map(|r| r.get(i).map_or(0, |c| cell(c).chars().count()))
                .chain([rows.columns[i].chars().count()])
                .max()
                .unwrap_or(0)
                .min(60)
        })
        .collect();
    let line = |cells: Vec<String>| {
        cells
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{:<w$}", c.chars().take(60).collect::<String>()))
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!("{}", output::heading(&line(rows.columns.clone())));
    for r in &rows.rows {
        println!("{}", line(r.iter().map(cell).collect()));
    }
    if rows.truncated {
        eprintln!("{}", output::dim("More rows were cut off. Add a LIMIT."));
    }
}

pub async fn query(trace: &Path, sql: &str, json: bool) -> Result<()> {
    let rows = open(trace).await?.query(sql).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        print_table(&rows);
    }
    Ok(())
}

pub async fn summary(trace: &Path, json: bool) -> Result<()> {
    let processor = open(trace).await?;
    let summary = summarize(&processor).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_summary(&summary);
    }
    Ok(())
}

fn ms(ns: u64) -> String {
    format!("{:.1} ms", ns as f64 / 1e6)
}

fn print_summary(s: &Summary) {
    println!(
        "{}  {}",
        output::heading(&format!("{:.1} s", s.duration_s)),
        output::dim(&format!("{} processes · {} threads", s.processes, s.threads))
    );
    if let Some(cpu) = &s.cpu {
        println!(
            "\n{}",
            output::heading(&format!("CPU: {:.0}% busy across {} cores", cpu.busy_fraction * 100.0, cpu.cpus))
        );
        for p in cpu.by_process.iter().take(8) {
            println!("  {:>10}  {} {}", ms(p.dur_ns), p.name, output::dim(&format!("pid {}", p.pid)));
        }
    }
    if let Some(f) = &s.frames {
        println!(
            "\n{}",
            output::heading(&format!(
                "Frames: {} total, {} late · p50 {:.1} ms · p90 {:.1} ms · p99 {:.1} ms",
                f.total, f.missed, f.p50_ms, f.p90_ms, f.p99_ms
            ))
        );
        for (kind, n) in f.jank.iter().take(6) {
            println!("  {n:>6}  {kind}");
        }
    }
    if let Some(m) = &s.memory {
        println!("\n{}", output::heading("Memory"));
        for p in m.processes.iter().take(8) {
            println!(
                "  {:>10}  {} {}",
                output::size(p.peak_bytes as u64),
                p.name,
                output::dim(&format!("pid {}", p.pid))
            );
        }
    }
    if !s.slices.is_empty() {
        println!("\n{}", output::heading("Busiest slices"));
        for sl in s.slices.iter().take(8) {
            println!("  {:>10}  {} {}", ms(sl.total_ns), sl.name, output::dim(&format!("{}×", sl.count)));
        }
    }
}

pub fn list(json: bool) -> Result<()> {
    let all = library::list();
    if json {
        println!("{}", serde_json::to_string_pretty(&all)?);
        return Ok(());
    }
    if all.is_empty() {
        println!(
            "{}",
            output::dim("No recordings in the library. Record one in the app, or use `emulabzip trace record`.")
        );
    }
    for r in &all {
        let what = match r.format {
            TraceFormat::Perfetto => "perfetto",
            TraceFormat::SimpleperfReport => "simpleperf",
            TraceFormat::InstrumentsBundle => "instruments",
        };
        println!("{}  {:<11}  {:<24}  {}", r.id, what, r.device, output::size(r.size_bytes));
        println!("  {}", output::dim(&r.path().display().to_string()));
    }
    Ok(())
}

/// Serves the trace to the Perfetto UI in the browser, and returns once it has been fetched.
async fn open_in_perfetto(trace: &Path) -> Result<()> {
    let handoff = ui::serve(trace).await?;
    println!("Opening the Perfetto UI. Waiting for the browser to load the trace…");
    open_browser(&handoff.url)?;
    handoff.finished().await;
    Ok(())
}

fn open_browser(url: &str) -> Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if cmd.status().is_ok_and(|s| s.success()) {
        Ok(())
    } else {
        bail!("could not open a browser. Open this address yourself:\n{url}")
    }
}

pub async fn perfetto_ui(trace: &Path) -> Result<()> {
    if !trace.is_file() {
        bail!("{} is not a file", trace.display());
    }
    open_in_perfetto(trace).await
}
