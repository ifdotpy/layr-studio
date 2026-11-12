//! Keep event retirement ordered within its active collection instead of all publications.
use crate::{StoreResult, Transaction};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    // Retain the prior collection/deleted index until the schema deletion wave.
    crate::queue_schema::install_index::<crate::publication_schema::Publications>(
        tx,
        "publications_entity_collection_members",
    )?;
    Ok(())
}
