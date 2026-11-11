//! Cover window membership reads and index exact string aliases without receipt scans.
use crate::StoreResult;
use rusqlite::{Connection, Transaction};
use studio_db_api::Table;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::queue_schema::install_index::<crate::clock_schema::DurableRows>(
        tx,
        "durable_rows_window_members",
    )?;
    install(tx)
}

pub(crate) fn install(tx: &Connection) -> StoreResult<()> {
    crate::queue_schema::ToolRequestAliasIndex::descriptor().install_if_absent(tx)?;
    // Trigger maintenance and JSON-array backfill are not expressible as db-api table descriptors.
    tx.execute_batch(include_str!("m2026_11_11_writer_receipt_indexes.sql"))?;
    Ok(())
}
