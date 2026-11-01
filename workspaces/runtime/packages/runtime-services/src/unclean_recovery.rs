//! Hold obligations whose sending commit may have been lost during an unclean stop.
use rusqlite::{Transaction, params};
use studio_runtime_schema::persistence::{OutboxKind, OutboxStatus, RestartRule};
use studio_runtime_store::outbox::Outbox;
use studio_runtime_store::remote_outbox::RemoteOutbox;
use studio_runtime_store::{RuntimeStore, SchedulerRepository, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Provider(#[from] crate::account_worker::WorkerError),
    #[error(transparent)]
    Callback(#[from] crate::provider_callbacks::CallbackError),
    #[error(transparent)]
    Voice(#[from] crate::voice::provider::VoiceProviderError),
    #[error(transparent)]
    Native(#[from] crate::native_actions::NativeError),
    #[error(transparent)]
    Title(#[from] crate::title_generation::TitleError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub async fn hold(store: &RuntimeStore) -> Result<(), RecoveryError> {
    store
        .transaction_with(|tx| {
            hold_entities(tx)?;
            SchedulerRepository::hold_unclean_starts(tx)?;
            RemoteOutbox::hold_unclean(tx).map_err(StoreError::from)?;
            crate::multi_server_orchestration::hold_unclean_server_requests(tx).map_err(
                |error| match error {
                    crate::multi_server_auth::AccessError::Store(error) => error,
                    other @ (crate::multi_server_auth::AccessError::Refused { .. }
                    | crate::multi_server_auth::AccessError::Stored { .. }
                    | crate::multi_server_auth::AccessError::Sqlite(_)) => {
                        StoreError::SchemaMismatch(other.to_string())
                    }
                },
            )?;
            Ok::<_, RecoveryError>(())
        })
        .await
}

/// The unclean-stop pass for every kind whose rule is `None`: the rule sets the row to
/// unknown, then the kind's entity hook writes its own row in the same transaction.
pub(crate) fn hold_entities(tx: &Transaction<'_>) -> Result<(), RecoveryError> {
    let kinds = {
        let mut query = tx.prepare("SELECT DISTINCT kind FROM runtime_outbox WHERE status=?1")?;
        query
            .query_map(params![OutboxStatus::Pending], |row| {
                row.get::<_, OutboxKind>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for kind in kinds {
        if kind.restart_rule() != RestartRule::None {
            continue;
        }
        for id in Outbox::hold_pending(tx, kind)? {
            hold_entity(tx, kind, &id)?;
        }
    }
    Ok(())
}

/// The entity hook of one held row. Kinds without a `None` rule have no hook.
fn hold_entity(tx: &Transaction<'_>, kind: OutboxKind, id: &str) -> Result<(), RecoveryError> {
    match kind {
        OutboxKind::ProviderCall => {}
        OutboxKind::ProviderCallback => crate::provider_callbacks::storage::hold_response(tx, id)?,
        OutboxKind::Steer => SchedulerRepository::hold_steer(tx, id)?,
        OutboxKind::VoiceStop | OutboxKind::VoiceDelete => crate::voice::hold_effect(tx, id)?,
        OutboxKind::NativeAction => crate::native_actions::hold_action(tx, id)?,
        OutboxKind::TitleGeneration => crate::title_generation::hold_job(tx, id)?,
        OutboxKind::JournalEntry
        | OutboxKind::Publication
        | OutboxKind::StreamFlush
        | OutboxKind::FollowupEffect
        | OutboxKind::WorkArchive
        | OutboxKind::WorkVm
        | OutboxKind::TerminalAction
        | OutboxKind::MonitorInput
        | OutboxKind::McpReconnect
        | OutboxKind::ProjectRegistration
        | OutboxKind::RemoteDelivery
        | OutboxKind::ManagedTool => {}
    }
    Ok(())
}
