use crate::{
    StoreError, StoreResult,
    transfer_record::{TransferRecord, UnreadableTransferKind, UnreadableTransferSummary},
};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum StoredTransferHold {
    Unreadable { reason: String, original: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyOwner {
    lead_id: Option<String>,
    status: Option<String>,
}

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_account_transfers")? {
        return Ok(());
    }
    let mut query = tx.prepare("SELECT id,record FROM runtime_account_transfers ORDER BY rowid")?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, raw) in rows {
        if matches!(
            serde_json::from_str::<StoredTransferHold>(&raw),
            Ok(StoredTransferHold::Unreadable { .. })
        ) {
            continue;
        }
        let result = serde_json::from_str::<TransferRecord>(&raw).and_then(|record| {
            if record.id != id {
                return Err(serde::de::Error::custom("transfer id does not match row"));
            }
            Ok(record)
        });
        let reason = match result {
            Ok(record) => {
                let value = serde::Serialize::serialize(&(record), serde_json::value::Serializer)?;
                if studio_runtime_schema::validate("PersistedAccountTransferRecord", &value)
                    .map_err(StoreError::SchemaValidation)?
                {
                    continue;
                }
                "transfer does not match the persisted schema".to_owned()
            }
            Err(error) => error.to_string(),
        };
        let held = serde_json::to_string(&StoredTransferHold::Unreadable {
            reason: reason.clone(),
            original: raw.clone(),
        })?;
        tx.execute(
            "UPDATE runtime_account_transfers SET record=?2 WHERE id=?1",
            params![id, held],
        )?;
        if crate::schema::table_exists(tx, "runtime_agents")?
            && let Ok(LegacyOwner {
                lead_id: Some(lead),
                status,
            }) = serde_json::from_str::<LegacyOwner>(&raw)
        {
            let marker = serde_json::to_string(&UnreadableTransferSummary {
                kind: UnreadableTransferKind::Unreadable,
                reason,
                original: None,
            })?;
            tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.accountTransfer',json(?2)) WHERE id=?1 AND json_valid(record) AND (?3='pending' OR json_extract(record,'$.accountTransfer.id')=?4 OR json_extract(record,'$.accountTransferId')=?4)", params![lead, marker, status, id])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn transfer_operations_hold_unsupported_records_without_losing_payload() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_account_transfers(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);").unwrap();
        let raw = r#"{"id":"move","leadId":"lead","targetAccountKey":"target","status":"pending","created":1,"members":{},"futureField":true}"#;
        let valid = r#"{"id":"valid","leadId":"lead","targetAccountKey":"target","status":"pending","created":1,"members":{}}"#;
        db.execute(
            "INSERT INTO runtime_account_transfers VALUES('move',?1)",
            [raw],
        )
        .unwrap();
        db.execute(
            "INSERT INTO runtime_account_transfers VALUES('valid',?1)",
            [valid],
        )
        .unwrap();
        db.execute(
            "INSERT INTO runtime_agents VALUES('lead','{\"id\":\"lead\"}')",
            [],
        )
        .unwrap();
        let historical = r#"{"id":"old","leadId":"other","targetAccountKey":"target","status":"completed","created":1,"members":{},"futureField":true}"#;
        db.execute(
            "INSERT INTO runtime_account_transfers VALUES('old',?1)",
            [historical],
        )
        .unwrap();
        db.execute("INSERT INTO runtime_agents VALUES('other','{\"id\":\"other\",\"accountTransfer\":{\"id\":\"new\",\"status\":\"pending\"}}')", []).unwrap();
        let cancelled = r#"{"id":"cancelled","leadId":"other","targetAccountKey":"target","status":"cancelled","created":2,"members":{},"futureField":true}"#;
        db.execute(
            "INSERT INTO runtime_account_transfers VALUES('cancelled',?1)",
            [cancelled],
        )
        .unwrap();
        let owned = r#"{"id":"owned","leadId":"third","targetAccountKey":"target","status":"completed","created":3,"members":{},"futureField":true}"#;
        db.execute(
            "INSERT INTO runtime_account_transfers VALUES('owned',?1)",
            [owned],
        )
        .unwrap();
        db.execute("INSERT INTO runtime_agents VALUES('third','{\"id\":\"third\",\"accountTransfer\":{\"id\":\"owned\",\"status\":\"completed\"}}')", []).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        let (kind, original): (String, String) = tx.query_row("SELECT json_extract(record,'$.kind'), json_extract(record,'$.original') FROM runtime_account_transfers WHERE id='move'", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(kind, "unreadable");
        assert_eq!(original, raw);
        let retained: String = tx
            .query_row(
                "SELECT record FROM runtime_account_transfers WHERE id='valid'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retained, valid);
        assert_eq!(
            crate::AccountTransferRepository::recovery_ids(&tx).unwrap(),
            vec!["valid"]
        );
        let marker: String = tx.query_row("SELECT json_extract(record,'$.accountTransfer.kind') FROM runtime_agents WHERE id='lead'", [], |row| row.get(0)).unwrap();
        assert_eq!(marker, "unreadable");
        let old_original: String = tx
            .query_row(
                "SELECT record->>'$.original' FROM runtime_account_transfers WHERE id='old'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_original, historical);
        let current: String = tx
            .query_row(
                "SELECT record->>'$.accountTransfer.id' FROM runtime_agents WHERE id='other'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(current, "new");
        let cancelled_original: String = tx
            .query_row(
                "SELECT record->>'$.original' FROM runtime_account_transfers WHERE id='cancelled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cancelled_original, cancelled);
        let owned_kind: String = tx
            .query_row(
                "SELECT record->>'$.accountTransfer.kind' FROM runtime_agents WHERE id='third'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(owned_kind, "unreadable");
    }
}
