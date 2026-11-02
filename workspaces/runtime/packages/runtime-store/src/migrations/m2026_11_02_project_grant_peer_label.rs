//! Store the peer server label on each project grant (decision 2026-11-01, "Leads in one
//! team"). The label is known at pairing time, so the peer lead shows without a paired
//! server row. Existing rows get an empty label.
use crate::StoreResult;
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute_batch("ALTER TABLE project_grants ADD COLUMN peer_label TEXT NOT NULL DEFAULT ''")?;
    Ok(())
}
