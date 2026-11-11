//! Publish epoch-scoped waits from turn preparation and account session owners.
use studio_runtime_schema::agent_progress::AgentStartWait;
use studio_runtime_store::{AgentId, AgentRepository, AgentStatus, RuntimeStore, StoreError};

/// Publish the current admission owner's wait through the existing agent projection.
pub async fn show_blocked(
    store: &RuntimeStore,
    waits: Vec<(String, AgentStartWait)>,
) -> Result<(), StoreError> {
    if waits.is_empty() {
        return Ok(());
    }
    crate::scheduler::lifecycle_transaction_with(store, move |tx| {
        for (agent, wait) in waits {
            let Some(mut record) = AgentRepository::get(tx, &AgentId::from(agent.as_str()))? else {
                continue;
            };
            if !matches!(record.status, AgentStatus::Queued | AgentStatus::Starting)
                || record.epoch != wait.epoch()
            {
                continue;
            }
            if record.start_wait.as_ref() != Some(&wait) {
                record.start_wait = Some(wait);
                AgentRepository::put(tx, &record)?;
            }
        }
        Ok::<_, StoreError>(())
    })
    .await
}
