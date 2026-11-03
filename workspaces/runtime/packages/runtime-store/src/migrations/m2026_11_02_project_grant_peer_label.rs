//! Store the peer server label on each project grant (decision 2026-11-01, "Leads in one
//! team"). The label is known at pairing time, so the peer lead shows without a paired
//! server row. Existing rows get an empty label. The step is repeatable: test fixtures
//! apply the catalog again on a database that already has the column.
use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let present: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('project_grants') WHERE name='peer_label')",
        [],
        |row| row.get(0),
    )?;
    if !present {
        tx.execute_batch(
            "ALTER TABLE project_grants ADD COLUMN peer_label TEXT NOT NULL DEFAULT ''",
        )?;
    }
    Ok(())
}
