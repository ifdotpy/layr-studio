//! Give older invitations with invalid creation times the Unix epoch.
use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if crate::schema::table_exists(tx, "runtime_access_invites")? {
        tx.execute(
            "UPDATE runtime_access_invites SET record=json_set(record,'$.created',0) WHERE CASE WHEN json_valid(record) THEN json_type(record,'$.created') IS NULL OR json_type(record,'$.created') NOT IN ('integer','real') OR json_extract(record,'$.created')<0 ELSE 0 END",
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalizes_invalid_times_and_preserves_fractional_seconds() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_access_invites(id TEXT PRIMARY KEY, record TEXT NOT NULL); INSERT INTO runtime_access_invites VALUES ('missing','{}'),('null','{\"created\":null}'),('float','{\"created\":1.25}'),('negative','{\"created\":-1}'),('text','{\"created\":\"old\"}');").unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let values: Vec<f64> = tx
            .prepare(
                "SELECT json_extract(record,'$.created') FROM runtime_access_invites ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(values, [1.25, 0.0, 0.0, 0.0, 0.0]);
        let changes = tx.total_changes();
        super::apply(&tx).unwrap();
        assert_eq!(changes, tx.total_changes());
    }
}
