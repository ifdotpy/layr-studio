//! The `resolve` command: the user settles an outbox entry whose outcome is unknown.
//! Tracking: https://github.com/ifdotpy/layr-studio/issues/630
//!
//! `retry` queues the entry again with its id and body. `delivered` commits it. `discard`
//! cancels it and keeps its payload. Each kind's hook changes its own rows in the same
//! transaction, and the entry status follows. Kinds with the `Local` restart rule are not
//! resolved by a command.

use rusqlite::{Transaction, params};
use studio_runtime_schema::command::ResolveAction;
use studio_runtime_schema::persistence::{
    EventStatus, OutboxKind, OutboxStatus, OwnerKind, RequestReferenceKind, RestartRule,
    ServerOperationKind,
};
use studio_runtime_store::outbox::{Outbox, OutboxRow};
use studio_runtime_store::remote_outbox::RemoteEffect;
use studio_runtime_store::request_results::RequestReference;

use crate::commands::{CommandError, server_draft};

/// One stored outbox entry, read before the decision.
pub struct Entry {
    pub id: String,
    pub kind: OutboxKind,
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    pub status: OutboxStatus,
    pub payload: serde_json::Value,
}

const NOT_RESENT: &str = "This entry cannot be sent again. Discard it or mark it delivered.";
const STEER_NOT_RESENT: &str = "A steer is never sent again. Discard it or mark it delivered.";

/// The chat of an agent-owned entry, or `None`. A missing entry has no chat.
pub fn owner_chat(tx: &Transaction<'_>, id: &str) -> Result<Option<String>, CommandError> {
    let row: Option<OutboxRow<serde_json::Value>> = Outbox::get(tx, id)?;
    Ok(row
        .filter(|row| row.owner_kind == OwnerKind::Agent)
        .map(|row| row.owner_id))
}

/// Step 2 for `resolve`, read part.
pub fn read(tx: &Transaction<'_>, id: &str) -> Result<Entry, CommandError> {
    let row: OutboxRow<serde_json::Value> =
        Outbox::get(tx, id)?.ok_or(CommandError::Missing("This outbox entry does not exist"))?;
    Ok(Entry {
        id: row.id,
        kind: row.kind,
        owner_kind: row.owner_kind,
        owner_id: row.owner_id,
        status: row.status,
        payload: row.payload,
    })
}

/// Step 2 for `resolve`, decide part: no I/O.
pub fn decide(entry: &Entry, action: ResolveAction) -> Result<(), CommandError> {
    if entry.kind.restart_rule() == RestartRule::Local {
        return Err(CommandError::Invalid("This entry has no user resolution"));
    }
    if entry.status != OutboxStatus::Unknown {
        return Err(CommandError::Conflict(
            "Only an entry with an unknown outcome can be resolved",
        ));
    }
    if action == ResolveAction::Retry {
        retry_allowed(entry)?;
    }
    Ok(())
}

/// Exhaustive: each kind states whether a second send with the same id is safe.
fn retry_allowed(entry: &Entry) -> Result<(), CommandError> {
    let safe = match entry.kind {
        OutboxKind::ProviderCall => true,
        OutboxKind::RemoteDelivery => remote_retry_safe(&entry.payload),
        OutboxKind::Steer => {
            return Err(CommandError::Invalid(STEER_NOT_RESENT));
        }
        OutboxKind::ProviderCallback
        | OutboxKind::VoiceStop
        | OutboxKind::VoiceDelete
        | OutboxKind::TitleGeneration
        | OutboxKind::NativeAction
        | OutboxKind::JournalEntry
        | OutboxKind::Publication
        | OutboxKind::StreamFlush
        | OutboxKind::FollowupEffect
        | OutboxKind::WorkArchive
        | OutboxKind::WorkVm
        | OutboxKind::TerminalAction
        | OutboxKind::MonitorInput
        | OutboxKind::McpReconnect
        | OutboxKind::ProjectRegistration
        | OutboxKind::ManagedTool => false,
    };
    safe.then_some(()).ok_or(CommandError::Invalid(NOT_RESENT))
}

/// A remote entry: a server request follows its action, other families their rule.
fn remote_retry_safe(payload: &serde_json::Value) -> bool {
    let Some(effect) = payload["effect"].as_str().and_then(wire::<RemoteEffect>) else {
        return false;
    };
    match effect {
        RemoteEffect::ServerRequest => payload["record"]["body"]["action"]
            .as_str()
            .and_then(wire::<ServerOperationKind>)
            .is_some_and(crate::multi_server_orchestration::outgoing_retry),
        RemoteEffect::AccessRequest
        | RemoteEffect::ServerFetch
        | RemoteEffect::UpdateOffer
        | RemoteEffect::UpdateExport
        | RemoteEffect::UpdateChunk
        | RemoteEffect::UpdateRelease
        | RemoteEffect::FederationMessage => {
            effect.restart_rule() == studio_runtime_schema::persistence::RestartRule::TargetDedupes
        }
    }
}

/// Decode a closed wire name into its enum. An unknown name decodes to `None`.
fn wire<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    T::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(name)).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Settled {
    Delivered,
    Discarded,
}

/// Step 2 for `resolve`, apply part: the status, the entity hook, and the composer draft.
/// The reference is the entry itself.
pub fn apply(
    tx: &Transaction<'_>,
    entry: &Entry,
    action: ResolveAction,
    now_ms: i64,
) -> Result<RequestReference, CommandError> {
    match action {
        ResolveAction::Retry => set_status(tx, &entry.id, OutboxStatus::Pending, Some(now_ms))?,
        ResolveAction::Delivered => {
            settle_entity(tx, entry, Settled::Delivered)?;
            set_status(tx, &entry.id, OutboxStatus::Committed, None)?;
        }
        ResolveAction::Discard => {
            let text = settle_entity(tx, entry, Settled::Discarded)?;
            set_status(tx, &entry.id, OutboxStatus::Cancelled, None)?;
            if let Some(text) = text {
                server_draft(tx, &entry.owner_id, &text, now_ms)?;
            }
        }
    }
    Ok(RequestReference {
        kind: RequestReferenceKind::Outbox,
        id: entry.id.clone(),
        revision: now_ms,
    })
}

fn set_status(
    tx: &Transaction<'_>,
    id: &str,
    status: OutboxStatus,
    next_at: Option<i64>,
) -> Result<(), CommandError> {
    tx.execute(
        "UPDATE runtime_outbox SET status=?2,next_at=?3 WHERE id=?1",
        params![id, status, next_at],
    )?;
    Ok(())
}

/// The entity hook of one kind. Returns the text of a user message that the composer
/// gets back on `discard`. Exhaustive: a new kind cannot compile without a hook.
fn settle_entity(
    tx: &Transaction<'_>,
    entry: &Entry,
    settled: Settled,
) -> Result<Option<String>, CommandError> {
    let entity = |error: String| CommandError::Entity(error);
    match entry.kind {
        OutboxKind::ProviderCall | OutboxKind::RemoteDelivery => Ok(None),
        OutboxKind::ProviderCallback => {
            let outcome = match settled {
                Settled::Delivered => {
                    crate::provider_callbacks::storage::DeliveryOutcome::Submitted
                }
                Settled::Discarded => crate::provider_callbacks::storage::DeliveryOutcome::NotSent,
            };
            crate::provider_callbacks::storage::resolve_response(tx, &entry.id, outcome)
                .map_err(|error| entity(error.to_string()))?;
            Ok(None)
        }
        OutboxKind::Steer => {
            let (status, drafted) = match settled {
                Settled::Delivered => (EventStatus::Delivered, false),
                Settled::Discarded => (EventStatus::Cancelled, true),
            };
            let texts = studio_runtime_store::SchedulerRepository::settle_unknown_steer(
                tx, &entry.id, status,
            )?;
            Ok((drafted && !texts.is_empty()).then(|| texts.join("\n\n")))
        }
        OutboxKind::VoiceStop | OutboxKind::VoiceDelete => {
            let outcome = match settled {
                Settled::Delivered => studio_runtime_schema::persistence::VoiceStopOutcome::Written,
                Settled::Discarded => studio_runtime_schema::persistence::VoiceStopOutcome::NotSent,
            };
            crate::voice::settle_effect(tx, &entry.id, outcome)
                .map_err(|error| entity(error.to_string()))?;
            Ok(None)
        }
        OutboxKind::NativeAction => {
            let outcome = match settled {
                Settled::Delivered => crate::native_actions::Outcome::Acknowledged { result: None },
                Settled::Discarded => crate::native_actions::Outcome::Failed {
                    error: Some("Discarded by the user".into()),
                },
            };
            crate::native_actions::settle_action(tx, &entry.id, outcome)
                .map_err(|error| entity(error.to_string()))?;
            Ok(None)
        }
        OutboxKind::TitleGeneration => {
            let status = match settled {
                Settled::Delivered => studio_runtime_schema::persistence::TitleStatus::Applied,
                Settled::Discarded => studio_runtime_schema::persistence::TitleStatus::Discarded,
            };
            crate::title_generation::resolve_job(tx, &entry.id, status)
                .map_err(|error| entity(error.to_string()))?;
            Ok(None)
        }
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
        | OutboxKind::ManagedTool => {
            Err(CommandError::Invalid("This entry has no user resolution"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: OutboxKind, payload: serde_json::Value) -> Entry {
        Entry {
            id: "entry".into(),
            kind,
            owner_kind: OwnerKind::Agent,
            owner_id: "agent".into(),
            status: OutboxStatus::Unknown,
            payload,
        }
    }

    #[test]
    fn a_steer_is_never_retried() {
        let error = decide(
            &entry(OutboxKind::Steer, serde_json::json!({})),
            ResolveAction::Retry,
        )
        .expect_err("steer");
        assert!(matches!(error, CommandError::Invalid(STEER_NOT_RESENT)));
        decide(
            &entry(OutboxKind::Steer, serde_json::json!({})),
            ResolveAction::Discard,
        )
        .expect("discard a steer");
        decide(
            &entry(OutboxKind::Steer, serde_json::json!({})),
            ResolveAction::Delivered,
        )
        .expect("mark a steer delivered");
    }

    #[test]
    fn a_provider_call_may_be_retried() {
        decide(
            &entry(OutboxKind::ProviderCall, serde_json::json!({})),
            ResolveAction::Retry,
        )
        .expect("retry");
    }

    #[test]
    fn a_server_request_is_retried_only_for_a_replay_safe_action() {
        let request = |action: &str| {
            serde_json::json!({
                "effect": "server_request",
                "record": {"body": {"action": action}}
            })
        };
        assert!(remote_retry_safe(&request("message")));
        assert!(!remote_retry_safe(&request("move_spawn")));
    }

    #[test]
    fn an_access_request_is_never_retried() {
        let payload = serde_json::json!({"effect": "access_request", "record": {}});
        assert!(!remote_retry_safe(&payload));
    }

    #[test]
    fn a_local_kind_has_no_resolution() {
        let error = decide(
            &entry(OutboxKind::ManagedTool, serde_json::json!({})),
            ResolveAction::Delivered,
        )
        .expect_err("local");
        assert!(matches!(error, CommandError::Invalid(_)));
    }

    #[test]
    fn only_an_unknown_entry_can_be_resolved() {
        let mut pending = entry(OutboxKind::ProviderCall, serde_json::json!({}));
        pending.status = OutboxStatus::Pending;
        let error = decide(&pending, ResolveAction::Delivered).expect_err("pending");
        assert!(matches!(error, CommandError::Conflict(_)));
    }
}
