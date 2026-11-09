//! Remove approved obsolete budget state and empty, unreachable delivery tables.
use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch("DROP TABLE IF EXISTS runtime_budget; DROP TRIGGER IF EXISTS summary_plan_delete; DROP TRIGGER IF EXISTS summary_annotation_delete; DROP TRIGGER IF EXISTS summary_questionHistory_delete;")?;
    // Historical migrations already moved delivery authority to runtime_outbox.
    // Nonempty leftovers can contain unknown-effect evidence; keep them.
    for table in [
        "runtime_server_outbox",
        "runtime_federation_outbox",
        "runtime_access_outbound",
        "runtime_server_fetch",
    ] {
        if crate::schema::table_exists(tx, table)? {
            let empty: bool = tx.query_row(
                &format!("SELECT NOT EXISTS(SELECT 1 FROM {table})"),
                [],
                |r| r.get(0),
            )?;
            if empty {
                tx.execute_batch(&format!("DROP TABLE {table}"))?;
            }
        }
    }
    Ok(())
}

/// These caches lost all readers when analytics capture moved to its current receipts.
pub(super) fn analytics(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch("DROP TABLE IF EXISTS analytics_capture_effect_receipts; DROP TABLE IF EXISTS analytics_capture_legacy_watermarks; DROP TABLE IF EXISTS analytics_capture_receipt_migrations;")?;
    Ok(())
}
