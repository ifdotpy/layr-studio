//! Upgrade the controls of existing projected radio rooms.
use crate::{StoreResult, SyncDocumentsRepository};
use rusqlite::Transaction;
use serde_json::Value;
use studio_runtime_schema::{radio_controls::RadioControls, room_record::Status};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut query = tx.prepare("SELECT id,payload FROM sync_entities WHERE collection='room' AND deleted=0 AND json_valid(payload) AND json_type(payload,'$.value.radio')='object' AND json_type(payload,'$.value.radio.controls') IS NULL ORDER BY id")?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, payload) in rows {
        let envelope: Value = serde_json::from_str(&payload)?;
        let mut room = envelope["value"].clone();
        let radio = &mut room["radio"];
        let status: Status = serde_json::from_value(radio["status"].clone())?;
        radio["controls"] =
            serde_json::to_value(RadioControls::derive(status, radio["active"].is_object()))?;
        SyncDocumentsRepository::put_entity(tx, "room", &id, room, false)?;
    }
    Ok(())
}
