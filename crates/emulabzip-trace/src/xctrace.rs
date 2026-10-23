//! Reads the CPU samples of an Instruments recording, exported with `xctrace export`.
//!
//! ```text
//! xcrun xctrace export --input app.trace \
//!     --xpath '/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]'
//! ```
//!
//! The XML lists one `<row>` per sample, with the thread, the weight in nanoseconds and the
//! `<stack>` of frames, innermost first. To keep the file small, `xctrace` writes each distinct
//! element once, with an `id`, and every later use as an empty element with a `ref` to it. Stacks,
//! frames, threads and binaries are all shared this way.
//!
//! The `time-profile` table is what the Time Profiler template records; `cpu-profile` (the CPU
//! Profiler template) has the same shape and is read the same way.

use std::collections::HashMap;

use emulabzip_core::{Error, Result};
use roxmltree::{Document, Node as Xml};

use crate::flame::{FlameBuilder, Node, Origin};

/// A function in a stack, with the binary it lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub name: String,
    /// The binary's path, when `xctrace` knows it.
    pub binary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub process: String,
    pub thread: String,
    /// Time this sample stands for, in nanoseconds.
    pub weight: u64,
    /// Innermost frame first.
    pub stack: Vec<Frame>,
}

/// A process and the CPU time its samples add up to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub name: String,
    pub weight: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    pub samples: Vec<Sample>,
}

/// Reads an exported `time-profile` (or `cpu-profile`) table.
pub fn parse(xml: &str) -> Result<Profile> {
    let doc = Document::parse_with_options(xml, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
        .map_err(|e| Error::Parse { what: "xctrace export", detail: e.to_string() })?;
    let ids: HashMap<&str, Xml> =
        doc.descendants().filter(|n| n.is_element()).filter_map(|n| n.attribute("id").map(|id| (id, n))).collect();
    let resolve = |n| resolve(&ids, n);

    let mut samples = Vec::new();
    for row in doc.descendants().filter(|n| n.has_tag_name("row")) {
        let child = |name: &str| row.children().find(|c| c.has_tag_name(name)).map(resolve);
        let Some(stack) = child("stack") else { continue };
        let frames: Vec<Frame> = stack
            .children()
            .filter(|c| c.has_tag_name("frame"))
            .map(resolve)
            .map(|f| {
                let binary = f.children().find(|c| c.has_tag_name("binary")).map(resolve);
                Frame {
                    name: f
                        .attribute("name")
                        .map(str::to_string)
                        .or_else(|| f.attribute("addr").map(str::to_string))
                        .unwrap_or_else(|| "<unknown>".into()),
                    binary: binary.and_then(|b| b.attribute("path").or(b.attribute("name"))).map(str::to_string),
                }
            })
            .collect();
        if frames.is_empty() {
            continue;
        }
        let (thread, process) = match child("thread") {
            Some(t) => (
                t.attribute("fmt").unwrap_or("thread").to_string(),
                t.children()
                    .find(|c| c.has_tag_name("process"))
                    .map(resolve)
                    .and_then(|p| p.attribute("fmt"))
                    .map(process_name)
                    .unwrap_or_default(),
            ),
            None => ("thread".into(), String::new()),
        };
        let process = match child("process") {
            Some(p) => p.attribute("fmt").map(process_name).unwrap_or(process),
            None => process,
        };
        let weight = child("weight").and_then(|w| w.text()).and_then(|t| t.trim().parse().ok()).unwrap_or(1);
        samples.push(Sample { process, thread: thread_name(&thread), weight, stack: frames });
    }
    if samples.is_empty() {
        return Err(Error::Parse { what: "xctrace export", detail: "no samples in the table".into() });
    }
    Ok(Profile { samples })
}

/// An element that only points at another one (`ref`) stands for it.
fn resolve<'a, 'i>(ids: &HashMap<&str, Xml<'a, 'i>>, n: Xml<'a, 'i>) -> Xml<'a, 'i> {
    n.attribute("ref").and_then(|r| ids.get(r).copied()).unwrap_or(n)
}

/// `App (1234)` is shown as `App`.
fn process_name(fmt: &str) -> String {
    fmt.rsplit_once(" (").map_or(fmt, |(name, _)| name).trim().to_string()
}

/// `Main Thread 0x1f3a (App, pid: 1234)` is shown as `Main Thread`.
fn thread_name(fmt: &str) -> String {
    let name = fmt.split(" (").next().unwrap_or(fmt).trim();
    match name.rsplit_once(char::is_whitespace) {
        Some((rest, last)) if last.starts_with("0x") && last[2..].chars().all(|c| c.is_ascii_hexdigit()) => {
            rest.trim().to_string()
        }
        _ => name.to_string(),
    }
}

/// Whose code a frame is, from the path of its binary.
pub fn origin_of(binary: Option<&str>) -> Origin {
    let Some(path) = binary else { return Origin::Native };
    if path.contains("/Kernels/") || path.ends_with("/kernel") || path == "kernel" {
        Origin::Kernel
    } else if path.contains("/System/Library/")
        && (path.contains(".framework/") || path.contains("/PrivateFrameworks/"))
    {
        Origin::Framework
    } else if path.contains(".app/") || path.contains("/Containers/Bundle/") || path.contains("/DerivedData/") {
        Origin::App
    } else {
        Origin::Native
    }
}

impl Profile {
    /// The processes that have samples, with the most CPU time first.
    pub fn processes(&self) -> Vec<ProcessInfo> {
        let mut by: HashMap<&str, u64> = HashMap::new();
        for s in &self.samples {
            *by.entry(&s.process).or_default() += s.weight;
        }
        let mut all: Vec<ProcessInfo> =
            by.into_iter().map(|(name, weight)| ProcessInfo { name: name.to_string(), weight }).collect();
        all.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.name.cmp(&b.name)));
        all
    }

    pub fn total_weight(&self) -> u64 {
        self.samples.iter().map(|s| s.weight).sum()
    }

    /// The call tree of one process, or of all of them. Each thread is a branch of its own.
    pub fn flame(&self, process: Option<&str>) -> Node {
        let mut tree = FlameBuilder::new(process.unwrap_or("all"));
        for s in self.samples.iter().filter(|s| process.is_none_or(|p| p == s.process)) {
            let mut names: Vec<(&str, Origin)> = Vec::with_capacity(s.stack.len() + 2);
            if process.is_none() {
                names.push((&s.process, Origin::Framework));
            }
            names.push((&s.thread, Origin::Framework));
            // `xctrace` lists the innermost frame first; a flame graph starts at the outermost.
            names.extend(s.stack.iter().rev().map(|f| (f.name.as_str(), origin_of(f.binary.as_deref()))));
            tree.add(names, s.weight);
        }
        tree.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written by hand after the shape `xctrace export` produces, since recording needs Developer
    /// Mode on the Mac. It uses the id/ref sharing on purpose: the second row has nothing but refs.
    const EXPORT: &str = r#"<?xml version="1.0"?>
<trace-query-result>
<node xpath='//trace-toc[1]/run[1]/data[1]/table[1]'>
<schema name="time-profile"><col><mnemonic>time</mnemonic></col></schema>
<row>
<sample-time id="1" fmt="00:00.010.000">10000000</sample-time>
<thread id="2" fmt="Main Thread 0x1f3a (Notes, pid: 4242)"><tid id="3" fmt="0x1f3a">7994</tid><process id="4" fmt="Notes (4242)"><pid id="5" fmt="4242">4242</pid></process></thread>
<weight id="6" fmt="1.00 ms">1000000</weight>
<stack id="7" fmt="draw ← main">
<frame id="8" name="draw" addr="0x1000"><binary id="9" name="Notes" path="/Users/me/Library/Developer/CoreSimulator/Devices/X/data/Containers/Bundle/Application/Y/Notes.app/Notes"/></frame>
<frame id="10" name="UIApplicationMain" addr="0x2000"><binary id="11" name="UIKitCore" path="/Library/Developer/CoreSimulator/RuntimeRoot/System/Library/PrivateFrameworks/UIKitCore.framework/UIKitCore"/></frame>
<frame id="12" name="start" addr="0x3000"><binary id="13" name="dyld" path="/usr/lib/dyld"/></frame>
</stack>
</row>
<row>
<sample-time id="14" fmt="00:00.020.000">20000000</sample-time>
<thread ref="2"/>
<weight id="15" fmt="2.00 ms">2000000</weight>
<stack id="16" fmt="UIApplicationMain ← start">
<frame ref="10"/>
<frame ref="12"/>
</stack>
</row>
<row>
<sample-time id="17" fmt="00:00.030.000">30000000</sample-time>
<thread id="18" fmt="com.apple.uikit.eventfetch-thread 0x1f40 (Notes, pid: 4242)"><process ref="4"/></thread>
<weight ref="6"/>
<stack ref="7"/>
</row>
<row>
<sample-time id="19" fmt="00:00.040.000">40000000</sample-time>
<thread id="20" fmt="Main Thread 0x77 (SpringBoard, pid: 51)"><process id="21" fmt="SpringBoard (51)"/></thread>
<weight ref="15"/>
<stack id="22" fmt="">
<frame id="23" addr="0x9000"/>
</stack>
</row>
</node>
</trace-query-result>"#;

    #[test]
    fn follows_refs_to_the_elements_they_share() {
        let p = parse(EXPORT).unwrap();
        assert_eq!(p.samples.len(), 4);
        let last = &p.samples[2];
        assert_eq!((last.weight, last.stack.len(), last.process.as_str()), (1_000_000, 3, "Notes"));
        assert_eq!(last.stack[0].name, "draw", "the stack came from a ref");
        assert_eq!(p.samples[1].weight, 2_000_000);
        assert_eq!(p.samples[1].stack[0].binary.as_deref().map(|b| b.ends_with("UIKitCore")), Some(true));
    }

    #[test]
    fn names_threads_and_processes_without_the_ids() {
        let p = parse(EXPORT).unwrap();
        assert_eq!(p.samples[0].thread, "Main Thread");
        assert_eq!(p.samples[2].thread, "com.apple.uikit.eventfetch-thread");
        assert_eq!(p.samples[3].process, "SpringBoard");
        assert_eq!(p.samples[3].stack[0].name, "0x9000", "a frame without a symbol shows its address");
    }

    #[test]
    fn sums_up_the_time_per_process() {
        let procs = parse(EXPORT).unwrap().processes();
        assert_eq!(
            procs.iter().map(|p| (p.name.as_str(), p.weight)).collect::<Vec<_>>(),
            [("Notes", 4_000_000), ("SpringBoard", 2_000_000)]
        );
    }

    #[test]
    fn tells_app_code_from_system_code() {
        let p = parse(EXPORT).unwrap();
        let origins: Vec<Origin> = p.samples[0].stack.iter().map(|f| origin_of(f.binary.as_deref())).collect();
        assert_eq!(origins, [Origin::App, Origin::Framework, Origin::Native]);
        assert_eq!(origin_of(Some("/System/Library/Kernels/kernel.release.t8112")), Origin::Kernel);
        assert_eq!(origin_of(None), Origin::Native);
    }

    #[test]
    fn builds_a_flame_graph_from_the_outermost_frame_in() {
        let p = parse(EXPORT).unwrap();
        let tree = p.flame(Some("Notes"));
        assert_eq!((tree.name.as_str(), tree.value), ("Notes", 4_000_000));
        let main = &tree.children[0];
        assert_eq!(main.name, "Main Thread");
        let start = &main.children[0];
        assert_eq!(start.name, "start");
        assert_eq!(start.children[0].name, "UIApplicationMain");
        // `draw` ran for 1 ms by itself, in the main thread and again in the other thread.
        let all = p.flame(None);
        assert_eq!(all.children.len(), 2, "one branch per process");
        assert_eq!(all.value, 6_000_000);
    }

    #[test]
    fn rejects_an_export_without_samples() {
        assert!(parse("<trace-query-result></trace-query-result>").is_err());
        assert!(parse("not xml").is_err());
    }
}
