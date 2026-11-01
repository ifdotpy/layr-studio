//! Remove retired fields from durable paired and discovered identities.
use crate::{StoreError, StoreResult};
use rusqlite::Transaction;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use studio_runtime_schema::multi_server_access::{
    AccessGrant, CLIENT_FIELDS, GRANT_OBSERVE, PersistedAccessClient,
};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for table in ["runtime_access_clients", "runtime_access_discovered"] {
        if !crate::schema::table_exists(tx, table)? {
            continue;
        }
        let mut statement = tx.prepare(&format!("SELECT id,record FROM {table} ORDER BY id"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (id, raw) in rows {
            let mut fields: BTreeMap<String, Box<RawValue>> =
                serde_json::from_str(&raw).map_err(|_| StoreError::InvalidStoredRecord {
                    object: table,
                    id: id.clone(),
                })?;
            fields.retain(|key, _| CLIENT_FIELDS.contains(&key.as_str()));
            if fields.get("grant").is_some_and(|grant| {
                serde_json::from_str::<Option<AccessGrant>>(grant.get()).is_err()
            }) {
                fields.insert(
                    "grant".into(),
                    RawValue::from_string(format!("\"{GRANT_OBSERVE}\"")).map_err(|_| {
                        StoreError::InvalidStoredRecord {
                            object: table,
                            id: id.clone(),
                        }
                    })?,
                );
            }
            let normalized =
                serde_json::to_string(&fields).map_err(|_| StoreError::InvalidStoredRecord {
                    object: table,
                    id: id.clone(),
                })?;
            let _: PersistedAccessClient =
                serde_json::from_str(&normalized).map_err(|_| StoreError::InvalidStoredRecord {
                    object: table,
                    id: id.clone(),
                })?;
            if normalized != raw {
                tx.execute(
                    &format!("UPDATE {table} SET record=?2 WHERE id=?1"),
                    (&id, &normalized),
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    #[test]
    fn preserves_pairing_fence_and_grant_when_removing_extensions() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_access_clients(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            CREATE TABLE runtime_access_discovered(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_access_clients VALUES('peer','{"id":"peer","clientId":"other-client-id","kind":"ui","label":"Phone","created":9007199254740993,"publicKey":"key","status":"paired","grant":"observe","pairingRequestId":"request","pairingHash":"hash","extra":1,"label":"Device"}');
            INSERT INTO runtime_access_discovered VALUES('other','{"id":"other","clientId":"other","kind":"server","label":"Peer","created":1,"publicKey":"key","status":"discovered","unknown":true}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let first: String = tx
            .query_row("SELECT record FROM runtime_access_clients", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(first.contains("\"grant\":\"observe\""));
        assert!(first.contains("\"clientId\":\"other-client-id\""));
        assert!(first.contains("\"pairingRequestId\":\"request\""));
        assert!(first.contains("\"pairingHash\":\"hash\""));
        assert!(first.contains("\"created\":9007199254740993"));
        assert!(first.contains("\"label\":\"Device\""));
        assert!(!first.contains("extra"));
        let changes = tx.total_changes();
        super::apply(&tx).unwrap();
        assert_eq!(changes, tx.total_changes());
    }

    #[test]
    fn keeps_credential_with_unknown_grant_at_observe() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_access_clients(id TEXT PRIMARY KEY,record TEXT NOT NULL);
            INSERT INTO runtime_access_clients VALUES('peer','{"id":"peer","clientId":"peer","kind":"ui","label":"Phone","created":1,"publicKey":"key","status":"paired","grant":"invalid"}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let record: String = tx
            .query_row("SELECT record FROM runtime_access_clients", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(record.contains("\"grant\":\"observe\""));
    }
}
