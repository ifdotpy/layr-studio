//! Atomic persistence for question and complaint mutations.

use crate::interaction_tables::ComplaintRow;
use crate::json_record_codec::RawJsonRecord;
use crate::peer_update_settings::canvas_tx;
use rusqlite::{OptionalExtension, Transaction};

use crate::StoreResult;
use crate::schema::{ConditionalTableRequirement, SchemaRequirements, TableRequirement};

const COMPLAINTS: &[TableRequirement] = &[TableRequirement {
    name: "runtime_complaints",
    columns: &["id", "record"],
}];

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[ConditionalTableRequirement {
        table_exists: "runtime_complaints",
        tables: COMPLAINTS,
    }],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Clone, Copy, Debug, Default)]
pub struct QuestionsWriteRepository;

impl QuestionsWriteRepository {
    pub fn complaint(transaction: &Transaction<'_>, id: &str) -> StoreResult<Option<String>> {
        // Raw fingerprint reads intentionally bypass typed validation for change detection.
        Ok(transaction
            .query_row(
                "SELECT record FROM runtime_complaints WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn put_complaint(transaction: &Transaction<'_>, id: &str, record: &str) -> StoreResult<()> {
        let typed: crate::ComplaintRecord = studio_json::from_str(record)?;
        if typed.id() != id {
            return Err(crate::StoreError::InvalidStoredObject {
                table: "runtime_complaints",
                id: id.to_owned(),
            });
        }
        canvas_tx(transaction).upsert(&ComplaintRow {
            id: id.to_owned(),
            record: RawJsonRecord::from_stored(transaction, id, record.to_owned())?,
        })?;
        Ok(())
    }

    pub fn put_typed_complaint(
        transaction: &Transaction<'_>,
        record: &crate::ComplaintRecord,
    ) -> StoreResult<()> {
        canvas_tx(transaction).upsert(&ComplaintRow {
            id: record.id().to_owned(),
            record: RawJsonRecord::from_record(transaction, record)?,
        })?;
        Ok(())
    }
}

inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "questions_write",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod complaint_tests {
    use super::QuestionsWriteRepository;
    use crate::ComplaintRecord;
    use rusqlite::Connection;

    #[test]
    fn typed_complaint_write_uses_native_json() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE runtime_complaints(id TEXT PRIMARY KEY,record TEXT NOT NULL)",
        )
        .unwrap();
        let raw = r#"{"id":"c","leadId":"l","author":"user","text":"é😀","recipient":"lead","version":1,"status":"open","created":1.0,"updated":1.0,"readAt":null,"responses":[]}"#;
        let complaint: ComplaintRecord = studio_json::from_str(raw).unwrap();
        let tx = db.transaction().unwrap();
        QuestionsWriteRepository::put_typed_complaint(&tx, &complaint).unwrap();
        let stored: String = tx
            .query_row(
                "SELECT record FROM runtime_complaints WHERE id='c'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stored.contains("é😀"));
        assert_eq!(
            studio_json::from_str::<ComplaintRecord>(&stored)
                .unwrap()
                .id(),
            "c"
        );
    }
}
