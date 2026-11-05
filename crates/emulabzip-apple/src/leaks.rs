//! Leaked objects in an iOS Simulator app, from the macOS `leaks` tool.
//!
//! The simulator's apps are ordinary processes on the Mac, so `leaks` can read them directly, or read
//! a `.memgraph` file written earlier. Its report lists groups of objects that nothing refers to:
//! retain cycles (`ROOT CYCLE`) and objects that were simply lost (`ROOT LEAK`).

use std::path::Path;

use emulabzip_core::{Error, LeakGroup, NativeLeaks, Result};
use tokio::process::Command;

/// Parses the text `leaks` prints.
pub fn parse(text: &str) -> Result<NativeLeaks> {
    let mut process = String::new();
    let mut summary = None;
    let mut nodes = 0;
    let mut groups: Vec<LeakGroup> = Vec::new();
    let mut in_groups = false;

    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Process:") {
            process = rest.trim().to_string();
        }
        // `Process 19344: 13672 nodes malloced for 1571 KB`
        if let Some((_, rest)) = t.strip_prefix("Process ").and_then(|r| r.split_once(": "))
            && let Some(n) = rest.split_once(" nodes malloced")
        {
            nodes = n.0.trim().parse().unwrap_or(0);
        }
        // `Process 19344: 16 leaks for 37120 total leaked bytes.`
        if let Some((_, rest)) = t.strip_prefix("Process ").and_then(|r| r.split_once(": "))
            && let Some((count, tail)) = rest.split_once(" leaks for ").or_else(|| rest.split_once(" leak for "))
            && let Some((bytes, _)) = tail.split_once(" total leaked bytes")
        {
            summary = Some((count.trim().parse().unwrap_or(0), bytes.trim().parse().unwrap_or(0)));
        }
        if t.contains("<< TOTAL >>") {
            in_groups = true;
            continue;
        }
        if !in_groups || t.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        // A group starts at the shallowest indent with ROOT CYCLE or ROOT LEAK; the lines under it are its detail.
        // (The same marker appears deeper in a tree, where it only names the next object of the cycle.)
        let top_level = indent <= 6;
        if let Some(group) = top_level.then(|| group_start(t)).flatten() {
            groups.push(group);
        } else if indent > 6
            && let Some(last) = groups.last_mut()
            && last.detail.len() < 40
        {
            last.detail.push(t.to_string());
        }
    }

    let (leaks, leaked_bytes) = summary.ok_or_else(|| Error::parse("leaks output", "there is no summary line"))?;
    Ok(NativeLeaks { process, leaks, leaked_bytes, malloced_nodes: nodes, groups })
}

/// `8 (18.1K) ROOT CYCLE: <CycleNode 0x6000002412a0> [32]`
fn group_start(line: &str) -> Option<LeakGroup> {
    let (kind, marker) = if line.contains(" ROOT CYCLE: ") {
        ("cycle", " ROOT CYCLE: ")
    } else if line.contains(" ROOT LEAK: ") {
        ("leak", " ROOT LEAK: ")
    } else {
        return None;
    };
    let (head, tail) = line.split_once(marker)?;
    let (count, size) = head.split_once(' ')?;
    let size = size.trim().trim_start_matches('(').trim_end_matches(')').to_string();
    let obj = tail.trim().strip_prefix('<')?.split_once('>')?.0;
    let (class, address) = obj.rsplit_once(' ')?;
    Some(LeakGroup {
        kind: kind.into(),
        class: class.to_string(),
        address: address.to_string(),
        count: count.parse().ok()?,
        size,
        detail: Vec::new(),
    })
}

async fn run(target: &str) -> Result<NativeLeaks> {
    // `leaks` exits with 1 when it finds leaks, so the output is what counts.
    let out = Command::new("leaks").arg(target).kill_on_drop(true).output().await?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse(&text).map_err(|e| {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let why = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("leaks printed no report");
        Error::Simctl(format!("{why} ({e})"))
    })
}

/// The leaks of a `.memgraph` file, such as the Memory view saves.
pub async fn of_file(path: &Path) -> Result<NativeLeaks> {
    if path.extension().is_none_or(|e| e != "memgraph") {
        return Err(Error::invalid("memory graph", path.display().to_string()));
    }
    run(&path.display().to_string()).await
}

/// The leaks of a running app, by process id.
pub async fn of_process(pid: u32) -> Result<NativeLeaks> {
    run(&pid.to_string()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../tests/fixtures/leaks-memgraph.txt");

    #[test]
    fn reads_a_real_report() {
        let r = parse(REAL).unwrap();
        assert!(r.process.starts_with("Crashy"));
        assert_eq!((r.leaks, r.leaked_bytes, r.malloced_nodes), (16, 37120, 13672));
        assert_eq!(r.groups.len(), 3);
        let g = &r.groups[0];
        assert_eq!((g.kind.as_str(), g.class.as_str(), g.count, g.size.as_str()), ("cycle", "CycleNode", 8, "18.1K"));
        assert_eq!(g.address, "0x6000002412a0");
        assert!(g.detail.iter().any(|d| d.contains("CYCLE BACK TO")), "{:?}", g.detail);
        assert!(g.detail.iter().any(|d| d.contains("payload")));
    }

    #[test]
    fn a_process_without_leaks_has_no_groups() {
        let text = "Process:         Calm [10]\n\nleaks Report Version: 4.0\nProcess 10: 500 nodes malloced for 40 KB\nProcess 10: 0 leaks for 0 total leaked bytes.\n";
        let r = parse(text).unwrap();
        assert_eq!((r.leaks, r.leaked_bytes, r.groups.len()), (0, 0, 0));
    }

    #[test]
    fn a_plain_lost_object_is_a_leak_not_a_cycle() {
        let text = "Process 7: 9 nodes malloced for 1 KB\nProcess 7: 1 leak for 48 total leaked bytes.\n\n    1 (48 bytes) << TOTAL >>\n\n      1 (48 bytes) ROOT LEAK: <Thing 0x600000001234> [48]\n";
        let r = parse(text).unwrap();
        assert_eq!(r.groups.len(), 1);
        assert_eq!(
            (r.groups[0].kind.as_str(), r.groups[0].class.as_str(), r.groups[0].size.as_str()),
            ("leak", "Thing", "48 bytes")
        );
    }

    #[test]
    fn text_that_is_not_a_report_is_an_error() {
        assert!(parse("Process 19344 is not debuggable.").is_err());
        assert!(parse("").is_err());
    }
}
