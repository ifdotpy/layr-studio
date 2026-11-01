//! Normalize the retained rate-limit observation through the typed provider boundary.
use crate::{StoreResult, SyncDocumentsRepository};
use rusqlite::Transaction;
use serde_json::{Value, value::RawValue};
use studio_runtime_schema::usage_limits::RateLimitView;

pub(super) fn apply(tx: &Transaction<'_>, now: f64) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let rows = {
        let mut query = tx.prepare(
            "SELECT id,payload FROM sync_entities WHERE collection='rateLimit' AND deleted=0",
        )?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, payload) in rows {
        let mut entity: Value = serde_json::from_str(&payload)?;
        let value = &mut entity["value"];
        let observation = if value.get("observation").is_some() {
            &mut value["observation"]
        } else {
            value
        };
        // Legacy eligibility is derived again from the normalized observation.
        if let Some(object) = observation.as_object_mut() {
            object.remove("eligibility");
        }
        let raw = RawValue::from_string(serde_json::to_string(observation)?)?;
        let view = RateLimitView::read(&raw, &id, now)?;
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &id, view, false)?;
    }
    Ok(())
}
