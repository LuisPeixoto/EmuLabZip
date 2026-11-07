//! `emulabzip i18n`: text that does not fit, on the screen as it is and across languages and text sizes.

use std::time::Duration;

use anyhow::{Result, bail};
use emulabzip_audit::sweep::{self, LabScreen, Plan};
use emulabzip_core::control::{is_app_id, is_locale_tag};
use emulabzip_core::i18n::{self, Options};
use emulabzip_core::{Lab, Severity};

use super::ci::Exit;
use super::layout::print_findings;
use crate::output::{self, cell};

const MAX_SETTINGS: usize = 6;

pub async fn check(lab: &Lab, device: Option<&str>, rtl: bool, json: bool, fail_on: Option<Severity>) -> Result<()> {
    let device = lab.resolve(device).await?;
    let snapshot = lab.layout(&device.target()).await?;
    let result = i18n::check(&snapshot, Options { rtl });
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_findings(&result.findings);
        if result.findings.is_empty() {
            println!("No text that looks cut off on this screen.");
        }
        eprintln!(
            "{}",
            output::dim(&format!(
                "{} views checked: {} warnings{}",
                result.checked,
                result.count(Severity::Warning),
                if result.contrast_skipped { " (no screenshot, so cut-off text was not looked for)" } else { "" }
            ))
        );
    }
    match fail_on {
        Some(level) if result.fails(level) => Err(Exit(1).into()),
        _ => Ok(()),
    }
}

pub struct SweepArgs {
    pub locales: Vec<String>,
    pub scales: Vec<u32>,
    pub app: Option<String>,
    pub settle_ms: u64,
    pub json: bool,
    pub fail_on_worse: bool,
}

/// What can be refused without a device is refused first.
pub fn check_sweep_args(a: &SweepArgs) -> Result<()> {
    for l in &a.locales {
        if !is_locale_tag(l) {
            bail!("`{l}` is not a language tag such as de-DE, en-XA or ar-XB");
        }
    }
    for &s in &a.scales {
        if !(50..=300).contains(&s) {
            bail!("a text size is between 50 and 300 percent, not {s}");
        }
    }
    if a.locales.len() > MAX_SETTINGS || a.scales.len() > MAX_SETTINGS {
        bail!("try at most {MAX_SETTINGS} languages and {MAX_SETTINGS} text sizes at a time");
    }
    if let Some(app) = &a.app
        && !is_app_id(app)
    {
        bail!("`{app}` is not a package name or bundle id");
    }
    if a.settle_ms > 10_000 {
        bail!("wait at most 10,000 ms for the screen to settle");
    }
    Ok(())
}

pub async fn sweep(lab: &Lab, device: Option<&str>, args: SweepArgs) -> Result<()> {
    check_sweep_args(&args)?;
    let device = lab.resolve(device).await?;
    let target = device.target();
    let plan = Plan {
        locales: args.locales.clone(),
        scales: args.scales.clone(),
        app: args.app.clone(),
        settle: Duration::from_millis(args.settle_ms),
    };
    let result = sweep::run(&LabScreen { lab, target: &target }, &plan).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let rows: Vec<_> = result
            .rows
            .iter()
            .map(|r| {
                let counts: Vec<String> = r.by_rule.iter().map(|(k, n)| format!("{k} {n}")).collect();
                vec![
                    cell(r.locale.clone().unwrap_or_else(|| "device".into())),
                    cell(format!("{}%", r.percent)),
                    cell(r.findings.len().to_string()),
                    cell(if counts.is_empty() { "-".to_string() } else { counts.join(", ") }),
                    cell(if r.worse.is_empty() {
                        String::new()
                    } else {
                        r.worse.iter().map(|(k, n)| format!("+{n} {k}")).collect::<Vec<_>>().join(", ")
                    }),
                ]
            })
            .collect();
        print!("{}", output::table(&["LANGUAGE", "TEXT", "FOUND", "WHAT", "WORSE THAN NORMAL"], &rows));
        for r in result.rows.iter().filter(|r| !r.worse.is_empty()) {
            println!("\n{} at {}%:", r.locale.as_deref().unwrap_or("device"), r.percent);
            let worse: Vec<&str> = r.worse.iter().map(|(k, _)| k.as_str()).collect();
            let list: Vec<_> = r.findings.iter().filter(|f| worse.contains(&f.rule.as_str())).cloned().collect();
            print_findings(&list);
        }
    }
    for n in &result.notes {
        eprintln!("{}", output::dim(n));
    }
    if args.fail_on_worse && result.rows.iter().any(|r| !r.worse.is_empty()) {
        return Err(Exit(1).into());
    }
    Ok(())
}
