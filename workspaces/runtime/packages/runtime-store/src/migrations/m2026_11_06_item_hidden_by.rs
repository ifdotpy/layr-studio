//! A transcript item hidden by a rewind keeps its row; the marker moves from
//! `afterRestore` inside the JSON record to the typed `hidden_by` column (#630 item 66).
//! A row is hidden when the column is not NULL. An empty marker (`""`, `false`, `0`,
//! `[]`, `{}`, null) never hid an item in history export, so it becomes NULL. The step is repeatable: test fixtures
//! apply the catalog again on a database that already has the column.
use crate::StoreResult;
use rusqlite::Transaction;

pub(crate) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let present: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('runtime_items') WHERE name='hidden_by')",
        [],
        |row| row.get(0),
    )?;
    if !present {
        tx.execute_batch("ALTER TABLE runtime_items ADD COLUMN hidden_by TEXT")?;
    }
    tx.execute_batch(
        "UPDATE runtime_items SET
             hidden_by=COALESCE(hidden_by,CASE json_type(record,'$.afterRestore')
                 WHEN 'text' THEN NULLIF(json_extract(record,'$.afterRestore'),'')
                 WHEN 'true' THEN 'true'
                 WHEN 'integer' THEN NULLIF(CAST(json_extract(record,'$.afterRestore') AS TEXT),'0')
                 WHEN 'real' THEN CASE WHEN json_extract(record,'$.afterRestore')<>0
                     THEN CAST(json_extract(record,'$.afterRestore') AS TEXT) END
                 WHEN 'array' THEN CASE WHEN json_array_length(record,'$.afterRestore')>0
                     THEN json_extract(record,'$.afterRestore') END
                 WHEN 'object' THEN NULLIF(json_extract(record,'$.afterRestore'),'{}')
                 END),
             record=json_remove(record,'$.afterRestore')
         WHERE json_valid(record) AND json_type(record,'$.afterRestore') IS NOT NULL;
         CREATE INDEX IF NOT EXISTS runtime_item_visible ON runtime_items(agent, created)
             WHERE hidden_by IS NULL;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn markers_move_to_the_column_and_the_step_repeats() {
        let mut db = Connection::open_in_memory().expect("db");
        db.execute_batch(
            "CREATE TABLE runtime_items(id TEXT PRIMARY KEY,agent TEXT NOT NULL,record TEXT NOT NULL,created REAL NOT NULL);
             INSERT INTO runtime_items VALUES
               ('shown','a','{\"id\":\"shown\"}',1),
               ('hidden','a','{\"id\":\"hidden\",\"afterRestore\":\"cp1\"}',2),
               ('null','a','{\"id\":\"null\",\"afterRestore\":null}',3),
               ('empty','a','{\"id\":\"empty\",\"afterRestore\":[]}',4);",
        )
        .expect("fixture");
        for _ in 0..2 {
            let tx = db.transaction().expect("tx");
            super::apply(&tx).expect("apply");
            tx.commit().expect("commit");
        }
        let rows = db
            .prepare("SELECT id,record,hidden_by FROM runtime_items ORDER BY created")
            .expect("query")
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .expect("rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect");
        assert_eq!(
            rows,
            [
                ("shown".into(), r#"{"id":"shown"}"#.into(), None),
                (
                    "hidden".into(),
                    r#"{"id":"hidden"}"#.into(),
                    Some("cp1".into())
                ),
                ("null".into(), r#"{"id":"null"}"#.into(), None),
                ("empty".into(), r#"{"id":"empty"}"#.into(), None),
            ]
        );
    }
}
