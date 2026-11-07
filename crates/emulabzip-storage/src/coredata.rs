//! Core Data stores: a SQLite file where each entity has a table named `Z<ENTITY>` and each attribute a column named
//! `Z<ATTRIBUTE>`, in capitals, with bookkeeping in `Z_PRIMARYKEY` and `Z_METADATA`.
//!
//! The names are shown without their `Z`. Core Data writes them in capitals, so the case of an attribute (`bornAt`) is not
//! in the store, only the letters (`ZBORNAT`); the compiled model that has them is not read.

use emulabzip_core::Result;
use serde::Serialize;

use crate::sqlite::Database;

/// Whether a database is a Core Data store: it has the two tables Core Data keeps for itself.
pub fn is_core_data(db: &Database) -> bool {
    db.tables().is_ok_and(|tables| {
        let has = |name: &str| tables.iter().any(|t| t.name == name);
        has("Z_PRIMARYKEY") && has("Z_METADATA")
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribute {
    /// The column, `ZNAME`.
    pub column: String,
    /// What it is called without the `Z`, in lower case: `name`.
    pub name: String,
    pub decl_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub name: String,
    /// The table, `ZPERSON`.
    pub table: String,
    pub rows: u64,
    pub attributes: Vec<Attribute>,
}

/// The entities of a store, from `Z_PRIMARYKEY`. An entity that shares its parent's table, or has no table, is left out.
pub fn entities(db: &Database) -> Result<Vec<Entity>> {
    let tables = db.tables()?;
    let names = db.query("SELECT Z_NAME FROM Z_PRIMARYKEY ORDER BY Z_ENT", 10_000)?;
    let mut out = Vec::new();
    for row in names.rows {
        let Some(name) = row.first().and_then(|v| v.as_str()) else { continue };
        let table = format!("Z{}", name.to_uppercase());
        let Some(info) = tables.iter().find(|t| t.name == table && t.kind == "table") else { continue };
        let attributes = info
            .columns
            .iter()
            .filter(|c| !matches!(c.name.as_str(), "Z_PK" | "Z_ENT" | "Z_OPT"))
            .map(|c| Attribute {
                column: c.name.clone(),
                name: c.name.strip_prefix('Z').unwrap_or(&c.name).trim_start_matches('_').to_lowercase(),
                decl_type: c.decl_type.clone(),
            })
            .collect();
        out.push(Entity { name: name.to_string(), table: table.clone(), rows: info.rows, attributes });
    }
    Ok(out)
}

/// The rows of an entity with its attributes by name, for display.
pub fn rows(db: &Database, entity: &Entity, offset: u64, limit: u64) -> Result<crate::sqlite::Rows> {
    let mut page = db.rows(&entity.table, offset, limit)?;
    for column in &mut page.columns {
        if let Some(a) = entity.attributes.iter().find(|a| a.column == *column) {
            *column = a.name.clone();
        } else if let Some(stripped) = column.strip_prefix("Z_") {
            *column = format!("_{}", stripped.to_lowercase());
        }
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn store() -> (PathBuf, Database) {
        let dir = std::env::temp_dir().join(format!(
            "emulabzip-coredata-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Store.sqlite");
        std::fs::write(&path, include_bytes!("../tests/fixtures/coredata-store.sqlite")).unwrap();
        let db = Database::open(&path).unwrap();
        (dir, db)
    }

    #[test]
    fn a_real_core_data_store_is_told_from_other_databases() {
        let (dir, db) = store();
        assert!(is_core_data(&db));
        let other = dir.join("other.db");
        rusqlite::Connection::open(&other).unwrap().execute_batch("CREATE TABLE t (a);").unwrap();
        assert!(!is_core_data(&Database::open(&other).unwrap()));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn entities_come_with_their_attributes_without_the_z() {
        let (dir, db) = store();
        let entities = entities(&db).unwrap();
        let names: Vec<_> = entities.iter().map(|e| (e.name.as_str(), e.table.as_str(), e.rows)).collect();
        assert_eq!(names, [("Person", "ZPERSON", 2), ("Pet", "ZPET", 2)]);
        let person = &entities[0];
        let attrs: Vec<_> = person.attributes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(attrs, ["age", "bornat", "score", "name"]);
        // The relationship of a pet is a column too.
        assert!(entities[1].attributes.iter().any(|a| a.name == "owner"));
        let page = rows(&db, person, 0, 10).unwrap();
        assert_eq!(page.columns, ["_pk", "_ent", "_opt", "age", "bornat", "score", "name"]);
        assert_eq!(page.rows[0][6], "Ana");
        assert_eq!(page.rows[1][3], 27);
        std::fs::remove_dir_all(dir).ok();
    }
}
