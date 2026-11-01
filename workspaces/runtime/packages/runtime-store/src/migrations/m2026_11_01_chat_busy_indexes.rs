//! Drop the agent wake and active indexes whose predicates read `in_flight`. Their
//! owners recreate them under the same names without that column, and the
//! migration runner recreates them after this step.
use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch(
        "DROP INDEX IF EXISTS runtime_agent_account_wake_candidates_v6;
         DROP INDEX IF EXISTS runtime_agent_active;",
    )?;
    Ok(())
}
