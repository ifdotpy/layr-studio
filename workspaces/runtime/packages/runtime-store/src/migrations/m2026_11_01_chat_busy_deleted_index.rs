//! Drop the deleted-unsettled agent index: its predicate read the agent `in_flight`
//! column. Startup recreates it under the same name without that column.
use crate::StoreResult;
use crate::startup_recovery::DELETED_UNSETTLED_INDEX;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !super::m2026_10_31_retired_start_indexes::has_retired_columns(tx)? {
        return Ok(());
    }
    tx.execute_batch(&format!("DROP INDEX IF EXISTS {DELETED_UNSETTLED_INDEX};"))?;
    Ok(())
}
