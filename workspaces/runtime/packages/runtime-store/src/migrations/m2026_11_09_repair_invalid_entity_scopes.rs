//! Repair unknowable historical ownership without moving any known selector.
use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    // F1 refreshes deletion producers together with all other writers at authority.
    crate::entity_scopes::install(tx)
}

#[cfg(test)]
mod tests {
    use crate::{AgentRecord, AgentRepository};

    fn guards(db: &rusqlite::Connection) -> Vec<(String, String)> {
        db.prepare("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND (name LIKE 'immutable_scope_%' OR name LIKE 'durable_%') ORDER BY name")
            .unwrap().query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap().collect::<rusqlite::Result<_>>().unwrap()
    }

    #[tokio::test]
    async fn entity_scopes_upgrade_repairs_w3_r_guards_and_matches_fresh_schema() {
        let root = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(root.path())).await.unwrap();
        store.transaction(|tx| {
            crate::entity_scopes::seed_legacy_reparented_rows(tx, || {
                tx.execute("INSERT INTO runtime_agents(id,record) VALUES('legacy','{\"id\":\"legacy\",\"rootId\":17,\"parentId\":\"parent\",\"provider\":\"codex\",\"accountKey\":\"default\",\"epoch\":1,\"compactions\":0,\"status\":\"idle\"}')", [])?;
                tx.execute("INSERT INTO runtime_plans(id,record) VALUES('legacy-plan','{\"id\":\"legacy-plan\",\"rootId\":1,\"rootId\":\"alpha\"}')", [])?;
                Ok(())
            })?;
            // The captured guard reads the historical durable source before its replacement.
            <crate::clock_schema::DurableRows as studio_db_api::Table>::descriptor().install_if_absent(tx)?;
            // Exact UPDATE and INSERT guards captured from origin/main 81ffe643.
            tx.execute_batch("DROP TRIGGER immutable_scope_runtime_agents; DROP TRIGGER immutable_scope_runtime_agents_insert; DROP TRIGGER durable_runtime_plans_delete;")?;
            tx.execute_batch(include_str!("fixtures/w3_r_agent_scope.sql"))?;
            let error = AgentRepository::put(tx, &repaired()).unwrap_err();
            assert!(error.to_string().contains(crate::entity_scopes::IMMUTABLE_SCOPE_ERROR), "{error}");
            super::apply(tx)?;
            crate::bootstrap::install_current_producers(tx)?;
            studio_db_api::migration::retire_source::<crate::clock_schema::DurableRows>(
                &mut crate::peer_update_settings::canvas_tx(tx))?;
            Ok(())
        }).await.unwrap();
        drop(store);
        let store = crate::RuntimeStore::open(Some(root.path())).await.unwrap();
        store
            .transaction(|tx| {
                AgentRepository::put(tx, &repaired())?;
                tx.execute("DELETE FROM runtime_plans WHERE id='legacy-plan'", [])?;
                assert!(tx.execute("INSERT INTO runtime_plans(id,record) VALUES('legacy-plan','{\"id\":\"legacy-plan\",\"rootId\":\"beta\"}')", []).is_err(), "upgraded deletion evidence retains the last historical owner");
                tx.execute("INSERT INTO runtime_plans(id,record) VALUES('legacy-plan','{\"id\":\"legacy-plan\",\"rootId\":\"alpha\"}')", [])?;
                let first = guards(tx);
                super::apply(tx)?;
                super::apply(tx)?;
                assert_eq!(guards(tx), first, "reinstall is idempotent");
                assert_eq!(guards(tx), guards(&crate::fresh_state::test_canvas()));
                let mut moved = repaired();
                moved.parent_id = Some("other".into());
                assert!(AgentRepository::put(tx, &moved).is_err());
                Ok::<_, crate::StoreError>(())
            })
            .await
            .unwrap();
    }

    fn repaired() -> AgentRecord {
        AgentRecord::from_value(studio_json::json!({
            "id":"legacy", "rootId":"repaired", "parentId":"parent", "provider":"codex",
            "accountKey":"default", "epoch":1, "compactions":0, "status":"idle"
        }))
        .unwrap()
    }
}
