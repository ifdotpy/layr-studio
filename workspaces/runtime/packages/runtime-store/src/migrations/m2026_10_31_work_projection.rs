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
        }
        let normalized = serde_json::to_string(&value)?;
        if normalized != raw {
            tx.execute(
                "UPDATE runtime_work SET record=?2 WHERE id=?1",
                (&id, normalized),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use studio_runtime_schema::work_item_record::WorkItem;

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

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn verify_work_projection_migration_on_copy() {
        let path = std::env::var_os("STUDIO_WORK_PROJECTION_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_work").unwrap() {
            println!("work table absent; work rows=0, refused=0");
            return;
        }
        apply(&tx).unwrap();
        super::super::m2026_10_31_work_records::apply(&tx).unwrap();
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let mut query = tx.prepare("SELECT id,record FROM runtime_work").unwrap();
        let rows = query
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap();
        let mut total = 0;
        let mut refused = 0;
        for row in rows {
            let (id, raw) = row.unwrap();
            total += 1;
            if !serde_json::from_str::<WorkItem>(&raw)
                .is_ok_and(|record| record.id().as_ref() == id)
            {
                refused += 1;
            }
        }
        println!("work rows={total}, refused={refused}");
        assert_eq!(refused, 0);
        drop(query);
        tx.commit().unwrap();
    }
}
