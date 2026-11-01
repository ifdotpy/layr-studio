use crate::StoreResult;
use rusqlite::Transaction;

/// Retire cached projections so the existing seed rebuilds them with the new status contract.
pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let legacy: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_entities e WHERE e.deleted=0 AND
         ((e.collection='messageReceipt' AND json_extract(e.payload,'$.value.status') IN ('pending','reserved','dispatching','accepted','sent')) OR
          (e.collection='queue' AND EXISTS(SELECT 1 FROM json_each(e.payload,'$.value.items') item WHERE json_extract(item.value,'$.status')='pending'))))",
        [], |row| row.get(0),
    )?;
    if !legacy {
        return Ok(());
    }
    tx.execute_batch(
        "UPDATE sync_generation SET value=MAX(value,COALESCE((SELECT MAX(seq) FROM sync_entities),0)) WHERE id=1;
         DELETE FROM sync_entities WHERE collection IN ('messageReceipt','queue');",
    )?;
    tx.execute(
        "DELETE FROM sync_entity_meta WHERE key=?1",
        [crate::SUMMARY_ENTITIES_SEED_MARKER],
    )?;
    Ok(())
}
