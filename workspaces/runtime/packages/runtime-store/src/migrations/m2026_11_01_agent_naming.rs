use crate::StoreResult;
use rusqlite::Transaction;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut query = tx.prepare("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND (json_type(record,'$.manualName') IS NOT NULL OR json_type(record,'$.needsTitle') IS NOT NULL OR json_type(record,'$.naming') IS NULL)")?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut changed = false;
    for row in rows {
        let (id, raw) = row?;
        let mut record: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&raw)?;
        let manual = record.remove("manualName");
        let title = record.remove("needsTitle");
        if manual.is_none() && title.is_none() && record.contains_key("naming") {
            continue;
        }
        let is_true = |v: Option<&Box<RawValue>>| v.is_some_and(|v| v.get() == "true");
        let naming = if is_true(manual.as_ref()) {
            "manual"
        } else if is_true(title.as_ref()) {
            "pending_title"
        } else {
            "automatic"
        };
        record.insert(
            "naming".into(),
            RawValue::from_string(serde_json::to_string(naming)?)?,
        );
        tx.execute(
            "UPDATE runtime_agents SET record=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&record)?, id],
        )?;
        changed = true;
    }
    if changed && crate::schema::table_exists(tx, "sync_entity_meta")? {
        tx.execute(
            "DELETE FROM sync_entity_meta WHERE key=?1",
            [crate::AGENT_PROJECTIONS_SEED_MARKER],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    #[test]
    fn naming_migration_normalizes_legacy_flags() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL)")
            .unwrap();
        for (id, source, expected) in [
            (
                "automatic",
                r#"{"id":"automatic","needsTitle":false}"#,
                "automatic",
            ),
            (
                "pending",
                r#"{"id":"pending","needsTitle":true}"#,
                "pending_title",
            ),
            (
                "manual",
                r#"{"id":"manual","manualName":true,"needsTitle":false}"#,
                "manual",
            ),
            (
                "contradictory",
                r#"{"id":"contradictory","manualName":true,"needsTitle":true}"#,
                "manual",
            ),
        ] {
            db.execute("INSERT INTO runtime_agents VALUES(?1,?2)", (id, source))
                .unwrap();
            let tx = db.transaction().unwrap();
            super::apply(&tx).unwrap();
            super::apply(&tx).unwrap();
            let raw: String = tx
                .query_row("SELECT record FROM runtime_agents WHERE id=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            let record: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(record["naming"], expected, "{id}");
            assert!(record.get("manualName").is_none() && record.get("needsTitle").is_none());
            tx.commit().unwrap();
        }
    }
}
