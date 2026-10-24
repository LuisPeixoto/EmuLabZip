//! `emulabzip compare`: what differs between two devices.

use anyhow::Result;
use emulabzip_core::{Lab, diff};

use crate::output;

pub async fn run(lab: &Lab, a: &str, b: &str, all: bool, json: bool) -> Result<()> {
    let (da, db) = (lab.resolve(Some(a)).await?, lab.resolve(Some(b)).await?);
    let (ta, tb) = (da.target(), db.target());
    let (ia, ib) = tokio::join!(lab.info(&ta), lab.info(&tb));
    let rows = diff(&ia?, &ib?);
    if json {
        let out = serde_json::json!({ "a": da.name, "b": db.name, "rows": rows });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    let shown: Vec<_> = rows.iter().filter(|r| all || !r.same).collect();
    let differing = rows.iter().filter(|r| !r.same).count();
    if shown.is_empty() {
        println!("{} and {} report the same facts.", da.name, db.name);
        return Ok(());
    }
    let table: Vec<Vec<(String, String)>> = shown
        .iter()
        .map(|r| {
            let (va, vb) = (r.a.clone().unwrap_or_else(|| "—".into()), r.b.clone().unwrap_or_else(|| "—".into()));
            let mark = if r.same { output::cell("") } else { output::cell("≠") };
            vec![output::cell(r.section.clone()), output::cell(r.key.clone()), output::cell(va), output::cell(vb), mark]
        })
        .collect();
    println!("{}", output::table(&["Section", "Fact", &da.name, &db.name, ""], &table));
    eprintln!(
        "{}",
        output::dim(&format!(
            "{differing} of {} facts differ{}",
            rows.len(),
            if all { "" } else { "; use --all to list the rest" }
        ))
    );
    Ok(())
}
