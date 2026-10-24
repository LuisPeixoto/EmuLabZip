//! `emulabzip crashes` and `emulabzip crash`

use anyhow::Result;
use emulabzip_core::{CrashKind, CrashReport, CrashSummary, FrameOrigin, Lab};

use crate::output;

/// `2026-10-23 23:59:28 UTC` from milliseconds since the epoch.
pub fn utc(ms: Option<u64>) -> String {
    let Some(ms) = ms else { return "unknown time".into() };
    let secs = (ms / 1000) as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from a day count (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC", rest / 3600, rest % 3600 / 60, rest % 60)
}

fn kind_label(kind: CrashKind) -> &'static str {
    match kind {
        CrashKind::Crash => "crash",
        CrashKind::NativeCrash => "native",
        CrashKind::Anr => "anr",
    }
}

pub fn parse_kind(text: &str) -> Result<CrashKind, String> {
    match text.to_ascii_lowercase().as_str() {
        "crash" => Ok(CrashKind::Crash),
        "native" | "native-crash" => Ok(CrashKind::NativeCrash),
        "anr" => Ok(CrashKind::Anr),
        other => Err(format!("`{other}` is not a kind: use crash, native or anr")),
    }
}

pub async fn list(
    lab: &Lab,
    device: Option<&str>,
    app: Option<&str>,
    kind: Option<CrashKind>,
    json: bool,
) -> Result<()> {
    let device = lab.resolve(device).await?;
    let all: Vec<CrashSummary> =
        lab.crashes(&device.target(), app).await?.into_iter().filter(|c| kind.is_none_or(|k| c.kind == k)).collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&all)?);
        return Ok(());
    }
    if all.is_empty() {
        println!("{}", output::dim("No crashes recorded on this device."));
        return Ok(());
    }
    println!("{}", output::heading(&format!("{:<22} {:<7} {:<32} TITLE", "TIME", "KIND", "APP")));
    for c in &all {
        let title: String = c.title.chars().take(70).collect();
        println!(
            "{:<22} {:<7} {:<32} {title}",
            utc(c.time_ms),
            kind_label(c.kind),
            c.app.as_deref().unwrap_or(&c.process)
        );
    }
    println!(
        "{}",
        output::dim(&format!("\n{} ids work with `emulabzip crash <id>`, for example {}", all.len(), all[0].id))
    );
    Ok(())
}

fn origin_mark(origin: FrameOrigin) -> &'static str {
    match origin {
        FrameOrigin::App => "●",
        FrameOrigin::Framework => "○",
        FrameOrigin::Native => "·",
        FrameOrigin::Kernel => "▪",
    }
}

fn print_report(r: &CrashReport, all_threads: bool) {
    let s = &r.summary;
    println!("{}", output::heading(&s.title));
    println!(
        "{}",
        output::dim(&format!(
            "{} · {}{} · {}",
            kind_label(s.kind),
            s.app.as_deref().unwrap_or(&s.process),
            s.pid.map(|p| format!(" · pid {p}")).unwrap_or_default(),
            utc(s.time_ms)
        ))
    );
    if !r.details.is_empty() {
        println!();
        for (k, v) in &r.details {
            println!("  {} {v}", output::dim(&format!("{k:<16}")));
        }
    }
    for t in r.threads.iter().filter(|t| all_threads || t.crashed) {
        println!("\n{}", output::heading(&format!("{}{}", t.name, if t.crashed { "  (crashed)" } else { "" })));
        for f in &t.frames {
            let place = match (&f.image, &f.location) {
                (Some(i), Some(l)) => format!("{i} {l}"),
                (Some(i), None) => i.clone(),
                (None, Some(l)) => l.clone(),
                (None, None) => String::new(),
            };
            let symbol = if f.symbol.is_empty() { "—" } else { &f.symbol };
            println!("  {:>3} {} {symbol}  {}", f.index, origin_mark(f.origin), output::dim(&place));
        }
    }
    let hidden = r.threads.iter().filter(|t| !t.crashed).count();
    if !all_threads && hidden > 0 {
        println!("{}", output::dim(&format!("\n{hidden} other threads. Use --all-threads to see them.")));
    }
    if !r.hints.is_empty() {
        println!("\n{}", output::heading("What to try"));
        for h in &r.hints {
            println!("  {}", h.text);
            if let Some(c) = &h.command {
                println!("    {}", output::dim(&format!("$ {c}")));
            }
        }
    }
    println!("{}", output::dim("\n● app  ○ framework  · native  ▪ kernel"));
}

pub async fn show(lab: &Lab, device: Option<&str>, id: &str, raw: bool, json: bool, all_threads: bool) -> Result<()> {
    let device = lab.resolve(device).await?;
    let report = lab.crash(&device.target(), id).await?;
    if raw {
        println!("{}", report.raw);
    } else if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report, all_threads);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_times() {
        assert_eq!(utc(Some(0)), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(Some(1_792_799_968_000)), "2026-10-23 23:59:28 UTC");
        assert_eq!(utc(Some(951_782_400_000)), "2000-02-29 00:00:00 UTC");
        assert_eq!(utc(None), "unknown time");
    }

    #[test]
    fn parses_kinds() {
        assert_eq!(parse_kind("ANR"), Ok(CrashKind::Anr));
        assert_eq!(parse_kind("native"), Ok(CrashKind::NativeCrash));
        assert!(parse_kind("boom").is_err());
    }
}
