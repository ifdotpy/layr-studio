use crate::StoreResult;
use rusqlite::Transaction;

/// Unset chats previously resolved reviews to the caller's model. The user chose
/// an explicit stored review model, initialized once from the Codex model policy.
pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute(
        "UPDATE runtime_agents SET record=json_set(record,'$.reviewDefaults',json_set(CASE WHEN json_type(record,'$.reviewDefaults')='object' THEN json_extract(record,'$.reviewDefaults') ELSE '{}' END,'$.model',?1)) WHERE json_valid(record) AND (json_extract(record,'$.isLead')=1 OR json_extract(record,'$.rootId')=id) AND (json_extract(record,'$.reviewDefaults.model') IS NULL OR json_extract(record,'$.reviewDefaults.model') IN ('','__model_default__'))",
        [studio_runtime_schema::provider_kind::initial_review_model()],
    )?;
    Ok(())
}
