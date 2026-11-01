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
        let normalized = serde_json::to_string(&fields)?;
        if fields != original
            || (serde_json::from_str::<studio_runtime_schema::annotation_record::AnnotationRecord>(&raw).is_err()
                && serde_json::from_str::<studio_runtime_schema::annotation_record::AnnotationRecord>(&normalized).is_ok())
        {
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
    fn normalizes_duplicate_fields_for_typed_decode() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE runtime_annotations(id TEXT PRIMARY KEY,record TEXT NOT NULL);",
        )
        .unwrap();
        db.execute(
            "INSERT INTO runtime_annotations VALUES('a',?1)",
            [r#"{"id":"a","text":"old","text":"new"}"#],
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        apply(&tx).unwrap();
        let raw: String = tx
            .query_row("SELECT record FROM runtime_annotations", [], |r| r.get(0))
            .unwrap();
        let annotation: AnnotationRecord = serde_json::from_str(&raw).unwrap();
        assert_eq!(annotation.value()["text"], "new");
    }

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
}
