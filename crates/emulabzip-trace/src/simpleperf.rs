//! Reading `simpleperf report-sample --protobuf --show-callchain`.
//!
//! The file is the magic `SIMPLEPERF`, a little-endian `u16` version, then records, each one a
//! little-endian `u32` size followed by that many bytes of protobuf, ending with a size of zero. The
//! messages are described by `cmd_report_sample.proto` in the NDK; they are written out here with
//! `prost` so no `protoc` is needed to build.

use std::collections::HashMap;

use emulabzip_core::{Error, Result};
use prost::Message;
use serde::Serialize;

use crate::flame::{FlameBuilder, Node, Origin};

const MAGIC: &[u8] = b"SIMPLEPERF";
const VERSION: u16 = 1;

mod proto {
    use prost::Message;

    #[derive(Clone, PartialEq, Message)]
    pub struct CallChainEntry {
        #[prost(uint64, optional, tag = "1")]
        pub vaddr_in_file: Option<u64>,
        #[prost(uint32, optional, tag = "2")]
        pub file_id: Option<u32>,
        #[prost(int32, optional, tag = "3")]
        pub symbol_id: Option<i32>,
        /// 0 native, 1 interpreted JVM method, 2 JIT JVM method, 3 ART method.
        #[prost(int32, optional, tag = "4")]
        pub execution_type: Option<i32>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Sample {
        #[prost(uint64, optional, tag = "1")]
        pub time: Option<u64>,
        #[prost(int32, optional, tag = "2")]
        pub thread_id: Option<i32>,
        #[prost(message, repeated, tag = "3")]
        pub callchain: Vec<CallChainEntry>,
        #[prost(uint64, optional, tag = "4")]
        pub event_count: Option<u64>,
        #[prost(uint32, optional, tag = "5")]
        pub event_type_id: Option<u32>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct LostSituation {
        #[prost(uint64, optional, tag = "1")]
        pub sample_count: Option<u64>,
        #[prost(uint64, optional, tag = "2")]
        pub lost_count: Option<u64>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct File {
        #[prost(uint32, optional, tag = "1")]
        pub id: Option<u32>,
        #[prost(string, optional, tag = "2")]
        pub path: Option<String>,
        #[prost(string, repeated, tag = "3")]
        pub symbol: Vec<String>,
        #[prost(string, repeated, tag = "4")]
        pub mangled_symbol: Vec<String>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Thread {
        #[prost(uint32, optional, tag = "1")]
        pub thread_id: Option<u32>,
        #[prost(uint32, optional, tag = "2")]
        pub process_id: Option<u32>,
        #[prost(string, optional, tag = "3")]
        pub thread_name: Option<String>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct MetaInfo {
        #[prost(string, repeated, tag = "1")]
        pub event_type: Vec<String>,
        #[prost(string, optional, tag = "2")]
        pub app_package_name: Option<String>,
        #[prost(string, optional, tag = "3")]
        pub app_type: Option<String>,
        #[prost(string, optional, tag = "4")]
        pub android_sdk_version: Option<String>,
        #[prost(string, optional, tag = "5")]
        pub android_build_type: Option<String>,
        #[prost(bool, optional, tag = "6")]
        pub trace_offcpu: Option<bool>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct ContextSwitch {
        #[prost(bool, optional, tag = "1")]
        pub switch_on: Option<bool>,
        #[prost(uint64, optional, tag = "2")]
        pub time: Option<u64>,
        #[prost(uint32, optional, tag = "3")]
        pub thread_id: Option<u32>,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Record {
        #[prost(oneof = "record::Data", tags = "1, 2, 3, 4, 5, 6")]
        pub data: Option<record::Data>,
    }

    pub mod record {
        use prost::Oneof;

        #[derive(Clone, PartialEq, Oneof)]
        pub enum Data {
            #[prost(message, tag = "1")]
            Sample(super::Sample),
            #[prost(message, tag = "2")]
            Lost(super::LostSituation),
            #[prost(message, tag = "3")]
            File(super::File),
            #[prost(message, tag = "4")]
            Thread(super::Thread),
            #[prost(message, tag = "5")]
            MetaInfo(super::MetaInfo),
            #[prost(message, tag = "6")]
            ContextSwitch(super::ContextSwitch),
        }
    }
}

/// What the profile says about how it was recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub event_types: Vec<String>,
    pub app_package: Option<String>,
    /// `debuggable`, `profileable` or `non_profileable` when an app was profiled.
    pub app_type: Option<String>,
    pub android_sdk: Option<String>,
    pub build_type: Option<String>,
}

/// How the weights of a profile should be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Unit {
    /// Nanoseconds of CPU time.
    Nanoseconds,
    /// Counts of an event, such as CPU cycles.
    Events,
}

struct Sample {
    tid: u32,
    weight: u64,
    event: u32,
    /// Innermost frame first, as simpleperf writes them.
    stack: Vec<Frame>,
}

#[derive(Clone, Copy)]
struct Frame {
    file: u32,
    symbol: i32,
    vaddr: u64,
    exec: i32,
}

struct FileInfo {
    path: String,
    symbols: Vec<String>,
}

/// A thread, as the profile names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadInfo {
    pub tid: u32,
    pub name: String,
    pub samples: u64,
    pub weight: u64,
}

/// A process with its threads, busiest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub samples: u64,
    pub weight: u64,
    pub threads: Vec<ThreadInfo>,
}

/// Which samples go into a flame graph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Filter {
    pub pid: Option<u32>,
    pub tid: Option<u32>,
    /// Which of the event types to use (see [`Meta::event_types`]).
    pub event: u32,
    /// Keep the idle thread, which is not work anyone did.
    pub include_idle: bool,
}

pub struct Profile {
    pub meta: Meta,
    files: HashMap<u32, FileInfo>,
    threads: HashMap<u32, (u32, String)>,
    samples: Vec<Sample>,
    /// Samples that simpleperf dropped because it could not keep up.
    pub lost: u64,
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profile")
            .field("meta", &self.meta)
            .field("samples", &self.samples.len())
            .field("lost", &self.lost)
            .finish()
    }
}

fn parse_err(detail: impl Into<String>) -> Error {
    Error::parse("simpleperf report", detail)
}

pub fn parse(bytes: &[u8]) -> Result<Profile> {
    let rest = bytes.strip_prefix(MAGIC).ok_or_else(|| parse_err("it does not start with SIMPLEPERF"))?;
    let (version, mut rest) = rest.split_at_checked(2).ok_or_else(|| parse_err("it ends in the header"))?;
    let version = u16::from_le_bytes([version[0], version[1]]);
    if version != VERSION {
        return Err(parse_err(format!("version {version} is not supported (expected {VERSION})")));
    }

    let mut profile =
        Profile { meta: Meta::default(), files: HashMap::new(), threads: HashMap::new(), samples: Vec::new(), lost: 0 };
    loop {
        let (size, tail) = rest.split_at_checked(4).ok_or_else(|| parse_err("it ends before the closing record"))?;
        let size = u32::from_le_bytes([size[0], size[1], size[2], size[3]]) as usize;
        if size == 0 {
            break;
        }
        let (record, tail) = tail.split_at_checked(size).ok_or_else(|| parse_err("a record is cut short"))?;
        rest = tail;
        let record = proto::Record::decode(record).map_err(|e| parse_err(e.to_string()))?;
        match record.data {
            Some(proto::record::Data::Sample(s)) => profile.samples.push(Sample {
                tid: s.thread_id.unwrap_or(0).max(0) as u32,
                weight: s.event_count.unwrap_or(0),
                event: s.event_type_id.unwrap_or(0),
                stack: s
                    .callchain
                    .iter()
                    .map(|c| Frame {
                        file: c.file_id.unwrap_or(u32::MAX),
                        symbol: c.symbol_id.unwrap_or(-1),
                        vaddr: c.vaddr_in_file.unwrap_or(0),
                        exec: c.execution_type.unwrap_or(0),
                    })
                    .collect(),
            }),
            Some(proto::record::Data::File(f)) => {
                // Prefer the demangled names; fall back to the mangled ones when they are missing.
                let symbols = if f.symbol.is_empty() { f.mangled_symbol } else { f.symbol };
                profile.files.insert(f.id.unwrap_or(0), FileInfo { path: f.path.unwrap_or_default(), symbols });
            }
            Some(proto::record::Data::Thread(t)) => {
                profile
                    .threads
                    .insert(t.thread_id.unwrap_or(0), (t.process_id.unwrap_or(0), t.thread_name.unwrap_or_default()));
            }
            Some(proto::record::Data::MetaInfo(m)) => {
                profile.meta = Meta {
                    event_types: m.event_type,
                    app_package: m.app_package_name.filter(|s| !s.is_empty()),
                    app_type: m.app_type.filter(|s| !s.is_empty()),
                    android_sdk: m.android_sdk_version,
                    build_type: m.android_build_type,
                };
            }
            Some(proto::record::Data::Lost(l)) => profile.lost += l.lost_count.unwrap_or(0),
            Some(proto::record::Data::ContextSwitch(_)) | None => {}
        }
    }
    Ok(profile)
}

/// Java frames from the platform, as opposed to the app's own.
const PLATFORM_PREFIXES: &[&str] = &[
    "java.",
    "javax.",
    "android.",
    "com.android.",
    "com.google.android.",
    "dalvik.",
    "libcore.",
    "sun.",
    "kotlin.",
    "kotlinx.",
];

/// Files that hold managed (Java or Kotlin) code, compiled ahead of time or not.
const MANAGED_FILES: &[&str] = &[".jar", ".apk", ".odex", ".oat", ".vdex", ".art", ".dex"];

/// The read-only partitions the platform and its preinstalled apps live on.
const SYSTEM_ROOTS: &[&str] = &["/system/", "/system_ext/", "/apex/", "/product/", "/vendor/", "/odm/"];

fn origin_of(path: &str, exec: i32, symbol: &str) -> Origin {
    if path.starts_with("[kernel") || path.ends_with(".ko") || path == "[vdso]" {
        return Origin::Kernel;
    }
    let installed =
        path.starts_with("/data/app/") || path.starts_with("/data/data/") || path.starts_with("/data/user/");
    if path.ends_with(".so") {
        return if installed {
            Origin::App
        } else if path.contains("libart") || path.starts_with("/apex/com.android.art/") {
            Origin::Framework
        } else {
            Origin::Native
        };
    }
    // Interpreted (1), JIT-compiled (2) and ART-internal (3) frames are managed code; so is
    // ahead-of-time compiled code, which simpleperf reports as native but which lives in these files.
    let managed = matches!(exec, 1..=3) || MANAGED_FILES.iter().any(|e| path.ends_with(e));
    if managed {
        return if installed {
            Origin::App
        } else if SYSTEM_ROOTS.iter().any(|r| path.starts_with(r))
            || PLATFORM_PREFIXES.iter().any(|p| symbol.starts_with(p))
        {
            Origin::Framework
        } else {
            Origin::App
        };
    }
    if installed { Origin::App } else { Origin::Native }
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

impl Profile {
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// The unit of the weights for `event`.
    pub fn unit(&self, event: u32) -> Unit {
        match self.meta.event_types.get(event as usize).map(String::as_str) {
            Some("cpu-clock" | "task-clock") => Unit::Nanoseconds,
            _ => Unit::Events,
        }
    }

    fn process_of(&self, tid: u32) -> (u32, &str) {
        let (pid, name) = self.threads.get(&tid).map_or((tid, ""), |(p, n)| (*p, n.as_str()));
        (pid, name)
    }

    fn process_name(&self, pid: u32) -> String {
        // The main thread carries the process name.
        match self.threads.get(&pid) {
            Some((_, name)) if !name.is_empty() => name.clone(),
            _ => format!("pid {pid}"),
        }
    }

    fn is_idle(&self, tid: u32) -> bool {
        tid == 0 || self.threads.get(&tid).is_some_and(|(pid, name)| *pid == 0 && name.starts_with("swapper"))
    }

    /// Processes and threads with samples for `event`, busiest first.
    pub fn processes(&self, event: u32, include_idle: bool) -> Vec<ProcessInfo> {
        let mut per_thread: HashMap<u32, (u64, u64)> = HashMap::new();
        for s in self.samples.iter().filter(|s| s.event == event && (include_idle || !self.is_idle(s.tid))) {
            let e = per_thread.entry(s.tid).or_default();
            e.0 += 1;
            e.1 += s.weight;
        }
        let mut per_process: HashMap<u32, ProcessInfo> = HashMap::new();
        for (tid, (samples, weight)) in per_thread {
            let (pid, name) = self.process_of(tid);
            let p = per_process.entry(pid).or_insert_with(|| ProcessInfo {
                pid,
                name: self.process_name(pid),
                samples: 0,
                weight: 0,
                threads: Vec::new(),
            });
            p.samples += samples;
            p.weight += weight;
            let thread_name = if name.is_empty() { format!("tid {tid}") } else { name.to_string() };
            p.threads.push(ThreadInfo { tid, name: thread_name, samples, weight });
        }
        let mut all: Vec<ProcessInfo> = per_process.into_values().collect();
        for p in &mut all {
            p.threads.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.tid.cmp(&b.tid)));
        }
        all.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.pid.cmp(&b.pid)));
        all
    }

    fn frame_name(&self, f: &Frame) -> (String, Origin) {
        let Some(file) = self.files.get(&f.file) else {
            return (format!("[unknown]+{:#x}", f.vaddr), Origin::Native);
        };
        let symbol = usize::try_from(f.symbol).ok().and_then(|i| file.symbols.get(i));
        let name = match symbol {
            Some(s) if !s.is_empty() => s.clone(),
            _ => format!("{}+{:#x}", basename(&file.path), f.vaddr),
        };
        let origin = origin_of(&file.path, f.exec, &name);
        (name, origin)
    }

    /// The flame graph of the samples that match `filter`. With no thread picked, the stacks start
    /// with the process and thread names, so the graph reads from the process down.
    pub fn flame(&self, filter: &Filter) -> Node {
        let root = match (filter.tid, filter.pid) {
            (Some(tid), _) => self.threads.get(&tid).map_or(format!("tid {tid}"), |(_, n)| n.clone()),
            (None, Some(pid)) => self.process_name(pid),
            (None, None) => "all".to_string(),
        };
        let mut tree = FlameBuilder::new(&root);
        let mut cache: HashMap<(u32, i32, u64), (String, Origin)> = HashMap::new();
        for s in self.samples.iter().filter(|s| s.event == filter.event) {
            let (pid, thread_name) = self.process_of(s.tid);
            if filter.tid.is_some_and(|t| t != s.tid) || filter.pid.is_some_and(|p| p != pid) {
                continue;
            }
            if !filter.include_idle && self.is_idle(s.tid) {
                continue;
            }
            let mut names: Vec<(String, Origin)> = Vec::with_capacity(s.stack.len() + 2);
            if filter.tid.is_none() {
                if filter.pid.is_none() {
                    names.push((self.process_name(pid), Origin::Framework));
                }
                let label = if thread_name.is_empty() { format!("tid {}", s.tid) } else { thread_name.to_string() };
                names.push((label, Origin::Framework));
            }
            // simpleperf lists the innermost frame first; a flame graph starts at the outermost.
            for f in s.stack.iter().rev() {
                // Unsymbolized frames repeat across samples, so resolve each address once.
                let key = (f.file, f.symbol, if f.symbol < 0 { f.vaddr } else { 0 });
                names.push(cache.entry(key).or_insert_with(|| self.frame_name(f)).clone());
            }
            tree.add(names.iter().map(|(n, o)| (n.as_str(), *o)), s.weight);
        }
        tree.build()
    }
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::proto::{self, CallChainEntry, record::Data};
    use super::*;

    /// A tiny report with two processes, built by hand.
    fn encode(records: Vec<Data>) -> Vec<u8> {
        let mut out = b"SIMPLEPERF".to_vec();
        out.extend(1u16.to_le_bytes());
        for r in records {
            let bytes = proto::Record { data: Some(r) }.encode_to_vec();
            out.extend((bytes.len() as u32).to_le_bytes());
            out.extend(bytes);
        }
        out.extend(0u32.to_le_bytes());
        out
    }

    fn entry(file: u32, symbol: i32, exec: i32) -> CallChainEntry {
        CallChainEntry {
            vaddr_in_file: Some(0x10),
            file_id: Some(file),
            symbol_id: Some(symbol),
            execution_type: Some(exec),
        }
    }

    fn thread(tid: u32, pid: u32, name: &str) -> Data {
        Data::Thread(proto::Thread { thread_id: Some(tid), process_id: Some(pid), thread_name: Some(name.into()) })
    }

    fn sample(tid: i32, weight: u64, stack: Vec<CallChainEntry>) -> Data {
        Data::Sample(proto::Sample {
            time: Some(1),
            thread_id: Some(tid),
            callchain: stack,
            event_count: Some(weight),
            event_type_id: Some(0),
        })
    }

    fn fixture() -> Vec<u8> {
        encode(vec![
            Data::MetaInfo(proto::MetaInfo {
                event_type: vec!["cpu-clock".into()],
                app_package_name: Some("com.example".into()),
                app_type: Some("debuggable".into()),
                ..Default::default()
            }),
            Data::File(proto::File {
                id: Some(0),
                path: Some("/data/app/com.example/lib/libgame.so".into()),
                symbol: vec!["Game::update".into(), "Game::tick".into()],
                mangled_symbol: vec![],
            }),
            Data::File(proto::File {
                id: Some(1),
                path: Some("[kernel.kallsyms]".into()),
                symbol: vec!["do_syscall".into()],
                mangled_symbol: vec![],
            }),
            Data::File(proto::File {
                id: Some(2),
                path: Some("[JIT cache]".into()),
                symbol: vec!["android.os.Handler.dispatchMessage".into(), "com.example.Main.run".into()],
                mangled_symbol: vec![],
            }),
            thread(100, 100, "com.example"),
            thread(101, 100, "RenderThread"),
            thread(7, 7, "surfaceflinger"),
            thread(0, 0, "swapper"),
            // Innermost frame first.
            sample(100, 4_000_000, vec![entry(0, 0, 0), entry(0, 1, 0)]),
            sample(100, 1_000_000, vec![entry(1, 0, 0), entry(0, 1, 0)]),
            sample(101, 2_000_000, vec![entry(2, 1, 2), entry(2, 0, 2)]),
            sample(7, 3_000_000, vec![entry(1, 0, 0)]),
            sample(0, 9_000_000, vec![entry(1, 0, 0)]),
        ])
    }

    #[test]
    fn reads_meta_threads_and_samples() {
        let p = parse(&fixture()).unwrap();
        assert_eq!(p.meta.app_package.as_deref(), Some("com.example"));
        assert_eq!(p.meta.app_type.as_deref(), Some("debuggable"));
        assert_eq!(p.sample_count(), 5);
        assert_eq!(p.unit(0), Unit::Nanoseconds);
    }

    #[test]
    fn groups_threads_by_process_and_leaves_out_idle() {
        let procs = parse(&fixture()).unwrap().processes(0, false);
        assert_eq!(
            procs.iter().map(|p| (p.name.as_str(), p.weight)).collect::<Vec<_>>(),
            [("com.example", 7_000_000), ("surfaceflinger", 3_000_000)]
        );
        assert_eq!(
            procs[0].threads.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["com.example", "RenderThread"]
        );
    }

    #[test]
    fn builds_a_flame_graph_from_the_outermost_frame_in() {
        let p = parse(&fixture()).unwrap();
        let tree = p.flame(&Filter { pid: Some(100), tid: Some(100), ..Default::default() });
        assert_eq!(tree.name, "com.example");
        assert_eq!(tree.value, 5_000_000);
        // `Game::tick` is the caller of `Game::update` and of the kernel call.
        let tick = &tree.children[0];
        assert_eq!((tick.name.as_str(), tick.value, tick.own), ("Game::tick", 5_000_000, 0));
        assert_eq!(
            tick.children.iter().map(|c| (c.name.as_str(), c.origin, c.own)).collect::<Vec<_>>(),
            [("Game::update", Origin::App, 4_000_000), ("do_syscall", Origin::Kernel, 1_000_000)]
        );
    }

    #[test]
    fn starts_with_process_and_thread_when_nothing_is_picked() {
        let tree = parse(&fixture()).unwrap().flame(&Filter::default());
        assert_eq!(tree.name, "all");
        assert_eq!(tree.value, 10_000_000, "idle time is left out");
        assert_eq!(
            tree.children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["com.example", "surfaceflinger"]
        );
        assert_eq!(
            tree.children[0].children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["com.example", "RenderThread"]
        );
        let with_idle = parse(&fixture()).unwrap().flame(&Filter { include_idle: true, ..Default::default() });
        assert_eq!(with_idle.value, 19_000_000);
    }

    #[test]
    fn tells_platform_java_from_the_apps_own() {
        let p = parse(&fixture()).unwrap();
        let tree = p.flame(&Filter { tid: Some(101), ..Default::default() });
        let handler = &tree.children[0];
        assert_eq!((handler.name.as_str(), handler.origin), ("android.os.Handler.dispatchMessage", Origin::Framework));
        assert_eq!(
            (handler.children[0].name.as_str(), handler.children[0].origin),
            ("com.example.Main.run", Origin::App)
        );
    }

    #[test]
    fn classifies_frames_by_file_and_location() {
        let kernel = "[kernel.kallsyms]";
        for (path, exec, symbol, want) in [
            (kernel, 0, "do_syscall", Origin::Kernel),
            ("/system/lib64/libc.so", 0, "memcpy", Origin::Native),
            ("/apex/com.android.art/lib64/libart.so", 0, "art::Thread::Run", Origin::Framework),
            // Ahead-of-time compiled platform code is reported as native but lives in these files.
            (
                "/system/framework/oat/x86_64/services.odex",
                0,
                "com.android.server.ServiceThread.run",
                Origin::Framework,
            ),
            ("/apex/com.android.art/javalib/core-oj.jar", 0, "java.lang.Thread.run", Origin::Framework),
            ("/product/priv-app/Velvet/Velvet.apk", 0, "com.google.android.Foo.bar", Origin::Framework),
            // The user's own code, native or managed.
            ("/data/app/~~x/com.example-y/lib/x86_64/libgame.so", 0, "Game::tick", Origin::App),
            ("/data/app/~~x/com.example-y/base.apk", 0, "com.example.Main.run", Origin::App),
            ("/data/app/~~x/com.example-y/oat/x86_64/base.odex", 0, "androidx.core.Foo.bar", Origin::App),
            // JIT code has no file, so the class name decides.
            ("[JIT cache]", 2, "android.os.Handler.dispatchMessage", Origin::Framework),
            ("[JIT cache]", 2, "com.example.Main.run", Origin::App),
        ] {
            assert_eq!(origin_of(path, exec, symbol), want, "{path} {symbol}");
        }
    }

    #[test]
    fn names_frames_without_a_symbol_after_their_library() {
        let bytes = encode(vec![
            Data::File(proto::File {
                id: Some(0),
                path: Some("/system/lib64/libc.so".into()),
                symbol: vec![],
                mangled_symbol: vec![],
            }),
            thread(1, 1, "init"),
            sample(
                1,
                5,
                vec![CallChainEntry {
                    vaddr_in_file: Some(0x1234),
                    file_id: Some(0),
                    symbol_id: Some(-1),
                    execution_type: Some(0),
                }],
            ),
        ]);
        let tree = parse(&bytes).unwrap().flame(&Filter { tid: Some(1), ..Default::default() });
        assert_eq!(tree.children[0].name, "libc.so+0x1234");
        assert_eq!(tree.children[0].origin, Origin::Native);
    }

    #[test]
    fn rejects_other_files_and_cut_ones() {
        assert!(parse(b"not simpleperf").is_err());
        let mut bytes = fixture();
        bytes[10] = 9; // version
        assert!(parse(&bytes).unwrap_err().to_string().contains("version 9"));
        let full = fixture();
        assert!(parse(&full[..full.len() - 30]).is_err(), "a report that ends early is an error");
    }

    #[test]
    fn reads_a_real_report_from_an_emulator() {
        let p = parse(include_bytes!("../tests/fixtures/system.simpleperf.pb")).unwrap();
        assert!(p.sample_count() > 100, "{} samples", p.sample_count());
        assert_eq!(p.meta.event_types.first().map(String::as_str), Some("cpu-clock"));
        let procs = p.processes(0, false);
        assert!(
            procs.iter().any(|x| x.name == "system_server"),
            "{:?}",
            procs.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
        let tree = p.flame(&Filter::default());
        assert!(tree.value > 0 && tree.count() > 50);
        // Stacks start at the thread entry point and end at what ran, so a frame that only ever runs
        // other frames has weight but no time of its own.
        assert!(tree.children.iter().all(|c| c.own == 0));

        // A system-wide profile shows every kind of code, each told apart.
        fn origins(n: &Node, out: &mut std::collections::HashSet<Origin>) {
            out.insert(n.origin);
            n.children.iter().for_each(|c| origins(c, out));
        }
        let mut seen = std::collections::HashSet::new();
        origins(&tree, &mut seen);
        assert!(
            seen.contains(&Origin::Kernel) && seen.contains(&Origin::Native) && seen.contains(&Origin::Framework),
            "{seen:?}"
        );
        // The platform's Java code is not mistaken for native code.
        fn find<'a>(n: &'a Node, name: &str) -> Option<&'a Node> {
            if n.name == name {
                return Some(n);
            }
            n.children.iter().find_map(|c| find(c, name))
        }
        if let Some(run) = find(&tree, "com.android.server.ServiceThread.run") {
            assert_eq!(run.origin, Origin::Framework);
        }
    }
}
