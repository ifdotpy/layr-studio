//! Serving indexes installed after the released F1 authority switch.
use crate::{StoreResult, Transaction};
pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::queue_schema::install_index::<crate::agent_tables::AgentsStorageRow>(
        tx,
        "runtime_agent_limit_recovery",
    )?;
    crate::queue_schema::install_index::<crate::queue_schema::ExecutionRunsRow>(
        tx,
        "runtime_execution_run_terminal",
    )?;
    crate::queue_schema::install_index::<crate::item_monitor_terminal::MonitorsRow>(
        tx,
        "runtime_monitor_window_status",
    )?;
    Ok(())
}
