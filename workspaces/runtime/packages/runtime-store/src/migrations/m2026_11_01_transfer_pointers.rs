//! Normalize transfer pointers before the strict AgentRecord decoder is used.
use crate::{
    StoreResult,
    transfer_record::{
        LazyTransfer, LazyTransferState, TransferId, UnreadableTransferKind,
        UnreadableTransferSummary,
    },
};
use rusqlite::{Transaction, params};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_agents")? {
        return Ok(());
    }
    let mut statement = tx.prepare("SELECT id,record->'$.accountTransferId',record->'$.lazyAccountTransfer' FROM runtime_agents WHERE json_valid(record) AND (json_type(record,'$.accountTransferId') NOT IN ('null') OR json_type(record,'$.lazyAccountTransfer') NOT IN ('null'))")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let (mut normalized, mut unreadable) = (0, 0);
    for (id, pointer, lazy) in rows {
        let mut failure = None;
        if let Some(raw) = pointer.as_deref().filter(|raw| *raw != "null") {
            match serde_json::from_str::<TransferId>(raw) {
                Ok(value) => {
                    let value = serde_json::to_string(&value)?;
                    tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.accountTransferId',json(?2)) WHERE id=?1",params![id,value])?;
                }
                Err(error) => {
                    failure = Some(error.to_string());
                    tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.accountTransferId',NULL) WHERE id=?1",[&id])?;
                }
            }
        }
        if let Some(raw) = lazy.as_deref().filter(|raw| *raw != "null") {
            let value = match serde_json::from_str::<LazyTransfer>(raw) {
                Ok(value) => LazyTransferState::Pending(value),
                Err(error) => match serde_json::from_str::<UnreadableTransferSummary>(raw) {
                    Ok(value) => LazyTransferState::Unreadable(value),
                    Err(_) => {
                        let reason = error.to_string();
                        failure = Some(reason.clone());
                        LazyTransferState::Unreadable(UnreadableTransferSummary {
                            kind: UnreadableTransferKind::Unreadable,
                            reason,
                        })
                    }
                },
            };
            let value = serde_json::to_string(&value)?;
            tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.lazyAccountTransfer',json(?2)) WHERE id=?1",params![id,value])?;
        }
        if let Some(reason) = failure {
            unreadable += 1;
            eprintln!("account transfer pointer migration: agent={id} reason={reason}");
            let marker = serde_json::to_string(&UnreadableTransferSummary {
                kind: UnreadableTransferKind::Unreadable,
                reason,
            })?;
            tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.accountTransfer',json(?2)) WHERE id=?1",params![id,marker])?;
        } else {
            normalized += 1;
        }
    }
    eprintln!(
        "account transfer pointer migration: normalized={normalized} unreadable={unreadable}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "run against a read-only copied graph"]
    fn copied_pointer_corpus_normalizes_without_loss() {
        let source = rusqlite::Connection::open_with_flags(
            std::env::var("STUDIO_TRANSFER_CORPUS").unwrap(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL)")
            .unwrap();
        let mut query = source.prepare("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND (json_type(record,'$.accountTransferId') NOT IN ('null') OR json_type(record,'$.lazyAccountTransfer') NOT IN ('null'))").unwrap();
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        let mut originals = Vec::new();
        for row in rows {
            let (id, raw) = row.unwrap();
            originals.push((
                id.clone(),
                serde_json::from_str::<serde_json::Value>(&raw).unwrap(),
            ));
            db.execute(
                "INSERT INTO runtime_agents VALUES(?1,?2)",
                rusqlite::params![id, raw],
            )
            .unwrap();
        }
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        for (id, before) in &originals {
            let raw: String = tx
                .query_row(
                    "SELECT record FROM runtime_agents WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            let after: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert!(
                after == *before,
                "migration changed a known transfer pointer"
            );
        }
        eprintln!(
            "copied pointer corpus: total={} unchanged={}",
            originals.len(),
            originals.len()
        );
        tx.rollback().unwrap();
    }

    #[test]
    fn unknown_lazy_field_becomes_visible_unreadable_hold() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL); INSERT INTO runtime_agents VALUES('lead','{"id":"lead","accountTransferId":"move","lazyAccountTransfer":{"id":"move","sourceAccountKey":"source","unexpected":true}}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let raw: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='lead'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["accountTransferId"], "move");
        assert_eq!(value["accountTransfer"]["kind"], "unreadable");
        assert_eq!(value["lazyAccountTransfer"]["kind"], "unreadable");
    }
}
