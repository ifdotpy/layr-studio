//! Normalize the retained rate-limit observation through the typed provider boundary.
use crate::{StoreResult, SyncDocumentsRepository};
use rusqlite::Transaction;

pub(super) fn apply(tx: &Transaction<'_>, now: f64) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_entities")? {
        return Ok(());
    }
    let rows = {
        let mut query = tx.prepare(
            "SELECT id,substr(CAST(payload AS BLOB),1,?1),length(CAST(payload AS BLOB)) FROM sync_entities WHERE collection='rateLimit' AND deleted=0",
        )?;
        let rows = query.query_map(
            [crate::account_state::MAX_PERSISTED_DOCUMENT_BYTES as i64 + 1],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, payload, length) in rows {
        let bytes = payload
            .as_deref()
            .filter(|bytes| {
                length.is_some_and(|length| {
                    length <= crate::account_state::MAX_PERSISTED_DOCUMENT_BYTES as i64
                }) && crate::account_state::persisted_document_within_bounds(bytes)
            })
            .unwrap_or_default();
        let (view, unreadable) = crate::account_state::normalized_limits_view(&id, bytes, now);
        if unreadable {
            // Update triggers inspect the old JSON; replace unreadable rows in this transaction.
            tx.execute(
                "DELETE FROM sync_entities WHERE collection='rateLimit' AND id=?1",
                [&id],
            )?;
        }
        SyncDocumentsRepository::put_entity(tx, "rateLimit", &id, view, false)?;
    }
    if crate::schema::table_exists(tx, "runtime_account_state")? {
        let rows = {
            let mut query = tx.prepare("SELECT account,substr(CAST(limits_observation AS BLOB),1,?1),length(CAST(limits_observation AS BLOB)) FROM runtime_account_state WHERE limits_observation IS NOT NULL")?;
            let rows = query.query_map(
                [crate::account_state::MAX_PERSISTED_DOCUMENT_BYTES as i64 + 1],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (account, bytes, length) in rows {
            let stamp = bytes
                .as_deref()
                .filter(|bytes| {
                    length.is_some_and(|length| {
                        length <= crate::account_state::MAX_PERSISTED_DOCUMENT_BYTES as i64
                    }) && crate::account_state::persisted_document_within_bounds(bytes)
                })
                .and_then(|bytes| {
                    serde_json::from_slice::<Option<crate::account_state::LimitsStamp>>(bytes).ok()
                })
                .flatten()
                .unwrap_or_else(|| {
                    eprintln!(
                        "limits migration: isolated unreadable receipt for account {account}"
                    );
                    crate::account_state::LimitsStamp::default()
                });
            crate::account_state::put_limits_observation(tx, &account, &stamp)?;
        }
    }
    Ok(())
}
