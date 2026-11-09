use crate::StoreResult;
use rusqlite::{OptionalExtension, Transaction};

pub(super) const ID: &str = "2026-11-07-explicit-review-model";

/// Unset chats previously resolved reviews to the caller's model. The user chose
/// an explicit stored review model, initialized once from the Codex model policy.
pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    super::apply_once(tx, ID, backfill)
}

fn backfill(tx: &Transaction<'_>) -> StoreResult<()> {
    let previous: Option<String> = tx
        .query_row(
            "SELECT value FROM sync_entity_meta WHERE key='migration_catalog'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let position = super::CATALOG
        .iter()
        .position(|step| step.definition().id == ID)
        .expect("registered review backfill");
    // Adopt the pre-ledger step only when an applied catalog includes it.
    if let Some((fingerprint, _)) = previous
        .as_deref()
        .and_then(|marker| marker.split_once(':'))
    {
        for end in position + 1..=super::CATALOG.len() {
            if fingerprint
                == super::catalog_fingerprint(
                    &super::CATALOG[..end],
                    super::MigrationTarget::Canvas,
                )
            {
                return Ok(());
            }
        }
    }
    tx.execute(
        "UPDATE runtime_agents SET record=json_set(record,'$.reviewDefaults',json_set(CASE WHEN json_type(record,'$.reviewDefaults')='object' THEN json_extract(record,'$.reviewDefaults') ELSE '{}' END,'$.model',?1)) WHERE json_valid(record) AND (json_extract(record,'$.isLead')=1 OR json_extract(record,'$.rootId')=id) AND (json_extract(record,'$.reviewDefaults.model') IS NULL OR json_extract(record,'$.reviewDefaults.model') IN ('','__model_default__'))",
        [studio_runtime_schema::provider_kind::initial_review_model()],
    )?;
    Ok(())
}
