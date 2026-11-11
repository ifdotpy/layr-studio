//! Index the item/turn and successful-rule lookups held under the writer.
use crate::{StoreResult, Transaction};
use studio_db_api::Table;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    for index in crate::planning_work::TaskRow::INDEXES
        .iter()
        .chain(crate::item_monitor_terminal::MonitorsRow::INDEXES)
        .filter(|index| {
            matches!(
                index.name,
                "runtime_task_item_turn" | "runtime_monitor_rule_success"
            )
        })
    {
        tx.execute_batch(
            &index
                .sql
                .replacen("CREATE INDEX ", "CREATE INDEX IF NOT EXISTS ", 1),
        )?;
    }
    Ok(())
}
