use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_agents")? {
        return Ok(());
    }
    let mut query = tx.prepare("SELECT id,record->'$.accountTransfer' FROM runtime_agents WHERE json_valid(record) AND json_type(record,'$.accountTransfer') NOT IN ('null')")?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let (mut normalized, mut unreadable) = (0, 0);
    for (id, raw) in rows {
        let state: crate::transfer_record::AgentTransferState = match serde_json::from_str::<
            crate::transfer_record::AgentTransferSummary,
        >(&raw)
        {
            Ok(summary) => {
                normalized += 1;
                crate::transfer_record::AgentTransferState::Summary(Box::new(summary))
            }
            Err(error) => {
                if let Ok(existing) =
                    serde_json::from_str::<crate::transfer_record::UnreadableTransferSummary>(&raw)
                {
                    normalized += 1;
                    let serialized = serde_json::to_string(&existing)?;
                    tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.accountTransfer',json(?2)) WHERE id=?1", rusqlite::params![id, serialized])?;
                    continue;
                }
                unreadable += 1;
                eprintln!("account transfer summary migration: agent={id} reason={error}");
                crate::transfer_record::AgentTransferState::Unreadable(
                    crate::transfer_record::UnreadableTransferSummary {
                        kind: crate::transfer_record::UnreadableTransferKind::Unreadable,
                        reason: error.to_string(),
                        original: None,
                    },
                )
            }
        };
        let normalized = serde_json::to_string(&state)?;
        tx.execute("UPDATE runtime_agents SET record=json_set(record,'$.accountTransfer',json(?2)) WHERE id=?1", rusqlite::params![id, normalized])?;
    }
    eprintln!(
        "account transfer summary migration: normalized={normalized} unreadable={unreadable}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "run against a disposable copy of the graph database"]
    fn copied_summary_corpus_migrates() {
        let path = std::env::var("STUDIO_TRANSFER_CORPUS").unwrap();
        let source =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL)")
            .unwrap();
        let mut query = source.prepare("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND json_type(record,'$.accountTransfer') NOT IN ('null')").unwrap();
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        let mut original = Vec::new();
        for row in rows {
            let (id, record) = row.unwrap();
            let summary =
                serde_json::from_str::<serde_json::Value>(&record).unwrap()["accountTransfer"]
                    .clone();
            original.push((id.clone(), summary));
            db.execute(
                "INSERT INTO runtime_agents VALUES(?1,?2)",
                rusqlite::params![id, record],
            )
            .unwrap();
        }
        let tx = db.transaction().unwrap();
        let total: i64 = tx.query_row("SELECT count(*) FROM runtime_agents WHERE json_valid(record) AND json_type(record,'$.accountTransfer') NOT IN ('null')", [], |row| row.get(0)).unwrap();
        super::apply(&tx).unwrap();
        let unreadable: i64 = tx.query_row("SELECT count(*) FROM runtime_agents WHERE json_valid(record) AND json_extract(record,'$.accountTransfer.kind')='unreadable'", [], |row| row.get(0)).unwrap();
        eprintln!(
            "copied summary corpus: total={total} normalized={} unreadable={unreadable}",
            total - unreadable
        );
        assert_eq!(unreadable, 0);
        for (id, summary) in original {
            let migrated: String = tx
                .query_row(
                    "SELECT record->'$.accountTransfer' FROM runtime_agents WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(
                serde_json::from_str::<serde_json::Value>(&migrated).unwrap() == summary,
                "migration changed a known summary"
            );
        }
        tx.rollback().unwrap();
    }

    #[test]
    fn unknown_transfer_summary_becomes_unreadable_without_losing_agent() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL);
            INSERT INTO runtime_agents VALUES ('lead','{"id":"lead","name":"Kept","accountTransfer":{"unknownField":true}}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let value: String = tx
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='lead'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let agent: serde_json::Value = serde_json::from_str(&value).unwrap();
        assert_eq!(agent["name"], "Kept");
        assert_eq!(agent["accountTransfer"]["kind"], "unreadable");
        assert!(
            agent["accountTransfer"]["reason"]
                .as_str()
                .unwrap()
                .contains("unknownField")
        );
    }
}
