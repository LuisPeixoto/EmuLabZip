//! What a Perfetto trace says at a glance: where the CPU went, how the frames did, how much memory
//! the processes used and what the busiest slices were.
//!
//! Every part is optional. A trace records what its configuration asked for, so a memory trace has
//! no scheduling and a CPU trace has no frames; a section whose data is missing is simply left out.

use std::collections::HashMap;

use async_trait::async_trait;
use emulabzip_core::Result;
use serde::Serialize;

use super::processor::{Processor, Rows};

/// Anything that answers SQL about a trace: the real processor, or a recording of one in tests.
#[async_trait]
pub trait SqlSource: Send + Sync {
    async fn query(&self, sql: &str) -> Result<Rows>;
}

#[async_trait]
impl SqlSource for Processor {
    async fn query(&self, sql: &str) -> Result<Rows> {
        Processor::query(self, sql).await
    }
}

pub const OVERVIEW: &str = "select (trace_end() - trace_start()) as dur_ns, \
    (select count(*) from process) as processes, (select count(*) from thread) as threads";

pub const CPU_TOTALS: &str = "select count(distinct s.cpu) as cpus, sum(s.dur) as busy_ns \
    from sched s join thread t using (utid) where t.tid != 0";

pub const CPU_BY_PROCESS: &str = "select coalesce(p.name, t.name, 'unknown') as name, coalesce(p.pid, t.tid) as pid, sum(s.dur) as dur_ns \
    from sched s join thread t using (utid) left join process p using (upid) where t.tid != 0 \
    group by coalesce(p.upid, -t.utid) order by dur_ns desc limit 12";

pub const CPU_BY_THREAD: &str = "select coalesce(t.name, 'tid ' || t.tid) as name, t.tid, coalesce(p.name, '') as process, sum(s.dur) as dur_ns \
    from sched s join thread t using (utid) left join process p using (upid) where t.tid != 0 \
    group by t.utid order by dur_ns desc limit 12";

pub const SLICES: &str = "select name, count(*) as n, sum(dur) as total_ns, max(dur) as max_ns \
    from slice where dur > 0 and name is not null group by name order by total_ns desc limit 15";

pub const FRAMES: &str = "select dur, jank_type, layer_name from actual_frame_timeline_slice";

pub const MEMORY_SYSTEM: &str = "select t.name as name, min(c.value) as min, max(c.value) as max, avg(c.value) as avg \
    from counter c join counter_track t on c.track_id = t.id \
    where t.name in ('MemTotal', 'MemFree', 'MemAvailable', 'Cached', 'Buffers', 'SwapTotal', 'SwapFree') group by t.name";

pub const MEMORY_PROCESSES: &str = "select p.name as name, p.pid as pid, max(c.value) as peak, min(c.value) as low \
    from counter c join process_counter_track t on c.track_id = t.id join process p using (upid) \
    where t.name = 'mem.rss' and p.name is not null group by p.upid order by peak desc limit 12";

/// The kinds of jank that mean a frame was late. `Prediction Error` and `Buffer Stuffing` are
/// reported too, but they do not mean the user saw a late frame.
const MISSED: &[&str] = &[
    "App Deadline Missed",
    "SurfaceFlinger CPU Deadline Missed",
    "SurfaceFlinger GPU Deadline Missed",
    "Display HAL",
    "Dropped Frame",
    "Unknown Jank",
    "SurfaceFlinger Scheduling",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub duration_s: f64,
    pub processes: u64,
    pub threads: u64,
    pub cpu: Option<CpuSummary>,
    pub frames: Option<FrameSummary>,
    pub memory: Option<MemorySummary>,
    /// The slices with the most time in total, busiest first.
    pub slices: Vec<SliceStat>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuSummary {
    pub cpus: u64,
    /// The share of all CPU time that was busy, from 0 to 1.
    pub busy_fraction: f64,
    pub by_process: Vec<CpuUse>,
    pub by_thread: Vec<ThreadUse>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuUse {
    pub name: String,
    pub pid: i64,
    pub dur_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUse {
    pub name: String,
    pub tid: i64,
    pub process: String,
    pub dur_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSummary {
    pub total: u64,
    /// Frames that missed a deadline.
    pub missed: u64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p99_ms: f64,
    pub worst_ms: f64,
    /// Every kind of jank and how many frames had it, most common first.
    pub jank: Vec<(String, u64)>,
    pub layers: Vec<LayerFrames>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerFrames {
    pub name: String,
    pub frames: u64,
    pub missed: u64,
    pub avg_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySummary {
    pub system: Vec<MemoryStat>,
    pub processes: Vec<ProcessMemory>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStat {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMemory {
    pub name: String,
    pub pid: i64,
    pub peak_bytes: f64,
    pub low_bytes: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceStat {
    pub name: String,
    pub count: u64,
    pub total_ns: u64,
    pub max_ns: u64,
}

fn text(r: &[super::processor::Cell], i: usize) -> String {
    r.get(i).and_then(|c| c.as_str()).unwrap_or_default().to_string()
}

fn int(r: &[super::processor::Cell], i: usize) -> i64 {
    r.get(i).and_then(|c| c.as_i64()).unwrap_or(0)
}

fn num(r: &[super::processor::Cell], i: usize) -> f64 {
    r.get(i).and_then(|c| c.as_f64()).unwrap_or(0.0)
}

/// The value below which `fraction` of the sorted `values` fall (nearest rank).
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (fraction * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn frames_from(rows: &Rows) -> Option<FrameSummary> {
    if rows.rows.is_empty() {
        return None;
    }
    let mut durations = Vec::with_capacity(rows.rows.len());
    let mut jank: HashMap<String, u64> = HashMap::new();
    let mut layers: HashMap<String, (u64, u64, f64)> = HashMap::new();
    let mut missed = 0;
    for r in &rows.rows {
        let ms = num(r, 0) / 1e6;
        durations.push(ms);
        let kinds = text(r, 1);
        let flags: Vec<&str> = kinds.split(',').map(str::trim).filter(|f| !f.is_empty() && *f != "None").collect();
        let late = flags.iter().any(|f| MISSED.contains(f));
        for f in &flags {
            *jank.entry(f.to_string()).or_default() += 1;
        }
        missed += u64::from(late);
        let layer = match text(r, 2) {
            l if l.is_empty() => "(no layer)".to_string(),
            l => l.trim_start_matches("TX - ").to_string(),
        };
        let e = layers.entry(layer).or_default();
        e.0 += 1;
        e.1 += u64::from(late);
        e.2 += ms;
    }
    durations.sort_by(f64::total_cmp);
    let mut jank: Vec<_> = jank.into_iter().collect();
    jank.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut layers: Vec<LayerFrames> = layers
        .into_iter()
        .map(|(name, (frames, missed, total))| LayerFrames { name, frames, missed, avg_ms: total / frames as f64 })
        .collect();
    layers.sort_by(|a, b| b.frames.cmp(&a.frames).then_with(|| a.name.cmp(&b.name)));
    layers.truncate(8);
    Some(FrameSummary {
        total: durations.len() as u64,
        missed,
        p50_ms: percentile(&durations, 0.50),
        p90_ms: percentile(&durations, 0.90),
        p99_ms: percentile(&durations, 0.99),
        worst_ms: durations.last().copied().unwrap_or(0.0),
        jank,
        layers,
    })
}

/// Runs a query for one section. A trace that does not have the data (or the table) gives `None`
/// for that section, not an error for the whole summary.
async fn section(src: &dyn SqlSource, sql: &str) -> Option<Rows> {
    match src.query(sql).await {
        Ok(rows) if !rows.rows.is_empty() => Some(rows),
        Ok(_) => None,
        Err(e) => {
            tracing::debug!("summary query skipped: {e}");
            None
        }
    }
}

pub async fn summarize(src: &dyn SqlSource) -> Result<Summary> {
    let overview = src.query(OVERVIEW).await?;
    let first = overview.rows.first().map(Vec::as_slice).unwrap_or_default();
    let duration_ns = num(first, 0);

    let cpu = match section(src, CPU_TOTALS).await {
        Some(totals) if int(&totals.rows[0], 0) > 0 => {
            let (cpus, busy) = (int(&totals.rows[0], 0) as u64, num(&totals.rows[0], 1));
            let capacity = duration_ns * cpus as f64;
            let by_process = section(src, CPU_BY_PROCESS)
                .await
                .map(|r| {
                    r.rows
                        .iter()
                        .map(|r| CpuUse { name: text(r, 0), pid: int(r, 1), dur_ns: int(r, 2) as u64 })
                        .collect()
                })
                .unwrap_or_default();
            let by_thread = section(src, CPU_BY_THREAD)
                .await
                .map(|r| {
                    r.rows
                        .iter()
                        .map(|r| ThreadUse {
                            name: text(r, 0),
                            tid: int(r, 1),
                            process: text(r, 2),
                            dur_ns: int(r, 3) as u64,
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(CpuSummary {
                cpus,
                busy_fraction: if capacity > 0.0 { (busy / capacity).clamp(0.0, 1.0) } else { 0.0 },
                by_process,
                by_thread,
            })
        }
        _ => None,
    };

    let frames = section(src, FRAMES).await.as_ref().and_then(frames_from);

    let system = section(src, MEMORY_SYSTEM).await;
    let processes = section(src, MEMORY_PROCESSES).await;
    let memory = (system.is_some() || processes.is_some()).then(|| MemorySummary {
        system: system
            .map(|r| {
                r.rows
                    .iter()
                    .map(|r| MemoryStat { name: text(r, 0), min: num(r, 1), max: num(r, 2), avg: num(r, 3) })
                    .collect()
            })
            .unwrap_or_default(),
        processes: processes
            .map(|r| {
                r.rows
                    .iter()
                    .map(|r| ProcessMemory {
                        name: text(r, 0),
                        pid: int(r, 1),
                        peak_bytes: num(r, 2),
                        low_bytes: num(r, 3),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    });

    let slices = section(src, SLICES)
        .await
        .map(|r| {
            r.rows
                .iter()
                .map(|r| SliceStat {
                    name: text(r, 0),
                    count: int(r, 1) as u64,
                    total_ns: int(r, 2) as u64,
                    max_ns: int(r, 3) as u64,
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Summary {
        duration_s: duration_ns / 1e9,
        processes: int(first, 1) as u64,
        threads: int(first, 2) as u64,
        cpu,
        frames,
        memory,
        slices,
    })
}

#[cfg(test)]
mod tests {
    use super::super::processor::Cell;
    use super::*;

    /// Answers each query with canned rows.
    struct Canned(HashMap<&'static str, Rows>);

    #[async_trait]
    impl SqlSource for Canned {
        async fn query(&self, sql: &str) -> Result<Rows> {
            self.0.get(sql).cloned().ok_or_else(|| emulabzip_core::Error::failed("no such table"))
        }
    }

    fn rows(columns: &[&str], rows: Vec<Vec<Cell>>) -> Rows {
        Rows { columns: columns.iter().map(|c| c.to_string()).collect(), rows, truncated: false }
    }

    fn t(s: &str) -> Cell {
        Cell::Text(s.into())
    }

    #[test]
    fn percentiles_use_the_nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(
            (percentile(&v, 0.5), percentile(&v, 0.9), percentile(&v, 0.99), percentile(&v, 1.0)),
            (50.0, 90.0, 99.0, 100.0)
        );
        assert_eq!(percentile(&[], 0.5), 0.0);
        assert_eq!(percentile(&[7.0], 0.99), 7.0);
    }

    #[test]
    fn counts_late_frames_by_kind_and_layer() {
        let frames = rows(
            &["dur", "jank_type", "layer_name"],
            vec![
                vec![Cell::Int(8_000_000), t("None"), t("TX - com.app/Main#1")],
                vec![Cell::Int(40_000_000), t("App Deadline Missed, Buffer Stuffing"), t("TX - com.app/Main#1")],
                vec![Cell::Int(16_000_000), t("Prediction Error"), t("TX - com.app/Main#1")],
                vec![Cell::Int(30_000_000), t("SurfaceFlinger GPU Deadline Missed"), Cell::Null(())],
            ],
        );
        let f = frames_from(&frames).unwrap();
        assert_eq!((f.total, f.missed), (4, 2), "Prediction Error and Buffer Stuffing alone are not late frames");
        assert_eq!(f.worst_ms, 40.0);
        assert_eq!(f.jank[0].1, 1);
        assert!(f.jank.iter().any(|(k, n)| k == "Buffer Stuffing" && *n == 1));
        assert_eq!(f.layers[0].name, "com.app/Main#1");
        assert_eq!((f.layers[0].frames, f.layers[0].missed), (3, 1));
        assert!(f.layers.iter().any(|l| l.name == "(no layer)"));
    }

    #[test]
    fn no_frames_means_no_frame_section() {
        assert!(frames_from(&rows(&["dur", "jank_type", "layer_name"], vec![])).is_none());
    }

    #[tokio::test]
    async fn leaves_out_sections_the_trace_does_not_have() {
        let src = Canned(HashMap::from([(
            OVERVIEW,
            rows(
                &["dur_ns", "processes", "threads"],
                vec![vec![Cell::Int(5_000_000_000), Cell::Int(10), Cell::Int(40)]],
            ),
        )]));
        let s = summarize(&src).await.unwrap();
        assert_eq!((s.duration_s, s.processes, s.threads), (5.0, 10, 40));
        assert!(s.cpu.is_none() && s.frames.is_none() && s.memory.is_none() && s.slices.is_empty());
    }

    #[tokio::test]
    async fn works_out_how_busy_the_cpu_was() {
        let src = Canned(HashMap::from([
            (OVERVIEW, rows(&["a", "b", "c"], vec![vec![Cell::Int(10_000_000_000), Cell::Int(1), Cell::Int(1)]])),
            (CPU_TOTALS, rows(&["cpus", "busy_ns"], vec![vec![Cell::Int(4), Cell::Int(10_000_000_000)]])),
            (
                CPU_BY_PROCESS,
                rows(&["n", "p", "d"], vec![vec![t("system_server"), Cell::Int(552), Cell::Int(6_000_000_000)]]),
            ),
        ]));
        let cpu = summarize(&src).await.unwrap().cpu.unwrap();
        assert_eq!(cpu.cpus, 4);
        assert!((cpu.busy_fraction - 0.25).abs() < 1e-9, "10 s busy out of 4 cores x 10 s");
        assert_eq!((cpu.by_process[0].name.as_str(), cpu.by_process[0].pid), ("system_server", 552));
        assert!(cpu.by_thread.is_empty());
    }

    #[tokio::test]
    async fn a_trace_that_cannot_be_measured_is_an_error() {
        assert!(summarize(&Canned(HashMap::new())).await.is_err());
    }
}
