use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut query = tx.prepare("SELECT id, record FROM runtime_agents WHERE json_valid(record) AND json_type(record, '$.turnId') = 'null'")?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, raw) = row?;
        let mut record: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(&raw)?;
        record.remove("turnId");
        tx.execute(
            "UPDATE runtime_agents SET record=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&record)?, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn removes_only_null_turn_identifiers() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT);
            INSERT INTO runtime_agents VALUES ('null', '{"turnId":null,"large":1e40}'), ('missing', '{}'), ('value', '{"turnId":"t"}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let read = |id: &str| {
            tx.query_row("SELECT record FROM runtime_agents WHERE id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
        };
        assert_eq!(read("null"), r#"{"large":1e40}"#);
        assert_eq!(read("missing"), "{}");
        assert_eq!(read("value"), r#"{"turnId":"t"}"#);
    }
}
