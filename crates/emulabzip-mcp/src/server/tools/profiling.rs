//! The profiling tools.

use crate::server::*;

#[tool_router(router = profiling_router, vis = "pub(in crate::server)")]
impl EmuLabServer {
    #[tool(
        description = "Read recent log lines (logcat on Android, the unified log on iOS), filtered by app, level, tag or text. Returns a snapshot, newest last."
    )]
    async fn read_logs(&self, Parameters(a): Parameters<args::Logs>) -> CallToolResult {
        reply(self.read_logs_inner(a).await)
    }

    #[tool(
        description = "Measure CPU, memory, frame rate and battery for a few seconds: samples one second apart, device-wide or for one app."
    )]
    async fn perf_snapshot(&self, Parameters(a): Parameters<args::Perf>) -> CallToolResult {
        reply(self.perf_inner(a).await)
    }

    #[tool(description = "Where an app's memory goes: PSS by category on Android, footprint by VM region on iOS.")]
    async fn memory_report(&self, Parameters(a): Parameters<args::App>) -> CallToolResult {
        reply(
            async { json(&self.lab.memory(&self.target(&a.device).await?, &a.app).await.map_err(|e| e.to_string())?) }
                .await,
        )
    }

    #[tool(
        description = "Time how long an app takes to open (cold, warm or hot), several times. Returns the median and every run. It stops and opens the app on the device."
    )]
    async fn measure_startup(&self, Parameters(a): Parameters<args::Startup>) -> CallToolResult {
        reply(
            async {
                let kind = match a.kind.as_deref() {
                    None => StartKind::Cold,
                    Some(k) => StartKind::parse(k).ok_or("kind is cold, warm or hot")?,
                };
                let runs = a.runs.unwrap_or(3).clamp(1, 20);
                let target = self.target(&a.device).await?;
                json(&self.lab.startup(&target, &a.app, kind, runs).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Measure the energy an app uses over a few seconds (at most 120). Android reports the system's estimate, wake locks, jobs and network; iOS reports CPU time. Use the app meanwhile."
    )]
    async fn measure_energy(&self, Parameters(a): Parameters<args::Energy>) -> CallToolResult {
        reply(
            async {
                let seconds = a.seconds.unwrap_or(20).clamp(1, 120);
                let target = self.target(&a.device).await?;
                self.lab.energy_begin(&target, &a.app).await.map_err(|e| e.to_string())?;
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                json(&self.lab.energy_end(&target, &a.app, seconds).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Take a heap dump of a debuggable Android app and look for leaks: destroyed Activities and removed Fragments still alive, each with the path from a GC root that keeps it, plus the objects that keep the most memory alive."
    )]
    async fn analyze_heap(&self, Parameters(a): Parameters<args::Heap>) -> CallToolResult {
        reply(
            async {
                let target = self.target(&a.device).await?;
                let dump = self.lab.heap_dump(&target, &a.app).await.map_err(|e| e.to_string())?;
                if !dump.file_name.ends_with(".hprof") {
                    return Err("only Android .hprof dumps can be analyzed here; open a .memgraph in Xcode".to_string());
                }
                let options = emulabzip_heap::Options { top: 10, all_heaps: a.all_heaps.unwrap_or(false) };
                let bytes = dump.bytes;
                let analysis = tokio::task::spawn_blocking(move || emulabzip_heap::analyze(&bytes, &options))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                json(&analysis)
            }
            .await,
        )
    }

    #[tool(
        description = "List the traces recorded with EmuLabZip (Perfetto system traces, simpleperf CPU profiles, Instruments recordings)."
    )]
    async fn list_traces(&self) -> CallToolResult {
        reply(json(&library::list()))
    }

    #[tool(
        description = "Summarize a recording: for a Perfetto trace, CPU by process and thread, frame times and jank, memory and the busiest slices; for a CPU profile, the hottest functions."
    )]
    async fn trace_summary(&self, Parameters(a): Parameters<args::Recording>) -> CallToolResult {
        reply(self.trace_summary_inner(&a.id).await)
    }

    #[tool(
        description = "Run PerfettoSQL on a Perfetto recording (tables such as slice, sched, thread, process, counter). Returns up to 5000 rows."
    )]
    async fn trace_query(&self, Parameters(a): Parameters<args::Query>) -> CallToolResult {
        reply(
            async {
                let rec = recording(&a.id)?;
                if rec.format != TraceFormat::Perfetto {
                    return Err("SQL works on Perfetto recordings.".into());
                }
                let processor = open_processor(&rec).await?;
                json(&processor.query(&a.sql).await.map_err(|e| e.to_string())?)
            }
            .await,
        )
    }

    #[tool(
        description = "Record a trace on a device and keep it in the library: a Perfetto system trace (Android), a simpleperf CPU profile (Android) or an Instruments template (iOS Simulator). Takes as long as the duration."
    )]
    async fn record_trace(&self, Parameters(a): Parameters<args::RecordTrace>) -> CallToolResult {
        reply(self.record_inner(a).await)
    }
}
