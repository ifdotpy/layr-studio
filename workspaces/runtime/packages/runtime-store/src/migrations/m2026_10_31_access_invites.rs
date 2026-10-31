//! Normalize legacy access invitations to the strict stored owner.
use crate::{StoreError, StoreResult};
use rusqlite::Transaction;
use studio_runtime_schema::multi_server_protocol::{
    ACCESS_PROTOCOL_VERSION, INVITE_FIELDS, PersistedAccessInvite,
};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_access_invites")? {
        return Ok(());
    }
    let mut statement = tx.prepare("SELECT id,record FROM runtime_access_invites ORDER BY id")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (id, raw) in rows {
        let invalid = || StoreError::InvalidStoredRecord {
            object: "runtime_access_invites",
            id: id.clone(),
        };
        let mut value: serde_json::Value = serde_json::from_str(&raw).map_err(|_| invalid())?;
        value
            .as_object_mut()
            .ok_or_else(invalid)?
            .retain(|key, _| INVITE_FIELDS.contains(&key.as_str()));
        if value
            .get("protocol")
            .and_then(serde_json::Value::as_number)
            .is_some_and(|n| n.is_f64() && n.as_f64() == Some(ACCESS_PROTOCOL_VERSION as f64))
        {
            value["protocol"] = serde_json::Value::from(ACCESS_PROTOCOL_VERSION);
        }
        let normalized = serde_json::to_string(&value)?;
        let invite: PersistedAccessInvite =
            serde_json::from_str(&normalized).map_err(|_| invalid())?;
        if invite.invite_id() != id {
            return Err(invalid());
        }
        if normalized != raw {
            tx.execute(
                "UPDATE runtime_access_invites SET record=?2 WHERE id=?1",
                (&id, &normalized),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn removes_unknown_fields_and_duplicate_keys_idempotently() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(&format!(r#"CREATE TABLE runtime_access_invites(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_access_invites VALUES('i','{{"inviteId":"i","status":"open","expires":1,"tokenHash":"{}","tailscaleUser":"owner","protocol":1.0,"grant":"observe","unknown":1,"label":"old","label":"new"}}');"#, "a".repeat(64))).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let first: String = tx
            .query_row("SELECT record FROM runtime_access_invites", [], |row| {
                row.get(0)
            })
            .unwrap();
        super::apply(&tx).unwrap();
        let second: String = tx
            .query_row("SELECT record FROM runtime_access_invites", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(first, second);
        assert!(!first.contains("unknown"));
        assert!(first.contains("\"label\":\"new\""));
        assert!(first.contains("\"grant\":\"observe\""));
        let value: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(
            value["protocol"].as_u64(),
            Some(super::ACCESS_PROTOCOL_VERSION)
        );
    }

    #[test]
    #[ignore = "manual verification on disposable reflink copies"]
    fn verifies_disposable_database_copies() {
        use studio_runtime_schema::multi_server_protocol::PersistedAccessInvite;
        for variable in ["STUDIO_INVITE_CANVAS_COPY", "STUDIO_INVITE_ANALYTICS_COPY"] {
            let path = std::env::var_os(variable).expect("copy path");
            let mut db = Connection::open(path).expect("open copy");
            let tx = db.transaction().expect("transaction");
            super::apply(&tx).expect("migrate invitations");
            let count = if crate::schema::table_exists(&tx, "runtime_access_invites").unwrap() {
                let mut statement = tx
                    .prepare("SELECT id,record FROM runtime_access_invites")
                    .unwrap();
                let records = statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .unwrap();
                let mut count = 0;
                for record in records {
                    let (id, raw) = record.unwrap();
                    let invite: PersistedAccessInvite =
                        serde_json::from_str(&raw).expect("migrated invitation");
                    assert_eq!(invite.invite_id(), id);
                    count += 1;
                }
                count
            } else {
                0
            };
            let changes = tx.total_changes();
            super::apply(&tx).expect("repeat migration");
            assert_eq!(tx.total_changes(), changes, "migration must be idempotent");
            tx.commit().expect("commit copy");
            println!("{variable}: {count} records, zero refused");
        }
    }
}
