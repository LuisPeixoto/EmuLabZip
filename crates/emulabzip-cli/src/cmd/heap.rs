//! `emulabzip heap analyze`

use std::path::Path;

use anyhow::{Context, Result};
use emulabzip_heap::{Analysis, Confidence, Options, Step};

use crate::output;

fn bytes(n: u64) -> String {
    output::size(n)
}

fn path_text(root: &str, steps: &[Step]) -> String {
    let steps: Vec<String> = steps
        .iter()
        .map(|s| match &s.field {
            Some(f) if f == "[]" => format!("{}[]", s.class),
            Some(f) => format!("{}.{f}", s.class),
            None => s.class.clone(),
        })
        .collect();
    format!("{}: {}", output::dim(root), steps.join(" → "))
}

pub fn print(a: &Analysis, size: u64) {
    println!(
        "{} MB, {} objects ({} reachable){}{}",
        size / (1024 * 1024),
        a.summary.objects,
        a.summary.reachable_objects,
        if a.summary.heaps.is_empty() { String::new() } else { format!(", heaps: {}", a.summary.heaps.join(", ")) },
        if a.summary.truncated { ", cut short" } else { "" }
    );
    println!("\n{}", output::heading(&format!("Suspected leaks ({})", a.leaks.len())));
    if a.leaks.is_empty() {
        println!("  None: no destroyed Activity or removed Fragment is still alive.");
    }
    for l in &a.leaks {
        let sure = if l.confidence == Confidence::Certain { "leak" } else { "maybe" };
        println!(
            "  {sure}  {} {}  {}",
            l.class,
            output::dim(&l.id),
            output::dim(&format!("keeps {} alive", bytes(l.retained_bytes)))
        );
        println!("        {}", l.reason);
        println!("        {}", path_text(&l.root, &l.path));
    }
    println!("\n{}", output::heading("Objects that keep the most alive"));
    for d in a.dominators.iter().take(10) {
        println!("  {:>9}  {} {}", bytes(d.retained_bytes), d.class, output::dim(&d.id));
    }
    println!("\n{}", output::heading("Classes by size"));
    for c in a.classes.iter().take(10) {
        println!("  {:>9}  {:>8}×  {}", bytes(c.shallow_bytes), c.count, c.name);
    }
}

#[cfg(target_os = "macos")]
pub async fn print_memgraph(path: &Path) -> Result<()> {
    let r = emulabzip_apple::leaks::of_file(path).await?;
    println!("{}: {} leaks, {} leaked, of {} objects", r.process, r.leaks, bytes(r.leaked_bytes), r.malloced_nodes);
    for g in &r.groups {
        println!("\n  {} of {} ({}, {}) at {}", g.kind, g.class, g.count, g.size, g.address);
        for line in g.detail.iter().take(8) {
            println!("    {}", output::dim(line));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub async fn print_memgraph(_: &Path) -> Result<()> {
    anyhow::bail!("a .memgraph is read with the macOS `leaks` tool, so it needs a Mac")
}

pub async fn analyze(file: &Path, json: bool, top: usize, all_heaps: bool) -> Result<()> {
    let is_memgraph = file.extension().is_some_and(|e| e == "memgraph");
    if is_memgraph {
        if json {
            #[cfg(target_os = "macos")]
            {
                println!("{}", serde_json::to_string_pretty(&emulabzip_apple::leaks::of_file(file).await?)?);
                return Ok(());
            }
            #[cfg(not(target_os = "macos"))]
            anyhow::bail!("a .memgraph is read with the macOS `leaks` tool, so it needs a Mac");
        }
        return print_memgraph(file).await;
    }
    let data = std::fs::read(file).with_context(|| format!("could not read {}", file.display()))?;
    let size = data.len() as u64;
    let options = Options { top: top.clamp(1, 200), all_heaps };
    let analysis = tokio::task::spawn_blocking(move || emulabzip_heap::analyze(&data, &options)).await??;
    if json {
        println!("{}", serde_json::to_string_pretty(&analysis)?);
    } else {
        print(&analysis, size);
    }
    Ok(())
}
