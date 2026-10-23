//! Memory reports with `footprint` and heap snapshots with `leaks --outputGraph`.

use emulabzip_core::{Error, HeapDump, MemoryCategory, MemoryReport, Result};
use serde_json::Value;
use tokio::process::Command;

use crate::perf::app_pid;

/// Builds a report from `footprint --json` output. Category sizes are dirty memory, which is
/// what counts toward the footprint and what makes iOS terminate an app.
pub fn parse_footprint(app: &str, json: &Value) -> Result<MemoryReport> {
    let process = json
        .get("processes")
        .and_then(Value::as_array)
        .and_then(|p| p.first())
        .ok_or_else(|| Error::parse("footprint output", "no process"))?;
    let pid = process.get("pid").and_then(Value::as_u64).unwrap_or(0) as u32;
    let footprint = process.get("footprint").and_then(Value::as_u64).unwrap_or(0);
    let mut categories = Vec::new();
    let (mut clean, mut reclaimable, mut regions) = (0u64, 0u64, 0u64);
    if let Some(map) = process.get("categories").and_then(Value::as_object) {
        for (name, c) in map {
            let field = |k: &str| c.get(k).and_then(Value::as_u64).unwrap_or(0);
            categories.push(MemoryCategory { name: name.clone(), kb: field("dirty") / 1024 });
            clean += field("clean");
            reclaimable += field("reclaimable");
            regions += field("regions");
        }
    }
    let details = vec![
        ("Clean".to_string(), format!("{} KB", clean / 1024)),
        ("Reclaimable".to_string(), format!("{} KB", reclaimable / 1024)),
        ("Regions".to_string(), regions.to_string()),
    ];
    Ok(MemoryReport {
        app: app.to_string(),
        pid,
        metric: "Footprint".to_string(),
        total_kb: footprint / 1024,
        categories,
        details,
    }
    .normalize())
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("emulabzip-{}-{name}", std::process::id()))
}

async fn running_pid(udid: &str, app: &str) -> Result<u32> {
    app_pid(udid, app).await.ok_or_else(|| Error::NotFound(format!("{app} is not running")))
}

pub async fn report(udid: &str, app: &str) -> Result<MemoryReport> {
    let pid = running_pid(udid, app).await?;
    let path = temp_path(&format!("{pid}.json"));
    let out = Command::new("footprint").arg("--json").arg(&path).arg(pid.to_string()).output().await;
    let json = tokio::fs::read(&path).await;
    let _ = tokio::fs::remove_file(&path).await;
    let out = out?;
    let json = json.map_err(|_| Error::Simctl(String::from_utf8_lossy(&out.stderr).trim().to_string()))?;
    let value: Value = serde_json::from_slice(&json).map_err(|e| Error::parse("footprint output", e.to_string()))?;
    parse_footprint(app, &value)
}

/// Writes a memory graph that Xcode's memory graph debugger and `leaks`, `heap` and
/// `vmmap` can open.
pub async fn memgraph(udid: &str, app: &str, stamp: &str) -> Result<HeapDump> {
    let pid = running_pid(udid, app).await?;
    let path = temp_path(&format!("{pid}.memgraph"));
    // `leaks` exits with 1 when it finds leaks, so judge success by the file.
    let out =
        Command::new("leaks").arg(pid.to_string()).arg(format!("--outputGraph={}", path.display())).output().await?;
    let bytes = tokio::fs::read(&path).await;
    let _ = tokio::fs::remove_file(&path).await;
    let bytes = bytes.map_err(|_| {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Error::Simctl(stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("leaks failed").trim().to_string())
    })?;
    Ok(HeapDump { file_name: format!("{app}-{stamp}.memgraph"), bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_footprint_json() {
        let json: Value = serde_json::from_str(include_str!("../tests/fixtures/footprint.json")).unwrap();
        let r = parse_footprint("com.apple.mobilecal", &json).unwrap();
        assert_eq!(r.pid, 5895);
        assert_eq!(r.metric, "Footprint");
        assert_eq!(r.total_kb, 28737536 / 1024);
        assert!(!r.categories.is_empty());
        assert!(r.categories.windows(2).all(|w| w[0].kb >= w[1].kb));
        assert!(r.details.iter().any(|(k, _)| k == "Regions"));
    }

    #[test]
    fn rejects_empty_output() {
        assert!(parse_footprint("x", &serde_json::json!({"processes": []})).is_err());
    }
}
