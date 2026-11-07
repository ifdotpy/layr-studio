use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if crate::schema::table_exists(tx, "approvals")? {
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS approvals_agent_state ON approvals(agent,state)",
        )?;
    }
    if crate::schema::table_exists(tx, "sync_entities")? {
        tx.execute_batch("CREATE INDEX IF NOT EXISTS sync_entities_id ON sync_entities(id)")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, types::Value};

    // sync_entity_context::pending_attention, shared by system reads, transfer and scheduler.
    const ATTENTION: &str = "SELECT EXISTS(SELECT 1 FROM approvals WHERE agent=?1 AND (epoch IS NULL OR epoch=?2) AND state='pending' AND COALESCE(deferral_active,0)=0 AND kind IN ('question','elicitation')), \
                EXISTS(SELECT 1 FROM approvals WHERE agent=?1 AND (epoch IS NULL OR epoch=?2) AND state='pending' AND COALESCE(deferral_active,0)=0 AND kind NOT IN ('question','elicitation'))";
    // commands::response, including its collection/id ordering.
    const COMMANDS: &str = "SELECT collection,id,seq,payload FROM sync_entities WHERE id IN (?1,?2) AND deleted=0 ORDER BY collection,id";

    fn fixture() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE approvals(id TEXT PRIMARY KEY,agent TEXT,epoch INTEGER,state TEXT,kind TEXT,deferral_active INTEGER,turn TEXT,delivery TEXT,fire_at REAL);
            CREATE INDEX approvals_turn ON approvals(turn,state);
            CREATE INDEX approvals_agent_open ON approvals(agent) WHERE state IN ('pending','answering');
            CREATE INDEX approvals_account_open ON approvals(json_extract(delivery,'$.account')) WHERE state IN ('pending','answering');
            CREATE INDEX approvals_fire_at ON approvals(fire_at) WHERE fire_at IS NOT NULL;
            INSERT INTO approvals(id,agent,epoch,state,kind,deferral_active) VALUES
              ('q','a',NULL,'pending','question',NULL),
              ('e','a',2,'pending','elicitation',0),
              ('p','a',2,'pending','command',0),
              ('deferred','deferred',2,'pending','question',1),
              ('old','old',1,'pending','command',0),
              ('done','done',2,'answered','question',0);
            CREATE TABLE sync_entities(collection TEXT NOT NULL,id TEXT NOT NULL,seq INTEGER NOT NULL,hash TEXT NOT NULL,payload TEXT,deleted INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(collection,id));
            CREATE INDEX sync_entities_collection_deleted ON sync_entities(collection,deleted);
            CREATE INDEX sync_entities_collection_seq ON sync_entities(collection,seq);
            CREATE INDEX sync_entities_seq ON sync_entities(seq);
            CREATE INDEX sync_entities_non_transcript_seq ON sync_entities(seq) WHERE collection NOT LIKE 'transcript:%';
            CREATE INDEX sync_entities_tombstone_order ON sync_entities(seq,collection,id) WHERE deleted=1 AND collection NOT LIKE 'transcript:%';
            INSERT INTO sync_entities VALUES
              ('request','q',3,'h','request',0),('chat','a',2,'h','chat',0),
              ('other','a',1,'h','same id',0),('deleted','q',4,'h','gone',1),
              ('unrelated','z',5,'h','other',0);").unwrap();
        db
    }

    fn rows(db: &Connection, sql: &str, params: &[Value]) -> Vec<Vec<Value>> {
        let mut stmt = db.prepare(sql).unwrap();
        let count = stmt.column_count();
        stmt.query_map(rusqlite::params_from_iter(params), |row| {
            (0..count).map(|column| row.get(column)).collect()
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    fn plan(db: &Connection, sql: &str, params: &[Value]) -> Vec<String> {
        db.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(rusqlite::params_from_iter(params), |row| row.get(3))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn attention_lookups_use_index_and_preserve_results() {
        let mut db = fixture();
        let params = [Value::Text("a".into()), Value::Integer(2)];
        let before = plan(&db, ATTENTION, &params);
        println!("attention before: {before:?}");
        let agents = ["a", "deferred", "old", "done", "missing"];
        let expected: Vec<_> = agents
            .iter()
            .map(|agent| {
                rows(
                    &db,
                    ATTENTION,
                    &[Value::Text((*agent).into()), Value::Integer(2)],
                )
            })
            .collect();
        assert_eq!(
            expected[0],
            vec![vec![Value::Integer(1), Value::Integer(1)]]
        );
        for result in &expected[1..] {
            assert_eq!(*result, vec![vec![Value::Integer(0), Value::Integer(0)]]);
        }
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        super::apply(&tx).unwrap();
        tx.commit().unwrap();
        for (agent, expected) in agents.iter().zip(expected) {
            assert_eq!(
                rows(
                    &db,
                    ATTENTION,
                    &[Value::Text((*agent).into()), Value::Integer(2)]
                ),
                expected
            );
        }
        let after = plan(&db, ATTENTION, &params);
        println!("attention after: {after:?}");
        assert_eq!(
            after
                .iter()
                .filter(|step| step.contains("SEARCH approvals USING INDEX approvals_agent_state"))
                .count(),
            2,
            "{after:?}"
        );
    }

    #[test]
    fn commands_lookup_uses_index_and_preserves_results() {
        let mut db = fixture();
        let params = [Value::Text("q".into()), Value::Text("a".into())];
        let expected = rows(&db, COMMANDS, &params);
        assert_eq!(expected.len(), 3);
        assert_eq!(
            expected
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>(),
            vec![
                Value::Text("chat".into()),
                Value::Text("other".into()),
                Value::Text("request".into())
            ]
        );
        println!("commands before: {:?}", plan(&db, COMMANDS, &params));
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        tx.commit().unwrap();
        assert_eq!(rows(&db, COMMANDS, &params), expected);
        let after = plan(&db, COMMANDS, &params);
        println!("commands after: {after:?}");
        assert!(
            after
                .iter()
                .any(|step| step.contains("SEARCH sync_entities USING INDEX sync_entities_id")),
            "{after:?}"
        );
    }

    #[test]
    fn current_canvas_installs_attention_lookup_indexes() {
        let db = crate::fresh_state::test_canvas();
        for index in ["approvals_agent_state", "sync_entities_id"] {
            let exists: bool = db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='index' AND name=?1)",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "{index}");
        }
    }
}
