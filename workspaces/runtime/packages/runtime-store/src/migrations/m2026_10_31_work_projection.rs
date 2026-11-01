use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_work")? {
        return Ok(());
    }
    let mut query = tx.prepare("SELECT id,record FROM runtime_work")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let raw: String = row.get(1)?;
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if let Some(fields) = value.as_object_mut()
            && fields
                .get("agent")
                .is_some_and(|value| !value.is_null() && !value.is_string())
        {
            fields.remove("agent");
        } else {
            continue;
        }
        let normalized = serde_json::to_string(&value)?;
        tx.execute(
            "UPDATE runtime_work SET record=?2 WHERE id=?1",
            (&id, normalized),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use studio_runtime_schema::work_item_record::WorkItem;

    #[test]
    fn preserves_already_valid_row_bytes() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_work(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE writes(id TEXT); CREATE TRIGGER log_write AFTER UPDATE ON runtime_work BEGIN INSERT INTO writes VALUES(new.id); END;").unwrap();
        let original = r#"{ "status": "ready", "id": "a" }"#;
        db.execute("INSERT INTO runtime_work VALUES('a',?1)", [original])
            .unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            tx.query_row("SELECT record FROM runtime_work", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            original
        );
        assert_eq!(
            tx.query_row("SELECT count(*) FROM writes", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn removes_non_string_agent_only_and_is_idempotent() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_work(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_work VALUES('a','{"id":"a","status":"ready","agent":42,"archive":{"x":1}}'),('b','{"id":"b","status":"ready","agent":null}'),('c','{"id":"c","status":"ready","agent":"worker"}');"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let raw: String = tx
            .query_row("SELECT record FROM runtime_work WHERE id='a'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let parsed: WorkItem = serde_json::from_str(&raw).unwrap();
        assert!(parsed.agent_field().is_missing());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&raw).unwrap()["archive"]["x"],
            1
        );
        let raw: String = tx
            .query_row("SELECT record FROM runtime_work WHERE id='b'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(matches!(
            serde_json::from_str::<WorkItem>(&raw)
                .unwrap()
                .agent_field(),
            studio_runtime_schema::queue_command::Field::Null
        ));
    }
}
