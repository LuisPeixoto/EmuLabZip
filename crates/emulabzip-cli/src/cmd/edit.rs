//! `emulabzip edit`: change a row, a preference or a property list in an app's storage. The app is stopped, the
//! change is made and read back, and the old file is kept so `edit undo` can put it back.

use std::path::Path;

use anyhow::{Result, bail};
use emulabzip_core::Lab;
use emulabzip_storage::edit::device::{Backup, Place, backup_dir, edit_database, edit_file, undo};
use emulabzip_storage::edit::plist::{self, PlistValue};
use emulabzip_storage::edit::prefs::{self, PrefValue};
use emulabzip_storage::edit::sqlite::{Cell, RowKey};

use super::files::pick_root;
use super::state::need_yes;

/// A value as typed: a whole number, a number with a point, or text.
fn guess(text: &str) -> Cell {
    if let Ok(i) = text.trim().parse::<i64>() {
        Cell::Integer(i)
    } else if let Ok(f) = text.trim().parse::<f64>()
        && f.is_finite()
    {
        Cell::Real(f)
    } else {
        Cell::Text(text.to_string())
    }
}

fn split(pair: &str) -> Result<(String, &str)> {
    match pair.split_once('=') {
        Some((k, v)) if !k.is_empty() => Ok((k.to_string(), v)),
        _ => bail!("expected column=value, got {pair}"),
    }
}

/// What was asked for in a row edit.
pub struct Assign {
    /// `column=value`, the value typed as it looks.
    pub set: Vec<String>,
    /// `column=value`, always text.
    pub text: Vec<String>,
    /// Columns to set to NULL.
    pub null: Vec<String>,
}

impl Assign {
    fn cells(&self) -> Result<Vec<(String, Cell)>> {
        let mut out = Vec::new();
        for p in &self.set {
            let (k, v) = split(p)?;
            out.push((k, guess(v)));
        }
        for p in &self.text {
            let (k, v) = split(p)?;
            out.push((k, Cell::Text(v.to_string())));
        }
        for k in &self.null {
            out.push((k.clone(), Cell::Null));
        }
        Ok(out)
    }
}

pub struct Target<'a> {
    pub app: &'a str,
    pub path: &'a str,
    pub root: Option<&'a str>,
}

fn key(rowid: Option<i64>, pk: &[String]) -> Result<RowKey> {
    match (rowid, pk.is_empty()) {
        (Some(r), true) => Ok(RowKey::Rowid(r)),
        (None, false) => {
            Ok(RowKey::Pk(pk.iter().map(|p| split(p).map(|(k, v)| (k, guess(v)))).collect::<Result<_>>()?))
        }
        _ => bail!("say which row: --rowid N, or --pk column=value for each primary key column"),
    }
}

fn done(backup: &Backup, what: &str) {
    println!("{what}");
    println!("Undo with: emulabzip edit undo {} --yes", backup.dir.display());
}

async fn place(lab: &Lab, device: Option<&str>, t: &Target<'_>) -> Result<(emulabzip_core::Target, String)> {
    let device = lab.resolve(device).await?;
    let root = pick_root(lab, &device, t.app, t.root).await?;
    Ok((device.target(), root.id))
}

/// Which row of which table.
pub struct Row<'a> {
    pub table: &'a str,
    pub rowid: Option<i64>,
    pub pk: &'a [String],
}

pub async fn cell(
    lab: &Lab,
    device: Option<&str>,
    t: Target<'_>,
    row: Row<'_>,
    assign: Assign,
    yes: bool,
) -> Result<()> {
    let Row { table, rowid, pk } = row;
    need_yes(yes, "Editing a row")?;
    let key = key(rowid, pk)?;
    let cells = assign.cells()?;
    if cells.is_empty() {
        bail!("nothing to set: use --set column=value, --text column=value or --null column");
    }
    let (target, root) = place(lab, device, &t).await?;
    let backup =
        edit_database(lab, &target, Place { app: t.app, root: &root, path: t.path }, true, &backup_dir(), |e| {
            for (column, value) in &cells {
                e.update(table, &key, column, value)?;
            }
            Ok(())
        })
        .await?;
    done(&backup, &format!("Changed {} column(s) of one row in {table}.", cells.len()));
    Ok(())
}

pub async fn insert(
    lab: &Lab,
    device: Option<&str>,
    t: Target<'_>,
    table: &str,
    assign: Assign,
    yes: bool,
) -> Result<()> {
    need_yes(yes, "Adding a row")?;
    let cells = assign.cells()?;
    let (target, root) = place(lab, device, &t).await?;
    let mut rowid = 0;
    let backup =
        edit_database(lab, &target, Place { app: t.app, root: &root, path: t.path }, true, &backup_dir(), |e| {
            rowid = e.insert(table, &cells)?;
            Ok(())
        })
        .await?;
    done(&backup, &format!("Added row {rowid} to {table}."));
    Ok(())
}

pub async fn delete(
    lab: &Lab,
    device: Option<&str>,
    t: Target<'_>,
    table: &str,
    rowid: Option<i64>,
    pk: &[String],
    yes: bool,
) -> Result<()> {
    need_yes(yes, "Deleting a row")?;
    let key = key(rowid, pk)?;
    let (target, root) = place(lab, device, &t).await?;
    let backup =
        edit_database(lab, &target, Place { app: t.app, root: &root, path: t.path }, true, &backup_dir(), |e| {
            e.delete(table, &key)
        })
        .await?;
    done(&backup, &format!("Deleted one row of {table}."));
    Ok(())
}

pub async fn pref_set(
    lab: &Lab,
    device: Option<&str>,
    t: Target<'_>,
    name: &str,
    kind: &str,
    value: &str,
    yes: bool,
) -> Result<()> {
    need_yes(yes, "Setting a preference")?;
    let value = PrefValue::parse(kind, value)?;
    let (target, root) = place(lab, device, &t).await?;
    let backup =
        edit_file(lab, &target, Place { app: t.app, root: &root, path: t.path }, true, &backup_dir(), |bytes| {
            let xml = String::from_utf8(bytes.to_vec())
                .map_err(|_| emulabzip_core::Error::failed("The file is not text."))?;
            Ok(prefs::set(&xml, name, value)?.into_bytes())
        })
        .await?;
    done(&backup, &format!("Set {name}."));
    Ok(())
}

pub async fn pref_remove(lab: &Lab, device: Option<&str>, t: Target<'_>, name: &str, yes: bool) -> Result<()> {
    need_yes(yes, "Removing a preference")?;
    let (target, root) = place(lab, device, &t).await?;
    let backup =
        edit_file(lab, &target, Place { app: t.app, root: &root, path: t.path }, true, &backup_dir(), |bytes| {
            let xml = String::from_utf8(bytes.to_vec())
                .map_err(|_| emulabzip_core::Error::failed("The file is not text."))?;
            Ok(prefs::remove(&xml, name)?.into_bytes())
        })
        .await?;
    done(&backup, &format!("Removed {name}."));
    Ok(())
}

pub async fn plist_set(
    lab: &Lab,
    device: Option<&str>,
    t: Target<'_>,
    key_path: &str,
    kind: &str,
    value: &str,
    yes: bool,
) -> Result<()> {
    need_yes(yes, "Setting a property list value")?;
    let value = PlistValue::parse(kind, value)?;
    let keys: Vec<String> = key_path.split('/').map(str::to_string).collect();
    let (target, root) = place(lab, device, &t).await?;
    let backup =
        edit_file(lab, &target, Place { app: t.app, root: &root, path: t.path }, true, &backup_dir(), |bytes| {
            plist::set(bytes, &keys, &value)
        })
        .await?;
    done(&backup, &format!("Set {key_path}."));
    Ok(())
}

pub async fn plist_remove(lab: &Lab, device: Option<&str>, t: Target<'_>, key_path: &str, yes: bool) -> Result<()> {
    need_yes(yes, "Removing a property list value")?;
    let keys: Vec<String> = key_path.split('/').map(str::to_string).collect();
    let (target, root) = place(lab, device, &t).await?;
    let backup =
        edit_file(lab, &target, Place { app: t.app, root: &root, path: t.path }, true, &backup_dir(), |bytes| {
            plist::remove(bytes, &keys)
        })
        .await?;
    done(&backup, &format!("Removed {key_path}."));
    Ok(())
}

pub async fn undo_edit(lab: &Lab, device: Option<&str>, dir: &Path, yes: bool) -> Result<()> {
    need_yes(yes, "Undoing an edit")?;
    let backup = Backup::load(dir)?;
    let device = lab.resolve(device).await?;
    undo(lab, &device.target(), &backup, true).await?;
    println!("Put back {} file(s) of {}.", backup.files.len(), backup.app);
    Ok(())
}
