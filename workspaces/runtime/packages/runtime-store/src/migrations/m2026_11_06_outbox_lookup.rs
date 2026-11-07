use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if crate::schema::table_exists(tx, "runtime_outbox")? {
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS runtime_outbox_kind_next_at ON runtime_outbox(kind,next_at)",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, types::Value};

    const QUERIES: [&str; 2] = [
        "SELECT id FROM runtime_outbox WHERE kind='remote_delivery' AND json_extract(payload,'$.effect')='federation_message' AND (NULL IS NULL OR target=NULL) ORDER BY rowid",
        "SELECT MIN(next_at) FROM runtime_outbox WHERE kind='remote_delivery' AND json_extract(payload,'$.effect')='server_request' AND next_at<100 AND (status='pending' OR (status='submitted' AND (json_extract(json_extract(payload,'$.record.body'),'$.action') IN ('exec') OR json_extract(json_extract(payload,'$.record.body'),'$.action') NOT IN ('exec','task'))))",
    ];

    fn rows(db: &Connection, sql: &str) -> Vec<Value> {
        db.prepare(sql)
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn outbox_lookups_use_index_and_preserve_results() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"
            CREATE TABLE runtime_outbox(id TEXT PRIMARY KEY, kind TEXT, payload TEXT, target TEXT, next_at INTEGER, status TEXT);
            INSERT INTO runtime_outbox VALUES
              ('o1','remote_delivery','{"effect":"federation_message"}','p',2,'pending'),
              ('o2','remote_delivery','{"effect":"server_request"}','p',3,'pending'),
              ('o3','remote_delivery','{"effect":"server_request"}','p',5,'pending'),
              ('o4','other','malformed','p',1,'pending');
        "#).unwrap();
        let before: Vec<_> = QUERIES.iter().map(|sql| rows(&db, sql)).collect();
        assert_eq!(
            before,
            [vec![Value::Text("o1".into())], vec![Value::Integer(3)]]
        );
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        tx.commit().unwrap();
        for (sql, expected) in QUERIES.iter().zip(before) {
            assert_eq!(rows(&db, sql), expected);
            let plan: Vec<String> = db
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map([], |row| row.get(3))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            println!("{sql}: {plan:?}");
            assert!(
                plan.iter().any(|step| step
                    .contains("SEARCH runtime_outbox USING INDEX runtime_outbox_kind_next_at")),
                "{plan:?}"
            );
        }
    }

    #[test]
    fn current_canvas_installs_the_outbox_index() {
        let db = crate::fresh_state::test_canvas();
        let count: i64 = db.query_row("SELECT count(*) FROM sqlite_schema WHERE type='index' AND name='runtime_outbox_kind_next_at'", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
