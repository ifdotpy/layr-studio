//! Upgrade room entities with their member roots before serving sync.
use crate::{StoreResult, SyncDocumentsRepository};
use rusqlite::Transaction;
use serde_json::Value;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut query = tx.prepare("SELECT id,payload FROM sync_entities WHERE collection='room' AND deleted=0 AND json_valid(payload) AND json_type(payload,'$.value.leadIds') IS NULL ORDER BY id")?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, payload) in rows {
        let envelope: Value = serde_json::from_str(&payload)?;
        let mut room = envelope["value"].clone();
        let mut leads = Vec::<String>::new();
        if room["kind"] == "broadcast" {
            if let Some(root) = room["rootId"]
                .as_str()
                .filter(|root| !root.is_empty() && *root != "all")
            {
                leads.push(root.to_owned());
            }
        } else {
            let members = serde_json::to_string(&room["members"])?;
            let mut agents = tx.prepare("SELECT CASE WHEN json_extract(record,'$.isLead')=1 THEN id ELSE root_id END FROM runtime_agents WHERE deleted_at IS NULL AND id IN (SELECT value FROM json_each(?1) WHERE type='text') ORDER BY rowid")?;
            for root in agents.query_map([members], |row| row.get::<_, Option<String>>(0))? {
                if let Some(root) = root?.filter(|root| !root.is_empty())
                    && !leads.contains(&root)
                {
                    leads.push(root);
                }
            }
        }
        leads.sort_unstable();
        room["leadIds"] = serde_json::to_value(leads)?;
        SyncDocumentsRepository::put_entity(tx, "room", &id, room, false)?;
    }
    Ok(())
}
