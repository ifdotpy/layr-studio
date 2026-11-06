//! Durable cleanup effects for agent workspaces.
//!
//! A cleanup intent is committed before its filesystem or guest effect. A
//! process restart turns an unfinished intent into `unknown`; callers must
//! inspect the external workspace and must never resend that effect.
use std::{path::PathBuf, sync::Arc, time::Duration};
use studio_runtime_store::path_identity::{CanonicalPath, LexicalPath};

use serde::{Deserialize, Serialize};
use studio_json::{OwnedRawJson, Value, json};
use studio_runtime_schema::queue_command::Field;
use studio_runtime_store::workspace_cleanup_intent::{
    AgentArchive, CleanupPhase, LifecycleIdentity, PartialWorktreeIdentity, StoredCleanupIntent,
    WorktreeIdentity,
};
pub use studio_runtime_store::workspace_cleanup_intent::{CleanupAction, CleanupEffect};
use studio_runtime_store::{AgentRecord, AgentRepository, RuntimeStore, StoreError};
use thiserror::Error;

#[path = "workspace_cleanup_submodules.rs"]
mod submodules;

/// Archive metadata retained for cleanup after final deletion revokes restoration.
pub(crate) const DELETED_ARCHIVE_FIELD: &str = "deletedArchive";
const INTENT_FIELD: &str = "workspaceCleanupIntent";

/// Lifecycle writers persist this before calling the cleanup service. No
/// external effect has started while an operation remains `queued`.
pub fn queued_cleanup_intent(
    agent_id: &str,
    epoch: i64,
    action: CleanupAction,
) -> StoredCleanupIntent {
    let operation_id = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        studio_json::to_string(&(agent_id, epoch, action))
            .expect("cleanup identity serializes")
            .as_bytes(),
    );
    StoredCleanupIntent {
        operation_id: Field::Value(receipt_json(&operation_id.to_string())),
        action,
        phase: CleanupPhase::Queued,
        request_id: Field::Missing,
        attempt: Field::Missing,
        effect: Field::Missing,
        identity: Field::Missing,
        receipt: Field::Missing,
        extra: Default::default(),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WorkspaceCleanupResult {
    pub agent_id: String,
    pub action: CleanupAction,
    pub workspace: Option<OwnedRawJson>,
    pub worktree: Option<OwnedRawJson>,
    pub held: bool,
}

#[derive(Debug, Error)]
pub enum WorkspaceCleanupError {
    #[error("Invalid agent workspace cleanup record")]
    InvalidRecord,
    #[error("Workspace cleanup is held for inspection")]
    Held,
    #[error("Workspace cleanup effect was not sent")]
    NotSent,
    #[error("Worktree retained: {0}")]
    UnsafeSubmodule(String),
    #[error("Workspace cleanup worker failed")]
    Worker,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Effects are split into one durable intent each, so later effects cannot be
/// sent until the earlier receipt has been committed.
/// Wire tags whose worktrees remain visible while cleanup is unresolved.
pub(crate) const RETAINED_WORKTREE_EFFECTS: [&str; 2] = ["remove_worktree", "save_worktree_ref"];

#[cfg(test)]
#[test]
fn retained_worktree_effect_tags_match_cleanup_effect_serialization() {
    let identity: Option<PartialWorktreeIdentity> = None;
    for (effect, tag) in [
        (
            CleanupEffect::RemoveWorktree {
                identity: identity.clone(),
            },
            RETAINED_WORKTREE_EFFECTS[0],
        ),
        (
            CleanupEffect::SaveWorktreeRef { identity },
            RETAINED_WORKTREE_EFFECTS[1],
        ),
    ] {
        assert_eq!(json!(effect)["kind"], tag);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EffectFailure {
    NotSent,
    Unavailable,
    UnsafeSubmodule(String),
    Unknown,
}

fn receipt_json<T: Serialize>(receipt: &T) -> OwnedRawJson {
    OwnedRawJson::from_string(studio_json::to_string(receipt).expect("receipt serializes"))
        .expect("serialized receipt is JSON")
}

fn null_receipt() -> OwnedRawJson {
    receipt_json(&())
}

#[derive(Serialize)]
struct ReasonReceipt<'a> {
    reason: &'a str,
}

#[derive(Serialize)]
struct KeptReceipt {
    state: &'static str,
    reason: &'static str,
}

#[derive(Serialize)]
struct StateReceipt {
    state: &'static str,
}

#[derive(Serialize)]
struct RemovedReceipt {
    state: &'static str,
    bytes: Option<u64>,
}

#[cfg(test)]
#[test]
fn persisted_vm_cleanup_effect_is_unavailable() {
    let persisted = json!({"action":"archive", "phase":"written", "requestId":"request",
        "effect":{"kind":"release_vm_line", "method":"line.remove"},
        "receipt":{"state":"released"}});
    let intent =
        CleanupIntent::from_raw(StoredCleanupIntent::deserialize(persisted.clone()).unwrap())
            .expect("historical receipt decodes");
    assert!(matches!(
        intent.progress,
        CleanupProgress::Written {
            effect: CompletedEffect::ReleaseVmLine,
            ..
        }
    ));
    let record: CleanupRecord =
        studio_json::from_str(&json!({"workspaceCleanupIntent": persisted}).to_string())
            .expect("historical stored cleanup intent decodes");
    assert_eq!(
        studio_runtime_store::projection_value(&record.workspace_cleanup_intent),
        persisted
    );
    assert!(matches!(
        CleanupIntent::from_raw(record.workspace_cleanup_intent)
            .expect("historical receipt is valid")
            .progress,
        CleanupProgress::Written {
            effect: CompletedEffect::ReleaseVmLine,
            ..
        }
    ));
    let effect: CleanupEffect = studio_json::from_str(&persisted["effect"].to_string()).unwrap();
    let ports = ProductionPorts {
        state_dir: PathBuf::new(),
    };
    assert_eq!(
        ports.perform(&AgentRecord::from_value(json!({"id":"a","rootId":"a","epoch":1,"status":"paused","provider":"codex","accountKey":"default","compactions":0})).unwrap(), &effect, "request"),
        Err(EffectFailure::Unavailable)
    );
    let unknown = CleanupIntent::from_raw(
        studio_json::from_str(r#"{"action":"archive","phase":"unknown","effect":{"kind":"save_vm_turn","turn_id":"turn"}}"#)
        .unwrap(),
    )
    .expect("uncertain historical intent decodes");
    assert!(matches!(unknown.progress, CleanupProgress::Unknown { .. }));
    for phase in ["unknown", "not_sent"] {
        let fixture = json!({"action":"archive", "phase":phase,
            "effect":{"kind":"remove_worktree", "identity":{"cwd":"/old/path", "future":true}},
            "receipt":null});
        let stored = StoredCleanupIntent::deserialize(fixture.clone()).unwrap();
        assert_eq!(studio_runtime_store::projection_value(&stored), fixture);
        let partial = CleanupIntent::from_raw(stored)
            .expect("retained historical partial worktree identity decodes");
        assert!(matches!(
            partial.progress,
            CleanupProgress::Unknown { .. } | CleanupProgress::NotSent { .. }
        ));
    }
    let null_identity = json!({"action":"archive", "phase":"unknown",
        "effect":{"kind":"remove_worktree", "identity":null}});
    let stored = StoredCleanupIntent::deserialize(null_identity.clone()).unwrap();
    assert_eq!(
        studio_runtime_store::projection_value(&stored),
        null_identity
    );
    assert!(matches!(
        CleanupIntent::from_raw(stored).unwrap().progress,
        CleanupProgress::Unknown { .. }
    ));
    let written = CleanupIntent::from_raw(
        studio_json::from_str(r#"{"action":"archive","phase":"written","requestId":"request","effect":{"kind":"save_worktree_ref","identity":{"cwd":"/old/path"}},"receipt":null}"#)
        .unwrap(),
    )
    .expect("historical written reference with partial identity decodes");
    assert!(matches!(
        written.progress,
        CleanupProgress::Written {
            effect: CompletedEffect::SaveWorktreeRef,
            ..
        }
    ));
    let pending = json!({"action":"delete", "phase":"pending", "requestId":"request",
        "attempt":1, "effect":{"kind":"remove_worktree", "identity":{"cwd":"/old/path"}},
        "identity":null, "receipt":null});
    let stored = StoredCleanupIntent::deserialize(pending.clone()).unwrap();
    assert_eq!(studio_runtime_store::projection_value(&stored), pending);
    assert!(matches!(
        CleanupIntent::from_raw(stored).unwrap().progress,
        CleanupProgress::Pending { .. }
    ));
}

/// Injectable boundary for deterministic corpus and fail-point tests.
pub trait CleanupPorts: Send + Sync {
    fn perform(
        &self,
        agent: &AgentRecord,
        effect: &CleanupEffect,
        request_id: &str,
    ) -> Result<OwnedRawJson, EffectFailure>;
}

/// Run the cleanup behavior for an already authorized stop, delete or archive
/// action. The caller commits the lifecycle decision first; this service owns
/// durable intent and workspace effects only.
pub async fn cleanup_agent_workspace(
    store: Arc<RuntimeStore>,
    agent_id: String,
    action: CleanupAction,
) -> Result<WorkspaceCleanupResult, WorkspaceCleanupError> {
    Box::pin(cleanup_agent_workspace_with_ports(
        Arc::clone(&store),
        agent_id,
        action,
        Arc::new(ProductionPorts {
            state_dir: store.root().to_path_buf(),
        }),
    ))
    .await
}

pub async fn cleanup_agent_workspace_with_ports(
    store: Arc<RuntimeStore>,
    agent_id: String,
    action: CleanupAction,
    ports: Arc<dyn CleanupPorts>,
) -> Result<WorkspaceCleanupResult, WorkspaceCleanupError> {
    let owner = read_agent(&store, agent_id.clone()).await?;
    let lifecycle = lifecycle_identity(&owner)?;
    if owner.deleted_at.is_none() {
        return Err(WorkspaceCleanupError::InvalidRecord);
    }
    if action == CleanupAction::Archive && !lifecycle.archive_present {
        return Err(WorkspaceCleanupError::InvalidRecord);
    }
    let effects = plan(&owner, action, store.state_dir())?;
    if effects.is_empty() {
        complete_without_effect(&store, &agent_id, action, &owner, &lifecycle.identity).await?;
        return Ok(result(&agent_id, action, None, None, false));
    }
    let mut workspace = None;
    let mut worktree = None;
    for effect in effects {
        let (request_id, replay) =
            begin_effect(&store, &agent_id, action, &effect, &lifecycle.identity).await?;
        if let Some((completed, raw)) = replay {
            finalize_effect(&store, &agent_id, &request_id, &completed).await?;
            if matches!(effect, CleanupEffect::RemoveWorktree { .. }) {
                worktree = Some(raw);
            } else {
                workspace = Some(raw);
            }
            continue;
        }
        let current = read_agent(&store, agent_id.clone()).await?;
        let stored = StoredCleanupIntent::deserialize(
            current
                .extra
                .get(INTENT_FIELD)
                .cloned()
                .ok_or(WorkspaceCleanupError::Held)?,
        )
        .map_err(|_| WorkspaceCleanupError::Held)?;
        let intent = CleanupIntent::from_raw(stored).map_err(|_| WorkspaceCleanupError::Held)?;
        if lifecycle_identity(&current)?.identity != lifecycle.identity {
            return Err(WorkspaceCleanupError::Held);
        }
        if !matches!(intent.progress, CleanupProgress::Pending { request_id: ref saved } if saved == &request_id)
        {
            return Err(WorkspaceCleanupError::Held);
        }
        let effect_ports = Arc::clone(&ports);
        let effect_agent = current;
        let effect_request_id = request_id.clone();
        let effect_for_worker = effect.clone();
        let outcome = studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-services/workspace_cleanup",
            move || effect_ports.perform(&effect_agent, &effect_for_worker, &effect_request_id),
        )
        .await
        .unwrap_or(Err(EffectFailure::Unknown));
        match outcome {
            Ok(receipt) => {
                complete_effect(&store, &agent_id, &request_id, &effect, &receipt).await?;
                if matches!(effect, CleanupEffect::RemoveWorktree { .. }) {
                    worktree = Some(receipt);
                } else {
                    workspace = Some(receipt);
                }
            }
            Err(EffectFailure::NotSent | EffectFailure::Unavailable) => {
                mark_effect(
                    &store,
                    &agent_id,
                    &request_id,
                    CleanupPhase::NotSent,
                    null_receipt(),
                )
                .await?;
                return Err(WorkspaceCleanupError::NotSent);
            }
            Err(EffectFailure::UnsafeSubmodule(reason)) => {
                mark_effect(
                    &store,
                    &agent_id,
                    &request_id,
                    CleanupPhase::NotSent,
                    receipt_json(&ReasonReceipt { reason: &reason }),
                )
                .await?;
                return Err(WorkspaceCleanupError::UnsafeSubmodule(reason));
            }
            Err(EffectFailure::Unknown) => {
                mark_effect(
                    &store,
                    &agent_id,
                    &request_id,
                    CleanupPhase::Unknown,
                    null_receipt(),
                )
                .await?;
                return Err(WorkspaceCleanupError::Held);
            }
        }
    }
    finalize_cleanup(&store, &agent_id, action).await?;
    Ok(result(&agent_id, action, workspace, worktree, false))
}

async fn complete_without_effect(
    store: &RuntimeStore,
    agent_id: &str,
    action: CleanupAction,
    expected_agent: &AgentRecord,
    expected_identity: &LifecycleIdentity,
) -> Result<(), WorkspaceCleanupError> {
    let agent_id = agent_id.to_owned();
    let expected_agent = Box::new(expected_agent.clone());
    let expected_identity = expected_identity.clone();
    store
        .transaction_with_error(move |tx| {
            let mut current =
                AgentRepository::get(tx, &studio_runtime_store::AgentId::from(agent_id.as_str()))?
                    .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            let current_identity = lifecycle_identity(&current)?.identity;
            if current_identity != expected_identity
                || current.extra.get(INTENT_FIELD) != expected_agent.extra.get(INTENT_FIELD)
            {
                return Err(WorkspaceCleanupError::Held);
            }
            if let Field::Value(stored) = take_intent(&mut current)? {
                let decoded = CleanupIntent::from_raw(stored)?;
                if decoded.action != action
                    || !matches!(
                        decoded.progress,
                        CleanupProgress::Queued { .. }
                            | CleanupProgress::Written { .. }
                            | CleanupProgress::NotSent { .. }
                    )
                    || match &decoded.identity {
                        Field::Missing => false,
                        Field::Value(identity) => !identity_matches(identity, &current_identity),
                        Field::Null => true,
                    }
                {
                    return Err(WorkspaceCleanupError::Held);
                }
            }
            if action == CleanupAction::Archive {
                // The planner emits no effects for an already absent owned
                // worktree. Only settle metadata; never mutate Git or a parent.
                if current.worktree_ready == Some(true) {
                    current.worktree_ready = Some(false);
                }
                if current.image_workspace.value() == Some(&true)
                    && current
                        .extra
                        .get("imageWorkspaceReady")
                        .cloned()
                        .and_then(|value| bool::deserialize(value).ok())
                        != Some(true)
                    && current
                        .extra
                        .get("imageWorkspacePhase")
                        .cloned()
                        .and_then(|value| String::deserialize(value).ok())
                        .as_deref()
                        != Some("archived")
                {
                    current.extra.insert(
                        "imageWorkspacePhase".into(),
                        studio_runtime_store::projection_value(&"archived"),
                    );
                }
                if let Some(archive) = &mut current.agent_archive
                    && archive.cleanup_pending == Some(true)
                {
                    archive.cleanup_pending = Some(false);
                }
            }
            AgentRepository::put(tx, &current)?;
            Ok::<(), WorkspaceCleanupError>(())
        })
        .await?;
    Ok(())
}

async fn finalize_cleanup(
    store: &RuntimeStore,
    agent_id: &str,
    action: CleanupAction,
) -> Result<(), WorkspaceCleanupError> {
    let agent_id = agent_id.to_owned();
    store
        .transaction_with_error(move |tx| {
            let mut agent =
                AgentRepository::get(tx, &studio_runtime_store::AgentId::from(agent_id.as_str()))?
                    .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            let Field::Value(intent) = take_intent(&mut agent)? else {
                return Err(WorkspaceCleanupError::InvalidRecord);
            };
            let intent =
                CleanupIntent::from_raw(intent).map_err(|_| WorkspaceCleanupError::Held)?;
            if intent.action != action
                || !matches!(intent.progress, CleanupProgress::Written { .. })
            {
                return Err(WorkspaceCleanupError::Held);
            }
            if let Field::Value(original) = &intent.identity
                && original.as_str().trim_start().starts_with('{')
            {
                let original: LifecycleIdentity = studio_json::from_str(original.as_str())
                    .map_err(|_| WorkspaceCleanupError::Held)?;
                let current = lifecycle_identity(&agent)?.identity;
                if original.id != current.id
                    || original.epoch != current.epoch
                    || !timestamp_matches(&original.deleted_at, &current.deleted_at)
                    || !timestamp_matches(&original.archive_at, &current.archive_at)
                    || original.archive_epoch != current.archive_epoch
                {
                    return Err(WorkspaceCleanupError::Held);
                }
            }
            if action == CleanupAction::Archive
                && let Some(archive) = &mut agent.agent_archive
                && archive.cleanup_pending == Some(true)
            {
                archive.cleanup_pending = Some(false);
            }
            AgentRepository::put(tx, &agent)?;
            Ok::<(), WorkspaceCleanupError>(())
        })
        .await?;
    Ok(())
}

/// Reconcile state left by a crash. Unfinished `pending` effects are changed
/// to `unknown` and held. A `written` receipt is safe to replay from the
/// store: it only finalizes that receipt and continues with the next effect.
pub async fn reconcile_pending_cleanup(
    store: Arc<RuntimeStore>,
) -> Result<Vec<WorkspaceCleanupResult>, WorkspaceCleanupError> {
    let rows = store
        .read_async("runtime-services/workspace_cleanup", |reader| {
            studio_runtime_store::StartupRecoveryRepository::agents_with_cleanup_intent(reader)
        })
        .await?;
    let mut held = Vec::new();
    for (id, raw) in rows {
        match reconcile_cleanup_intent(&store, &id, &raw).await {
            Ok(cleanup) => held.push(cleanup),
            Err(error) => eprintln!("Workspace cleanup remains pending for {id}: {error}"),
        }
    }
    Ok(held)
}

// This is a view of a persisted agent record, whose other fields belong to
// AgentRecord. The intent's receipt/effect fields remain owned by the effect runner.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CleanupRecord {
    workspace_cleanup_intent: StoredCleanupIntent,
}

// The storage envelope preserves malformed legacy rows; this type validates usable progress once.
struct CleanupIntent {
    action: CleanupAction,
    attempt: Option<OwnedRawJson>,
    identity: Field<OwnedRawJson>,
    effect: Option<OwnedRawJson>,
    progress: CleanupProgress,
}

enum CleanupProgress {
    Queued {
        request_id: Option<String>,
    },
    Written {
        request_id: String,
        effect: CompletedEffect,
        raw: OwnedRawJson,
    },
    NotSent {
        request_id: Option<String>,
    },
    Pending {
        request_id: String,
    },
    Unknown {
        request_id: Option<String>,
    },
}

impl CleanupProgress {
    fn request_id(&self) -> Option<&str> {
        match self {
            Self::Queued { request_id }
            | Self::NotSent { request_id }
            | Self::Unknown { request_id } => request_id.as_deref(),
            Self::Pending { request_id } | Self::Written { request_id, .. } => Some(request_id),
        }
    }
}

#[derive(Clone)]
enum CompletedEffect {
    ArchiveImage,
    RemoveImage,
    SaveNativeTurn {
        state_id: String,
    },
    ReleaseNativeLine,
    SaveVmTurn {
        turn_id: String,
        state_id: String,
    },
    ReleaseVmLine,
    SaveWorktreeRef,
    RemoveWorktree {
        identity: Option<Box<PartialWorktreeIdentity>>,
        bytes: Option<OwnedRawJson>,
    },
}

trait RetainedIdentity {
    fn retained(&self) -> Result<Option<Box<PartialWorktreeIdentity>>, WorkspaceCleanupError>;
}

impl RetainedIdentity for WorktreeIdentity {
    fn retained(&self) -> Result<Option<Box<PartialWorktreeIdentity>>, WorkspaceCleanupError> {
        Ok(Some(Box::new(self.clone().into())))
    }
}

impl RetainedIdentity for Option<OwnedRawJson> {
    fn retained(&self) -> Result<Option<Box<PartialWorktreeIdentity>>, WorkspaceCleanupError> {
        self.as_ref()
            .map(|identity| studio_json::from_str(identity.as_str()).map(Box::new))
            .transpose()
            .map_err(|_| WorkspaceCleanupError::InvalidRecord)
    }
}

impl CompletedEffect {
    fn checked<I: RetainedIdentity>(
        effect: &CleanupEffect<I>,
        receipt: &OwnedRawJson,
    ) -> Result<Self, WorkspaceCleanupError> {
        let state_id = || {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct StateId {
                state_id: Option<String>,
            }
            studio_json::from_str::<StateId>(receipt.as_str())
                .ok()
                .and_then(|receipt| receipt.state_id)
                .filter(|id| !id.is_empty())
                .ok_or(WorkspaceCleanupError::Held)
        };
        Ok(match effect {
            CleanupEffect::ArchiveImage => Self::ArchiveImage,
            CleanupEffect::RemoveImage => Self::RemoveImage,
            CleanupEffect::SaveNativeTurn => Self::SaveNativeTurn {
                state_id: state_id()?,
            },
            CleanupEffect::ReleaseNativeLine => Self::ReleaseNativeLine,
            CleanupEffect::SaveVmTurn { turn_id } => Self::SaveVmTurn {
                turn_id: turn_id.clone(),
                state_id: state_id()?,
            },
            CleanupEffect::ReleaseVmLine { .. } => Self::ReleaseVmLine,
            CleanupEffect::SaveWorktreeRef { .. } => Self::SaveWorktreeRef,
            CleanupEffect::RemoveWorktree { identity } => Self::RemoveWorktree {
                identity: identity.retained()?,
                bytes: {
                    #[derive(Deserialize)]
                    struct Bytes {
                        bytes: Option<OwnedRawJson>,
                    }
                    studio_json::from_str::<Bytes>(receipt.as_str())
                        .ok()
                        .and_then(|receipt| receipt.bytes)
                },
            },
        })
    }
}

impl CleanupIntent {
    fn from_raw(raw: StoredCleanupIntent) -> Result<Self, WorkspaceCleanupError> {
        fn field_value<T>(field: Field<T>) -> Option<T> {
            match field {
                Field::Value(value) => Some(value),
                Field::Missing | Field::Null => None,
            }
        }
        let receipt = field_value(raw.receipt).unwrap_or_else(null_receipt);
        let request_id = field_value(raw.request_id);
        let attempt = field_value(raw.attempt);
        let identity = raw.identity;
        let stored_effect = field_value(raw.effect);
        let effect: Option<CleanupEffect<Option<OwnedRawJson>>> = stored_effect
            .as_ref()
            .map(|effect| studio_json::from_str(effect.as_str()))
            .transpose()
            .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
        let progress = match raw.phase {
            CleanupPhase::Queued => CleanupProgress::Queued { request_id },
            CleanupPhase::Written => CleanupProgress::Written {
                request_id: request_id.ok_or(WorkspaceCleanupError::InvalidRecord)?,
                effect: CompletedEffect::checked(
                    effect
                        .as_ref()
                        .ok_or(WorkspaceCleanupError::InvalidRecord)?,
                    &receipt,
                )?,
                raw: receipt,
            },
            CleanupPhase::NotSent => CleanupProgress::NotSent { request_id },
            CleanupPhase::Pending => CleanupProgress::Pending {
                request_id: request_id.ok_or(WorkspaceCleanupError::InvalidRecord)?,
            },
            CleanupPhase::Unknown => CleanupProgress::Unknown { request_id },
        };
        Ok(Self {
            action: raw.action,
            attempt,
            identity,
            effect: stored_effect,
            progress,
        })
    }
}

async fn reconcile_cleanup_intent(
    store: &Arc<RuntimeStore>,
    id: &str,
    raw: &str,
) -> Result<WorkspaceCleanupResult, WorkspaceCleanupError> {
    let record: CleanupRecord =
        studio_json::from_str(raw).map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    let intent = CleanupIntent::from_raw(record.workspace_cleanup_intent)?;
    match intent.progress {
        CleanupProgress::Queued { .. }
        | CleanupProgress::Written { .. }
        | CleanupProgress::NotSent { .. } => {
            match cleanup_agent_workspace(Arc::clone(store), id.to_owned(), intent.action).await {
                Ok(cleanup) => return Ok(cleanup),
                Err(
                    WorkspaceCleanupError::Held
                    | WorkspaceCleanupError::NotSent
                    | WorkspaceCleanupError::UnsafeSubmodule(_),
                ) => {}
                Err(error) => return Err(error),
            }
        }
        CleanupProgress::Pending { request_id } => {
            mark_effect(
                store,
                id,
                &request_id,
                CleanupPhase::Unknown,
                null_receipt(),
            )
            .await?;
        }
        CleanupProgress::Unknown { .. } => {}
    }
    Ok(result(id, intent.action, None, None, true))
}

fn plan(
    owner: &AgentRecord,
    action: CleanupAction,
    state_dir: &std::path::Path,
) -> Result<Vec<CleanupEffect>, WorkspaceCleanupError> {
    if let Some(intent) = owner
        .extra
        .get(INTENT_FIELD)
        .filter(|intent| !intent.is_null())
    {
        let stored = StoredCleanupIntent::deserialize(intent.clone())
            .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
        let intent = CleanupIntent::from_raw(stored)?;
        if intent.action != action {
            return Err(WorkspaceCleanupError::Held);
        }
        if !matches!(
            intent.progress,
            CleanupProgress::Queued { .. }
                | CleanupProgress::NotSent { .. }
                | CleanupProgress::Written { .. }
        ) {
            return Err(WorkspaceCleanupError::Held);
        }
    }
    let mut effects = Vec::new();
    let is_vm = owner.execution_mode == Some(studio_runtime_store::AgentExecutionMode::Vm);

    let vm_dispose = is_vm && (owner.layr_ready == Some(true) || action == CleanupAction::Archive);
    if owner.workspace_mode == Some(studio_runtime_schema::records::WorkspaceMode::LayrNative)
        && owner.layr_ready == Some(true)
    {
        effects.push(CleanupEffect::SaveNativeTurn);
        effects.push(CleanupEffect::ReleaseNativeLine);
    } else if vm_dispose {
        if let Some(turns) = turn_saves(owner) {
            for (turn, state) in turns {
                if !turn_is_saved(&state) {
                    effects.push(CleanupEffect::SaveVmTurn { turn_id: turn });
                }
            }
        }
        effects.push(CleanupEffect::ReleaseVmLine {
            method: if owner.is_lead == Some(true) {
                "agent.release".into()
            } else {
                "line.remove".into()
            },
        });
    } else if action == CleanupAction::Archive && owner.worktree_ready == Some(true) {
        let Some(identity) = worktree_identity(owner, state_dir)? else {
            return Ok(effects);
        };
        effects.push(CleanupEffect::SaveWorktreeRef {
            identity: identity.clone(),
        });
        effects.push(CleanupEffect::RemoveWorktree { identity });
    }
    Ok(effects)
}

fn turn_saves(agent: &AgentRecord) -> Option<std::collections::BTreeMap<String, OwnedRawJson>> {
    agent.extra.get("layrTurnSaves").cloned().and_then(|value| {
        std::collections::BTreeMap::<String, OwnedRawJson>::deserialize(value).ok()
    })
}

fn turn_is_saved(value: &OwnedRawJson) -> bool {
    #[derive(Deserialize)]
    struct Status {
        status: Option<String>,
    }
    studio_json::from_str::<Status>(value.as_str())
        .ok()
        .is_some_and(|status| status.status.as_deref() == Some("saved"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedTurn<'a> {
    status: &'static str,
    state_id: &'a str,
}

/// The batch archive preflight uses the same original-link, clean-tree and
/// nested-worktree checks as the effect owner. Ignored build output is removable.
pub(crate) fn check_archive_worktree(
    agent: &Value,
    state_dir: &std::path::Path,
) -> Result<(), WorkspaceCleanupError> {
    // ManagementSnapshot still owns this open agent value; its typed owner is a follow-up.
    let ready = agent
        .get("worktreeReady")
        .cloned()
        .map(bool::deserialize)
        .transpose()
        .map_err(|_| WorkspaceCleanupError::InvalidRecord)?
        .unwrap_or(false);
    if ready
        && let Some(identity) =
            worktree_identity_from_input(WorktreeInput::from_snapshot(agent)?, state_dir)?
    {
        checked_cleanup_worktree(&identity).map_err(|_| WorkspaceCleanupError::NotSent)?;
    }
    Ok(())
}

/// Describe stale worktrees through the cleanup owner's exact path checks.
/// A clean Git tree alone does not authorize removal of the worker.
// ManagementSnapshot still supplies and consumes open JSON; its typed owner is a follow-up.
pub(crate) fn maintenance_worktree(agent: &Value, state_dir: &std::path::Path) -> Option<Value> {
    if agent["worktreeReady"] != true {
        return None;
    }
    let id = agent["id"].as_str()?;
    let deleted = studio_runtime_store::truthy(agent.get("deletedAt"));
    let root = agent["cwd"]
        .as_str()
        .and_then(|cwd| cleanup_cwd(std::path::Path::new(cwd)))
        .and_then(|cwd| {
            owned_worktree_root(&cwd, id)
                .ok()
                .map(|owned| owned.root().to_path_buf())
        });
    let Some(root) = root else {
        return deleted.then(|| {
            json!({"id":id,"folderMissing":false,
            "reason":"The recorded path is outside the Studio worktree folder"})
        });
    };
    let metadata = std::fs::symlink_metadata(&root);
    let missing = matches!(&metadata, Err(error) if error.kind() == std::io::ErrorKind::NotFound);
    if !deleted && !missing {
        return None;
    }
    let reason = if missing {
        "worktree folder missing"
    } else if metadata.is_err() {
        "The worktree folder state is unavailable"
    } else if check_archive_worktree(agent, state_dir).is_ok() {
        "The Git worktree is clean. Inspect the worker before removal"
    } else {
        "The worktree is changed or unverified. Preserve its files for inspection"
    };
    Some(json!({"id":id,"folderMissing":missing,"reason":reason}))
}

/// The owned worktree root of worker `id` that contains `cwd`
/// (`crate::worktree_root`). Layout only; the caller proves ownership.
fn owned_worktree_root<'a>(
    cwd: &'a std::path::Path,
    id: &str,
) -> Result<crate::worktree_root::OwnedRoot<'a>, WorkspaceCleanupError> {
    crate::worktree_root::owned_root(cwd, id).ok_or(WorkspaceCleanupError::InvalidRecord)
}

/// The main worktree of the repository that owns linked worktree `root`.
fn owning_repository(root: &std::path::Path) -> Result<PathBuf, WorkspaceCleanupError> {
    let common = String::from_utf8(
        git_output(
            root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            None,
        )
        .map_err(|_| WorkspaceCleanupError::InvalidRecord)?,
    )
    .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    let common = CanonicalPath::existing(common.trim_end_matches('\n'))
        .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    if common.file_name().and_then(|name| name.to_str()) != Some(".git") {
        return Err(WorkspaceCleanupError::InvalidRecord);
    }
    common
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or(WorkspaceCleanupError::InvalidRecord)
}

// Older agent records can contain one trailing directory separator. Keep the
// stored lifecycle spelling unchanged; do not accept traversal or other aliases.
fn cleanup_cwd(path: &std::path::Path) -> Option<LexicalPath> {
    LexicalPath::exact(path).or_else(|| {
        let legacy = path.to_str()?.strip_suffix(std::path::MAIN_SEPARATOR)?;
        LexicalPath::exact(std::path::Path::new(legacy))
    })
}

fn worktree_identity(
    owner: &AgentRecord,
    state_dir: &std::path::Path,
) -> Result<Option<WorktreeIdentity>, WorkspaceCleanupError> {
    worktree_identity_from_input(WorktreeInput::from_owner(owner)?, state_dir)
}

struct WorktreeInput<'a> {
    id: &'a str,
    cwd: &'a str,
    common_dir: Option<PathBuf>,
    epoch: Option<i64>,
    deleted_at: Option<f64>,
    archive: Option<AgentArchive>,
}

impl<'a> WorktreeInput<'a> {
    fn from_owner(owner: &'a AgentRecord) -> Result<Self, WorkspaceCleanupError> {
        Ok(Self {
            id: &owner.id.0,
            cwd: owner
                .cwd
                .as_deref()
                .ok_or(WorkspaceCleanupError::InvalidRecord)?,
            common_dir: worktree_common_dir(owner.extra.get("worktreeCommonDir"))?,
            epoch: Some(owner.epoch),
            deleted_at: owner.deleted_at,
            archive: owner.agent_archive.clone(),
        })
    }

    fn from_snapshot(agent: &'a Value) -> Result<Self, WorkspaceCleanupError> {
        Ok(Self {
            id: agent
                .get("id")
                .and_then(Value::as_str)
                .ok_or(WorkspaceCleanupError::InvalidRecord)?,
            cwd: agent
                .get("cwd")
                .and_then(Value::as_str)
                .ok_or(WorkspaceCleanupError::InvalidRecord)?,
            common_dir: worktree_common_dir(agent.get("worktreeCommonDir"))?,
            epoch: None,
            deleted_at: None,
            archive: None,
        })
    }
}

// AgentRecord.extra and ManagementSnapshot carry this field until their typed owners land.
fn worktree_common_dir(value: Option<&Value>) -> Result<Option<PathBuf>, WorkspaceCleanupError> {
    value
        .cloned()
        .map(Option::<PathBuf>::deserialize)
        .transpose()
        .map_err(|_| WorkspaceCleanupError::InvalidRecord)
        .map(Option::flatten)
}

fn worktree_identity_from_input(
    owner: WorktreeInput<'_>,
    _state_dir: &std::path::Path,
) -> Result<Option<WorktreeIdentity>, WorkspaceCleanupError> {
    let cwd = PathBuf::from(owner.cwd);
    let lexical = cleanup_cwd(&cwd).ok_or(WorkspaceCleanupError::InvalidRecord)?;
    let lexical_owned = owned_worktree_root(&lexical, owner.id)?;
    let lexical_root = lexical_owned.root();
    // Inspect the exact owned root, not a canonicalized parent or a missing cwd
    // subdirectory. A dangling link is still present and must stay held. The
    // worker directory itself must never be a link: cleanup would act on its
    // target, which may belong to another repository. Ancestors may be links.
    match std::fs::symlink_metadata(lexical_root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(WorkspaceCleanupError::InvalidRecord),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(WorkspaceCleanupError::InvalidRecord);
        }
        Ok(_) => {}
    }
    let id = owner.id;
    let cwd = CanonicalPath::existing(&cwd).map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    let owned = owned_worktree_root(&cwd, id)?;
    // The recorded spelling and the resolved directory must name the same
    // worker directory; otherwise a substituted path redirects cleanup.
    let resolved_root =
        CanonicalPath::existing(lexical_root).map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    if resolved_root.as_path() != owned.root() {
        return Err(WorkspaceCleanupError::InvalidRecord);
    }
    let root = owned.root();
    // Git registration proves repository membership, not Studio ownership: a
    // replaced ancestor can lead to another repository's registered worktree.
    // Require the Git common directory Studio provisioned from. Records from
    // before that field existed are accepted only in the `<repository>/.worktrees`
    // layout of their recorded path.
    let expected = match (&owner.common_dir, lexical_owned) {
        (Some(recorded), _) => CanonicalPath::existing(recorded),
        (None, crate::worktree_root::OwnedRoot::Repository { repository, .. }) => {
            let git = repository.join(".git");
            if git.is_file() {
                // Legacy workers can belong to a linked project. Prove that
                // project is registered before using its common directory.
                let project = CanonicalPath::existing(repository)
                    .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
                let main = owning_repository(&project)?;
                registered_worktree_branch(&main, &project)?;
                CanonicalPath::existing(main.join(".git"))
            } else {
                CanonicalPath::existing(git)
            }
        }
        (None, crate::worktree_root::OwnedRoot::Linked(_)) => {
            return Err(WorkspaceCleanupError::InvalidRecord);
        }
    }
    .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    let repo = owning_repository(root)?;
    if repo.join(".git").as_path() != expected.as_path() {
        return Err(WorkspaceCleanupError::InvalidRecord);
    }
    let repo = repo.as_path();
    let relative = cwd
        .strip_prefix(root)
        .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    let head = String::from_utf8(
        git_output(&cwd, &["rev-parse", "HEAD"], None)
            .map_err(|_| WorkspaceCleanupError::InvalidRecord)?,
    )
    .map_err(|_| WorkspaceCleanupError::InvalidRecord)?
    .trim()
    .to_owned();
    if head.is_empty() {
        return Err(WorkspaceCleanupError::InvalidRecord);
    }
    let registered_branch = registered_worktree_branch(repo, root)?;
    Ok(Some(WorktreeIdentity {
        id: id.to_owned(),
        cwd: cwd.to_string_lossy().into_owned(),
        root: root.to_string_lossy().into_owned(),
        repository: repo.to_string_lossy().into_owned(),
        relative: relative.to_string_lossy().into_owned(),
        head,
        branch: registered_branch,
        lifecycle_identity: Some((owner.epoch, Some(owner.cwd.to_owned()), owner.deleted_at)),
        epoch: owner.epoch,
        deleted_at: owner.deleted_at,
        archive: owner.archive,
    }))
}

fn registered_worktree_branch(
    repository: &std::path::Path,
    target_root: &std::path::Path,
) -> Result<Option<String>, WorkspaceCleanupError> {
    let listing = String::from_utf8(
        git_output(repository, &["worktree", "list", "--porcelain", "-z"], None)
            .map_err(|_| WorkspaceCleanupError::InvalidRecord)?,
    )
    .map_err(|_| WorkspaceCleanupError::InvalidRecord)?;
    let mut path: Option<PathBuf> = None;
    let mut branch = None;
    let mut detached = false;
    let finish_entry = |path: Option<&PathBuf>, branch: &Option<String>, detached: bool| {
        if path.is_some_and(|path| path == target_root) {
            if branch.is_some() || detached {
                Some(branch.clone())
            } else {
                None
            }
        } else {
            None
        }
    };
    for field in listing.split('\0').filter(|field| !field.is_empty()) {
        if let Some(value) = field.strip_prefix("worktree ") {
            if let Some(result) = finish_entry(path.as_ref(), &branch, detached) {
                return Ok(result);
            }
            path = CanonicalPath::existing(PathBuf::from(value))
                .ok()
                .map(CanonicalPath::into_path_buf);
            branch = None;
            detached = false;
        } else if field == "detached" {
            detached = true;
        } else if let Some(value) = field.strip_prefix("branch refs/heads/") {
            branch = Some(value.to_owned());
        }
    }
    finish_entry(path.as_ref(), &branch, detached).ok_or(WorkspaceCleanupError::InvalidRecord)
}

async fn read_agent(
    store: &RuntimeStore,
    agent_id: String,
) -> Result<Box<AgentRecord>, WorkspaceCleanupError> {
    store
        .read_async("runtime-services/workspace_cleanup", move |tx| {
            AgentRepository::get(tx, &studio_runtime_store::AgentId::from(agent_id.as_str()))?
                .map(Box::new)
                .ok_or(WorkspaceCleanupError::InvalidRecord)
        })
        .await
}

fn take_intent(
    agent: &mut AgentRecord,
) -> Result<Field<StoredCleanupIntent>, WorkspaceCleanupError> {
    agent
        .extra
        .remove(INTENT_FIELD)
        .map_or(Ok(Field::Missing), |value| {
            Field::<StoredCleanupIntent>::deserialize(value)
                .map_err(|_| WorkspaceCleanupError::InvalidRecord)
        })
}

fn set_intent(agent: &mut AgentRecord, intent: &Field<StoredCleanupIntent>) {
    if !intent.is_missing() {
        agent.extra.insert(
            INTENT_FIELD.to_owned(),
            studio_runtime_store::projection_value(intent),
        );
    }
}

fn same_cleanup_effect(stored: Option<&OwnedRawJson>, effect: &CleanupEffect) -> bool {
    // Match consumed fields only while preserving the stored raw effect unchanged.
    stored
        .and_then(|raw| studio_json::from_str::<CleanupEffect>(raw.as_str()).ok())
        .is_some_and(|stored| stored == *effect)
}

async fn begin_effect(
    store: &RuntimeStore,
    agent_id: &str,
    action: CleanupAction,
    effect: &CleanupEffect,
    expected_identity: &LifecycleIdentity,
) -> Result<(String, Option<(CompletedEffect, OwnedRawJson)>), WorkspaceCleanupError> {
    let agent_id = agent_id.to_owned();
    let effect = effect.clone();
    let identity = expected_identity.clone();
    let result = store
        .transaction_with_error(move |tx| {
            let mut agent =
                AgentRepository::get(tx, &studio_runtime_store::AgentId::from(agent_id.as_str()))?
                    .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            if lifecycle_identity(&agent)?.identity != identity {
                return Err(WorkspaceCleanupError::Held);
            }
            let attempt = if let Field::Value(stored) = take_intent(&mut agent)? {
                let decoded = CleanupIntent::from_raw(stored)?;
                match &decoded.identity {
                    Field::Missing => {}
                    Field::Value(previous) if identity_matches(previous, &identity) => {}
                    Field::Null | Field::Value(_) => return Err(WorkspaceCleanupError::Held),
                }
                let same_effect = decoded.action == action
                    && same_cleanup_effect(decoded.effect.as_ref(), &effect);
                match decoded.progress {
                    CleanupProgress::Written {
                        request_id,
                        effect: completed,
                        raw,
                    } if same_effect => {
                        return Ok((request_id, Some((completed, raw))));
                    }
                    CleanupProgress::Written { .. } => 0,
                    CleanupProgress::Queued { .. } if decoded.action == action => 0,
                    CleanupProgress::NotSent { .. } if same_effect => {
                        decoded
                            .attempt
                            .as_ref()
                            .and_then(|attempt| studio_json::from_str::<u64>(attempt.as_str()).ok())
                            .unwrap_or(0)
                            + 1
                    }
                    CleanupProgress::Queued { .. }
                    | CleanupProgress::NotSent { .. }
                    | CleanupProgress::Pending { .. }
                    | CleanupProgress::Unknown { .. } => return Err(WorkspaceCleanupError::Held),
                }
            } else {
                0
            };
            let request_id = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                studio_runtime_store::projection_value(&(&agent_id, action, &effect, attempt))
                    .to_string()
                    .as_bytes(),
            )
            .to_string();
            let intent = StoredCleanupIntent {
                operation_id: Field::Missing,
                request_id: Field::Value(request_id.clone()),
                attempt: Field::Value(receipt_json(&attempt)),
                action,
                phase: CleanupPhase::Pending,
                effect: Field::Value(receipt_json(&studio_runtime_store::projection_value(
                    &effect,
                ))),
                identity: Field::Value(receipt_json(&identity)),
                receipt: Field::Null,
                extra: Default::default(),
            };
            set_intent(&mut agent, &Field::Value(intent));
            AgentRepository::put(tx, &agent)?;
            Ok((request_id, None))
        })
        .await?;
    Ok(result)
}

struct Lifecycle {
    identity: LifecycleIdentity,
    archive_present: bool,
}

fn lifecycle_identity(agent: &AgentRecord) -> Result<Lifecycle, WorkspaceCleanupError> {
    let deleted_archive: Option<AgentArchive> = if agent.agent_archive.is_none() {
        agent
            .extra
            .get(DELETED_ARCHIVE_FIELD)
            .cloned()
            .map(Option::<AgentArchive>::deserialize)
            .transpose()
            .map_err(|_| WorkspaceCleanupError::InvalidRecord)?
            .flatten()
    } else {
        None
    };
    let archive = agent.agent_archive.as_ref().or(deleted_archive.as_ref());
    fn nullable<T>(value: Option<T>) -> Field<T> {
        value.map_or(Field::Null, Field::Value)
    }
    Ok(Lifecycle {
        identity: LifecycleIdentity {
            id: Field::Value(agent.id.0.clone()),
            epoch: Field::Value(agent.epoch),
            cwd: nullable(agent.cwd.clone()),
            deleted_at: nullable(agent.deleted_at.and_then(studio_json::Number::from_f64)),
            archive_at: nullable(
                archive.and_then(|archive| studio_json::Number::from_f64(archive.at)),
            ),
            archive_epoch: nullable(archive.map(|archive| archive.epoch)),
            extra: Default::default(),
        },
        archive_present: archive.is_some(),
    })
}

fn identity_matches(raw: &OwnedRawJson, current: &LifecycleIdentity) -> bool {
    studio_json::from_str::<LifecycleIdentity>(raw.as_str()).is_ok_and(|identity| {
        identity.id == current.id
            && identity.epoch == current.epoch
            && identity.cwd == current.cwd
            && timestamp_matches(&identity.deleted_at, &current.deleted_at)
            && timestamp_matches(&identity.archive_at, &current.archive_at)
            && identity.archive_epoch == current.archive_epoch
            && identity.extra == current.extra
    })
}

fn timestamp_matches(
    left: &Field<studio_json::Number>,
    right: &Field<studio_json::Number>,
) -> bool {
    match (left, right) {
        (Field::Value(left), Field::Value(right)) => left.as_f64() == right.as_f64(),
        _ => left == right,
    }
}

async fn complete_effect(
    store: &RuntimeStore,
    agent_id: &str,
    request_id: &str,
    effect: &CleanupEffect,
    receipt: &OwnedRawJson,
) -> Result<(), WorkspaceCleanupError> {
    let agent_id = agent_id.to_owned();
    let request_id = request_id.to_owned();
    let effect = effect.clone();
    let receipt = receipt.clone();
    let completed = store
        .transaction_with_error(move |tx| {
            let mut agent =
                AgentRepository::get(tx, &studio_runtime_store::AgentId::from(agent_id.as_str()))?
                    .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            let Field::Value(mut intent) = take_intent(&mut agent)? else {
                return Err(WorkspaceCleanupError::InvalidRecord);
            };
            let decoded =
                CleanupIntent::from_raw(intent.clone()).map_err(|_| WorkspaceCleanupError::Held)?;
            if decoded.progress.request_id() != Some(request_id.as_str()) {
                return Ok::<bool, WorkspaceCleanupError>(false);
            }
            let current_identity = lifecycle_identity(&agent)?.identity;
            if !decoded
                .identity
                .value()
                .is_some_and(|identity| identity_matches(identity, &current_identity))
            {
                intent.phase = CleanupPhase::Unknown;
                set_intent(&mut agent, &Field::Value(intent));
                AgentRepository::put(tx, &agent)?;
                return Ok(false);
            }
            let completed = match CompletedEffect::checked(&effect, &receipt) {
                Ok(receipt) => receipt,
                Err(_) => {
                    intent.phase = CleanupPhase::Unknown;
                    intent.receipt = Field::Value(receipt);
                    set_intent(&mut agent, &Field::Value(intent));
                    AgentRepository::put(tx, &agent)?;
                    return Ok(false);
                }
            };
            intent.phase = CleanupPhase::Written;
            intent.receipt = Field::Value(receipt);
            apply_effect_result(&mut agent, &completed)?;
            intent.identity = Field::Value(receipt_json(&lifecycle_identity(&agent)?.identity));
            set_intent(&mut agent, &Field::Value(intent));
            AgentRepository::put(tx, &agent)?;
            Ok(true)
        })
        .await?;
    if !completed {
        return Err(WorkspaceCleanupError::Held);
    }
    Ok(())
}

async fn finalize_effect(
    store: &RuntimeStore,
    agent_id: &str,
    request_id: &str,
    effect: &CompletedEffect,
) -> Result<(), WorkspaceCleanupError> {
    let agent_id = agent_id.to_owned();
    let request_id = request_id.to_owned();
    let effect = effect.clone();
    store
        .transaction_with_error(move |tx| {
            let mut agent =
                AgentRepository::get(tx, &studio_runtime_store::AgentId::from(agent_id.as_str()))?
                    .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            let Field::Value(intent) = take_intent(&mut agent)? else {
                return Err(WorkspaceCleanupError::InvalidRecord);
            };
            let decoded = CleanupIntent::from_raw(intent.clone())?;
            let current_identity = lifecycle_identity(&agent)?.identity;
            if decoded.progress.request_id() != Some(request_id.as_str())
                || !decoded
                    .identity
                    .value()
                    .is_some_and(|identity| identity_matches(identity, &current_identity))
            {
                return Err(WorkspaceCleanupError::Held);
            }
            apply_effect_result(&mut agent, &effect)?;
            set_intent(&mut agent, &Field::Value(intent));
            AgentRepository::put(tx, &agent)?;
            Ok::<(), WorkspaceCleanupError>(())
        })
        .await?;
    Ok(())
}

fn apply_effect_result(
    agent: &mut AgentRecord,
    effect: &CompletedEffect,
) -> Result<(), WorkspaceCleanupError> {
    match effect {
        CompletedEffect::ArchiveImage | CompletedEffect::RemoveImage => {}
        CompletedEffect::ReleaseNativeLine | CompletedEffect::ReleaseVmLine => {
            agent.layr_ready = Some(false);
        }
        CompletedEffect::RemoveWorktree { identity, bytes } => {
            let identity = identity
                .as_ref()
                .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            let repository = identity
                .repository
                .value()
                .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            let relative = identity
                .relative
                .value()
                .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            agent.cwd = Some(
                std::path::PathBuf::from(repository)
                    .join(relative)
                    .to_string_lossy()
                    .into_owned(),
            );
            agent.worktree_ready = Some(false);
            agent.cleaned_worktree = Some(
                studio_runtime_store::workspace_cleanup_intent::CleanedWorktree {
                    root: identity.root.value().cloned(),
                    repo: Some(repository.clone()),
                    relative: Some(relative.clone()),
                    branch: identity.branch.value().cloned(),
                    head: identity.head.value().cloned(),
                    identity: identity.lifecycle_identity.value().cloned(),
                    bytes: bytes
                        .as_ref()
                        .map(|bytes| studio_json::from_str(bytes.as_str()))
                        .transpose()
                        .map_err(|_| WorkspaceCleanupError::InvalidRecord)?,
                    missing: None,
                    note: None,
                },
            );
        }
        CompletedEffect::SaveNativeTurn { state_id } => {
            agent.extra.insert(
                "layrStateId".into(),
                studio_runtime_store::projection_value(state_id),
            );
            if let Some(mut turns) = turn_saves(agent) {
                for save in turns.values_mut() {
                    if !turn_is_saved(save) {
                        *save = receipt_json(&SavedTurn {
                            status: "saved",
                            state_id,
                        });
                    }
                }
                agent.extra.insert(
                    "layrTurnSaves".into(),
                    studio_runtime_store::projection_value(&turns),
                );
            }
        }
        CompletedEffect::SaveWorktreeRef => {}
        CompletedEffect::SaveVmTurn { turn_id, state_id } => {
            let mut turns = match turn_saves(agent) {
                Some(turns) => turns,
                None if agent
                    .extra
                    .get("layrTurnSaves")
                    .is_none_or(|value| value.is_null()) =>
                {
                    Default::default()
                }
                None => return Err(WorkspaceCleanupError::InvalidRecord),
            };
            turns.insert(
                turn_id.clone(),
                receipt_json(&SavedTurn {
                    status: "saved",
                    state_id,
                }),
            );
            agent.extra.insert(
                "layrTurnSaves".into(),
                studio_runtime_store::projection_value(&turns),
            );
            agent.extra.insert(
                "layrStateId".into(),
                studio_runtime_store::projection_value(state_id),
            );
        }
    }
    Ok(())
}

async fn mark_effect(
    store: &RuntimeStore,
    agent_id: &str,
    request_id: &str,
    phase: CleanupPhase,
    receipt: OwnedRawJson,
) -> Result<(), WorkspaceCleanupError> {
    let agent_id = agent_id.to_owned();
    let request_id = request_id.to_owned();
    store
        .transaction_with_error(move |tx| {
            let mut agent =
                AgentRepository::get(tx, &studio_runtime_store::AgentId::from(agent_id.as_str()))?
                    .ok_or(WorkspaceCleanupError::InvalidRecord)?;
            let Field::Value(mut intent) = take_intent(&mut agent)? else {
                return Err(WorkspaceCleanupError::InvalidRecord);
            };
            let decoded =
                CleanupIntent::from_raw(intent.clone()).map_err(|_| WorkspaceCleanupError::Held)?;
            if decoded.progress.request_id() != Some(request_id.as_str()) {
                return Err(WorkspaceCleanupError::Held);
            }
            intent.phase = phase;
            intent.receipt = Field::Value(receipt);
            set_intent(&mut agent, &Field::Value(intent));
            AgentRepository::put(tx, &agent)?;
            Ok::<(), WorkspaceCleanupError>(())
        })
        .await?;
    Ok(())
}

fn result(
    agent_id: &str,
    action: CleanupAction,
    workspace: Option<OwnedRawJson>,
    worktree: Option<OwnedRawJson>,
    held: bool,
) -> WorkspaceCleanupResult {
    WorkspaceCleanupResult {
        agent_id: agent_id.to_owned(),
        action,
        workspace,
        worktree,
        held,
    }
}

struct ProductionPorts {
    state_dir: PathBuf,
}
impl CleanupPorts for ProductionPorts {
    fn perform(
        &self,
        agent: &AgentRecord,
        effect: &CleanupEffect,
        request_id: &str,
    ) -> Result<OwnedRawJson, EffectFailure> {
        match effect {
            CleanupEffect::SaveNativeTurn => {
                crate::layr_native::save(&self.state_dir, &agent.value(), request_id)
                    .map(|receipt| receipt_json(&receipt))
                    .map_err(|_| EffectFailure::Unknown)
            }
            CleanupEffect::ReleaseNativeLine => {
                crate::layr_native::remove(&self.state_dir, &agent.value(), request_id)
                    .map(|receipt| receipt_json(&receipt))
                    .map_err(|_| EffectFailure::Unknown)
            }
            // Historical image records keep their files. New cleanup plans never emit these effects.
            CleanupEffect::ArchiveImage | CleanupEffect::RemoveImage => {
                Ok(receipt_json(&KeptReceipt {
                    state: "kept",
                    reason: "legacy image workspace",
                }))
            }
            CleanupEffect::SaveVmTurn { .. } | CleanupEffect::ReleaseVmLine { .. } => {
                Err(EffectFailure::Unavailable)
            }
            CleanupEffect::SaveWorktreeRef { identity } => {
                save_worktree_archive_ref(identity)?;
                Ok(receipt_json(&StateReceipt { state: "saved" }))
            }
            CleanupEffect::RemoveWorktree { identity } => {
                remove_worktree(identity)?;
                Ok(receipt_json(&RemovedReceipt {
                    state: "removed",
                    bytes: None,
                }))
            }
        }
    }
}

fn git_output(
    repository: &std::path::Path,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, ()> {
    let environment = [
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_OPTIONAL_LOCKS", "0"),
        ("GIT_ALLOW_PROTOCOL", ""),
    ];
    let mut safe_args = vec!["-c", "protocol.allow=never"];
    safe_args.extend_from_slice(args);
    let output = crate::bounded_child::run_without_git_environment(crate::bounded_child::Request {
        program: std::ffi::OsStr::new("git"),
        git_directory: Some(repository.as_os_str()),
        args: &safe_args,
        environment: &environment,
        input,
        stdout_limit: 1024 * 1024,
        stderr_limit: 4096,
        timeout: Duration::from_secs(300),
    })
    .map_err(|_| ())?;
    if output.stdout.truncated {
        return Err(());
    }
    Ok(output.stdout.bytes)
}

fn check_worktree(identity: &WorktreeIdentity) -> Result<(PathBuf, PathBuf, String, String), ()> {
    let (repo, root, id, saved_head) = (
        PathBuf::from(&identity.repository),
        PathBuf::from(&identity.root),
        identity.id.clone(),
        identity.head.clone(),
    );
    let repository_root =
        String::from_utf8(git_output(&repo, &["rev-parse", "--show-toplevel"], None)?)
            .map_err(|_| ())?;
    if CanonicalPath::existing(PathBuf::from(repository_root.trim())).map_err(|_| ())?
        != CanonicalPath::existing(&repo).map_err(|_| ())?
    {
        return Err(());
    }
    if root.is_symlink() || !root.is_dir() || root.join(".git").is_symlink() {
        return Err(());
    }
    if !std::fs::symlink_metadata(root.join(".git"))
        .map_err(|_| ())?
        .file_type()
        .is_file()
    {
        return Err(());
    }
    let git_link = std::fs::read_to_string(root.join(".git")).map_err(|_| ())?;
    let linked_gitdir = git_link
        .strip_prefix("gitdir: ")
        .and_then(|path| path.strip_suffix('\n'))
        .filter(|path| !path.is_empty() && !path.contains('\n'))
        .map(PathBuf::from)
        .ok_or(())?;
    if !linked_gitdir.is_absolute() {
        return Err(());
    }
    let actual_gitdir = String::from_utf8(git_output(
        &root,
        &["rev-parse", "--absolute-git-dir"],
        None,
    )?)
    .map_err(|_| ())?;
    let common = String::from_utf8(git_output(
        &repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        None,
    )?)
    .map_err(|_| ())?;
    let common = CanonicalPath::existing(PathBuf::from(common.trim())).map_err(|_| ())?;
    let worker_common = String::from_utf8(git_output(
        &root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        None,
    )?)
    .map_err(|_| ())?;
    let linked_gitdir = CanonicalPath::existing(&linked_gitdir).map_err(|_| ())?;
    if CanonicalPath::existing(PathBuf::from(actual_gitdir.trim())).map_err(|_| ())?
        != linked_gitdir
        || CanonicalPath::existing(PathBuf::from(worker_common.trim())).map_err(|_| ())? != common
        || !linked_gitdir
            .starts_with(CanonicalPath::existing(common.join("worktrees")).map_err(|_| ())?)
    {
        return Err(());
    }
    let backlink = linked_gitdir.join("gitdir");
    if !std::fs::symlink_metadata(&backlink)
        .map_err(|_| ())?
        .file_type()
        .is_file()
    {
        return Err(());
    }
    let backlink = std::fs::read_to_string(backlink).map_err(|_| ())?;
    if CanonicalPath::existing(PathBuf::from(backlink.trim())).map_err(|_| ())?
        != CanonicalPath::existing(root.join(".git")).map_err(|_| ())?
    {
        return Err(());
    }
    let listing = String::from_utf8(git_output(
        &repo,
        &["worktree", "list", "--porcelain", "-z"],
        None,
    )?)
    .map_err(|_| ())?;
    let mut entry = std::collections::BTreeMap::new();
    let mut belongs = false;
    for field in listing.split('\0').filter(|field| !field.is_empty()) {
        if let Some((key, value)) = field.split_once(' ') {
            if key == "worktree" {
                if belongs {
                    break;
                }
                entry.clear();
                belongs = CanonicalPath::existing(PathBuf::from(value)).ok()
                    == CanonicalPath::existing(&root).ok();
            }
            if belongs {
                entry.insert(key, value);
            }
        } else if field == "bare" {
            continue;
        }
    }
    if !belongs || entry.get("HEAD").copied() != Some(saved_head.as_str()) {
        return Err(());
    }
    let branch = identity.branch.as_deref();
    if entry
        .get("branch")
        .and_then(|v| v.strip_prefix("refs/heads/"))
        != branch
    {
        return Err(());
    }
    for field in listing
        .split('\0')
        .filter(|field| field.starts_with("worktree "))
    {
        let registered_path = PathBuf::from(field.trim_start_matches("worktree "));
        if let Ok(registered_path) = CanonicalPath::existing(&registered_path)
            && registered_path != root
            && registered_path.starts_with(&root)
        {
            return Err(());
        }
    }
    let cwd = &identity.cwd;
    let relative = &identity.relative;
    if PathBuf::from(cwd)
        .strip_prefix(&root)
        .map_err(|_| ())?
        .to_string_lossy()
        != relative.as_str()
    {
        return Err(());
    }
    Ok((repo, root, id, saved_head))
}

fn checked_cleanup_worktree(
    identity: &WorktreeIdentity,
) -> Result<(PathBuf, PathBuf, String, String), EffectFailure> {
    let checked = check_worktree(identity).map_err(|_| EffectFailure::NotSent)?;
    submodules::inspect(&checked.1, &checked.0).map_err(EffectFailure::UnsafeSubmodule)?;
    let status = git_output(
        &checked.1,
        &[
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ],
        None,
    )
    .map_err(|_| EffectFailure::NotSent)?;
    if !status.is_empty() {
        return Err(EffectFailure::NotSent);
    }
    Ok(checked)
}

fn save_worktree_archive_ref(identity: &WorktreeIdentity) -> Result<(), EffectFailure> {
    let (repo, _root, id, head) = checked_cleanup_worktree(identity)?;
    let current = format!("refs/codex-agents/archive/{id}");
    let previous = String::from_utf8(
        git_output(
            &repo,
            &["for-each-ref", "--format=%(objectname)", &current],
            None,
        )
        .map_err(|_| EffectFailure::Unknown)?,
    )
    .map_err(|_| EffectFailure::Unknown)?;
    let previous = previous.trim();
    let zero = "0".repeat(head.len());
    let mut commands = String::from("start\n");
    if !previous.is_empty() && previous != head {
        let history = format!("refs/codex-agents/archive-history/{id}/{previous}");
        let existing = String::from_utf8(
            git_output(
                &repo,
                &["for-each-ref", "--format=%(objectname)", &history],
                None,
            )
            .map_err(|_| EffectFailure::Unknown)?,
        )
        .map_err(|_| EffectFailure::Unknown)?;
        if existing.trim().is_empty() {
            commands.push_str(&format!("create {history} {previous}\n"));
        } else if existing.trim() != previous {
            return Err(EffectFailure::Unknown);
        }
    }
    commands.push_str(&format!(
        "update {current} {head} {}\nprepare\ncommit\n",
        if previous.is_empty() {
            zero.as_str()
        } else {
            previous
        }
    ));
    git_output(&repo, &["update-ref", "--stdin"], Some(commands.as_bytes()))
        .map_err(|_| EffectFailure::Unknown)?;
    Ok(())
}

fn remove_worktree(identity: &WorktreeIdentity) -> Result<(), EffectFailure> {
    let (repo, root, _, _) = checked_cleanup_worktree(identity)?;
    let root = root.to_str().ok_or(EffectFailure::NotSent)?;
    git_output(&repo, &["worktree", "remove", "--force", root], None)
        .map_err(|_| EffectFailure::Unknown)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use studio_json::{Value, json};
    use studio_runtime_store::{AgentRecord, AgentRepository, RuntimeStore};

    use super::{
        CleanupAction, CleanupEffect, CleanupPorts, EffectFailure, WorkspaceCleanupError,
        begin_effect, cleanup_agent_workspace_with_ports, complete_effect, queued_cleanup_intent,
        reconcile_pending_cleanup,
    };

    #[test]
    fn cleanup_effect_comparison_uses_typed_values() {
        let effect = CleanupEffect::SaveVmTurn {
            turn_id: "turn-1".into(),
        };
        let raw = |s: &str| studio_json::OwnedRawJson::from_string(s.to_owned()).unwrap();
        assert!(super::same_cleanup_effect(
            Some(&raw(
                r#"{ "turn_id": "turn\u002d1", "kind": "save_vm_turn" }"#
            )),
            &effect,
        ));
        assert!(!super::same_cleanup_effect(
            Some(&raw(r#"{"kind":"save_vm_turn","turn_id":"turn-2"}"#)),
            &effect,
        ));
        assert!(!super::same_cleanup_effect(None, &effect));
    }

    #[tokio::test]
    async fn equivalent_persisted_effect_replays_or_retries_by_phase() {
        let (_directory, store) = fixture().await;
        let legacy_identity = json!({
            "id":"cleanup-agent", "cwd":"/work", "root":"/work", "repository":"/repo",
            "relative":"work", "head":"abc", "branch":null,
            "lifecycleIdentity":[7, "/work", 7], "epoch":7, "deletedAt":7,
            "archive":{"at":7, "epoch":7}
        });
        let identity: super::WorktreeIdentity =
            studio_json::from_value(legacy_identity.clone()).unwrap();
        let effect = CleanupEffect::SaveWorktreeRef { identity };
        for phase in ["written", "not_sent"] {
            put_agent(
                &store,
                json!({"workspaceCleanupIntent": {
                    "action": "archive", "phase": phase, "requestId": "old-request",
                    "attempt": 4, "effect": {"identity": legacy_identity, "kind": "save_worktree_ref"},
                    "receipt": null
                }}),
            )
            .await;
            let owner = worktree_owner(agent(&store));
            let identity = super::lifecycle_identity(&owner).unwrap().identity;
            let (request, replay) = begin_effect(
                &store,
                "cleanup-agent",
                CleanupAction::Archive,
                &effect,
                &identity,
            )
            .await
            .expect("equivalent effect accepted");
            if phase == "written" {
                assert_eq!(request, "old-request");
                assert!(replay.is_some());
            } else {
                assert_ne!(request, "old-request");
                assert!(replay.is_none());
                assert_eq!(agent(&store)["workspaceCleanupIntent"]["attempt"], 5);
            }
        }
    }

    struct FakePorts {
        outcomes: Mutex<Vec<Result<Value, EffectFailure>>>,
        calls: Mutex<Vec<(CleanupEffect, String)>>,
    }

    impl FakePorts {
        fn new(outcomes: Vec<Result<Value, EffectFailure>>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl CleanupPorts for FakePorts {
        fn perform(
            &self,
            _agent: &AgentRecord,
            effect: &CleanupEffect,
            request_id: &str,
        ) -> Result<studio_json::OwnedRawJson, EffectFailure> {
            self.calls
                .lock()
                .expect("calls mutex")
                .push((effect.clone(), request_id.to_owned()));
            self.outcomes
                .lock()
                .expect("outcomes mutex")
                .remove(0)
                .map(|value| super::receipt_json(&value))
        }
    }

    async fn fixture() -> (tempfile::TempDir, Arc<RuntimeStore>) {
        let (directory, _root, store) =
            crate::remote_transport::tests::runtime_store_fixture().await;
        (directory, store)
    }

    async fn put_agent(store: &RuntimeStore, mut record: Value) {
        record["provider"] = json!(studio_runtime_schema::ProviderKind::Codex);
        record["accountKey"] = json!("default");
        record["id"] = json!("cleanup-agent");
        record["rootId"] = json!("cleanup-agent");
        record["epoch"] = json!(7);
        record["status"] = json!("paused");
        record["compactions"] = json!(0);
        record["deletedAt"] = json!(7);
        if record["agentArchive"].is_object() {
            record["agentArchive"]
                .as_object_mut()
                .expect("archive object")
                .entry("at")
                .or_insert(json!(7));
            record["agentArchive"]
                .as_object_mut()
                .expect("archive object")
                .entry("epoch")
                .or_insert(json!(7));
        }
        let agent = AgentRecord::from_value(record).expect("agent record parses");
        store
            .transaction(move |tx| AgentRepository::put(tx, &agent))
            .await
            .expect("agent stores");
    }

    fn agent(store: &RuntimeStore) -> Value {
        let record = store
            .test_reader()
            .expect("test reader")
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='cleanup-agent'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("agent reads");
        studio_json::from_str(&record).expect("stored agent parses")
    }

    fn worktree_owner(mut value: Value) -> AgentRecord {
        let object = value.as_object_mut().expect("agent object");
        object.entry("rootId").or_insert(json!("cleanup-agent"));
        object.entry("epoch").or_insert(json!(1));
        object.entry("status").or_insert(json!("paused"));
        object.entry("provider").or_insert(json!("codex"));
        object.entry("accountKey").or_insert(json!("default"));
        object.entry("compactions").or_insert(json!(0));
        AgentRecord::from_value(value).expect("worktree owner")
    }

    #[test]
    fn restore_keeps_cleaned_workspaces_deleted_and_refuses_a_cleaned_root() {
        use crate::agent_management::{Action, ManagementSnapshot, Records, decide_with_effects};
        use std::collections::BTreeMap;

        for cleaned_root in [false, true] {
            for native in [false, true] {
                let make = |id: &str, parent: Option<&str>| {
                    json!({
                        "id":id, "rootId":"root", "parentId":parent, "epoch":1,
                        "status":"paused", "isLead":parent.is_none(), "compactions":0,
                        "provider":"codex", "accountKey":"default", "workspaceMode":"layr-native",
                        "layrReady":true, "layrLine":id, "cwd":"/project"
                    })
                };
                let snapshot = ManagementSnapshot::from_records(Records {
                    tables: BTreeMap::from([(
                        "runtime_agents".into(),
                        vec![make("root", None), make("child", Some("root"))],
                    )]),
                })
                .expect("snapshot");
                let deleted = decide_with_effects(
                    &snapshot,
                    &Action::Delete {
                        target: "root".into(),
                        request_id: "delete".into(),
                    },
                    10.0,
                )
                .expect("delete");
                let mut rows: Vec<_> = deleted
                    .writes
                    .into_iter()
                    .filter(|w| w.table == "runtime_agents")
                    .map(|w| w.record)
                    .collect();
                for row in &mut rows {
                    row.as_object_mut()
                        .expect("record")
                        .remove("workspaceCleanupIntent");
                    if row["id"] == if cleaned_root { "root" } else { "child" } {
                        let effect = if native {
                            super::CompletedEffect::ReleaseNativeLine
                        } else {
                            row["workspaceMode"] = json!("worktree");
                            super::CompletedEffect::RemoveWorktree {
                                identity: json!({"repository":"/project","relative":"worker"}),
                                bytes: Value::Null,
                            }
                        };
                        super::apply_effect_result(row, &effect).expect("cleanup completed");
                    }
                }
                let snapshot = ManagementSnapshot::from_records(Records {
                    tables: BTreeMap::from([("runtime_agents".into(), rows)]),
                })
                .expect("deleted snapshot");
                let restore = decide_with_effects(
                    &snapshot,
                    &Action::RestoreDeletedRoot {
                        target: "root".into(),
                    },
                    20.0,
                );
                if cleaned_root {
                    assert!(restore.is_err(), "cleaned root, native={native}");
                } else {
                    let restored = restore.expect("restore root");
                    assert_eq!(restored.writes.len(), 1, "cleaned child, native={native}");
                    assert_eq!(restored.writes[0].record["id"], "root");
                }
            }
        }
    }

    fn restore_after_vm_cleanup(cleaned_root: bool) {
        use crate::agent_management::{Action, ManagementSnapshot, Records, decide_with_effects};
        use std::collections::BTreeMap;
        let make = |id: &str, parent: Option<&str>| {
            json!({
                "id":id, "rootId":"root", "parentId":parent, "epoch":1,
                "status":"paused", "isLead":parent.is_none(), "compactions":0,
                "provider":"codex", "accountKey":"default", "executionMode":"vm",
                "layrReady":true, "cwd":"/project"
            })
        };
        let snapshot = ManagementSnapshot::from_records(Records {
            tables: BTreeMap::from([(
                "runtime_agents".into(),
                vec![make("root", None), make("child", Some("root"))],
            )]),
        })
        .expect("snapshot");
        let deleted = decide_with_effects(
            &snapshot,
            &Action::Delete {
                target: "root".into(),
                request_id: "delete".into(),
            },
            10.0,
        )
        .expect("delete");
        let mut rows: Vec<_> = deleted
            .writes
            .into_iter()
            .filter(|w| w.table == "runtime_agents")
            .map(|w| w.record)
            .collect();
        for row in &mut rows {
            row.as_object_mut()
                .expect("record")
                .remove("workspaceCleanupIntent");
            if row["id"] == if cleaned_root { "root" } else { "child" } {
                super::apply_effect_result(row, &super::CompletedEffect::ReleaseVmLine)
                    .expect("cleanup completed");
            }
        }
        let snapshot = ManagementSnapshot::from_records(Records {
            tables: BTreeMap::from([("runtime_agents".into(), rows)]),
        })
        .expect("deleted snapshot");
        let restore = decide_with_effects(
            &snapshot,
            &Action::RestoreDeletedRoot {
                target: "root".into(),
            },
            20.0,
        );
        if cleaned_root {
            assert!(restore.is_err(), "cleaned VM root");
        } else {
            let restored = restore.expect("restore root");
            assert_eq!(restored.writes.len(), 1, "cleaned VM child");
            assert_eq!(restored.writes[0].record["id"], "root");
        }
    }

    #[test]
    fn restore_refuses_cleaned_vm_root() {
        restore_after_vm_cleanup(true);
    }

    #[test]
    fn restore_keeps_cleaned_vm_child_deleted() {
        restore_after_vm_cleanup(false);
    }

    #[test]
    fn shared_workspace_agent_never_plans_a_directory_removal() {
        let agent = json!({
            "id":"shared-child", "cwd":"/project", "workspaceMode":"shared",
            "worktree":false, "rootId":"shared-child", "epoch":1, "status":"paused",
            "provider":"codex", "accountKey":"default", "compactions":0,
        });
        let owner = AgentRecord::from_value(agent).expect("agent record");
        for action in [CleanupAction::Archive, CleanupAction::Delete] {
            assert!(
                super::plan(&owner, action, std::path::Path::new("/nonexistent-state"))
                    .expect("plan")
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn missing_archive_worktree_settles_without_external_effects() {
        let (directory, store) = fixture().await;
        let cwd = directory
            .path()
            .join("project/.worktrees/codex-agents/cleanup-agent");
        put_agent(&store, json!({
            "cwd":cwd, "worktreeReady":true,
            "agentArchive":{"cleanupPending":true},
            "workspaceCleanupIntent":super::queued_cleanup_intent("cleanup-agent", 7, CleanupAction::Archive)
        })).await;
        let ports = Arc::new(FakePorts::new(vec![]));
        let cleaned = cleanup_agent_workspace_with_ports(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
            ports.clone(),
        )
        .await
        .expect("missing workspace already cleaned");
        assert!(!cleaned.held);
        assert!(ports.calls.lock().expect("calls").is_empty());
        let saved = agent(&store);
        assert_eq!(saved["worktreeReady"], false);
        assert_eq!(saved["cwd"], json!(cwd));
        assert_eq!(saved["agentArchive"]["cleanupPending"], false);
        assert!(saved.get("workspaceCleanupIntent").is_none());
    }

    #[tokio::test]
    async fn missing_worktree_subdirectory_does_not_settle_a_live_root() {
        let (directory, store) = fixture().await;
        let root = directory
            .path()
            .join("project/.worktrees/codex-agents/cleanup-agent");
        std::fs::create_dir_all(&root).expect("live root");
        let marker = root.join("owned.txt");
        std::fs::write(&marker, "keep").expect("live marker");
        put_agent(&store, json!({
            "cwd":root.join("missing-subdirectory"), "worktreeReady":true,
            "agentArchive":{"cleanupPending":true},
            "workspaceCleanupIntent":super::queued_cleanup_intent("cleanup-agent", 7, CleanupAction::Archive)
        })).await;
        let ports = Arc::new(FakePorts::new(vec![]));
        assert!(
            cleanup_agent_workspace_with_ports(
                Arc::clone(&store),
                "cleanup-agent".into(),
                CleanupAction::Archive,
                ports.clone()
            )
            .await
            .is_err()
        );
        assert!(ports.calls.lock().expect("calls").is_empty());
        assert_eq!(
            std::fs::read_to_string(marker).expect("live marker"),
            "keep"
        );
        assert_eq!(agent(&store)["worktreeReady"], true);
        assert!(agent(&store).get("workspaceCleanupIntent").is_some());
    }

    #[tokio::test]
    async fn cleanup_failure_does_not_block_unrelated_queued_intents() {
        let (directory, store) = fixture().await;
        put_agent(&store, json!({
            "cwd":directory.path(), "worktreeReady":true,
            "agentArchive":{"cleanupPending":true},
            "workspaceCleanupIntent":super::queued_cleanup_intent("cleanup-agent", 7, CleanupAction::Archive)
        })).await;
        let other = AgentRecord::from_value(json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,
            "id":"z-unrelated", "rootId":"z-unrelated", "epoch":1, "status":"paused", "accountKey":"default", "deletedAt":1,
            "workspaceCleanupIntent":super::queued_cleanup_intent("z-unrelated", 1, CleanupAction::Delete)
        })).expect("unrelated record");
        store
            .transaction(move |tx| AgentRepository::put(tx, &other))
            .await
            .expect("seed unrelated");
        super::reconcile_pending_cleanup(Arc::clone(&store))
            .await
            .expect("isolate failure");
        assert!(
            agent(&store).get("workspaceCleanupIntent").is_some(),
            "failed intent is retained"
        );
        let raw: String = store
            .test_reader()
            .expect("reader")
            .query_row(
                "SELECT record FROM runtime_agents WHERE id='z-unrelated'",
                [],
                |row| row.get(0),
            )
            .expect("unrelated row");
        let saved: Value = studio_json::from_str(&raw).expect("record");
        assert!(
            saved.get("workspaceCleanupIntent").is_none(),
            "unrelated intent drains"
        );
    }

    #[tokio::test]
    async fn malformed_pending_legacy_intent_is_retained_for_inspection() {
        let (_directory, store) = fixture().await;
        let intent = json!({"action":"archive", "phase":"pending", "receipt":null});
        put_agent(
            &store,
            json!({"agentArchive":{"cleanupPending":true},
            "workspaceCleanupIntent":intent}),
        )
        .await;
        super::reconcile_pending_cleanup(Arc::clone(&store))
            .await
            .expect("reconciliation continues");
        assert_eq!(agent(&store)["workspaceCleanupIntent"], intent);
    }

    #[tokio::test]
    async fn pending_recovery_preserves_exact_identity_numbers() {
        let (_directory, store) = fixture().await;
        put_agent(
            &store,
            json!({"workspaceCleanupIntent":{
                "action":"archive", "phase":"pending", "requestId":"request",
                "identity":{"deletedAt":9007199254740993_u64,"archiveAt":7},
                "effect":{"kind":"archive_image"}, "receipt":null
            }}),
        )
        .await;
        let identity = |store: &RuntimeStore| -> String {
            store.test_reader().expect("reader").query_row(
                "SELECT record -> '$.workspaceCleanupIntent.identity' FROM runtime_agents WHERE id='cleanup-agent'",
                [], |row| row.get(0),
            ).expect("stored identity")
        };
        let before = identity(&store);
        super::reconcile_pending_cleanup(Arc::clone(&store))
            .await
            .expect("reconcile");
        assert_eq!(agent(&store)["workspaceCleanupIntent"]["phase"], "unknown");
        assert_eq!(identity(&store), before);
        assert!(before.contains("9007199254740993"));
        assert!(before.contains("\"archiveAt\":7"));
    }

    #[tokio::test]
    async fn pending_recovery_preserves_malformed_unused_envelope_fields() {
        let (_directory, store) = fixture().await;
        put_agent(
            &store,
            json!({"workspaceCleanupIntent":{
                "action":"archive", "phase":"pending", "requestId":"request",
                "attempt":-1, "operationId":{"legacy":true},
                "effect":{"kind":"archive_image"}, "receipt":null
            }}),
        )
        .await;
        super::reconcile_pending_cleanup(Arc::clone(&store))
            .await
            .expect("reconcile");
        let intent = &agent(&store)["workspaceCleanupIntent"];
        assert_eq!(intent["phase"], "unknown");
        assert_eq!(intent["attempt"], -1);
        assert_eq!(intent["operationId"], json!({"legacy":true}));
    }

    #[tokio::test]
    async fn pending_recovery_accepts_incomplete_nested_archive() {
        let (_directory, store) = fixture().await;
        put_agent(
            &store,
            json!({"workspaceCleanupIntent":{
                "action":"archive", "phase":"pending", "requestId":"request",
                "effect":{"kind":"save_worktree_ref", "identity":{"cwd":"/old/path","archive":{}}},
                "receipt":null
            }}),
        )
        .await;
        super::reconcile_pending_cleanup(Arc::clone(&store))
            .await
            .expect("reconcile");
        let intent = &agent(&store)["workspaceCleanupIntent"];
        assert_eq!(intent["phase"], "unknown");
        assert_eq!(intent["effect"]["identity"]["archive"], json!({}));
    }

    #[tokio::test]
    async fn legacy_image_cleanup_preserves_files_and_record_fields() {
        let (_directory, store) = fixture().await;
        put_agent(
            &store,
            json!({"imageWorkspace":true,"imageWorkspaceReady":true}),
        )
        .await;
        let ports = Arc::new(FakePorts::new(vec![]));
        cleanup_agent_workspace_with_ports(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Delete,
            ports.clone(),
        )
        .await
        .expect("metadata cleanup");
        assert!(
            ports
                .calls
                .lock()
                .expect("fixture operation succeeds")
                .is_empty()
        );
        assert_eq!(agent(&store)["imageWorkspaceReady"], true);
    }

    #[tokio::test]
    async fn invalid_written_save_is_not_settled_when_plan_is_empty() {
        let (_directory, store) = fixture().await;
        put_agent(&store, json!({"agentArchive":{"cleanupPending":true}})).await;
        let mut saved = agent(&store);
        let identity = super::lifecycle_identity(&worktree_owner(saved.clone()))
            .unwrap()
            .identity;
        saved["workspaceCleanupIntent"] = json!({"action":"archive", "phase":"written", "effect":{"kind":"save_native_turn"}, "requestId":"request", "identity":identity, "receipt":null});
        store
            .transaction(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(saved).expect("record"))
            })
            .await
            .expect("stored");
        let ports = Arc::new(FakePorts::new(vec![]));
        assert!(
            cleanup_agent_workspace_with_ports(
                Arc::clone(&store),
                "cleanup-agent".into(),
                CleanupAction::Archive,
                ports.clone()
            )
            .await
            .is_err()
        );
        assert!(ports.calls.lock().expect("calls").is_empty());
        assert_eq!(agent(&store)["workspaceCleanupIntent"]["phase"], "written");
    }

    #[tokio::test]
    async fn written_worktree_ref_with_partial_identity_settles_and_keeps_agent() {
        let (_directory, store) = fixture().await;
        put_agent(&store, json!({"agentArchive":{"cleanupPending":true}})).await;
        let mut saved = agent(&store);
        let identity = super::lifecycle_identity(&worktree_owner(saved.clone()))
            .unwrap()
            .identity;
        saved["workspaceCleanupIntent"] = json!({"action":"archive", "phase":"written",
            "effect":{"kind":"save_worktree_ref", "identity":{"cwd":"/old/path","archive":{}}},
            "requestId":"request", "identity":identity, "receipt":null});
        store
            .transaction(move |tx| AgentRepository::put(tx, &AgentRecord::from_value(saved)?))
            .await
            .expect("historical row stored");
        let result = cleanup_agent_workspace_with_ports(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
            Arc::new(FakePorts::new(vec![])),
        )
        .await
        .expect("written receipt settles");
        assert!(!result.held);
        let saved = agent(&store);
        assert_eq!(saved["id"], "cleanup-agent");
        assert!(saved.get("workspaceCleanupIntent").is_none());
    }

    #[tokio::test]
    async fn written_legacy_integer_timestamps_match_current_agent() {
        let (_directory, store) = fixture().await;
        put_agent(&store, json!({"agentArchive":{"cleanupPending":true}})).await;
        let mut saved = agent(&store);
        let identity = super::lifecycle_identity(&worktree_owner(saved.clone()))
            .unwrap()
            .identity;
        let mut legacy_identity = studio_runtime_store::projection_value(&identity);
        legacy_identity["deletedAt"] = json!(7);
        legacy_identity["archiveAt"] = json!(7);
        saved["workspaceCleanupIntent"] = json!({"action":"archive", "phase":"written",
            "effect":{"kind":"save_worktree_ref", "identity":{"cwd":"/old/path"}},
            "requestId":"request", "identity":legacy_identity, "receipt":null});
        store
            .transaction(move |tx| AgentRepository::put(tx, &AgentRecord::from_value(saved)?))
            .await
            .expect("legacy row stored");
        let result = cleanup_agent_workspace_with_ports(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
            Arc::new(FakePorts::new(vec![])),
        )
        .await
        .expect("integer timestamps match");
        assert!(!result.held);
        assert!(agent(&store).get("workspaceCleanupIntent").is_none());
    }

    #[tokio::test]
    async fn written_save_without_state_id_is_held_without_replay() {
        let (_directory, store) = fixture().await;
        put_agent(&store, json!({})).await;
        let expected = worktree_owner(agent(&store));
        let expected = super::lifecycle_identity(&expected).unwrap().identity;
        let effect = CleanupEffect::SaveNativeTurn;
        let (request_id, _) = begin_effect(
            &store,
            "cleanup-agent",
            CleanupAction::Archive,
            &effect,
            &expected,
        )
        .await
        .expect("reserved");
        super::mark_effect(
            &store,
            "cleanup-agent",
            &request_id,
            super::CleanupPhase::Written,
            super::null_receipt(),
        )
        .await
        .expect("stored");
        let saved = worktree_owner(agent(&store));
        let saved = super::lifecycle_identity(&saved).unwrap().identity;
        assert!(
            begin_effect(
                &store,
                "cleanup-agent",
                CleanupAction::Archive,
                &effect,
                &saved
            )
            .await
            .is_err()
        );
        assert_eq!(agent(&store)["workspaceCleanupIntent"]["phase"], "written");
    }

    #[tokio::test]
    async fn late_receipt_after_lifecycle_change_is_held_without_applying_it() {
        let (_directory, store) = fixture().await;
        put_agent(
            &store,
            json!({"imageWorkspace":true,"imageWorkspaceReady":true}),
        )
        .await;
        let expected = worktree_owner(agent(&store));
        let expected = super::lifecycle_identity(&expected).unwrap().identity;
        let effect = CleanupEffect::RemoveImage;
        let (request_id, replay) = begin_effect(
            &store,
            "cleanup-agent",
            CleanupAction::Delete,
            &effect,
            &expected,
        )
        .await
        .expect("intent commits");
        assert!(replay.is_none());
        let mut changed = agent(&store);
        changed["epoch"] = json!(8);
        store
            .transaction(move |tx| {
                AgentRepository::put(
                    tx,
                    &AgentRecord::from_value(changed).expect("record parses"),
                )
            })
            .await
            .expect("new lifecycle commits");
        assert!(matches!(
            complete_effect(
                &store,
                "cleanup-agent",
                &request_id,
                &effect,
                &super::receipt_json(&json!({"state":"removed"}))
            )
            .await,
            Err(WorkspaceCleanupError::Held)
        ));
        let saved = agent(&store);
        assert_eq!(saved["epoch"], 8);
        assert_eq!(saved["imageWorkspaceReady"], true);
        assert_eq!(saved["workspaceCleanupIntent"]["phase"], "unknown");
    }

    #[tokio::test]
    async fn no_effect_completion_preserves_a_replacement_lifecycle_and_intent() {
        let (_directory, store) = fixture().await;
        put_agent(&store, json!({"agentArchive":{"cleanupPending":true}})).await;
        let mut queued = agent(&store);
        queued["workspaceCleanupIntent"] = studio_runtime_store::projection_value(
            &queued_cleanup_intent("cleanup-agent", 7, CleanupAction::Archive),
        );
        store
            .transaction(move |tx| {
                AgentRepository::put(tx, &AgentRecord::from_value(queued).expect("queued record"))
            })
            .await
            .expect("intent stages");
        let expected = agent(&store);
        let expected_owner = AgentRecord::from_value(expected.clone()).expect("agent record");
        let mut replacement = expected.clone();
        replacement["epoch"] = json!(8);
        replacement["workspaceCleanupIntent"]["operationId"] = json!("replacement-operation");
        store
            .transaction(move |tx| {
                AgentRepository::put(
                    tx,
                    &AgentRecord::from_value(replacement).expect("replacement record"),
                )
            })
            .await
            .expect("replacement lifecycle commits");

        assert!(matches!(
            super::complete_without_effect(
                &store,
                "cleanup-agent",
                CleanupAction::Archive,
                &expected_owner,
                &super::lifecycle_identity(&expected_owner).unwrap().identity
            )
            .await,
            Err(WorkspaceCleanupError::Held)
        ));
        let saved = agent(&store);
        assert_eq!(saved["epoch"], 8);
        assert_eq!(saved["agentArchive"]["cleanupPending"], true);
        assert_eq!(
            saved["workspaceCleanupIntent"]["operationId"],
            "replacement-operation"
        );
    }

    #[tokio::test]
    async fn archive_saves_a_fixture_repository_ref_before_removing_its_worktree() {
        let (directory, store) = fixture().await;
        // Git reports the repository in its resolved form.
        let repository = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder")
            .join("project");
        let root = repository.join(".worktrees/codex-agents/cleanup-agent");
        std::fs::create_dir_all(root.parent().expect("worktree parent creates"))
            .expect("fixture parent creates");
        fixture_git(
            directory.path(),
            &["init", "-q", repository.to_str().expect("repository path")],
        );
        let git = |args: &[&str]| fixture_git(&repository, args);
        std::fs::write(repository.join("README.md"), "fixture\n").expect("fixture file writes");
        git(&["add", "README.md"]);
        git(&["commit", "-q", "-m", "fixture"]);
        let previous = git(&["rev-parse", "HEAD"]);
        std::fs::write(repository.join("README.md"), "second fixture\n")
            .expect("second fixture file writes");
        git(&["commit", "-q", "-am", "second fixture"]);
        git(&[
            "update-ref",
            "refs/codex-agents/archive/cleanup-agent",
            &previous,
        ]);
        git(&[
            "update-ref",
            &format!("refs/codex-agents/archive-history/cleanup-agent/{previous}"),
            &previous,
        ]);
        git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "cleanup-agent",
            root.to_str().expect("fixture path is UTF-8"),
        ]);
        let head = git(&["rev-parse", "cleanup-agent"]);
        put_agent(
            &store,
            json!({
                "cwd":root, "branch":"cleanup-agent", "worktreeReady":true,
                "agentArchive":{"at":7,"epoch":7,"cleanupPending":true}
            }),
        )
        .await;
        super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".to_owned(),
            CleanupAction::Archive,
        )
        .await
        .expect("fixture worktree archives");
        assert!(!root.exists(), "target worktree is removed");
        assert_eq!(
            git(&["rev-parse", "refs/codex-agents/archive/cleanup-agent"]),
            head
        );
        let saved = agent(&store);
        assert_eq!(saved["worktreeReady"], false);
        assert_eq!(
            saved["cleanedWorktree"]["repo"],
            repository.to_string_lossy().to_string()
        );
        assert!(saved["cleanedWorktree"]["identity"].is_array());
        assert_eq!(
            saved.pointer("/agentArchive/cleanupPending"),
            Some(&json!(false))
        );
    }

    /// Archive a clean fixture worktree at `root` while `configured` names the
    /// worktree root in `worktree-root.json`.
    async fn archive_fixture_worktree_at(
        directory: &tempfile::TempDir,
        store: &Arc<RuntimeStore>,
        repository: &std::path::Path,
        root: &std::path::Path,
        configured: &std::path::Path,
    ) {
        std::fs::create_dir_all(configured).expect("configured root creates");
        std::fs::write(
            store
                .state_dir()
                .join(crate::worktree_root::WORKTREE_ROOT_FILE),
            json!({ "root": configured }).to_string(),
        )
        .expect("worktree root configuration writes");
        std::fs::create_dir_all(root.parent().expect("worktree parent"))
            .expect("fixture parent creates");
        fixture_git(
            directory.path(),
            &["init", "-q", repository.to_str().expect("repository path")],
        );
        let git = |args: &[&str]| fixture_git(repository, args);
        std::fs::write(repository.join("README.md"), "fixture\n").expect("fixture file writes");
        git(&["add", "README.md"]);
        git(&["commit", "-q", "-m", "fixture"]);
        git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "cleanup-agent",
            root.to_str().expect("fixture path is UTF-8"),
        ]);
        let head = git(&["rev-parse", "cleanup-agent"]);
        put_agent(
            store,
            json!({
                "cwd":root, "branch":"cleanup-agent", "worktreeReady":true,
                "worktreeCommonDir":repository.join(".git"),
                "agentArchive":{"at":7,"epoch":7,"cleanupPending":true}
            }),
        )
        .await;
        super::cleanup_agent_workspace(
            Arc::clone(store),
            "cleanup-agent".to_owned(),
            CleanupAction::Archive,
        )
        .await
        .expect("fixture worktree archives");
        assert!(!root.exists(), "target worktree is removed");
        assert_eq!(
            git(&["rev-parse", "refs/codex-agents/archive/cleanup-agent"]),
            head
        );
        let saved = agent(store);
        assert_eq!(saved["worktreeReady"], false);
        assert_eq!(
            saved["cleanedWorktree"]["repo"],
            repository.to_string_lossy().to_string()
        );
    }

    #[tokio::test]
    async fn archive_removes_a_worktree_under_the_configured_root() {
        let (directory, store) = fixture().await;
        let base = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder");
        let repository = base.join("project");
        let configured = base.join("other-disk/worktrees");
        let root = configured.join("codex-agents/cleanup-agent");
        archive_fixture_worktree_at(&directory, &store, &repository, &root, &configured).await;
    }

    #[tokio::test]
    async fn archive_keeps_repository_default_worktrees_manageable_after_configuration() {
        let (directory, store) = fixture().await;
        let base = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder");
        let repository = base.join("project");
        let root = repository.join(".worktrees/codex-agents/cleanup-agent");
        let configured = base.join("other-disk/worktrees");
        archive_fixture_worktree_at(&directory, &store, &repository, &root, &configured).await;
    }

    #[tokio::test]
    async fn archive_manages_a_configured_root_named_like_the_default() {
        let (directory, store) = fixture().await;
        let base = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder");
        let repository = base.join("project");
        let configured = base.join("disk/.worktrees");
        let root = configured.join("codex-agents/cleanup-agent");
        archive_fixture_worktree_at(&directory, &store, &repository, &root, &configured).await;
    }

    fn fixture_repository(base: &std::path::Path, name: &str) -> std::path::PathBuf {
        let repository = base.join(name);
        fixture_git(base, &["init", "-q", repository.to_str().expect("path")]);
        std::fs::write(repository.join("README.md"), "fixture\n").expect("file writes");
        fixture_git(&repository, &["add", "README.md"]);
        fixture_git(&repository, &["commit", "-q", "-m", "fixture"]);
        repository
    }

    /// A worker provisioned from a lead's linked worktree records the shared
    /// common directory, which cleanup must accept.
    #[test]
    fn worker_provisioned_from_a_linked_source_stays_owned() {
        let directory = tempfile::tempdir().expect("fixture directory");
        let base = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder");
        let repository = fixture_repository(&base, "project");
        let lead = base.join("lead-checkout");
        fixture_git(
            &repository,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                lead.to_str().expect("lead path"),
            ],
        );
        let root = base.join("configured/codex-agents/cleanup-agent");
        std::fs::create_dir_all(root.parent().expect("workers")).expect("workers create");
        fixture_git(
            &lead,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "cleanup-agent",
                root.to_str().expect("root path"),
            ],
        );
        let common = fixture_git(
            &lead,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        );
        let archive = json!({"at":7.0,"epoch":7,"by":"lead","reason":"accepted",
            "cleanupPending":true,"unknownToolRequests":["request"],"unassignedWork":["task"]});
        let agent = json!({"id": "cleanup-agent", "cwd": root, "worktreeCommonDir": common.trim(),
            "agentArchive":archive});
        let identity = super::worktree_identity(&worktree_owner(agent), &base)
            .expect("linked-source worker is owned")
            .expect("worker worktree identity");
        assert_eq!(PathBuf::from(identity.repository), repository);
        assert_eq!(
            studio_runtime_store::projection_value(&identity.archive),
            archive
        );
    }

    /// A legacy worker reached through a `.worktrees` link to another disk,
    /// recorded before the common directory existed, stays owned.
    #[test]
    fn legacy_worker_under_a_linked_default_root_stays_owned() {
        let directory = tempfile::tempdir().expect("fixture directory");
        let base = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder");
        let repository = fixture_repository(&base, "project");
        let disk = base.join("other-disk");
        std::fs::create_dir_all(disk.join("codex-agents")).expect("disk root creates");
        std::os::unix::fs::symlink(&disk, repository.join(".worktrees")).expect("root link");
        let root = repository.join(".worktrees/codex-agents/cleanup-agent");
        fixture_git(
            &repository,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "cleanup-agent",
                root.to_str().expect("root path"),
            ],
        );
        let agent = json!({"id": "cleanup-agent", "cwd": root});
        let identity = super::worktree_identity(&worktree_owner(agent), &base)
            .expect("legacy linked-root worker is owned")
            .expect("worker worktree identity");
        assert_eq!(PathBuf::from(identity.repository), repository);
    }

    /// A replaced `codex-agents` ancestor that leads to another repository's
    /// registered worktree must not pass ownership, with or without a record.
    #[test]
    fn substituted_worker_ancestor_is_refused_and_target_survives() {
        let directory = tempfile::tempdir().expect("fixture directory");
        let base = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder");
        let mut repositories = Vec::new();
        for name in ["project", "foreign"] {
            let repository = base.join(name);
            fixture_git(&base, &["init", "-q", repository.to_str().expect("path")]);
            std::fs::write(repository.join("README.md"), "fixture\n").expect("file writes");
            fixture_git(&repository, &["add", "README.md"]);
            fixture_git(&repository, &["commit", "-q", "-m", "fixture"]);
            repositories.push(repository);
        }
        let target = repositories[1].join(".worktrees/codex-agents/cleanup-agent");
        fixture_git(
            &repositories[1],
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                target.to_str().expect("target path"),
            ],
        );
        let configured = base.join("configured");
        std::fs::create_dir_all(&configured).expect("configured root creates");
        std::os::unix::fs::symlink(
            target.parent().expect("foreign workers"),
            configured.join("codex-agents"),
        )
        .expect("substitute ancestor link creates");
        let cwd = configured.join("codex-agents/cleanup-agent");
        for agent in [
            json!({"id": "cleanup-agent", "cwd": cwd, "worktreeCommonDir": repositories[0].join(".git")}),
            json!({"id": "cleanup-agent", "cwd": cwd}),
        ] {
            assert!(matches!(
                super::worktree_identity(&worktree_owner(agent), &base),
                Err(WorkspaceCleanupError::InvalidRecord)
            ));
        }
        assert!(target.join("README.md").exists());
    }

    #[test]
    fn worktree_outside_a_worker_directory_is_not_owned() {
        for path in [
            "/elsewhere/cleanup-agent",
            "/elsewhere/codex-agents/other-agent",
        ] {
            assert!(matches!(
                super::owned_worktree_root(std::path::Path::new(path), "cleanup-agent"),
                Err(WorkspaceCleanupError::InvalidRecord)
            ));
        }
    }

    /// A worker directory replaced by a link to another repository's worktree
    /// must never become the cleanup target.
    #[test]
    fn substituted_worker_link_is_refused_and_target_survives() {
        let directory = tempfile::tempdir().expect("fixture directory");
        let base = studio_runtime_store::path_identity::resolve(directory.path())
            .expect("resolved fixture folder");
        let foreign = base.join("foreign");
        fixture_git(
            &base,
            &["init", "-q", foreign.to_str().expect("foreign path")],
        );
        std::fs::write(foreign.join("README.md"), "foreign\n").expect("file writes");
        fixture_git(&foreign, &["add", "README.md"]);
        fixture_git(&foreign, &["commit", "-q", "-m", "foreign"]);
        let target = foreign.join(".worktrees/codex-agents/cleanup-agent");
        fixture_git(
            &foreign,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                target.to_str().expect("target path"),
            ],
        );
        let workers = base.join("configured/codex-agents");
        std::fs::create_dir_all(&workers).expect("workers directory creates");
        let link = workers.join("cleanup-agent");
        std::os::unix::fs::symlink(&target, &link).expect("substitute link creates");
        let agent = json!({"id": "cleanup-agent", "cwd": link, "worktreeReady": true});
        assert!(matches!(
            super::worktree_identity(&worktree_owner(agent), &base),
            Err(WorkspaceCleanupError::InvalidRecord)
        ));
        assert!(target.join("README.md").exists());
    }

    #[tokio::test]
    async fn archive_allows_a_verified_detached_worktree() {
        let (directory, store) = fixture().await;
        let repository = directory.path().join("detached-project");
        let root = repository.join(".worktrees/codex-agents/cleanup-agent");
        std::fs::create_dir_all(root.parent().expect("worktree parent creates"))
            .expect("fixture parent creates");
        fixture_git(
            directory.path(),
            &["init", "-q", repository.to_str().expect("repository path")],
        );
        let git = |args: &[&str]| fixture_git(&repository, args);
        std::fs::write(repository.join("README.md"), "fixture\n").expect("fixture file writes");
        git(&["add", "README.md"]);
        git(&["commit", "-q", "-m", "fixture"]);
        git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "codex-agent/cleanup-agent",
            root.to_str().expect("fixture path is UTF-8"),
        ]);
        fixture_git(&root, &["switch", "--detach", "HEAD"]);
        put_agent(
            &store,
            json!({
                "cwd":root, "branch":"codex-agent/cleanup-agent", "worktreeReady":true,
                "agentArchive":{"at":7,"epoch":7,"cleanupPending":true}
            }),
        )
        .await;
        super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".to_owned(),
            CleanupAction::Archive,
        )
        .await
        .expect("detached worktree archives");
        assert!(!root.exists(), "detached worktree is removed");
        assert_eq!(agent(&store)["cleanedWorktree"]["branch"], Value::Null);
    }

    #[tokio::test]
    async fn archive_keeps_parent_when_a_registered_nested_worktree_exists() {
        let (directory, store) = fixture().await;
        let repository = directory.path().join("nested-project");
        let root = repository.join(".worktrees/codex-agents/cleanup-agent");
        let nested = root.join(".worktrees/nested");
        std::fs::create_dir_all(root.parent().expect("worktree parent creates"))
            .expect("fixture parent creates");
        fixture_git(
            directory.path(),
            &["init", "-q", repository.to_str().expect("repository path")],
        );
        let git = |args: &[&str]| fixture_git(&repository, args);
        std::fs::write(repository.join("README.md"), "fixture\n").expect("fixture file writes");
        git(&["add", "README.md"]);
        git(&["commit", "-q", "-m", "fixture"]);
        git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "cleanup-agent",
            root.to_str().expect("fixture path is UTF-8"),
        ]);
        std::fs::write(repository.join(".git/info/exclude"), ".worktrees/\n")
            .expect("nested worktree path ignored");
        git(&[
            "worktree",
            "add",
            "-q",
            "--detach",
            nested.to_str().expect("nested path is UTF-8"),
        ]);
        put_agent(
            &store,
            json!({
                "cwd":root, "branch":"cleanup-agent", "worktreeReady":true,
                "agentArchive":{"at":7,"epoch":7,"cleanupPending":true}
            }),
        )
        .await;

        let error = super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".to_owned(),
            CleanupAction::Archive,
        )
        .await
        .expect_err("nested registration is unsafe to remove");
        assert!(matches!(error, WorkspaceCleanupError::NotSent));
        assert!(root.exists(), "parent worktree remains on disk");
        assert!(nested.exists(), "nested worktree remains on disk");
    }

    fn fixture_git(directory: &std::path::Path, args: &[&str]) -> String {
        let mut command = std::process::Command::new("git");
        for (key, _) in
            std::env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("GIT_"))
        {
            command.env_remove(key);
        }
        let output = command
            .arg("-C")
            .arg(directory)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Cleanup Fixture")
            .env("GIT_AUTHOR_EMAIL", "cleanup@example.invalid")
            .env("GIT_COMMITTER_NAME", "Cleanup Fixture")
            .env("GIT_COMMITTER_EMAIL", "cleanup@example.invalid")
            .output()
            .expect("fixture Git starts");
        assert!(
            output.status.success(),
            "git {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("fixture Git output is UTF-8")
            .trim()
            .to_owned()
    }

    #[test]
    fn linked_project_cleanup_path_preserves_only_legacy_trailing_separator() {
        let directory = tempfile::tempdir().expect("owned path fixture");
        let base = std::fs::canonicalize(directory.path()).expect("fixture path resolves");
        let root = base.join(".worktrees/codex-agents/cleanup-agent");
        for spelling in [root.display().to_string(), format!("{}/", root.display())] {
            assert_eq!(
                super::cleanup_cwd(std::path::Path::new(&spelling))
                    .map(|path| path.into_path_buf()),
                Some(root.clone())
            );
        }
        for spelling in [
            format!("{}//", root.display()),
            format!("{}/./", root.display()),
            format!("{}/../cleanup-agent", root.display()),
        ] {
            assert!(
                super::cleanup_cwd(std::path::Path::new(&spelling)).is_none(),
                "an alias must not become cleanup authority: {spelling}"
            );
        }
    }

    struct LinkedProjectFixture {
        store: Arc<RuntimeStore>,
        main: PathBuf,
        project: PathBuf,
        worker: PathBuf,
        _directory: tempfile::TempDir,
    }

    async fn linked_project_fixture() -> LinkedProjectFixture {
        let (directory, store) = fixture().await;
        let base = std::fs::canonicalize(directory.path()).expect("fixture path resolves");
        let main = base.join("main-project");
        let project = base.join("linked-project");
        let worker = project.join(".worktrees/codex-agents/cleanup-agent");
        std::fs::create_dir(&main).expect("main project creates");
        fixture_git(&main, &["init", "-q"]);
        std::fs::write(main.join("file.txt"), "original\n").expect("fixture file writes");
        fixture_git(&main, &["add", "file.txt"]);
        fixture_git(&main, &["commit", "-qm", "base"]);
        fixture_git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                project.to_str().expect("project path is UTF-8"),
            ],
        );
        fixture_git(
            &project,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                worker.to_str().expect("worker path is UTF-8"),
            ],
        );
        put_agent(
            &store,
            json!({"cwd":format!("{}/",worker.display()),"worktreeReady":true,
                "agentArchive":{"at":7,"epoch":7,"cleanupPending":true}}),
        )
        .await;
        LinkedProjectFixture {
            _directory: directory,
            store,
            main,
            project,
            worker,
        }
    }

    #[tokio::test]
    async fn linked_project_cleanup_retries_a_proven_unsent_archive_without_user_input() {
        let f = linked_project_fixture().await;
        let expected = agent(&f.store);
        let owner = AgentRecord::from_value(expected).expect("agent record");
        let effect = super::plan(&owner, CleanupAction::Archive, f.store.state_dir())
            .expect("archive plan")
            .remove(0);
        let (request, _) = begin_effect(
            &f.store,
            "cleanup-agent",
            CleanupAction::Archive,
            &effect,
            &super::lifecycle_identity(&owner).unwrap().identity,
        )
        .await
        .expect("original attempt reserves");
        super::mark_effect(
            &f.store,
            "cleanup-agent",
            &request,
            super::CleanupPhase::NotSent,
            super::null_receipt(),
        )
        .await
        .expect("old preflight refusal stores");
        let head = fixture_git(&f.main, &["rev-parse", "HEAD"]);

        let results = reconcile_pending_cleanup(Arc::clone(&f.store))
            .await
            .expect("automatic recovery runs");

        assert_eq!(results.len(), 1);
        assert!(
            !results[0].held,
            "a clean registered child of a linked project must clean automatically"
        );
        assert!(!f.worker.exists(), "only the worker is removed");
        assert!(
            f.project.join(".git").is_file(),
            "the linked project survives"
        );
        assert_eq!(fixture_git(&f.project, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            fixture_git(
                &f.main,
                &["rev-parse", "refs/codex-agents/archive/cleanup-agent"]
            ),
            head
        );
        assert_eq!(
            std::fs::read_to_string(f.main.join("file.txt")).expect("main file reads"),
            "original\n"
        );
        let saved = agent(&f.store);
        assert_eq!(saved["worktreeReady"], false);
        assert_eq!(saved["agentArchive"]["cleanupPending"], false);
        assert!(saved.get("workspaceCleanupIntent").is_none());
    }

    #[tokio::test]
    async fn linked_project_cleanup_preserves_modified_and_untracked_files() {
        let f = linked_project_fixture().await;
        std::fs::write(f.worker.join("file.txt"), "worker changes\n").expect("worker file changes");
        std::fs::write(f.worker.join("notes.txt"), "private notes\n")
            .expect("untracked file writes");
        let outcome = super::cleanup_agent_workspace(
            Arc::clone(&f.store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await;

        assert!(matches!(outcome, Err(WorkspaceCleanupError::NotSent)));
        assert_eq!(
            std::fs::read_to_string(f.worker.join("file.txt")).expect("worker file reads"),
            "worker changes\n"
        );
        assert_eq!(
            std::fs::read_to_string(f.worker.join("notes.txt")).expect("untracked file reads"),
            "private notes\n"
        );
        assert_eq!(agent(&f.store)["worktreeReady"], true);
    }

    #[tokio::test]
    async fn linked_project_cleanup_rejects_another_registered_worktree_gitdir() {
        let f = linked_project_fixture().await;
        let other = f.project.join(".worktrees/codex-agents/other-agent");
        fixture_git(
            &f.project,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                other.to_str().expect("other path is UTF-8"),
            ],
        );
        let link = std::fs::read(other.join(".git")).expect("other Git link reads");
        std::fs::write(f.worker.join(".git"), link).expect("worker Git link redirects");

        let outcome = super::cleanup_agent_workspace(
            Arc::clone(&f.store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await;

        assert!(matches!(outcome, Err(WorkspaceCleanupError::NotSent)));
        assert!(f.worker.exists(), "the unverified worker survives");
        assert!(other.exists(), "the other registered worktree survives");
        assert_eq!(agent(&f.store)["worktreeReady"], true);
    }

    async fn submodule_fixture() -> (
        tempfile::TempDir,
        Arc<RuntimeStore>,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        submodule_fixture_with_nested(false).await
    }

    async fn submodule_fixture_with_nested(
        nested: bool,
    ) -> (
        tempfile::TempDir,
        Arc<RuntimeStore>,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let (directory, store) = fixture().await;
        let base = std::fs::canonicalize(directory.path()).expect("fixture path resolves");
        let source = base.join("submodule-source");
        std::fs::create_dir(&source).expect("submodule source creates");
        fixture_git(&source, &["init", "-q"]);
        std::fs::write(source.join("file.txt"), "original\n").expect("source writes");
        if nested {
            let nested_source = base.join("nested-source");
            std::fs::create_dir(&nested_source).expect("nested source creates");
            fixture_git(&nested_source, &["init", "-q"]);
            std::fs::write(nested_source.join("file.txt"), "nested original\n")
                .expect("nested source writes");
            fixture_git(&nested_source, &["add", "."]);
            fixture_git(&nested_source, &["commit", "-qm", "nested base"]);
            fixture_git(
                &source,
                &[
                    "-c",
                    "protocol.file.allow=always",
                    "submodule",
                    "add",
                    nested_source.to_str().expect("nested source path is UTF-8"),
                    "nested",
                ],
            );
            fixture_git(
                &source,
                &[
                    "config",
                    "-f",
                    ".gitmodules",
                    "submodule.nested.ignore",
                    "all",
                ],
            );
        }
        fixture_git(&source, &["add", "."]);
        fixture_git(&source, &["commit", "-qm", "submodule base"]);
        let repository = base.join("project");
        std::fs::create_dir(&repository).expect("repository creates");
        fixture_git(&repository, &["init", "-q"]);
        fixture_git(
            &repository,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                source.to_str().expect("source path is UTF-8"),
                "module",
            ],
        );
        fixture_git(
            &repository,
            &[
                "config",
                "-f",
                ".gitmodules",
                "submodule.module.ignore",
                "all",
            ],
        );
        fixture_git(&repository, &["add", "."]);
        fixture_git(&repository, &["commit", "-qm", "parent base"]);
        let root = repository.join(".worktrees/codex-agents/cleanup-agent");
        fixture_git(
            &repository,
            &[
                "worktree",
                "add",
                "-qb",
                "cleanup-agent",
                root.to_str().expect("worktree path is UTF-8"),
            ],
        );
        fixture_git(
            &root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "update",
                "--init",
                "--recursive",
            ],
        );
        put_agent(
            &store,
            json!({
                "cwd":root, "branch":"cleanup-agent", "worktreeReady":true,
                "agentArchive":{"at":7,"epoch":7,"cleanupPending":true}
            }),
        )
        .await;
        (directory, store, repository, root)
    }

    async fn assert_submodule_edit_survives_archive(change: &str) {
        let (_directory, store, repository, root) = submodule_fixture().await;
        let module = root.join("module");
        let saved_gitdir = fixture_git(&module, &["rev-parse", "--absolute-git-dir"]);
        let file = module.join(if change == "untracked" {
            "untracked.txt"
        } else {
            "file.txt"
        });
        std::fs::write(&file, "worker data\n").expect("worker edit writes");
        let commit = if change == "commit" {
            fixture_git(&module, &["commit", "-qam", "worker commit"]);
            Some(fixture_git(&module, &["rev-parse", "HEAD"]))
        } else {
            None
        };
        assert_eq!(
            fixture_git(&root, &["status", "--porcelain"]),
            "",
            "ignore=all hides submodule changes"
        );
        let outcome = super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await;
        let archive = fixture_git(
            &repository,
            &[
                "for-each-ref",
                "--format=%(objectname)",
                "refs/codex-agents/archive/cleanup-agent",
            ],
        );
        eprintln!(
            "submodule reproduction: change={change}, outcome={outcome:?}, file_exists={}, module_gitdir_exists={}, archive_ref={archive}, worker_commit={commit:?}",
            file.exists(),
            std::path::Path::new(&saved_gitdir).exists()
        );
        assert!(
            root.exists(),
            "archive must retain the worktree with {change} submodule data"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("worker file remains"),
            "worker data\n"
        );
        let error = outcome.expect_err("unsafe submodule prevents archive removal");
        assert!(
            matches!(error, WorkspaceCleanupError::UnsafeSubmodule(_)),
            "{error}"
        );
        assert_eq!(agent(&store)["worktreeReady"], true);
        assert!(
            agent(&store)["workspaceCleanupIntent"]["receipt"]["reason"]
                .as_str()
                .expect("cleanup refusal is text")
                .contains("submodule")
        );
    }

    #[tokio::test]
    async fn archive_keeps_submodule_modified_file_despite_ignore_all() {
        assert_submodule_edit_survives_archive("modified").await;
    }

    #[tokio::test]
    async fn archive_keeps_submodule_untracked_file_despite_ignore_all() {
        assert_submodule_edit_survives_archive("untracked").await;
    }

    #[tokio::test]
    async fn archive_keeps_submodule_unpushed_commit_despite_ignore_all() {
        assert_submodule_edit_survives_archive("commit").await;
    }

    async fn assert_submodule_held(
        store: &Arc<RuntimeStore>,
        root: &std::path::Path,
        reason: &str,
    ) {
        let error = super::cleanup_agent_workspace(
            Arc::clone(store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await
        .expect_err("unsafe submodule retains worktree");
        assert!(
            matches!(error, WorkspaceCleanupError::UnsafeSubmodule(_)),
            "{error}"
        );
        assert!(error.to_string().contains(reason), "{error}");
        assert!(root.exists());
        let saved = agent(store);
        assert_eq!(saved["worktreeReady"], true);
        assert_eq!(saved["workspaceCleanupIntent"]["phase"], "not_sent");
        assert!(
            saved["workspaceCleanupIntent"]["receipt"]["reason"]
                .as_str()
                .expect("saved refusal is text")
                .contains(reason)
        );
    }

    #[tokio::test]
    async fn archive_cleans_recursive_submodules_and_preserves_archive_ref() {
        let (_directory, store, repository, root) = submodule_fixture_with_nested(true).await;
        assert!(root.join("module/nested/file.txt").exists());
        let head = fixture_git(&root, &["rev-parse", "HEAD"]);
        super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await
        .expect("clean recursive submodules archive");
        assert!(!root.exists());
        assert_eq!(
            fixture_git(
                &repository,
                &["rev-parse", "refs/codex-agents/archive/cleanup-agent"]
            ),
            head
        );
    }

    #[tokio::test]
    async fn archive_cleans_uninitialized_submodules() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        fixture_git(&root, &["submodule", "deinit", "module"]);
        super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await
        .expect("uninitialized submodule archives");
        assert!(!root.exists());
    }

    #[tokio::test]
    async fn archive_keeps_submodule_local_branch_with_recorded_head() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        let module = root.join("module");
        let recorded = fixture_git(&module, &["rev-parse", "HEAD"]);
        fixture_git(&module, &["checkout", "-qb", "worker"]);
        std::fs::write(module.join("file.txt"), "local branch\n")
            .expect("local branch file writes");
        fixture_git(&module, &["commit", "-qam", "local branch"]);
        fixture_git(&module, &["checkout", "--detach", &recorded]);
        assert_submodule_held(&store, &root, "local commits").await;
        let held = reconcile_pending_cleanup(Arc::clone(&store))
            .await
            .expect("recovery retains unsafe worktree");
        assert_eq!(held.len(), 1);
        assert!(held[0].held);
        assert!(root.exists());
    }

    #[tokio::test]
    async fn archive_keeps_submodule_recursive_local_commit_with_clean_status() {
        let (_directory, store, _repository, root) = submodule_fixture_with_nested(true).await;
        let nested = root.join("module/nested");
        let recorded = fixture_git(&nested, &["rev-parse", "HEAD"]);
        std::fs::write(nested.join("file.txt"), "nested worker data\n")
            .expect("nested worker file writes");
        fixture_git(&nested, &["commit", "-qam", "nested detached commit"]);
        let local = fixture_git(&nested, &["rev-parse", "HEAD"]);
        fixture_git(&nested, &["checkout", "--detach", &recorded]);
        assert_eq!(
            fixture_git(
                &root,
                &["status", "--porcelain", "--ignore-submodules=none"]
            ),
            ""
        );
        assert_submodule_held(&store, &root, "module/nested has local commits").await;
        assert_eq!(fixture_git(&nested, &["rev-parse", "HEAD@{1}"]), local);
    }

    #[tokio::test]
    async fn archive_keeps_submodule_recursive_modified_file() {
        let (_directory, store, _repository, root) = submodule_fixture_with_nested(true).await;
        let file = root.join("module/nested/file.txt");
        std::fs::write(&file, "nested edit\n").expect("nested edit writes");
        assert_eq!(fixture_git(&root, &["status", "--porcelain"]), "");
        assert_submodule_held(&store, &root, "modified or untracked").await;
        assert_eq!(
            std::fs::read_to_string(file).expect("nested edit remains readable"),
            "nested edit\n"
        );
    }

    #[tokio::test]
    async fn archive_keeps_submodule_unreadable_head() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        let gitdir = fixture_git(&root.join("module"), &["rev-parse", "--absolute-git-dir"]);
        std::fs::write(std::path::Path::new(&gitdir).join("HEAD"), "invalid head\n")
            .expect("invalid HEAD writes");
        assert_submodule_held(&store, &root, "cannot be read").await;
    }

    #[tokio::test]
    async fn archive_keeps_submodule_foreign_git_directory() {
        let (directory, store, _repository, root) = submodule_fixture().await;
        let foreign = std::fs::canonicalize(directory.path().join("submodule-source/.git"))
            .expect("foreign Git directory resolves");
        std::fs::write(
            root.join("module/.git"),
            format!("gitdir: {}\n", foreign.display()),
        )
        .expect("foreign Git link writes");
        assert_submodule_held(&store, &root, "foreign Git directory").await;
    }

    #[tokio::test]
    async fn archive_keeps_submodule_without_git_directory_and_with_files() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        std::fs::remove_file(root.join("module/.git")).expect("submodule Git link removes");
        assert_submodule_held(&store, &root, "contains files but has no Git directory").await;
    }

    #[tokio::test]
    async fn archive_rechecks_submodule_after_saving_parent_ref() {
        let (_directory, store, repository, root) = submodule_fixture().await;
        let identity = super::worktree_identity(&worktree_owner(agent(&store)), store.state_dir())
            .expect("worktree identity reads")
            .expect("owned worktree identity");
        super::save_worktree_archive_ref(&identity).expect("clean parent ref saves");
        std::fs::write(root.join("module/file.txt"), "late edit\n").expect("late edit writes");
        let error =
            super::remove_worktree(&identity).expect_err("new submodule edit blocks removal");
        assert!(matches!(error, EffectFailure::UnsafeSubmodule(_)));
        assert!(root.exists());
        assert_eq!(
            fixture_git(
                &repository,
                &["rev-parse", "refs/codex-agents/archive/cleanup-agent"]
            ),
            identity.head
        );
    }

    fn record_local_submodule_commit(root: &std::path::Path) -> String {
        let module = root.join("module");
        std::fs::write(module.join("file.txt"), "recorded local commit\n")
            .expect("local commit file writes");
        fixture_git(&module, &["commit", "-qam", "local submodule commit"]);
        let head = fixture_git(&module, &["rev-parse", "HEAD"]);
        fixture_git(
            root,
            &[
                "update-index",
                "--cacheinfo",
                &format!("160000,{head},module"),
            ],
        );
        fixture_git(
            root,
            &[
                "-c",
                "submodule.module.ignore=none",
                "commit",
                "-qm",
                "record local submodule commit",
            ],
        );
        assert_eq!(fixture_git(root, &["rev-parse", "HEAD:module"]), head);
        head
    }

    #[tokio::test]
    async fn archive_keeps_submodule_recorded_unpushed_commit() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        let local = record_local_submodule_commit(&root);
        assert_submodule_held(&store, &root, "recorded commit without a remote ref").await;
        assert_eq!(
            fixture_git(&root.join("module"), &["rev-parse", "HEAD"]),
            local
        );
    }

    #[tokio::test]
    async fn archive_keeps_submodule_without_remote_or_main_copy() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        record_local_submodule_commit(&root);
        fixture_git(&root.join("module"), &["remote", "remove", "origin"]);
        assert_submodule_held(&store, &root, "recorded commit without a remote ref").await;
    }

    #[tokio::test]
    async fn archive_cleans_submodule_without_remote_with_main_copy() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        fixture_git(&root.join("module"), &["remote", "remove", "origin"]);
        super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await
        .expect("main submodule object store preserves the recorded commit");
        assert!(!root.exists());
    }

    #[tokio::test]
    async fn archive_cleans_recorded_submodule_commit_copied_to_main() {
        let (_directory, store, repository, root) = submodule_fixture().await;
        let local = record_local_submodule_commit(&root);
        fixture_git(
            &repository.join("module"),
            &[
                "-c",
                "protocol.file.allow=always",
                "fetch",
                root.join("module").to_str().expect("module path is UTF-8"),
                &local,
            ],
        );
        fixture_git(&root.join("module"), &["remote", "remove", "origin"]);
        super::cleanup_agent_workspace(
            Arc::clone(&store),
            "cleanup-agent".into(),
            CleanupAction::Archive,
        )
        .await
        .expect("main copy preserves the otherwise unpublished commit");
        assert!(!root.exists());
        assert_eq!(
            fixture_git(&repository.join("module"), &["cat-file", "-t", &local]),
            "commit"
        );
    }

    #[tokio::test]
    async fn archive_keeps_submodule_with_foreign_object_alternates() {
        let (_directory, store, repository, root) = submodule_fixture().await;
        let gitdir = fixture_git(&root.join("module"), &["rev-parse", "--absolute-git-dir"]);
        let foreign = fixture_git(
            &repository.join("module"),
            &["rev-parse", "--absolute-git-dir"],
        );
        std::fs::write(
            std::path::Path::new(&gitdir).join("objects/info/alternates"),
            format!("{foreign}/objects\n"),
        )
        .expect("foreign object alternate writes");
        assert_submodule_held(&store, &root, "foreign Git directory or configuration").await;
    }

    #[tokio::test]
    async fn archive_keeps_submodule_with_unreadable_configuration() {
        let (_directory, store, _repository, root) = submodule_fixture().await;
        std::fs::write(root.join(".gitmodules"), "[broken\n")
            .expect("invalid Git configuration writes");
        assert_submodule_held(&store, &root, "cannot be read").await;
    }

    #[tokio::test]
    async fn lifecycle_intent_has_stable_operation_identity() {
        let first = queued_cleanup_intent("cleanup-agent", 7, CleanupAction::Delete);
        let replay = queued_cleanup_intent("cleanup-agent", 7, CleanupAction::Delete);
        assert_eq!(first, replay);
        assert_eq!(
            first.operation_id.value().map(|raw| raw.as_str()),
            Some("\"c275e839-b289-5724-b845-7d0123668fc1\"")
        );
        assert_eq!(first.phase, super::CleanupPhase::Queued);
        assert_eq!(first.action, CleanupAction::Delete);
    }
}
