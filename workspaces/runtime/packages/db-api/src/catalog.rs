//! Catalog export from descriptor owners and explicitly registered legacy SQL.
use rusqlite::Connection;

#[derive(Debug, serde::Serialize)]
pub struct Entry {
    pub create_order: i64,
    pub name: String,
    pub sql: String,
    pub table: String,
    pub r#type: String,
}

/// SQLite supplies object metadata, including objects declared in SQL batches.
/// FTS shadow tables and SQLite's internal indexes are not application owners.
pub fn export(connection: &Connection) -> rusqlite::Result<Vec<Entry>> {
    let mut statement = connection.prepare(
        "SELECT rowid,name,sql,tbl_name,type FROM sqlite_master
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
         AND name NOT IN (SELECT name FROM pragma_table_list WHERE type='shadow')
         ORDER BY type,name",
    )?;
    statement
        .query_map([], |row| {
            Ok(Entry {
                create_order: row.get(0)?,
                name: row.get(1)?,
                sql: row.get(2)?,
                table: row.get(3)?,
                r#type: row.get(4)?,
            })
        })?
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_triggers_and_fts_export_without_shadow_tables() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE source(id INTEGER PRIMARY KEY);
            CREATE INDEX source_id ON source(id);
            CREATE TRIGGER source_delete AFTER DELETE ON source BEGIN SELECT 1; END;
            CREATE VIRTUAL TABLE search USING fts5(body);",
            )
            .unwrap();
        let entries = export(&connection).unwrap();
        assert_eq!(entries.len(), 4);
        assert!(entries.iter().any(|entry| entry.r#type == "trigger"));
        assert!(entries.iter().any(|entry| entry.name == "search"));
        assert!(
            connection
                .execute_batch("CREATE TABLE duplicate(id); CREATE TABLE duplicate(id);")
                .is_err()
        );
    }
}
