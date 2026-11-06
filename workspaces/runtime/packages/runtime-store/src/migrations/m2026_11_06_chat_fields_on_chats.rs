//! Auto-wake, usage resume, execution mode and turn epoch leave the agent record
//! (#630 item 63). Their owner is the chat row (`chats`); every agent write already
//! projects them there. This step projects each stored record once more, then removes the
//! keys from the record. Repeatable: a record without the keys is not touched.
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
               json_type(record,'$.autoWake') IS NOT NULL
               OR json_type(record,'$.usageResumeEnabled') IS NOT NULL
               OR json_type(record,'$.executionMode') IS NOT NULL
               OR json_type(record,'$.turnEpoch') IS NOT NULL)",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (id, raw) in rows {
        // Project the merged record: the start attempt already left the record for its
        // turn row, and a projection without it would clear that attempt.
        let record: Value =
            studio_json::from_str(&crate::agent_volatile::merged_text(tx, &id, raw)?)?;
        crate::domain::project(tx, &id, &record)?;
        tx.execute(
            "UPDATE runtime_agents SET record=json_remove(record,
               '$.autoWake','$.usageResumeEnabled','$.executionMode','$.turnEpoch') WHERE id=?1",
            [&id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{AgentId, AgentRepository, RuntimeStore};

    #[tokio::test]
    async fn the_fields_move_to_the_chat_row_and_readers_still_see_them() {
        let root = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_agents(id,record) VALUES('a',?1)",
                    [r#"{"id":"a","rootId":"a","epoch":1,"status":"idle","compactions":0,"provider":"codex","accountKey":"default","cwd":"/w","autoWake":true,"usageResumeEnabled":false,"executionMode":"vm","turnEpoch":4}"#],
                )?;
                super::apply(tx)?;
                super::apply(tx)?;
                Ok(())
            })
            .await
            .expect("migration");
        let db = store.test_reader().expect("reader");
        let raw: String = db
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='a'",
                [],
                |row| row.get(0),
            )
            .expect("record");
        for key in [
            "autoWake",
            "usageResumeEnabled",
            "executionMode",
            "turnEpoch",
        ] {
            assert!(!raw.contains(key), "{key} left the record");
        }
        let value = AgentRepository::get(&db, &AgentId::from("a"))
            .expect("read")
            .expect("agent")
            .value();
        assert_eq!(value["autoWake"], true);
        assert_eq!(value["usageResumeEnabled"], false);
        assert_eq!(value["executionMode"], "vm");
        assert_eq!(value["turnEpoch"], 4);
    }
}

#[cfg(test)]
mod attempt_tests {
    use crate::RuntimeStore;

    #[tokio::test]
    async fn moving_the_chat_fields_keeps_the_start_attempt_on_its_turn() {
        let root = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_agents(id,record) VALUES('a',?1)",
                    [r#"{"id":"a","rootId":"a","epoch":1,"status":"running","compactions":0,"provider":"codex","accountKey":"default","cwd":"/w","inFlight":true,"autoWake":true,"startAttempt":{"id":"attempt-1"}}"#],
                )?;
                crate::migrations::migrate_start_attempts_to_turns(tx)?;
                super::apply(tx)?;
                Ok(())
            })
            .await
            .expect("migrations");
        let attempt: String = store
            .test_reader()
            .expect("reader")
            .query_row(
                "SELECT attempt FROM turns WHERE chat='a' AND attempt IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("the attempt stays on its turn");
        assert!(attempt.contains("attempt-1"));
    }
}
