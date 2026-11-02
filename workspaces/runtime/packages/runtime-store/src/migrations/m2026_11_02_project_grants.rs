//! Create the `project_grants` table and its check triggers (decision 2026-11-01,
//! "Pairing per project"). The table is new, so no data moves.
use crate::{StoreResult, project_grants};
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    project_grants::install(tx)
}
