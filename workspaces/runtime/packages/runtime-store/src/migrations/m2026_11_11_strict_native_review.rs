//! Retire non-conforming native-review markers only on deleted agents
//! (deleted means a non-zero deletedAt, as in `AgentRecord::is_deleted`).
use crate::{StoreResult, Transaction};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut statement = tx.prepare("SELECT id,record->'$.nativeReview',coalesce(json_extract(record,'$.deletedAt'),0)!=0 FROM runtime_agents WHERE json_type(record,'$.nativeReview') IS NOT NULL AND json_type(record,'$.nativeReview')!='null' ORDER BY id")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<bool>>(2)?.unwrap_or(false),
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut retired = Vec::new();
    for (id, raw, deleted) in rows {
        if studio_json::from_str::<crate::StoredNativeReview>(&raw).is_ok() {
            continue;
        }
        if !deleted {
            return Err(crate::StoreError::SchemaValidation(format!(
                "invalid stored object runtime_agents/{id} at /nativeReview: marker does not match the current writer"
            )));
        }
        retired.push((id, raw));
    }
    for (id, raw) in retired {
        tx.execute("UPDATE runtime_agents SET record=json_remove(record,'$.nativeReview') WHERE id=?1 AND coalesce(json_extract(record,'$.deletedAt'),0)!=0 AND record->'$.nativeReview'=?2", rusqlite::params![id, raw])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn strict_native_review_migration_drops_only_deleted_legacy_marker() {
        // Live deleted agents carry a non-zero deletedAt without a deleted flag.
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        tx.execute("INSERT INTO runtime_agents(id,record) VALUES('deleted-review',?1)",
            [r#"{"id":"deleted-review","rootId":"deleted-review","epoch":1,"status":"paused","accountKey":"default","provider":"codex","deletedAt":1.0,"nativeReview":{"status":"started"},"name":"preserved"}"#]).unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let record: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='deleted-review'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let agent: crate::AgentRecord = studio_json::from_str(&record).unwrap();
        assert!(agent.native_review.is_missing());
        assert_eq!(agent.name.as_deref(), Some("preserved"));
        assert!(agent.is_deleted());
    }

    #[test]
    fn strict_native_review_migration_refuses_active_legacy_marker() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().unwrap();
        let record = r#"{"id":"active-review","rootId":"active-review","epoch":1,"status":"paused","accountKey":"default","provider":"codex","nativeReview":{"status":"started"}}"#;
        tx.execute(
            "INSERT INTO runtime_agents(id,record) VALUES('active-review',?1)",
            [record],
        )
        .unwrap();
        let error = super::apply(&tx).unwrap_err().to_string();
        assert!(
            error.contains("active-review") && error.contains("nativeReview"),
            "{error}"
        );
        let stored: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='active-review'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, record);
    }
}
