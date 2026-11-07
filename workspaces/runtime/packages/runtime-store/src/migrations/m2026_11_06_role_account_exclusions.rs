use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let records = {
        let mut query = tx.prepare("SELECT id, json_extract(record, '$.excludedAccountKeys') FROM runtime_agents WHERE json_valid(record) AND json_type(record, '$.excludedAccountKeys') IS NOT NULL")?;
        query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (id, raw) in records {
        let keys: Vec<String> = match raw {
            Some(raw) => studio_json::from_str(&raw)?,
            None => Vec::new(),
        };
        let valid: Vec<studio_runtime_schema::account_key::AccountKey> = keys
            .into_iter()
            .filter_map(|key| key.try_into().ok())
            .collect();
        let json = studio_json::to_string(&valid)?;
        tx.execute("UPDATE runtime_agents SET record = json_remove(json_set(record, '$.workerExcludedAccountKeys', json(?1), '$.reviewerExcludedAccountKeys', json(?1)), '$.excludedAccountKeys') WHERE id=?2", rusqlite::params![json, id])?;
    }
    Ok(())
}
