//! Parsing `dumpsys meminfo <package>`.

use emulabzip_core::{Error, MemoryCategory, MemoryReport, Result};

/// Reads the *App Summary* section (PSS per category), the totals, and the *Objects* section.
///
/// Since Android 10 the summary has a `Pss(KB)` and an `Rss(KB)` column, and some rows only fill
/// one of them, so a value is assigned to a column by its position under the headers.
pub fn parse(app: &str, text: &str) -> Result<MemoryReport> {
    if text.contains("No process found") {
        return Err(Error::NotFound(format!("{app} is not running")));
    }
    let pid = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("** MEMINFO in pid ")?.split_whitespace().next()?.parse().ok())
        .ok_or_else(|| Error::parse("meminfo", "no MEMINFO header"))?;

    let mut categories = Vec::new();
    let mut details = Vec::new();
    let mut total_pss = None;
    let mut section = "";
    let mut rss_col: Option<usize> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        match trimmed {
            "App Summary" | "Objects" | "SQL" | "DATABASES" | "Native Allocations" => {
                section = trimmed;
                continue;
            }
            _ => {}
        }
        match section {
            "App Summary" => {
                if line.contains("Pss(KB)") {
                    rss_col = line.find("Rss(KB)");
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("TOTAL PSS:") {
                    total_pss = rest.split_whitespace().next().and_then(|v| v.parse().ok());
                    if let Some(rss) = trimmed.split("TOTAL RSS:").nth(1).and_then(|r| r.split_whitespace().next()) {
                        details.push(("Total RSS".to_string(), format!("{rss} KB")));
                    }
                    if let Some(swap) =
                        trimmed.split("TOTAL SWAP PSS:").nth(1).and_then(|r| r.split_whitespace().next())
                    {
                        details.push(("Swap PSS".to_string(), format!("{swap} KB")));
                    }
                    continue;
                }
                let Some((name, _)) = line.split_once(':') else { continue };
                let name = name.trim();
                if name.is_empty() || name.starts_with('-') {
                    continue;
                }
                if name == "TOTAL" {
                    // Older layout: `TOTAL:  3000  TOTAL SWAP PSS:  0`.
                    total_pss = trimmed["TOTAL:".len()..].split_whitespace().next().and_then(|v| v.parse().ok());
                    continue;
                }
                // The first number is PSS unless it sits under the RSS header.
                let after = line.find(':').map_or(0, |i| i + 1);
                let mut offset = after;
                for token in line[after..].split_whitespace() {
                    let start = line[offset..].find(token).map_or(offset, |i| offset + i);
                    offset = start + token.len();
                    let Ok(kb) = token.parse::<u64>() else { continue };
                    let under_rss = rss_col.is_some_and(|c| start + token.len() > c);
                    if !under_rss {
                        categories.push(MemoryCategory { name: name.to_string(), kb });
                    }
                    break;
                }
            }
            "Objects" => {
                // Two `Name: value` pairs per line.
                let mut rest = trimmed;
                while let Some((name, tail)) = rest.split_once(':') {
                    let tail = tail.trim_start();
                    let value: String = tail.chars().take_while(char::is_ascii_digit).collect();
                    if value.is_empty() {
                        break;
                    }
                    details.push((name.trim().to_string(), value.clone()));
                    rest = tail[value.len()..].trim_start();
                }
            }
            _ => {}
        }
    }

    let total_kb = total_pss.unwrap_or_else(|| categories.iter().map(|c| c.kb).sum());
    Ok(MemoryReport { app: app.to_string(), pid, metric: "PSS".to_string(), total_kb, categories, details }.normalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/meminfo-app.txt");

    #[test]
    fn parses_the_app_summary() {
        let r = parse("com.android.settings", FIXTURE).unwrap();
        assert_eq!(r.pid, 1158);
        assert_eq!(r.metric, "PSS");
        assert!(r.total_kb > 10_000, "total {}", r.total_kb);
        let names: Vec<&str> = r.categories.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Java Heap"));
        assert!(names.contains(&"Native Heap"));
        assert!(names.contains(&"Code"));
        // `Unknown` only has an RSS value on this device and must not be read as PSS.
        assert!(!names.contains(&"Unknown"), "{names:?}");
        assert!(r.categories.windows(2).all(|w| w[0].kb >= w[1].kb));
    }

    #[test]
    fn reads_totals_and_objects() {
        let r = parse("com.android.settings", FIXTURE).unwrap();
        let get = |k: &str| r.details.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert!(get("Total RSS").is_some());
        assert!(get("Views").is_some());
        assert_eq!(get("Activities"), Some("1"));
    }

    #[test]
    fn handles_the_single_column_layout() {
        let text = "** MEMINFO in pid 42 [com.old] **\n App Summary\n                       Pss(KB)\n                        ------\n           Java Heap:     1000\n         Native Heap:     2000\n           TOTAL:     3000       TOTAL SWAP PSS:       0\n";
        let r = parse("com.old", text).unwrap();
        assert_eq!(r.categories.len(), 2);
        assert_eq!(r.categories[0].name, "Native Heap");
        assert_eq!(r.total_kb, 3000);
    }

    #[test]
    fn reports_a_missing_process() {
        let err = parse("com.example", "No process found for: com.example\n").unwrap_err();
        assert_eq!(err.to_string(), "com.example is not running");
    }
}
