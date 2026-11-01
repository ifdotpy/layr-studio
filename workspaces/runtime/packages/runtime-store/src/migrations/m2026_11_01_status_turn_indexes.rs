//! Drop the agent capacity and active indexes whose predicates read the agent
//! `status` column. The chat status indexes replace them, and the domain owner
//! creates those.
use crate::StoreResult;
use crate::domain::status_indexes_sql;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch(
        "DROP INDEX IF EXISTS runtime_agent_capacity_slots_v3;
         DROP INDEX IF EXISTS runtime_agent_active;",
    )?;
    tx.execute_batch(&status_indexes_sql())?;
    Ok(())
}
