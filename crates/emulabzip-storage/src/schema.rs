//! What is inside an app's databases and preferences, kept beside a snapshot so two snapshots can be compared (to test a
//! migration, say) without unpacking either.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use emulabzip_core::{Error, Result, tarball};
use serde::{Deserialize, Serialize};

use crate::detect::{FileKind, detect};
use crate::sqlite::Database;

/// Files bigger than this are listed without being looked into.
const MAX_FILE: u64 = 256 * 1024 * 1024;
/// The most files looked into, so a sandbox of a million small files does not take for ever.
const MAX_FILES: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSchema {
    pub name: String,
    pub decl_type: String,
    pub not_null: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    /// Indexes of the table, by name, with the statement that made each.
    pub indexes: Vec<(String, String)>,
    pub rows: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySchema {
    pub key: String,
    /// `string`, `int`, `set`… as the format names the type.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSchema {
    pub path: String,
    /// `sqlite`, `realm`, `sharedPrefs`, `dataStore`, `mmkv`, `plist` or `asyncStorage`.
    pub kind: String,
    pub tables: Vec<TableSchema>,
    pub keys: Vec<KeySchema>,
    /// Why the inside could not be read.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schemas {
    pub files: Vec<FileSchema>,
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
        if meta.is_dir() {
            walk(&e.path(), root, out);
        } else if meta.is_file() && out.len() < MAX_FILES {
            out.push(e.path().strip_prefix(root).unwrap_or(&e.path()).to_path_buf());
        }
    }
}

fn sqlite_tables(path: &Path) -> Result<Vec<TableSchema>> {
    let db = Database::open(path)?;
    let indexes =
        db.query("SELECT tbl_name, name, sql FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL", 100_000)?;
    let mut by_table: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for row in indexes.rows {
        let text = |i: usize| row.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
        by_table.entry(text(0)).or_default().push((text(1), text(2)));
    }
    Ok(db
        .tables()?
        .into_iter()
        .filter(|t| t.kind == "table")
        .map(|t| TableSchema {
            columns: t
                .columns
                .iter()
                .map(|c| ColumnSchema {
                    name: c.name.clone(),
                    decl_type: c.decl_type.clone(),
                    not_null: c.not_null,
                    primary_key: c.primary_key,
                })
                .collect(),
            indexes: by_table.remove(&t.name).unwrap_or_default(),
            rows: t.rows,
            name: t.name,
        })
        .collect())
}

fn keys_of(prefs: Vec<crate::prefs::Pref>) -> Vec<KeySchema> {
    prefs.into_iter().map(|p| KeySchema { key: p.key, kind: p.kind }).collect()
}

fn describe(path: &str, full: &Path) -> Option<FileSchema> {
    let meta = std::fs::metadata(full).ok()?;
    let head = {
        use std::io::Read;
        let mut buf = vec![0u8; 4096];
        let n = std::fs::File::open(full).ok()?.read(&mut buf).ok()?;
        buf.truncate(n);
        buf
    };
    let mut kind = detect(path, &head);
    if !matches!(
        kind,
        FileKind::Sqlite
            | FileKind::DataStore
            | FileKind::Mmkv
            | FileKind::Realm
            | FileKind::SharedPrefs
            | FileKind::Plist
            | FileKind::AsyncStorage
    ) {
        return None;
    }
    // A SharedPreferences file is told by its content, which may be past the head of a large one.
    if kind == FileKind::Plist && path.contains("shared_prefs/") {
        kind = FileKind::SharedPrefs;
    }
    let name = match kind {
        FileKind::Sqlite => "sqlite",
        FileKind::Realm => "realm",
        FileKind::SharedPrefs => "sharedPrefs",
        FileKind::DataStore => "dataStore",
        FileKind::Mmkv => "mmkv",
        FileKind::Plist => "plist",
        _ => "asyncStorage",
    };
    let mut file = FileSchema {
        path: path.to_string(),
        kind: name.to_string(),
        tables: Vec::new(),
        keys: Vec::new(),
        error: None,
    };
    if meta.len() > MAX_FILE {
        file.error = Some("The file is too big to look into.".into());
        return Some(file);
    }
    let result: Result<()> = (|| {
        let bytes = || std::fs::read(full).map_err(Error::from);
        match kind {
            FileKind::Sqlite => file.tables = sqlite_tables(full)?,
            FileKind::Realm => {
                file.tables = crate::realm::read(&bytes()?)?
                    .tables
                    .into_iter()
                    .map(|t| TableSchema {
                        name: t.name,
                        rows: t.rows,
                        indexes: Vec::new(),
                        columns: t
                            .columns
                            .into_iter()
                            .map(|c| ColumnSchema {
                                decl_type: match &c.collection {
                                    Some(coll) => format!("{coll}<{}>", c.kind),
                                    None => c.kind,
                                },
                                name: c.name,
                                not_null: !c.nullable,
                                primary_key: false,
                            })
                            .collect(),
                    })
                    .collect();
            }
            FileKind::SharedPrefs => file.keys = keys_of(crate::prefs::parse(&String::from_utf8_lossy(&bytes()?))?),
            FileKind::DataStore => file.keys = keys_of(crate::datastore::parse_preferences(&bytes()?)?),
            FileKind::Mmkv => {
                file.keys = crate::mmkv::parse(&bytes()?)?
                    .into_iter()
                    .map(|e| KeySchema { key: e.key, kind: e.kind })
                    .collect();
            }
            FileKind::Plist => {
                if let serde_json::Value::Object(map) = crate::plist::to_json(&bytes()?)? {
                    file.keys =
                        map.into_iter().map(|(k, v)| KeySchema { key: k, kind: json_kind(&v).to_string() }).collect();
                }
            }
            _ => {}
        }
        Ok(())
    })();
    if let Err(e) = result {
        file.error = Some(e.to_string());
    }
    file.keys.sort_by(|a, b| a.key.cmp(&b.key));
    Some(file)
}

fn json_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(n) if n.is_f64() => "real",
        serde_json::Value::Number(_) => "integer",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "dictionary",
    }
}

/// Describes every database and preferences file inside a folder, with paths relative to it. A `-wal` and a `-shm` file
/// next to a database are used with it by SQLite as it opens.
pub fn scan_dir(root: &Path) -> Schemas {
    let mut paths = Vec::new();
    walk(root, root, &mut paths);
    let files = paths
        .iter()
        .filter_map(|p| {
            let rel = p.to_string_lossy().replace('\\', "/");
            describe(&rel, &root.join(p))
        })
        .collect();
    Schemas { files }
}

/// Describes the inside of a state archive's `data.tar`.
pub fn scan(archive: &Path) -> Result<Schemas> {
    let dir = std::env::temp_dir().join(format!("emulabzip-schema-{}-{:x}", std::process::id(), {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())
    }));
    std::fs::create_dir_all(&dir)?;
    let result = tarball::unpack(archive, &dir).map(|_| scan_dir(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// [`scan`], as the JSON kept in a state file.
pub fn scan_json(archive: &Path) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(&scan(archive)?).map_err(|e| Error::failed(e.to_string()))
}

/// What kind of difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Change {
    Added,
    Removed,
    Changed,
}

/// What a difference is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    File,
    Table,
    Column,
    Index,
    Key,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    pub file: String,
    pub scope: Scope,
    /// `cart`, `cart.note`, `idx_cart_sku`, `theme`.
    pub name: String,
    pub change: Change,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl Difference {
    /// One line for people.
    pub fn line(&self) -> String {
        let sign = match self.change {
            Change::Added => '+',
            Change::Removed => '-',
            Change::Changed => '~',
        };
        let what = match self.scope {
            Scope::File => "file",
            Scope::Table => "table",
            Scope::Column => "column",
            Scope::Index => "index",
            Scope::Key => "key",
        };
        let detail = match (&self.before, &self.after) {
            (Some(b), Some(a)) => format!("  {b} → {a}"),
            (None, Some(a)) => format!("  {a}"),
            (Some(b), None) => format!("  {b}"),
            (None, None) => String::new(),
        };
        format!("{sign} {}: {what} {}{detail}", self.file, self.name)
    }
}

fn describe_column(c: &ColumnSchema) -> String {
    let mut s = if c.decl_type.is_empty() { "(no type)".to_string() } else { c.decl_type.clone() };
    if c.not_null {
        s.push_str(" NOT NULL");
    }
    if c.primary_key {
        s.push_str(" PRIMARY KEY");
    }
    s
}

fn files_by_path(s: &Schemas) -> BTreeMap<String, &FileSchema> {
    s.files.iter().map(|f| (f.path.clone(), f)).collect()
}

fn tables_by_name(f: &FileSchema) -> BTreeMap<String, &TableSchema> {
    f.tables.iter().map(|t| (t.name.clone(), t)).collect()
}

fn columns_by_name(t: &TableSchema) -> BTreeMap<String, &ColumnSchema> {
    t.columns.iter().map(|c| (c.name.clone(), c)).collect()
}

/// What changed between two descriptions: files, tables, columns, indexes and keys, added, removed or changed.
pub fn diff(a: &Schemas, b: &Schemas) -> Vec<Difference> {
    let mut out = Vec::new();
    let (fa, fb) = (files_by_path(a), files_by_path(b));
    let mut paths: Vec<&String> = fa.keys().chain(fb.keys()).collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        let push =
            |out: &mut Vec<Difference>, scope, name: String, change, before: Option<String>, after: Option<String>| {
                out.push(Difference { file: path.clone(), scope, name, change, before, after })
            };
        match (fa.get(path), fb.get(path)) {
            (None, Some(f)) => push(&mut out, Scope::File, path.clone(), Change::Added, None, Some(f.kind.clone())),
            (Some(f), None) => push(&mut out, Scope::File, path.clone(), Change::Removed, Some(f.kind.clone()), None),
            (Some(x), Some(y)) => {
                if x.kind != y.kind {
                    push(
                        &mut out,
                        Scope::File,
                        path.clone(),
                        Change::Changed,
                        Some(x.kind.clone()),
                        Some(y.kind.clone()),
                    );
                }
                let (ta, tb) = (tables_by_name(x), tables_by_name(y));
                let mut names: Vec<&String> = ta.keys().chain(tb.keys()).collect();
                names.sort();
                names.dedup();
                for name in names {
                    match (ta.get(name), tb.get(name)) {
                        (None, Some(t)) => push(
                            &mut out,
                            Scope::Table,
                            name.clone(),
                            Change::Added,
                            None,
                            Some(format!("{} columns", t.columns.len())),
                        ),
                        (Some(t), None) => push(
                            &mut out,
                            Scope::Table,
                            name.clone(),
                            Change::Removed,
                            Some(format!("{} columns", t.columns.len())),
                            None,
                        ),
                        (Some(t), Some(u)) => {
                            let (ca, cb) = (columns_by_name(t), columns_by_name(u));
                            for (col, c) in &cb {
                                match ca.get(col) {
                                    None => push(
                                        &mut out,
                                        Scope::Column,
                                        format!("{name}.{col}"),
                                        Change::Added,
                                        None,
                                        Some(describe_column(c)),
                                    ),
                                    Some(old) if old != c => push(
                                        &mut out,
                                        Scope::Column,
                                        format!("{name}.{col}"),
                                        Change::Changed,
                                        Some(describe_column(old)),
                                        Some(describe_column(c)),
                                    ),
                                    _ => {}
                                }
                            }
                            for (col, c) in &ca {
                                if !cb.contains_key(col) {
                                    push(
                                        &mut out,
                                        Scope::Column,
                                        format!("{name}.{col}"),
                                        Change::Removed,
                                        Some(describe_column(c)),
                                        None,
                                    );
                                }
                            }
                            let idx = |t: &TableSchema| t.indexes.iter().cloned().collect::<BTreeMap<_, _>>();
                            let (ia, ib) = (idx(t), idx(u));
                            for (i, sql) in &ib {
                                match ia.get(i) {
                                    None => {
                                        push(&mut out, Scope::Index, i.clone(), Change::Added, None, Some(sql.clone()))
                                    }
                                    Some(old) if old != sql => push(
                                        &mut out,
                                        Scope::Index,
                                        i.clone(),
                                        Change::Changed,
                                        Some(old.clone()),
                                        Some(sql.clone()),
                                    ),
                                    _ => {}
                                }
                            }
                            for (i, sql) in &ia {
                                if !ib.contains_key(i) {
                                    push(&mut out, Scope::Index, i.clone(), Change::Removed, Some(sql.clone()), None);
                                }
                            }
                        }
                        (None, None) => {}
                    }
                }
                let keys =
                    |f: &FileSchema| f.keys.iter().map(|k| (k.key.clone(), k.kind.clone())).collect::<BTreeMap<_, _>>();
                let (ka, kb) = (keys(x), keys(y));
                for (k, kind) in &kb {
                    match ka.get(k) {
                        None => push(&mut out, Scope::Key, k.clone(), Change::Added, None, Some(kind.clone())),
                        Some(old) if old != kind => push(
                            &mut out,
                            Scope::Key,
                            k.clone(),
                            Change::Changed,
                            Some(old.clone()),
                            Some(kind.clone()),
                        ),
                        _ => {}
                    }
                }
                for (k, kind) in &ka {
                    if !kb.contains_key(k) {
                        push(&mut out, Scope::Key, k.clone(), Change::Removed, Some(kind.clone()), None);
                    }
                }
            }
            (None, None) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

    fn sandbox(name: &str, shop: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emulabzip-schema-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["databases", "shared_prefs", "files/datastore", "files/mmkv"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        let copy = |from: &str, to: &str| std::fs::copy(Path::new(FIXTURES).join(from), dir.join(to)).unwrap();
        copy(shop, "databases/shop.db");
        copy("settings.xml", "shared_prefs/settings.xml");
        copy("demo.preferences_pb", "files/datastore/demo.preferences_pb");
        copy("mmkv.default", "files/mmkv/mmkv.default");
        copy("default.realm", "files/default.realm");
        copy("coredata-store.sqlite", "files/Store.sqlite");
        std::fs::write(dir.join("files/notes.txt"), "not a database").unwrap();
        dir
    }

    fn file<'a>(s: &'a Schemas, path: &str) -> &'a FileSchema {
        s.files
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("{path} is not in {:?}", s.files.iter().map(|f| &f.path).collect::<Vec<_>>()))
    }

    #[test]
    fn describes_what_the_demo_app_keeps() {
        let dir = sandbox("describe", "shop-v1.db");
        let s = scan_dir(&dir);
        let paths: Vec<_> = s.files.iter().map(|f| (f.path.as_str(), f.kind.as_str())).collect();
        assert_eq!(
            paths,
            [
                ("databases/shop.db", "sqlite"),
                ("files/Store.sqlite", "sqlite"),
                ("files/datastore/demo.preferences_pb", "dataStore"),
                ("files/default.realm", "realm"),
                ("files/mmkv/mmkv.default", "mmkv"),
                ("shared_prefs/settings.xml", "sharedPrefs"),
            ],
            "only what has a schema is listed, not notes.txt"
        );
        let shop = file(&s, "databases/shop.db");
        let tables: Vec<_> = shop.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(tables.contains(&"cart") && tables.contains(&"customers"), "{tables:?}");
        let cart = shop.tables.iter().find(|t| t.name == "cart").unwrap();
        assert_eq!(
            cart.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["id", "sku", "qty", "price", "customer_id"]
        );
        assert!(cart.columns[0].primary_key && cart.columns[1].not_null);
        assert_eq!(cart.rows, 3);
        assert_eq!(file(&s, "shared_prefs/settings.xml").keys.len(), 6);
        assert_eq!(file(&s, "files/datastore/demo.preferences_pb").keys.len(), 6);
        assert_eq!(file(&s, "files/mmkv/mmkv.default").keys.len(), 7);
        let realm = file(&s, "files/default.realm");
        assert_eq!(realm.tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Big", "Person", "Pet"]);
        assert!(realm.tables[1].columns.iter().any(|c| c.name == "tags" && c.decl_type == "list<string>"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_migration_shows_as_a_column_an_index_and_a_table() {
        let (a, b) = (sandbox("v1", "shop-v1.db"), sandbox("v2", "shop-v2.db"));
        let changes = diff(&scan_dir(&a), &scan_dir(&b));
        let lines: Vec<String> = changes.iter().map(Difference::line).collect();
        assert_eq!(changes.len(), 3, "{lines:#?}");
        assert!(lines.contains(&"+ databases/shop.db: column cart.note  TEXT".to_string()), "{lines:#?}");
        assert!(
            lines
                .iter()
                .any(|l| l
                    .starts_with("+ databases/shop.db: index idx_cart_sku  CREATE INDEX idx_cart_sku ON cart (sku)")),
            "{lines:#?}"
        );
        assert!(lines.contains(&"+ databases/shop.db: table orders  3 columns".to_string()), "{lines:#?}");
        // The same thing against itself has no differences.
        assert!(diff(&scan_dir(&a), &scan_dir(&a)).is_empty());
        // And the other way round is the same changes undone.
        let back = diff(&scan_dir(&b), &scan_dir(&a));
        assert_eq!(back.len(), 3);
        assert!(back.iter().all(|d| d.change == Change::Removed));
        std::fs::remove_dir_all(a).ok();
        std::fs::remove_dir_all(b).ok();
    }

    #[test]
    fn files_keys_and_types_that_change_are_reported() {
        let mk = |kind: &str, keys: &[(&str, &str)]| Schemas {
            files: vec![FileSchema {
                path: "p.xml".into(),
                kind: kind.into(),
                tables: vec![],
                keys: keys.iter().map(|(k, t)| KeySchema { key: k.to_string(), kind: t.to_string() }).collect(),
                error: None,
            }],
        };
        let a = mk("sharedPrefs", &[("theme", "string"), ("launches", "int"), ("gone", "int")]);
        let b = mk("sharedPrefs", &[("theme", "string"), ("launches", "long"), ("new", "boolean")]);
        let lines: Vec<String> = diff(&a, &b).iter().map(Difference::line).collect();
        assert_eq!(lines, ["~ p.xml: key launches  int → long", "+ p.xml: key new  boolean", "- p.xml: key gone  int"]);
        let empty = Schemas::default();
        let all = diff(&empty, &a);
        assert_eq!(all.len(), 1, "a file that appears is one difference");
        assert_eq!(all[0].scope, Scope::File);
        assert_eq!(diff(&a, &empty)[0].change, Change::Removed);
    }

    #[test]
    fn a_broken_file_is_listed_with_why_and_does_not_stop_the_scan() {
        let dir = std::env::temp_dir().join(format!("emulabzip-schema-broken-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("shared_prefs")).unwrap();
        std::fs::write(dir.join("shared_prefs/bad.xml"), "<map><int name=\"a\" ").unwrap();
        std::fs::write(dir.join("shared_prefs/ok.xml"), "<map><int name=\"a\" value=\"1\"/></map>").unwrap();
        let s = scan_dir(&dir);
        assert!(file(&s, "shared_prefs/bad.xml").error.is_some());
        assert_eq!(file(&s, "shared_prefs/ok.xml").keys.len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn schemas_round_trip_through_json_and_a_tar() {
        let dir = sandbox("tar", "shop-v1.db");
        let tar = dir.with_extension("tar");
        tarball::pack_dir(&dir, &tar, &[]).unwrap();
        let json = scan_json(&tar).unwrap();
        let back: Schemas = serde_json::from_slice(&json).unwrap();
        assert_eq!(back, scan_dir(&dir));
        std::fs::remove_file(tar).ok();
        std::fs::remove_dir_all(dir).ok();
    }
}
