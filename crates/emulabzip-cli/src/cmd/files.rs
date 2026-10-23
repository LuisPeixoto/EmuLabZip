//! `emulabzip files`, `emulabzip pull` and `emulabzip db`

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use emulabzip_core::{Device, Lab, StorageRoot};
use emulabzip_storage::sqlite::{Database, Rows};

use crate::output::{self, cell};

/// The requested root, or the first readable one.
async fn pick_root(lab: &Lab, device: &Device, app: &str, root: Option<&str>) -> Result<StorageRoot> {
    let roots = lab.storage_roots(&device.target(), app).await?;
    let found = match root {
        Some(id) => roots.iter().find(|r| r.id == id).cloned(),
        None => roots.iter().find(|r| r.unavailable.is_none()).cloned(),
    };
    match found {
        Some(r) if r.unavailable.is_none() => Ok(r),
        Some(r) => bail!("{}: {}", r.label, r.unavailable.unwrap_or_default()),
        None => {
            let ids: Vec<_> = roots.iter().map(|r| r.id.as_str()).collect();
            bail!("no readable storage root; available: {}", ids.join(", "))
        }
    }
}

pub async fn list(
    lab: &Lab,
    device: Option<&str>,
    app: &str,
    root: Option<&str>,
    path: &str,
    json: bool,
) -> Result<()> {
    let device = lab.resolve(device).await?;
    let root = pick_root(lab, &device, app, root).await?;
    let entries = lab.list_files(&device.target(), app, &root.id, path.trim_matches('/')).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }
    eprintln!("{}", output::dim(&format!("{} · {}/{}", root.label, root.location, path.trim_matches('/'))));
    let rows: Vec<_> = entries
        .iter()
        .map(|e| {
            vec![
                cell(if e.is_dir { format!("{}/", e.name) } else { e.name.clone() }),
                cell(if e.is_dir { String::new() } else { e.size.to_string() }),
            ]
        })
        .collect();
    print!("{}", output::table(&["NAME", "SIZE"], &rows));
    Ok(())
}

pub async fn pull(
    lab: &Lab,
    device: Option<&str>,
    app: &str,
    root: Option<&str>,
    path: &str,
    out: Option<PathBuf>,
) -> Result<()> {
    let device = lab.resolve(device).await?;
    let root = pick_root(lab, &device, app, root).await?;
    let bytes = lab.read_file(&device.target(), app, &root.id, path).await?;
    let dest = out.unwrap_or_else(|| PathBuf::from(path.rsplit('/').next().unwrap_or("file")));
    tokio::fs::write(&dest, &bytes).await.with_context(|| format!("could not write {}", dest.display()))?;
    println!("Saved {} ({} bytes)", dest.display(), bytes.len());
    Ok(())
}

fn print_rows(rows: &Rows) {
    let text = |v: &serde_json::Value| match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::String(s) => s.chars().take(60).collect(),
        other => other.to_string(),
    };
    let headers: Vec<&str> = rows.columns.iter().map(String::as_str).collect();
    let body: Vec<_> = rows.rows.iter().map(|r| r.iter().map(|v| cell(text(v))).collect()).collect();
    print!("{}", output::table(&headers, &body));
    if rows.truncated {
        eprintln!("{}", output::dim("… more rows; use --limit or --offset"));
    }
}

pub struct DbArgs {
    pub app: String,
    pub path: String,
    pub root: Option<String>,
    pub table: Option<String>,
    pub query: Option<String>,
    pub limit: u64,
    pub offset: u64,
    pub json: bool,
}

pub async fn db(lab: &Lab, device: Option<&str>, args: DbArgs) -> Result<()> {
    let device = lab.resolve(device).await?;
    let root = pick_root(lab, &device, &args.app, args.root.as_deref()).await?;
    let target = device.target();

    // Work on a private copy, together with its write-ahead log.
    let dir = std::env::temp_dir().join(format!("emulabzip-cli-{}", std::process::id()));
    tokio::fs::create_dir_all(&dir).await?;
    let name = args.path.rsplit('/').next().unwrap_or("database.db").to_string();
    let local = dir.join(&name);
    tokio::fs::write(&local, lab.read_file(&target, &args.app, &root.id, &args.path).await?).await?;
    for suffix in ["-wal", "-shm"] {
        if let Ok(bytes) = lab.read_file(&target, &args.app, &root.id, &format!("{}{suffix}", args.path)).await {
            tokio::fs::write(dir.join(format!("{name}{suffix}")), bytes).await?;
        }
    }
    let result = (|| -> Result<()> {
        let db = Database::open(&local)?;
        if let Some(sql) = &args.query {
            let rows = db.query(sql, args.limit)?;
            if args.json { println!("{}", serde_json::to_string_pretty(&rows)?) } else { print_rows(&rows) }
        } else if let Some(table) = &args.table {
            let rows = db.rows(table, args.offset, args.limit)?;
            if args.json { println!("{}", serde_json::to_string_pretty(&rows)?) } else { print_rows(&rows) }
        } else {
            let tables = db.tables()?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&tables)?);
            } else {
                let rows: Vec<_> = tables
                    .iter()
                    .map(|t| {
                        vec![
                            cell(t.name.clone()),
                            cell(t.kind.clone()),
                            cell(t.rows.to_string()),
                            cell(t.columns.len().to_string()),
                        ]
                    })
                    .collect();
                print!("{}", output::table(&["NAME", "TYPE", "ROWS", "COLUMNS"], &rows));
            }
        }
        Ok(())
    })();
    let _ = tokio::fs::remove_dir_all(&dir).await;
    result
}
