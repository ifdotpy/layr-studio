//! Releases lead holds left by completed VM account switches of the retired backend.
//! Those switches wrote a top-level `vm` flag that the closed transfer record rejected,
//! so the 2026-11-01 operations migration held each lead. A terminal record that decodes
//! again is restored. A lead is released only when all its held transfers are restored.
//! A pending transfer that stays unreadable keeps its lead on hold.
use crate::{
    StoreError, StoreResult,
    transfer_record::{TransferRecord, TransferStatus},
};
use rusqlite::{Transaction, params};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_account_transfers")? {
        return Ok(());
    }
    let rows = {
        let mut query =
            tx.prepare("SELECT id,record FROM runtime_account_transfers ORDER BY rowid")?;
        query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    // Lead id -> true while every held transfer of the lead is restored.
    let mut all_restored: BTreeMap<String, bool> = BTreeMap::new();
    for (id, raw) in rows {
        let Some(original) = serde_json::from_str::<Value>(&raw)
            .ok()
            .and_then(|held| held.get("original")?.as_str().map(str::to_owned))
        else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&original) else {
            continue;
        };
        let terminal = value
            .get("status")
            .and_then(|status| serde_json::from_value::<TransferStatus>(status.clone()).ok())
            .is_some_and(|status| {
                matches!(
                    status,
                    TransferStatus::Completed | TransferStatus::Cancelled
                )
            });
        let restorable = terminal
            && match serde_json::from_str::<TransferRecord>(&original) {
                Ok(record) if record.id == id => {
                    let encoded =
                        serde::Serialize::serialize(&record, serde_json::value::Serializer)?;
                    studio_runtime_schema::validate("PersistedAccountTransferRecord", &encoded)
                        .map_err(StoreError::SchemaValidation)?
                }
                _ => false,
            };
        if let Some(lead) = value.get("leadId").and_then(Value::as_str) {
            let entry = all_restored.entry(lead.to_owned()).or_insert(true);
            *entry &= restorable;
        }
        if restorable {
            tx.execute(
                "UPDATE runtime_account_transfers SET record=?2 WHERE id=?1",
                params![id, original],
            )?;
        }
    }
    for (lead, _) in all_restored.into_iter().filter(|(_, restored)| *restored) {
        tx.execute(
            "UPDATE runtime_agents SET record=json_remove(record,'$.accountTransfer') WHERE id=?1 AND json_valid(record) AND json_extract(record,'$.accountTransfer.kind')='unreadable' AND json_extract(record,'$.accountTransfer.original') IS NULL AND json_extract(record,'$.accountTransferId') IS NULL",
            params![lead],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn completed_legacy_vm_transfer_releases_its_lead_hold() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_account_transfers(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);").unwrap();
        let legacy = r#"{"id":"move","leadId":"lead","targetAccountKey":"target","targetProvider":"claude","status":"completed","scope":"team","created":1,"members":{},"requests":{},"vm":true,"updated":2}"#;
        let pending = r#"{"id":"next","leadId":"other","targetAccountKey":"target","status":"pending","scope":"team","created":3,"members":{},"vm":true}"#;
        let held = |original: &str| {
            serde_json::json!({"kind":"unreadable","reason":"unknown field `vm`","original":original})
                .to_string()
        };
        db.execute(
            "INSERT INTO runtime_account_transfers VALUES('move',?1)",
            [held(legacy)],
        )
        .unwrap();
        db.execute(
            "INSERT INTO runtime_account_transfers VALUES('next',?1)",
            [held(pending)],
        )
        .unwrap();
        db.execute("INSERT INTO runtime_agents VALUES('lead',?1)", [r#"{"id":"lead","accountTransfer":{"kind":"unreadable","reason":"unknown field `vm`"}}"#]).unwrap();
        db.execute("INSERT INTO runtime_agents VALUES('other',?1)", [r#"{"id":"other","accountTransfer":{"kind":"unreadable","reason":"unknown field `vm`"}}"#]).unwrap();
        {
            let tx = db.transaction().unwrap();
            super::apply(&tx).unwrap();
            super::apply(&tx).unwrap();
            tx.commit().unwrap();
        }
        let restored: String = db
            .query_row(
                "SELECT record FROM runtime_account_transfers WHERE id='move'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restored, legacy);
        let lead: String = db
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='lead'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!lead.contains("accountTransfer"), "{lead}");
        let other: String = db
            .query_row("SELECT json_extract(record,'$.accountTransfer.kind') FROM runtime_agents WHERE id='other'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(other, "unreadable");
        let still_held: String = db
            .query_row("SELECT json_extract(record,'$.kind') FROM runtime_account_transfers WHERE id='next'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(still_held, "unreadable");
    }
}
