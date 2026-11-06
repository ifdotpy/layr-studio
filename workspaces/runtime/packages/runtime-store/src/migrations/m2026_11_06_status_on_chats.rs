//! The status and the in-flight flag leave the agent record (#630 item 63). The chat row
//! owns the status (`chats.status`) and an unfinished turn row is the in-flight flag;
//! every agent write already projects both. This step projects each stored record once
//! more, then removes the keys from the record. Repeatable: a record without the keys is
//! not touched.
use crate::StoreResult;
use rusqlite::Transaction;
use studio_json::Value;

pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_agents")?
        || !crate::schema::table_exists(tx, "chats")?
    {
        return Ok(());
    }
    let rows = {
        let mut statement = tx.prepare(
            "SELECT id,record FROM runtime_agents WHERE json_valid(record) AND (
               json_type(record,'$.status') IS NOT NULL
               OR json_type(record,'$.inFlight') IS NOT NULL)",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (id, raw) in rows {
        // Project the merged record: fields that already left the record would be
        // cleared by a projection of the raw one.
        let record: Value =
            studio_json::from_str(&crate::agent_volatile::merged_text(tx, &id, raw)?)?;
        crate::domain::project(tx, &id, &record)?;
        tx.execute(
            "UPDATE runtime_agents SET record=json_remove(record,'$.status','$.inFlight') WHERE id=?1",
            [&id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{AgentId, AgentRepository, RuntimeStore};

    #[tokio::test]
    async fn status_and_in_flight_move_to_the_chat_and_turn_rows() {
        let root = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_agents(id,record) VALUES('a',?1),('b',?2)",
                    [
                        r#"{"id":"a","rootId":"a","epoch":1,"status":"running","inFlight":true,"turnId":"t1","threadId":"th","compactions":0,"provider":"codex","accountKey":"default","cwd":"/w","autoWake":true,"startAttempt":{"id":"attempt-1","turnId":"t1"}}"#,
                        r#"{"id":"b","rootId":"a","parentId":"a","epoch":1,"status":"paused","inFlight":false,"compactions":0,"provider":"codex","accountKey":"default","cwd":"/w"}"#,
                    ],
                )?;
                crate::migrations::migrate_start_attempts_to_turns(tx)?;
                crate::migrations::migrate_chat_fields_to_chats(tx)?;
                super::apply(tx)?;
                super::apply(tx)?;
                Ok(())
            })
            .await
            .expect("migration");
        let db = store.test_reader().expect("reader");
        for id in ["a", "b"] {
            let raw: String = db
                .query_row(
                    "SELECT record FROM runtime_agents WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .expect("record");
            assert!(!raw.contains("\"status\""), "{id}: status left the record");
            assert!(!raw.contains("inFlight"), "{id}: inFlight left the record");
        }
        let a = AgentRepository::get(&db, &AgentId::from("a"))
            .expect("read")
            .expect("agent")
            .value();
        assert_eq!(a["status"], "running");
        assert_eq!(a["inFlight"], true);
        assert_eq!(a["autoWake"], true);
        assert_eq!(a["startAttempt"]["id"], "attempt-1");
        let b = AgentRepository::get(&db, &AgentId::from("b"))
            .expect("read")
            .expect("agent")
            .value();
        assert_eq!(b["status"], "paused");
        assert_eq!(b["inFlight"], false);
    }
}
