//! `emulabzip mappings` and `emulabzip symbolicate`: the files that make a crash readable.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use emulabzip_symbols::store::{Kind, Store};
use emulabzip_symbols::symbolicate::{Source, symbolicate_text};

use crate::output;

pub fn parse_kind(text: &str) -> Result<Kind, String> {
    Kind::parse(text).ok_or_else(|| format!("`{text}` is not a kind: use mapping, sourcemap or dsym"))
}

pub fn add(app: &str, version: &str, file: &Path, kind: Option<Kind>) -> Result<()> {
    let stored = Store::default().add(app, version, file, kind)?;
    println!(
        "Stored {} for {} {} ({}).",
        stored.kind.label(),
        stored.app,
        stored.version,
        output::size(stored.size_bytes)
    );
    println!("{}", output::dim("Crashes of that app and version are read with it from now on."));
    Ok(())
}

pub fn list(app: Option<&str>, json: bool) -> Result<()> {
    let all = Store::default().list(app);
    if json {
        println!("{}", serde_json::to_string_pretty(&all)?);
        return Ok(());
    }
    if all.is_empty() {
        println!(
            "{}",
            output::dim("Nothing stored. Add a mapping.txt, a source map or a .dSYM with `emulabzip mappings add`.")
        );
        return Ok(());
    }
    let rows: Vec<Vec<(String, String)>> = all
        .iter()
        .map(|s| {
            vec![
                output::cell(&s.app),
                output::cell(&s.version),
                output::cell(s.kind.label()),
                output::cell(output::size(s.size_bytes)),
            ]
        })
        .collect();
    println!("{}", output::table(&["APP", "VERSION", "KIND", "SIZE"], &rows));
    Ok(())
}

pub fn remove(app: &str, version: &str, kind: Option<Kind>) -> Result<()> {
    let n = Store::default().remove(app, version, kind)?;
    if n == 0 {
        bail!("nothing is stored for {app} {version}");
    }
    println!("Removed {n} stored {}.", if n == 1 { "file" } else { "files" });
    Ok(())
}

/// Reads a pasted stack (a file, or `-` for standard input) with the named files.
pub fn symbolicate(stack: &Path, files: &[PathBuf]) -> Result<()> {
    if files.is_empty() {
        bail!("name a mapping.txt or a source map with --mapping");
    }
    let sources = files.iter().map(|f| Source::detect(f)).collect::<Result<Vec<_>, _>>()?;
    let mut text = String::new();
    if stack == Path::new("-") {
        std::io::stdin().read_to_string(&mut text)?;
    } else {
        text =
            std::fs::read_to_string(stack).map_err(|e| anyhow::anyhow!("could not read {}: {e}", stack.display()))?;
    }
    match symbolicate_text(&text, &sources) {
        Ok(out) => {
            print!("{out}");
            if !out.ends_with('\n') {
                println!();
            }
            Ok(())
        }
        Err(e) => bail!("{e}"),
    }
}
