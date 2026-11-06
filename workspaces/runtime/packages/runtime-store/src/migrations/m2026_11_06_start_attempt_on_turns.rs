//! The start attempt leaves the agent record (#630 item 63). Its owner is the turn row
//! (`turns.attempt`); every agent write already places it there. This step places the
//! attempt of each stored record once more, then removes the key from the record.
//! Repeatable: a record without the key is not touched.
use crate::StoreResult;
use rusqlite::Transaction;
use studio_json::Value;

pub fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "runtime_agents")?
        || !crate::schema::table_exists(tx, "turns")?
    {
        return Ok(());
    }
    let rows = {
        let mut statement = tx.prepare(
            "SELECT id,record FROM runtime_agents
             WHERE json_valid(record) AND json_type(record,'$.startAttempt') IS NOT NULL",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (id, raw) in rows {
        let record: Value = studio_json::from_str(&raw)?;
        crate::domain::project(tx, &id, &record)?;
        tx.execute(
            "UPDATE runtime_agents SET record=json_remove(record,'$.startAttempt') WHERE id=?1",
            [&id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{AgentId, AgentRepository, RuntimeStore};

    #[tokio::test]
    async fn the_attempt_moves_to_the_turn_row_and_readers_still_see_it() {
        let root = tempfile::tempdir().expect("state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_agents(id,record) VALUES('a',?1)",
                    [r#"{"id":"a","rootId":"a","epoch":1,"status":"running","compactions":0,"provider":"codex","accountKey":"default","cwd":"/w","inFlight":true,"startAttempt":{"id":"attempt-1","submitted":false}}"#],
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
        assert!(
            !raw.contains("startAttempt"),
            "the record no longer holds the attempt"
        );
        let attempt: String = db
            .query_row(
                "SELECT attempt FROM turns WHERE chat='a' AND attempt IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("the turn row holds the attempt");
        assert!(attempt.contains("attempt-1"));
        let agent = AgentRepository::get(&db, &AgentId::from("a"))
            .expect("read")
            .expect("agent");
        assert_eq!(agent.value()["startAttempt"]["id"], "attempt-1");
    }
}
