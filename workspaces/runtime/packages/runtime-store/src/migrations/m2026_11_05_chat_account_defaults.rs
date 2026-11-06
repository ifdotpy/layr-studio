use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute(
        "UPDATE runtime_agents SET record = json_remove(record, '$.workerDefaults.accountKey', '$.reviewDefaults.accountKey') WHERE json_valid(record) AND (json_type(record, '$.workerDefaults.accountKey') IS NOT NULL OR json_type(record, '$.reviewDefaults.accountKey') IS NOT NULL)",
        [],
    )?;
    Ok(())
}
