use crate::StoreResult;
use rusqlite::Transaction;
use serde_json::Value;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_annotations")? {
        return Ok(());
    }
    let mut query = tx.prepare("SELECT id,record FROM runtime_annotations")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let raw: String = row.get(1)?;
        let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let original = fields.clone();
        fields.retain(|key, value| match key.as_str() {
            "id" => true,
            "agent" | "rootId" | "path" | "text" | "turnId" => value.is_null() || value.is_string(),
            "created" => value.is_null() || value.is_number(),
            "line" => studio_runtime_schema::annotation_record::valid_line(value),
            _ => false,
        });
        if fields != original {
            let normalized = serde_json::to_string(&fields)?;
            tx.execute(
                "UPDATE runtime_annotations SET record=?2 WHERE id=?1",
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
    use studio_runtime_schema::annotation_record::AnnotationRecord;

    #[test]
    fn preserves_already_valid_row_bytes() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_annotations(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE writes(id TEXT); CREATE TRIGGER log_write AFTER UPDATE ON runtime_annotations BEGIN INSERT INTO writes VALUES(new.id); END;").unwrap();
        let original = r#"{ "text": "x", "id": "a" }"#;
        db.execute("INSERT INTO runtime_annotations VALUES('a',?1)", [original])
            .unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        assert_eq!(
            tx.query_row("SELECT record FROM runtime_annotations", [], |r| r
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
    fn normalizes_annotations_idempotently() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_annotations(id TEXT PRIMARY KEY,record TEXT NOT NULL);
        INSERT INTO runtime_annotations VALUES('a','{"id":"a","rootId":"r","turnId":null,"text":"x","extra":1,"line":1.5}');"#).unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        let first: String = tx
            .query_row("SELECT record FROM runtime_annotations", [], |row| {
                row.get(0)
            })
            .unwrap();
        apply(&tx).unwrap();
        let second: String = tx
            .query_row("SELECT record FROM runtime_annotations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(first, second);
        let record: AnnotationRecord = serde_json::from_str(&first).unwrap();
        assert_eq!(record.value()["rootId"], "r");
        assert!(record.value()["turnId"].is_null());
        assert!(record.value().get("extra").is_none());
        assert!(record.value().get("line").is_none());
    }

    #[test]
    #[ignore = "manual verification on a disposable reflink copy"]
    fn verify_annotation_migration_on_copy() {
        let path = std::env::var_os("STUDIO_ANNOTATION_MIGRATION_COPY").unwrap();
        let mut db = Connection::open(path).unwrap();
        crate::schema::register_python_trigger_functions(&db).unwrap();
        let tx = db.transaction().unwrap();
        if !crate::schema::table_exists(&tx, "runtime_annotations").unwrap() {
            println!("annotation table absent; rows=0, refused=0");
            return;
        }
        apply(&tx).unwrap();
        apply(&tx).unwrap();
        let mut statement = tx
            .prepare("SELECT id,record FROM runtime_annotations")
            .unwrap();
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        let mut total = 0;
        let mut refused = 0;
        for row in rows {
            let (id, raw) = row.unwrap();
            total += 1;
            if !serde_json::from_str::<AnnotationRecord>(&raw)
                .is_ok_and(|record| record.id() == &id)
            {
                refused += 1;
            }
        }
        println!("annotation rows={total}, refused={refused}");
        assert_eq!(refused, 0);
        drop(statement);
        tx.commit().unwrap();
    }
}
