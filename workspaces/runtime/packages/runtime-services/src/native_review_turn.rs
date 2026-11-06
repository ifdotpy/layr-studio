//! Settle queued native reviews created by older versions without starting them.
use crate::{
    account_worker::{WorkerError, preparation},
    native_actions::NativeError,
};
use std::sync::Arc;
use studio_json::{Value, json};
use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, RuntimeStore};

/// Only a queued, unsubmitted legacy review may be failed here. Submitted or
/// uncertain provider requests stay owned by their existing recovery path.
pub fn claimable(record: &Value) -> bool {
    let attempt = record.get("startAttempt").unwrap_or(&Value::Null);
    record["nativeReview"]["status"] == "pending"
        && record["status"] == "queued"
        && record["autoWake"] == true
        && !studio_runtime_store::truthy(record.get("inFlight"))
        && !studio_runtime_store::truthy(record.get("deletedAt"))
        && !studio_runtime_store::truthy(record.get("nativeFailureHold"))
        && (attempt.is_null()
            || (attempt["submitted"] == false
                && !studio_runtime_store::truthy(attempt.get("turnId"))
                && !studio_runtime_store::truthy(attempt.get("observedTurnId"))))
}

pub(crate) async fn candidate(
    store: &Arc<RuntimeStore>,
    agent_id: &str,
    epoch: i64,
    account: &str,
) -> Result<bool, WorkerError> {
    let agent_id = agent_id.to_owned();
    let account = account.to_owned();
    store
        .read_async("runtime-services/legacy-review", move |tx| {
            let Some(record) = AgentRepository::get(tx, &AgentId::from(agent_id.as_str()))? else {
                return Ok(false);
            };
            let value = record.value();
            Ok(record.epoch == epoch && record.account_key == account && claimable(&value))
        })
        .await
}

pub(crate) async fn run(
    store: Arc<RuntimeStore>,
    _resources: &preparation::PreparationResources,
    agent_id: &str,
    epoch: i64,
) -> Result<(), NativeError> {
    let id = agent_id.to_owned();
    store
        .transaction_with(move |tx| {
            let Some(record) = AgentRepository::get(tx, &AgentId::from(id.as_str()))? else {
                return Ok(());
            };
            let mut value = record.value();
            if record.epoch != epoch || !claimable(&value) {
                return Ok(());
            }
            let before = value.clone();
            value["status"] = json!("failed");
            value["inFlight"] = json!(false);
            value["error"] =
                json!("This review was requested by an older version; request it again.");
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            enum LegacyStatus {
                Refused,
            }
            value["nativeReview"]["status"] = json!(LegacyStatus::Refused);
            AgentRepository::put(tx, &AgentRecord::from_value(value.clone())?)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |duration| duration.as_secs_f64());
            crate::lifecycle_effects::notify_parent_of_start_failure(
                tx,
                &before,
                &value,
                "legacy-review",
                now,
            )?;
            Ok::<_, NativeError>(())
        })
        .await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unsubmitted_legacy_review_fails_once_without_provider_calls() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        let lead = json!({"compactions":0,"id":"lead","rootId":"lead","parentId":null,"epoch":0,"status":"running", "name":"Lead","cwd":directory.path().to_str().unwrap(),"isLead":true,"autoWake":true,"inFlight":true,"created":1.0,"accountKey":"default","provider":"codex","model":"gpt-6"});
        let child = json!({"compactions":0,"id":"review","rootId":"lead","parentId":"lead","epoch":0,"status":"queued","name":"Review","cwd":directory.path().to_str().unwrap(),"isLead":false,"autoWake":true,"inFlight":false,"created":2.0,"accountKey":"default","provider":"codex","role":"reviewer","model":"gpt-6","nativeReview":{"status":"pending","target":{"type":"uncommittedChanges"}}});
        store
            .transaction_with(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(lead)?)?;
                AgentRepository::put(tx, &AgentRecord::from_value(child)?)
            })
            .await
            .unwrap();
        let resources = preparation::PreparationResources::bundled(
            String::new(),
            crate::model_catalog::ModelCatalogService::new(),
        )
        .unwrap();
        assert!(candidate(&store, "review", 0, "default").await.unwrap());
        run(Arc::clone(&store), &resources, "review", 0)
            .await
            .unwrap();
        run(Arc::clone(&store), &resources, "review", 0)
            .await
            .unwrap();
        let reader = store.test_reader().unwrap();
        let record = AgentRepository::get(&reader, &AgentId::from("review"))
            .unwrap()
            .unwrap()
            .value();
        assert_eq!(record["status"], "failed");
        assert!(record["error"].as_str().unwrap().contains("older version"));
        assert_eq!(record["nativeReview"]["status"], "refused");
        let events: i64 = reader
            .query_row(
                "SELECT count(*) FROM runtime_events WHERE agent='lead' AND kind='child_result'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 1);
    }

    #[test]
    fn submitted_or_uncertain_legacy_review_is_not_claimed() {
        let mut record = json!({"nativeReview":{"status":"pending"},"status":"queued","autoWake":true,"startAttempt":{"submitted":true}});
        assert!(!claimable(&record));
        record["startAttempt"] = json!({"submitted":false,"observedTurnId":"turn"});
        assert!(!claimable(&record));
    }
}
