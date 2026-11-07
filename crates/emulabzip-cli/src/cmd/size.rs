//! `emulabzip size`: where the bytes of an APK, an AAB or an IPA are, and what changed between two builds.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use emulabzip_audit::package::{self, Diff, Limits, Package};
use emulabzip_session::Config;
use emulabzip_session::config::SizeConfig;

use super::ci::Exit;
use crate::output::{self, cell};

/// The limits of the `[size]` section.
pub fn limits(c: &SizeConfig) -> Limits {
    Limits {
        total_mb: c.max_total_mb,
        dex_mb: c.max_dex_mb,
        native_mb: c.max_native_mb,
        resources_mb: c.max_resources_mb,
        methods: c.max_methods,
        growth_percent: c.max_growth_percent,
    }
}

fn mb(bytes: u64) -> String {
    output::size(bytes)
}

fn load_config(config: Option<&Path>) -> Result<Config> {
    match config {
        Some(path) => Ok(Config::load(path)?),
        None => match Config::find(&std::env::current_dir()?) {
            Some(path) => Ok(Config::load(&path)?),
            None => bail!("there is no emulabzip.toml with a [size] section here; pass --config <file>"),
        },
    }
}

pub fn show(
    file: &Path,
    json: bool,
    top: usize,
    budget: bool,
    config: Option<PathBuf>,
    baseline: Option<PathBuf>,
) -> Result<()> {
    // The limits are read before the build, so a bad config is said first.
    let size_limits = if budget {
        let cfg = load_config(config.as_deref())?;
        let Some(s) = cfg.size else { bail!("emulabzip.toml has no [size] section") };
        Some(limits(&s))
    } else {
        None
    };
    let pkg = package::analyze_file(file)?;
    let base: Option<Package> = baseline.as_deref().map(package::analyze_file).transpose()?;
    let breaches = size_limits.as_ref().map(|l| package::check(&pkg, l, base.as_ref()));

    if json {
        let mut value = serde_json::to_value(&pkg)?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("breaches".into(), serde_json::to_value(&breaches)?);
            obj.insert("methods".into(), pkg.methods().into());
        }
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        print_package(file, &pkg, top);
        if let Some(b) = &breaches {
            if b.is_empty() {
                println!("\nWithin every [size] limit.");
            } else {
                println!("\n{}", output::heading("Over the limits"));
                for x in b {
                    println!("  {}: allowed {}, was {}", x.what, x.limit, x.actual);
                }
            }
        }
    }
    match breaches {
        Some(b) if !b.is_empty() => Err(Exit(1).into()),
        _ => Ok(()),
    }
}

fn print_package(file: &Path, p: &Package, top: usize) {
    println!(
        "{} {} · {} download · {} installed · {} files",
        output::heading(
            &file.file_name().map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned())
        ),
        p.format.label(),
        mb(p.total.compressed),
        mb(p.total.uncompressed),
        p.total.files
    );
    let kinds: Vec<_> = p
        .kinds
        .iter()
        .map(|(k, s)| {
            vec![
                cell(k.label()),
                cell(mb(s.compressed)),
                cell(mb(s.uncompressed)),
                cell(s.files.to_string()),
                cell(pct(s.compressed, p.total.compressed)),
            ]
        })
        .collect();
    print!("{}", output::table(&["KIND", "DOWNLOAD", "INSTALLED", "FILES", "SHARE"], &kinds));
    if !p.native.is_empty() {
        println!("\n{}", output::heading("Native code"));
        let rows: Vec<_> = p
            .native
            .iter()
            .map(|(k, s)| vec![cell(k.clone()), cell(mb(s.compressed)), cell(s.files.to_string())])
            .collect();
        print!("{}", output::table(&["WHERE", "DOWNLOAD", "FILES"], &rows));
    }
    if p.modules.len() > 1 {
        println!("\n{}", output::heading("Modules"));
        let rows: Vec<_> = p
            .modules
            .iter()
            .map(|(k, s)| vec![cell(k.clone()), cell(mb(s.compressed)), cell(s.files.to_string())])
            .collect();
        print!("{}", output::table(&["MODULE", "DOWNLOAD", "FILES"], &rows));
    }
    if !p.dex.is_empty() {
        println!("\n{}", output::heading("Dex"));
        let rows: Vec<_> = p
            .dex
            .iter()
            .map(|d| {
                vec![cell(d.path.clone()), cell(mb(d.bytes)), cell(d.classes.to_string()), cell(d.methods.to_string())]
            })
            .collect();
        print!("{}", output::table(&["FILE", "SIZE", "CLASSES", "METHODS"], &rows));
        println!("{}", output::dim(&format!("{} methods in all; one dex file holds at most 65,536.", p.methods())));
    }
    println!("\n{}", output::heading("Biggest files"));
    let rows: Vec<_> = p
        .largest(top)
        .iter()
        .map(|e| vec![cell(e.path.clone()), cell(e.kind.label()), cell(mb(e.compressed)), cell(mb(e.uncompressed))])
        .collect();
    print!("{}", output::table(&["FILE", "KIND", "DOWNLOAD", "INSTALLED"], &rows));
}

fn pct(part: u64, whole: u64) -> String {
    if whole == 0 { "0%".into() } else { format!("{:.1}%", part as f64 * 100.0 / whole as f64) }
}

fn signed(delta: i64) -> String {
    let sign = if delta > 0 {
        "+"
    } else if delta < 0 {
        "−"
    } else {
        ""
    };
    format!("{sign}{}", mb(delta.unsigned_abs()))
}

pub fn diff(a: &Path, b: &Path, json: bool, top: usize, fail_on_growth: Option<f64>) -> Result<()> {
    let (pa, pb) = (package::analyze_file(a)?, package::analyze_file(b)?);
    let d: Diff = package::diff(&pa, &pb);
    if json {
        println!("{}", serde_json::to_string_pretty(&d)?);
    } else {
        println!(
            "{} → {}: {} → {} ({}, {:+.1}%)",
            a.display(),
            b.display(),
            mb(d.before),
            mb(d.after),
            signed(d.delta),
            d.percent
        );
        if d.methods_before != d.methods_after {
            println!("methods: {} → {}", d.methods_before, d.methods_after);
        }
        if !d.kinds.is_empty() {
            println!("\n{}", output::heading("By kind"));
            for (k, delta) in &d.kinds {
                println!("  {:<10} {}", k.label(), signed(*delta));
            }
        }
        if !d.folders.is_empty() {
            println!("\n{}", output::heading("By folder"));
            for (f, delta) in d.folders.iter().take(top) {
                println!("  {:<24} {}", f, signed(*delta));
            }
        }
        println!("\n{}", output::heading(&format!("Biggest changes ({} added, {} removed)", d.added, d.removed)));
        for f in d.files.iter().take(top) {
            let what = match (f.before, f.after) {
                (None, _) => "added",
                (_, None) => "removed",
                _ => "changed",
            };
            println!("  {:>10}  {what:<8} {}", signed(f.delta), f.path);
        }
    }
    match fail_on_growth {
        Some(limit) if d.percent > limit => {
            eprintln!("error: the build grew {:.1}%, more than the {limit}% allowed.", d.percent);
            Err(Exit(1).into())
        }
        _ => Ok(()),
    }
}
