//! `emulabzip script`: see and approve the WebAssembly scripts that rules may run.

use std::path::Path;

use anyhow::{Result, bail};
use emulabzip_net::script::{Approvals, Script};

use crate::output;

/// Describes a script file, and whether it is approved.
pub fn show(path: &Path) -> Result<()> {
    let script = Script::load(path)?;
    let approved = Approvals::load().is_approved(&script.sha256);
    println!("{} {}", output::heading(&script.name), output::dim(&script.sha256));
    println!("  on request:  {}", if script.on_request { "yes" } else { "no" });
    println!("  on response: {}", if script.on_response { "yes" } else { "no" });
    println!(
        "  {}",
        if approved {
            "approved: it runs when a rule uses it"
        } else {
            "not approved: it will not run until you approve it"
        }
    );
    Ok(())
}

pub fn approve(path: &Path) -> Result<()> {
    let script = Script::load(path)?;
    let mut approvals = Approvals::load();
    approvals.approve(&script)?;
    println!("Approved {} {}", script.name, output::dim(&script.sha256));
    println!(
        "{}",
        output::dim("The approval is for these exact bytes. Change the file and it has to be approved again.")
    );
    Ok(())
}

pub fn revoke(target: &str) -> Result<()> {
    // A path, or the hash itself (or its beginning, as `list` shows it).
    let sha = if Path::new(target).is_file() {
        Script::load(Path::new(target))?.sha256
    } else {
        let approvals = Approvals::load();
        let mut found = approvals.list().iter().filter(|a| a.sha256.starts_with(target)).map(|a| a.sha256.clone());
        match (found.next(), found.next()) {
            (Some(one), None) if target.len() >= 8 => one,
            (None, _) => bail!("no approved script matches `{target}`"),
            _ => bail!("`{target}` is not enough to tell one approved script from another: give more of the hash"),
        }
    };
    let mut approvals = Approvals::load();
    approvals.revoke(&sha)?;
    println!("Approval revoked {}", output::dim(&sha));
    Ok(())
}

pub fn list(json: bool) -> Result<()> {
    let approvals = Approvals::load();
    if json {
        println!("{}", serde_json::to_string_pretty(approvals.list())?);
        return Ok(());
    }
    if approvals.list().is_empty() {
        println!("No script is approved. `emulabzip script approve FILE.wasm` approves one.");
    }
    for a in approvals.list() {
        println!("{}  {}", &a.sha256[..16], a.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_a_script_is_refused() {
        assert!(approve(Path::new("/no/such/file.wasm")).is_err());
        assert!(show(Path::new("/no/such/file.wasm")).is_err());
        assert!(revoke("zz").is_err(), "a short or unknown hash matches nothing");
    }
}
