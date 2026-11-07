//! Remove old index dependencies before retiring the agent start, auto-wake and execution mode columns.
use crate::startup_recovery::DELETED_UNSETTLED_INDEX;
use crate::{StoreResult, repositories::agent_indexed_reads::ACTIVE_INDEX};
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !has_retired_columns(tx)? {
        return Ok(());
    }
    // Startup recreates the retained index names from their current owners.
    tx.execute_batch(&format!(
        "DROP INDEX IF EXISTS runtime_agent_account_wake_candidates_v5;
         DROP INDEX IF EXISTS runtime_agent_callback_context_v2;
         DROP INDEX IF EXISTS {DELETED_UNSETTLED_INDEX};
         DROP INDEX IF EXISTS {ACTIVE_INDEX};
         DROP INDEX IF EXISTS runtime_agent_account_wake_candidates_v6;
         DROP INDEX IF EXISTS runtime_agent_callback_context_v3;
         DROP INDEX IF EXISTS runtime_agent_vm_stream_owner;
         DROP INDEX IF EXISTS runtime_agent_thread;"
    ))?;
    Ok(())
}

pub(super) fn has_retired_columns(tx: &Transaction<'_>) -> StoreResult<bool> {
    for column in super::m2026_10_30_unused_agent_columns::RETIRED
        .into_iter()
        .chain(super::m2026_11_02_retired_status_columns::RETIRED)
    {
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo('runtime_agents') WHERE name=?1)",
            [column],
            |row| row.get(0),
        )?;
        if exists {
            return Ok(true);
        }
    }
    Ok(false)
}
