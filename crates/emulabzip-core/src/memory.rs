//! Memory reports, snapshots and heap dumps.

use serde::{Deserialize, Serialize};

/// One slice of an app's memory, for example `Java Heap` or `MALLOC_SMALL`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCategory {
    pub name: String,
    pub kb: u64,
}

/// A breakdown of an app's memory at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReport {
    pub app: String,
    pub pid: u32,
    /// What `total_kb` measures: `PSS` on Android, `Footprint` on iOS.
    pub metric: String,
    pub total_kb: u64,
    /// Largest first.
    pub categories: Vec<MemoryCategory>,
    /// Extra facts, such as object counts or the resident size.
    pub details: Vec<(String, String)>,
}

impl MemoryReport {
    /// Sorts categories by size and drops empty ones.
    pub fn normalize(mut self) -> Self {
        self.categories.retain(|c| c.kb > 0);
        self.categories.sort_by(|a, b| b.kb.cmp(&a.kb).then_with(|| a.name.cmp(&b.name)));
        self
    }
}

/// How a category changed between two reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDelta {
    pub name: String,
    pub before_kb: u64,
    pub after_kb: u64,
    pub delta_kb: i64,
}

/// Compares two reports, largest change first. Categories that exist in only one report count
/// as zero in the other.
pub fn diff(before: &MemoryReport, after: &MemoryReport) -> Vec<CategoryDelta> {
    let mut names: Vec<&str> = before.categories.iter().chain(&after.categories).map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let kb = |r: &MemoryReport, n: &str| r.categories.iter().find(|c| c.name == n).map_or(0, |c| c.kb);
    let mut out: Vec<CategoryDelta> = names
        .into_iter()
        .map(|n| {
            let (b, a) = (kb(before, n), kb(after, n));
            CategoryDelta { name: n.to_string(), before_kb: b, after_kb: a, delta_kb: a as i64 - b as i64 }
        })
        .filter(|d| d.delta_kb != 0)
        .collect();
    out.sort_by(|x, y| y.delta_kb.abs().cmp(&x.delta_kb.abs()).then_with(|| x.name.cmp(&y.name)));
    out
}

/// A heap snapshot file produced on the device or the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapDump {
    /// Suggested file name, for example `com.example.app-20261023-131500.hprof`.
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(cats: &[(&str, u64)]) -> MemoryReport {
        MemoryReport {
            app: "com.example".into(),
            pid: 1,
            metric: "PSS".into(),
            total_kb: cats.iter().map(|c| c.1).sum(),
            categories: cats.iter().map(|(n, kb)| MemoryCategory { name: n.to_string(), kb: *kb }).collect(),
            details: vec![],
        }
    }

    #[test]
    fn normalizes_categories() {
        let r = report(&[("Stack", 448), ("Graphics", 0), ("Code", 17708)]).normalize();
        assert_eq!(r.categories.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Code", "Stack"]);
    }

    #[test]
    fn diffs_reports_largest_change_first() {
        let before = report(&[("Java Heap", 8000), ("Native Heap", 10000), ("Code", 500)]);
        let after = report(&[("Java Heap", 12000), ("Native Heap", 9000), ("Graphics", 2000), ("Code", 500)]);
        let d = diff(&before, &after);
        assert_eq!(d.len(), 3);
        assert_eq!((d[0].name.as_str(), d[0].delta_kb), ("Java Heap", 4000));
        assert_eq!((d[1].name.as_str(), d[1].delta_kb), ("Graphics", 2000));
        assert_eq!((d[2].name.as_str(), d[2].delta_kb), ("Native Heap", -1000));
    }
}
