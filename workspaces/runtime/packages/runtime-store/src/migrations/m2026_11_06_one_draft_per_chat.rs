//! A chat has one draft, shared by every device; the last write wins. Drafts were
//! versions per device, writer and chat. Keep the newest version of each chat as the
//! draft under the chat id, and delete the other versions (a delete, so clients drop
//! them too).
use rusqlite::Transaction;

use crate::StoreResult;
use crate::repositories::sync_documents::SyncDocumentsRepository;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "sync_documents")? {
        return Ok(());
    }
    // Live versions, newest first in each chat.
    let versions: Vec<(String, String, String)> = tx
        .prepare(
            "SELECT id,json_extract(payload,'$.session'),payload FROM sync_documents
             WHERE scope='drafts' AND deleted=0 AND json_valid(payload)
               AND json_extract(payload,'$.session') IS NOT NULL
             ORDER BY json_extract(payload,'$.session'),
                      COALESCE(json_extract(payload,'$.updated'),0) DESC, seq DESC",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut kept = std::collections::BTreeSet::new();
    for (id, chat, payload) in versions {
        if id == chat {
            kept.insert(chat);
            continue;
        }
        if kept.insert(chat.clone()) {
            let mut value: studio_json::Value = studio_json::from_str(&payload)?;
            value["id"] = studio_json::json!(chat);
            SyncDocumentsRepository::put_draft(tx, &chat, &value.to_string(), false)?;
        }
        SyncDocumentsRepository::put_draft(tx, &id, "{}", true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_version_becomes_the_chat_draft_and_the_rest_go() {
        let mut db = crate::fresh_state::test_canvas();
        let tx = db.transaction().expect("transaction");
        for (id, chat, text, updated) in [
            ("phone:w1:c", "c", "old", 1),
            ("mac:w2:c", "c", "new", 5),
            ("mac:w2:d", "d", "other", 2),
        ] {
            let payload = studio_json::json!({"id":id,"session":chat,"device":"x","text":text,"updated":updated,"seen":{}});
            SyncDocumentsRepository::put_draft(&tx, id, &payload.to_string(), false)
                .expect("old version");
        }
        apply(&tx).expect("migration applies");
        let live: Vec<(String, String)> = tx
            .prepare(
                "SELECT id,json_extract(payload,'$.text') FROM sync_documents
                 WHERE scope='drafts' AND deleted=0 ORDER BY id",
            )
            .expect("query")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("rows")
            .collect::<Result<_, _>>()
            .expect("drafts");
        assert_eq!(
            live,
            [
                ("c".to_owned(), "new".to_owned()),
                ("d".to_owned(), "other".to_owned())
            ]
        );
        apply(&tx).expect("the migration is repeatable");
        tx.commit().expect("commit");
    }
}
