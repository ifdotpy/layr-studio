use crate::StoreResult;
use rusqlite::Transaction;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let mut query = tx.prepare("SELECT id,record FROM runtime_agents WHERE json_valid(record) AND (json_type(record,'$.maxAgents') IS NOT NULL OR json_type(record,'$.maxAgentsExplicit') IS NOT NULL OR json_type(record,'$.capacity') IS NULL)")?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut changed = false;
    for row in rows {
        let (id, raw) = row?;
        let mut record: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&raw)?;
        let old_max = record.remove("maxAgents");
        let old_explicit = record.remove("maxAgentsExplicit");
        if old_max.is_none() && old_explicit.is_none() && record.contains_key("capacity") {
            continue;
        }
        let max = old_max.as_ref().and_then(|raw| {
            serde_json::from_str::<studio_runtime_schema::AgentCounter>(raw.get()).ok()
        });
        let explicit = old_explicit
            .as_ref()
            .and_then(|raw| serde_json::from_str::<bool>(raw.get()).ok())
            .unwrap_or(false);
        let kind = if explicit { "explicit" } else { "automatic" };
        let limit = max.map_or(
            serde_json::json!(studio_runtime_schema::agent_limits::DEFAULT_MAX_TEAM_AGENTS),
            |value| serde_json::json!(value),
        );
        record.insert(
            "capacity".into(),
            RawValue::from_string(serde_json::json!({"kind":kind,"max":limit}).to_string())?,
        );
        tx.execute(
            "UPDATE runtime_agents SET record=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&record)?, id],
        )?;
        changed = true;
    }
    if changed && crate::schema::table_exists(tx, "sync_entity_meta")? {
        tx.execute(
            "DELETE FROM sync_entity_meta WHERE key=?1",
            [crate::AGENT_PROJECTIONS_SEED_MARKER],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    #[test]
    fn capacity_migration_normalizes_and_reseeds_once() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE sync_entity_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO sync_entity_meta VALUES('agent_projections_v1','1');").unwrap();
        for (id, raw, kind, max) in [
            ("default", r#"{"id":"default"}"#, "automatic", 64),
            (
                "automatic",
                r#"{"id":"automatic","maxAgents":70,"maxAgentsExplicit":false}"#,
                "automatic",
                70,
            ),
            (
                "explicit",
                r#"{"id":"explicit","maxAgents":8,"maxAgentsExplicit":true}"#,
                "explicit",
                8,
            ),
            (
                "malformed",
                r#"{"id":"malformed","maxAgents":-1,"maxAgentsExplicit":true}"#,
                "explicit",
                64,
            ),
        ] {
            db.execute("INSERT INTO runtime_agents VALUES(?1,?2)", (id, raw))
                .unwrap();
            let tx = db.transaction().unwrap();
            super::apply(&tx).unwrap();
            let raw: String = tx
                .query_row("SELECT record FROM runtime_agents WHERE id=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(
                value["capacity"],
                serde_json::json!({"kind":kind,"max":max}),
                "{id}"
            );
            assert!(value.get("maxAgents").is_none() && value.get("maxAgentsExplicit").is_none());
            assert_eq!(
                tx.query_row(
                    "SELECT count(*) FROM sync_entity_meta WHERE key='agent_projections_v1'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
            tx.execute(
                "INSERT INTO sync_entity_meta VALUES('agent_projections_v1','1')",
                [],
            )
            .unwrap();
            super::apply(&tx).unwrap();
            assert_eq!(
                tx.query_row(
                    "SELECT count(*) FROM sync_entity_meta WHERE key='agent_projections_v1'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                1
            );
            tx.commit().unwrap();
        }
    }

    #[test]
    fn capacity_migration_preserves_explicit_flag_with_missing_or_null_max() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL)")
            .unwrap();
        for (id, raw) in [
            ("missing", r#"{"id":"missing","maxAgentsExplicit":true}"#),
            (
                "null",
                r#"{"id":"null","maxAgents":null,"maxAgentsExplicit":true}"#,
            ),
        ] {
            db.execute("INSERT INTO runtime_agents VALUES(?1,?2)", (id, raw))
                .unwrap();
        }
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        for id in ["missing", "null"] {
            let raw: String = tx
                .query_row("SELECT record FROM runtime_agents WHERE id=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(
                value["capacity"],
                serde_json::json!({"kind":"explicit","max":64}),
                "{id}"
            );
        }
    }
}
