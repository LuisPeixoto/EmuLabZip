//! `emulabzip layout` and `emulabzip a11y`

use std::path::PathBuf;

use anyhow::{Context, Result};
use emulabzip_core::a11y::{Severity, audit};
use emulabzip_core::{Lab, UiNode};

use super::ci::Exit;
use crate::output;

fn describe(n: &UiNode) -> String {
    let name = n
        .label
        .as_deref()
        .or(n.text.as_deref())
        .map(|t| format!(" “{}”", t.chars().take(50).collect::<String>()))
        .unwrap_or_default();
    let id =
        n.resource_id.as_deref().map(|i| format!(" #{}", i.rsplit(['/', ':']).next().unwrap_or(i))).unwrap_or_default();
    let flags = [n.clickable.then_some("tap"), n.editable.then_some("edit"), (!n.enabled).then_some("off")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(",");
    let b = n.bounds;
    format!(
        "{}{name}{id}  {}",
        n.class.rsplit('.').next().unwrap_or(&n.class),
        output::dim(&format!(
            "{}×{} at {},{}{}",
            b.width,
            b.height,
            b.x,
            b.y,
            if flags.is_empty() { String::new() } else { format!("  [{flags}]") }
        ))
    )
}

fn print_tree(n: &UiNode, depth: usize, max: usize) {
    println!("{}{}", "  ".repeat(depth), describe(n));
    if depth < max {
        for c in &n.children {
            print_tree(c, depth + 1, max);
        }
    }
}

pub async fn layout(lab: &Lab, device: Option<&str>, json: bool, png: Option<PathBuf>, depth: usize) -> Result<()> {
    let device = lab.resolve(device).await?;
    let snapshot = lab.layout(&device.target()).await?;
    if let Some(path) = &png {
        tokio::fs::write(path, &snapshot.screenshot)
            .await
            .with_context(|| format!("could not write {}", path.display()))?;
        eprintln!("Saved the screenshot to {}", path.display());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        print_tree(&snapshot.root, 0, depth);
        eprintln!(
            "{}",
            output::dim(&format!(
                "{} views on a {}×{} screen",
                snapshot.root.count(),
                snapshot.screen.width,
                snapshot.screen.height
            ))
        );
    }
    Ok(())
}

pub async fn a11y(lab: &Lab, device: Option<&str>, json: bool, fail_on: Option<Severity>) -> Result<()> {
    let device = lab.resolve(device).await?;
    let snapshot = lab.layout(&device.target()).await?;
    let result = audit(&snapshot);
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        for f in &result.findings {
            let tag = match f.severity {
                Severity::Error => "error  ",
                Severity::Warning => "warning",
                Severity::Info => "note   ",
            };
            println!("{tag} {:<20} {}", f.rule, f.message);
            println!(
                "        {}",
                output::dim(&format!(
                    "{} at {},{} {}×{} — {}",
                    f.class.rsplit('.').next().unwrap_or(&f.class),
                    f.bounds.x,
                    f.bounds.y,
                    f.bounds.width,
                    f.bounds.height,
                    f.fix
                ))
            );
        }
        if result.findings.is_empty() {
            println!("Nothing found on this screen.");
        }
        eprintln!(
            "{}",
            output::dim(&format!(
                "{} views checked: {} errors, {} warnings, {} notes{}",
                result.checked,
                result.count(Severity::Error),
                result.count(Severity::Warning),
                result.count(Severity::Info),
                if result.contrast_skipped { " (contrast not checked)" } else { "" }
            ))
        );
    }
    match fail_on {
        Some(level) if result.fails(level) => Err(Exit(1).into()),
        _ => Ok(()),
    }
}
