use crate::StoreResult;
use rusqlite::Transaction;
use serde_json::Value;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_plans")? {
        return Ok(());
    }
    let mut query = tx.prepare("SELECT id,record FROM runtime_plans")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let raw: String = row.get(1)?;
        let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        fields.retain(|key, _| {
            matches!(
                key.as_str(),
                "id" | "rootId" | "text" | "version" | "updated" | "agent" | "native" | "steps"
            )
        });
        if fields.get("steps").is_some_and(|value| !value.is_array()) {
            fields.remove("steps");
        }
        if fields
            .get("agent")
            .is_some_and(|value| !value.is_null() && !value.is_string())
        {
            fields.remove("agent");
        }
        if let Some(native) = fields.get_mut("native") {
            if let Some(object) = native.as_object_mut() {
                object.retain(|key, value| match key.as_str() {
                    "threadId" | "turnId" | "explanation" => value.is_null() || value.is_string(),
                    "plan" => value.is_array(),
                    _ => false,
                });
            } else if !native.is_null() {
                fields.remove("native");
            }
        }
        let normalized = serde_json::to_string(&fields)?;
        if normalized != raw {
            tx.execute(
                "UPDATE runtime_plans SET record=?2 WHERE id=?1",
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
    use studio_runtime_schema::plan_record::PlanRecord;

    #[test]
    fn removes_unused_fields_and_is_idempotent() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_plans(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_plans VALUES('a','{"id":"a","rootId":"r","text":"x","version":1,"updated":null,"unused":1,"native":{"threadId":"t","extension":1,"plan":[{"step":"x"}]}}');"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        let first: String = tx
            .query_row("SELECT record FROM runtime_plans", [], |row| row.get(0))
            .unwrap();
        apply(&tx).unwrap();
        let second: String = tx
            .query_row("SELECT record FROM runtime_plans", [], |row| row.get(0))
            .unwrap();
        assert_eq!(first, second);
        let record: PlanRecord = serde_json::from_str(&first).unwrap();
        assert_eq!(record.value()["native"]["threadId"], "t");
        assert!(record.value().get("unused").is_none());
        assert!(record.value()["native"].get("extension").is_none());
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn verify_plan_migration_on_copy() {
        let path = std::env::var_os("STUDIO_PLAN_MIGRATION_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_plans").unwrap() {
            println!("plan table absent; rows=0, refused=0");
            return;
        }
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let mut statement = tx.prepare("SELECT record FROM runtime_plans").unwrap();
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap();
        let mut total = 0;
        let mut refused = 0;
        for row in rows {
            total += 1;
            if serde_json::from_str::<PlanRecord>(&row.unwrap()).is_err() {
                refused += 1;
            }
        }
        println!("plan rows={total}, refused={refused}");
        assert_eq!(refused, 0);
        drop(statement);
        tx.commit().unwrap();
    }
}
