use std::path::Path;

use serde::Deserialize;
use studio_json::Value;
use studio_runtime_store::RuntimeStore;

pub(crate) const STATE_PLACEHOLDER: &str = "/fixture/state";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRow {
    id: String,
    record: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tables {
    runtime_agents: Vec<StoredRow>,
    runtime_projects: Vec<StoredRow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFixture {
    agents: Value,
    tables: Tables,
}

#[derive(Clone, Copy)]
pub(crate) enum FixtureKind {
    Dispatch,
    Recovery,
}

pub(crate) async fn seed(store: &RuntimeStore, root: &Path, kind: FixtureKind) -> Value {
    let data = match kind {
        FixtureKind::Dispatch => include_str!("fixtures/dispatch.json"),
        FixtureKind::Recovery => include_str!("fixtures/recovery.json"),
    };
    let mut fixture: StateFixture =
        studio_json::from_str(&data.replace(STATE_PLACEHOLDER, &root.to_string_lossy()))
            .expect("frozen state fixture");
    // Frozen expectations describe stored source rows; admit them through the same dated transform.
    let expected = if let Some(stored) = fixture.agents.get_mut("stored") {
        stored
            .as_object_mut()
            .expect("frozen stored agents")
            .values_mut()
            .collect::<Vec<_>>()
    } else {
        fixture
            .agents
            .as_array_mut()
            .expect("frozen dispatch agents")
            .iter_mut()
            .collect::<Vec<_>>()
    };
    for agent in expected {
        let raw = studio_json::to_string(agent).expect("frozen agent JSON");
        let current = studio_runtime_store::test_fixtures::normalize_retired_agent_record(
            agent["id"].as_str().expect("frozen agent id"),
            &raw,
        )
        .expect("dated frozen agent transform");
        *agent = studio_json::from_str(&current).expect("migrated frozen agent");
    }
    store
        .transaction(move |tx| {
            for row in fixture.tables.runtime_projects {
                tx.execute(
                    "INSERT INTO runtime_projects(id,record) VALUES (?1,?2)",
                    [&row.id, &row.record],
                )?;
            }
            // The frozen records predate the project record kind.
            studio_runtime_store::test_fixtures::project_record_kind(tx)?;
            for row in fixture.tables.runtime_agents {
                let current = studio_runtime_store::test_fixtures::normalize_retired_agent_record(
                    &row.id,
                    &row.record,
                )?;
                let record: Value = studio_json::from_str(&current)?;
                studio_runtime_store::agent_volatile::put(tx, &row.id, &record, true)?;
            }
            Ok(())
        })
        .await
        .expect("seed frozen runtime records");
    fixture.agents
}
