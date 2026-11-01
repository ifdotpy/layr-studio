//! Discard the old derived account snapshot; startup rebuilds it from the registry.
use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute("DELETE FROM sync_entities WHERE collection='accounts' AND id='current' AND CASE WHEN NOT json_valid(payload) THEN 1 WHEN json_type(payload,'$.value.accounts') IS NOT 'array' THEN 1 ELSE EXISTS (SELECT 1 FROM json_each(payload,'$.value.accounts') WHERE CASE WHEN type != 'object' THEN 1 ELSE json_type(value,'$.eligibility') IS NULL END) END", [])?;
    Ok(())
}
