//! Seek one durable label and sequence interval in the shared changes table.
use crate::{StoreResult, Transaction};
pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::queue_schema::install_index::<crate::change_schema::Changes>(
        tx,
        "changes_durable_label_sequence",
    )?;
    Ok(())
}
