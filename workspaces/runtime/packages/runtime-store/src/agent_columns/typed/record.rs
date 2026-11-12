//! Agent persistence payloads. The records module re-exports these single owners.
use crate::records::{
    AgentActiveTool, AgentActivity, AgentArchive, AgentCapacity, AgentContextUsage,
    AgentExecutionMode, AgentId, AgentNaming, AgentSandbox, AgentStatus, CleanedWorktree,
    CredentialCopy, NativeReleaseScope, NativeSafetyBuffering, NativeStatus, StartAttemptView,
    StartIdentity, TerminalFault, VmAuthBootstrap, deserialize_optional_safe_counter,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use studio_runtime_schema::raw_json::RawJson;

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentDraft {
    pub text: String,
    pub prefix_text: String,
    pub assets: Vec<studio_runtime_schema::transcript_asset::TranscriptAssetView>,
    @extensions pub extra,
}
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentRecord {
    pub id: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<Box<AgentDraft>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub active_execution:
        studio_runtime_schema::queue_command::Field<StoredTransferObject<ActiveExecutionView>>,
    pub root_id: AgentId,
    #[serde(deserialize_with = "deserialize_agent_epoch")]
    pub epoch: i64,
    pub status: AgentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// None is live; Some is a finite, nonzero deletion timestamp in seconds.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_deleted_at",
        serialize_with = "serialize_deleted_at"
    )]
    pub deleted_at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_archive: Option<AgentArchive>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub deleted_archive: studio_runtime_schema::queue_command::Field<
        crate::json_record_codec::ExactJson<AgentArchive>,
    >,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<AgentSandbox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleaned_worktree: Option<CleanedWorktree>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_safety_buffering: Option<NativeSafetyBuffering>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_status: Option<NativeStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_lead: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_mode: Option<studio_runtime_schema::records::WorkspaceMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_assignment: Option<studio_runtime_schema::review::ReviewAssignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<studio_runtime_schema::agent_limits::SubagentConcurrency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_mode: Option<studio_runtime_schema::agent_entity_nested::AgentMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_safe_counter"
    )]
    pub agent_mode_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_concurrency_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layr_store_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layr_project_id: Option<Box<str>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub layr_line: studio_runtime_schema::queue_command::Field<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layr_read_cwd: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layr_subpath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layr_base_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_server_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_base_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_mode_explicit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_in_git: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_create: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_defaults: Option<Box<studio_runtime_schema::agent_entity_remote::ReviewDefaultsDto>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_warning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_base_commit: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_safe_counter"
    )]
    pub worker_base_behind_main: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_base_main_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_create_request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<studio_runtime_schema::claude_permissions::PermissionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_counter")]
    pub compactions: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daybreak_enabled: Option<bool>,
    #[serde(default)]
    pub naming: AgentNaming,
    #[serde(default)]
    pub capacity: AgentCapacity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_mode_supported: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layr_ready: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub image_workspace: studio_runtime_schema::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub archived: studio_runtime_schema::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub pinned: studio_runtime_schema::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub prompt: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub profile_id: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub profile_instructions: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub effort: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_effort: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub project_folder: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub checkpoint_history_head: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub workspace_reservation_id: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub workspace_operation: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub worker_excluded_account_keys: studio_runtime_schema::queue_command::Field<
        Vec<studio_runtime_schema::account_key::AccountKey>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub reviewer_excluded_account_keys: studio_runtime_schema::queue_command::Field<
        Vec<studio_runtime_schema::account_key::AccountKey>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub role: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::creation_intent::AgentRole,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub provider: studio_runtime_schema::queue_command::Field<studio_runtime_schema::ProviderKind>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub created:
        studio_runtime_schema::queue_command::Field<studio_runtime_schema::NonnegativeNumber>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_safe_counter"
    )]
    pub queue_mutation_revision: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_safe_counter"
    )]
    pub auth_resume_attempt: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub last_completed_turn: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub host_project_path: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub execution_settings_account_key: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub last_completed_turn_status: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub error: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::provider_errors::AgentError,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub last_completed_turn_error: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::provider_errors::AgentError,
    >,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_response_turn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parked_event: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_wait: Option<studio_runtime_schema::agent_progress::AgentStartWait>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_safe_counter",
        skip_serializing_if = "Option::is_none"
    )]
    pub events: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<AgentActivity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_tools: Option<Vec<AgentActiveTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_usage: Option<AgentContextUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage_accounting: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_flight: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_wake: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_resume_enabled: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub environment: studio_runtime_schema::queue_command::Field<
        StoredTransferValue<studio_runtime_schema::project_ingress::WorkerEnvironment>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub usage_resume: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<Box<studio_runtime_schema::continuation_snapshot::UsageContinuation>>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_failure_hold: studio_runtime_schema::queue_command::Field<NativeFailureHold>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub prepare_attempt: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub worker_defaults: studio_runtime_schema::queue_command::Field<StoredWorkerDefaults>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub pending_settings: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<Box<crate::transfer_record::TransferSettings>>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub pending_settings_account_key: studio_runtime_schema::queue_command::Field<
        StoredTransferValue<studio_runtime_schema::account_key::AccountKey>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub updated_at: studio_runtime_schema::queue_command::Field<StoredTransferValue<f64>>,

    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub portable_history: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<crate::transfer_record::PortableHistory>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub account_history: studio_runtime_schema::queue_command::Field<
        StoredTransferValue<
            Vec<StoredTransferObject<crate::transfer_record::TransferAccountHistoryEntry>>,
        >,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub cyber_access_program:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub yolo_mode: studio_runtime_schema::queue_command::Field<StoredTransferValue<bool>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub claude_options: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<crate::transfer_record::TransferClaudeOptions>,
    >,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<AgentExecutionMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_epoch: Option<i64>,
    pub account_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_job: Option<StoredTitleJob>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_attempt: Option<Box<StartAttemptView>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disconnect_recovery: Option<Box<studio_runtime_schema::recovery_record::RecoveryFields>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_recovery: Option<Box<studio_runtime_schema::recovery_record::RecoveryFields>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_outcome_hold: Option<StoredStartOutcomeHold>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub fired_events: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_review_thread: Option<Box<NativeReviewThread>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_launch: Option<VmLaunch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_copy: Option<CredentialCopy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_auth_bootstrap: Option<VmAuthBootstrap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_release: Option<NativeRelease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_intent: Option<studio_runtime_schema::creation_intent::CreationIntent>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub account_transfer: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<crate::transfer_record::AgentTransferState>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing",
        deserialize_with = "deserialize_transfer_id"
    )]
    pub account_transfer_id:
        studio_runtime_schema::queue_command::Field<crate::transfer_record::TransferId>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub lazy_account_transfer:
        studio_runtime_schema::queue_command::Field<crate::transfer_record::LazyTransferState>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub shared_room_id: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub project_folder_revision:
        studio_runtime_schema::queue_command::Field<studio_runtime_schema::raw_json::RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub task_default_gate:
        studio_runtime_schema::queue_command::Field<studio_runtime_schema::task_gate::Gate>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub moved_from:
        studio_runtime_schema::queue_command::Field<studio_runtime_schema::raw_json::RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub connection_check:
        studio_runtime_schema::queue_command::Field<studio_runtime_schema::raw_json::RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub idle_summary_prepared:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub idle_compaction_episode:
        studio_runtime_schema::queue_command::Field<StoredTransferObject<IdleCompactionEpisode>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub idle_cache: studio_runtime_schema::queue_command::Field<StoredTransferObject<IdleCache>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_thread_block:
        studio_runtime_schema::queue_command::Field<StoredTransferObject<NativeThreadBlock>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_turn_error:
        studio_runtime_schema::queue_command::Field<StoredTransferObject<NativeTurnError>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub deleted: studio_runtime_schema::queue_command::Field<StoredTransferValue<bool>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub real_cwd: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub worktree_common_dir:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub remote_epoch: studio_runtime_schema::queue_command::Field<StoredTransferValue<i64>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub remote_control_epoch: studio_runtime_schema::queue_command::Field<StoredTransferValue<i64>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub remote_stop_request:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub connection_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub project: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub image_workspace_ready:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<bool>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub image_workspace_phase:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub layr_scratch:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<std::path::PathBuf>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub steer_rejected_turn_id:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub auth_wait: studio_runtime_schema::queue_command::Field<RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub moved_to: studio_runtime_schema::queue_command::Field<RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub move_import_pending: studio_runtime_schema::queue_command::Field<RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub park_after_turn: studio_runtime_schema::queue_command::Field<RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub layr_state_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub empty_transfer_recovery: studio_runtime_schema::queue_command::Field<RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub context_repair_wait: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<
            crate::json_record_codec::ExactJson<
                studio_runtime_schema::recovery_record::RecoveryFields,
            >,
        >,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub connection_recovery: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<
            crate::json_record_codec::ExactJson<
                studio_runtime_schema::recovery_record::ConnectionRecoveryFields,
            >,
        >,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub capacity_retry_count: studio_runtime_schema::queue_command::Field<StoredTransferValue<u64>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub shell_policy_last_active_at:
        studio_runtime_schema::queue_command::Field<StoredTransferValue<studio_json::Number>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub claude_pre_input_retry: studio_runtime_schema::queue_command::Field<RawJson>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub provider_stream: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<
            crate::json_record_codec::ExactJson<crate::account_state::StreamFields>,
        >,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub capacity_retry: studio_runtime_schema::queue_command::Field<
        Box<studio_runtime_schema::continuation_snapshot::CapacityContinuation>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub read_state: studio_runtime_schema::queue_command::Field<
        Box<studio_runtime_schema::agent_entity_nested_extra::ReadStateDto>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub thread_provenance: studio_runtime_schema::queue_command::Field<
        BTreeMap<String, studio_runtime_schema::provider_call_contract::ThreadProvenance>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub workspace_cleanup_intent: studio_runtime_schema::queue_command::Field<
        Box<crate::workspace_cleanup_intent::StoredCleanupIntent>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub layr_turn_saves: studio_runtime_schema::queue_command::Field<
        BTreeMap<String, super::retained::LayrTurnSave>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub cancelled_park:
        studio_runtime_schema::queue_command::Field<Box<super::retained::CancelledPark>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub park_receipt:
        studio_runtime_schema::queue_command::Field<Box<super::retained::ParkReceipt>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub manual_recovery_delivery_hold: studio_runtime_schema::queue_command::Field<
        Box<super::retained::ManualRecoveryDeliveryHold>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_safety_retry: studio_runtime_schema::queue_command::Field<
        Box<studio_runtime_schema::agent_entity_native::NativeSafetyRetryDto>,
    >,
    /// Last reap timestamp in seconds, distinct from last-active time.
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub shell_policy_last_reap_at: studio_runtime_schema::queue_command::Field<studio_json::Number>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_review:
        studio_runtime_schema::queue_command::Field<Box<super::retained::StoredNativeReview>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub branch: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub layr_cache_warning: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub layr_version_warning: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_role_guidance: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_teleport_tool: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::native_thread_record::SessionTool,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_tool_catalog: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::native_thread_record::NativeToolCatalog,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub native_tool_source: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::native_thread_record::CatalogSource,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub park_sequence:
        studio_runtime_schema::queue_command::Field<studio_runtime_schema::AgentCounter>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub prepared_context: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::native_thread_record::PreparedContext,
    >,
    /// Provider active-permission profile; distinct from the creation profile ID.
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub profile: studio_runtime_schema::queue_command::Field<
        studio_runtime_schema::CodexActivePermissionProfile,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub review_archive_attempts:
        studio_runtime_schema::queue_command::Field<studio_runtime_schema::AgentCounter>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub review_archive_error: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub review_archive_next_at: studio_runtime_schema::queue_command::Field<f64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub review_archive_scheduled: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub context_repair:
        studio_runtime_schema::queue_command::Field<Box<super::context_repair::ContextRepair>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub context_repair_history:
        studio_runtime_schema::queue_command::Field<Vec<super::context_repair::ContextRepair>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub last_workspace_wait:
        studio_runtime_schema::queue_command::Field<Box<super::retained::LastWorkspaceWait>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub forked_from: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub source_message: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub restored_checkpoint: studio_runtime_schema::queue_command::Field<String>,
    /// Historical activity timestamp in Unix seconds, distinct from settings updatedAt.
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub updated: studio_runtime_schema::queue_command::Field<studio_json::Number>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub worktree_preparation: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub legacy_workspace_execution:
        studio_runtime_schema::queue_command::Field<Box<LegacyWorkspaceExecution>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub legacy_workspace_finished: studio_runtime_schema::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub legacy_workspace_settled: studio_runtime_schema::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub image_workspace_repo: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub image_workspace_mount: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub auth_resume_proof_pending: studio_runtime_schema::queue_command::Field<
        BTreeMap<
            studio_runtime_schema::account_key::AccountKey,
            super::retained::AuthResumeProofCursor,
        >,
    >,
}

/// Receipts retained when the removed image executor is settled; never replayed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyWorkspaceExecution {
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub turn_id: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub start_attempt: studio_runtime_schema::queue_command::Field<Box<StartAttemptView>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub restart_recovery: studio_runtime_schema::queue_command::Field<
        Box<studio_runtime_schema::recovery_record::RecoveryFields>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub disconnect_recovery: studio_runtime_schema::queue_command::Field<
        Box<studio_runtime_schema::recovery_record::RecoveryFields>,
    >,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub connection_recovery: studio_runtime_schema::queue_command::Field<
        StoredTransferObject<
            crate::json_record_codec::ExactJson<
                studio_runtime_schema::recovery_record::ConnectionRecoveryFields,
            >,
        >,
    >,
}
studio_runtime_schema::raw_json_payload! { flat
/// The episode identity is read by the runtime; other fields retain their wire representation.
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IdleCompactionEpisode {
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    @extensions pub extra,
}
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IdleCache {
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub ttl: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub request_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    @extensions pub extra,
}
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeThreadBlock {
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub thread_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    @extensions pub extra,
}
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeTurnError {
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub turn_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    @extensions pub extra,
}
}

/// Historical non-boolean holds retain their original JSON and truthiness.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum NativeFailureHold {
    Boolean(bool),
    Terminal {
        #[serde(rename = "terminalFault")]
        terminal_fault: TerminalFault,
    },
    Unreadable(RawJson),
}

impl<'de> Deserialize<'de> for NativeFailureHold {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawJson::deserialize(deserializer)?;
        if let Ok(value) = studio_json::from_str::<bool>(raw.0.as_str()) {
            return Ok(Self::Boolean(value));
        }
        #[derive(Deserialize)]
        struct Terminal {
            #[serde(rename = "terminalFault")]
            terminal_fault: TerminalFault,
        }
        if let Ok(value) = studio_json::from_str::<Terminal>(raw.0.as_str()) {
            return Ok(Self::Terminal {
                terminal_fault: value.terminal_fault,
            });
        }
        Ok(Self::Unreadable(raw))
    }
}

impl NativeFailureHold {
    /// The sync projection admits legacy objects and refuses legacy nonobjects.
    /// Runtime hold eligibility separately retains truthiness for every legacy shape.
    pub fn projected_truthiness(&self) -> crate::StoreResult<bool> {
        match self {
            Self::Boolean(value) => Ok(*value),
            Self::Terminal { .. } => Ok(true),
            Self::Unreadable(raw) if raw.0.as_str().trim_start().starts_with('{') => {
                Ok(crate::schema::raw_truthy(Some(&raw.0)))
            }
            Self::Unreadable(_) => Err(crate::StoreError::RecordContract(
                "legacy native hold projection requires an object",
            )),
        }
    }
}

/// Normalize transfer values on decode while retaining historical JSON on disk.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StoredTransferValue<T> {
    Known(T),
    Unreadable(RawJson),
}

/// An object-shaped typed value or its unchanged legacy representation.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StoredTransferObject<T> {
    Known(T),
    Unreadable(RawJson),
}

fn deserialize_transfer_id<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<studio_runtime_schema::queue_command::Field<crate::transfer_record::TransferId>, D::Error>
{
    use studio_runtime_schema::queue_command::Field;
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(Field::Null),
        Some(value) if value.is_empty() => Ok(Field::Missing),
        Some(value) => value
            .try_into()
            .map(Field::Value)
            .map_err(serde::de::Error::custom),
    }
}

pub(crate) fn deserialize_agent_epoch<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<i64, D::Error> {
    let epoch = i64::deserialize(deserializer)?;
    if (0..=studio_runtime_schema::shared_values::MAX_SAFE_INTEGER).contains(&epoch) {
        Ok(epoch)
    } else {
        Err(serde::de::Error::custom("agent epoch outside safe range"))
    }
}

/// Typed defaults or an unreadable legacy value.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StoredWorkerDefaults {
    Known(crate::transfer_record::TransferWorkerDefaults),
    Unreadable(RawJson),
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeReviewThread {
    pub sequence: u64,
    pub attempt: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_request_id: Option<String>,
    #[serde(default)]
    pub admitted_request: Option<crate::request_results::AdmittedRequest>,
    pub generation: u64,
    pub connection: String,
    pub request_digest: String,
    pub status: studio_runtime_schema::persistence::NativeReviewThreadStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    @extensions pub extra,
}
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmLaunch {
    pub identity: StartIdentity,
    pub signature: String,
    pub status: studio_runtime_schema::persistence::WorkEffectStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<RawJson>,
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeRelease {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted_request: Option<crate::request_results::AdmittedRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<NativeReleaseScope>,
    @extensions pub extra,
}
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TitleJob {
    pub request_id: String,
    pub agent_id: String,
    pub root_request_id: String,
    pub admitted_request: crate::request_results::AdmittedRequest,
    pub status: studio_runtime_schema::persistence::TitleStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    @extensions pub extra,
}
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StoredTitleJob {
    Job(Box<TitleJob>),
    Unrecognized(RawJson),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum StartOutcomeHold {
    Held {
        #[serde(rename = "operationId", skip_serializing_if = "Option::is_none")]
        operation_id: Option<String>,
        #[serde(
            rename = "connectionGeneration",
            skip_serializing_if = "Option::is_none"
        )]
        connection_generation: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(rename = "attemptId", skip_serializing_if = "Option::is_none")]
        attempt_id: Option<String>,
        #[serde(flatten)]
        extra: studio_runtime_schema::raw_json::RawExtensions,
    },
    Recovery {
        epoch: i64,
        reason: String,
        #[serde(flatten)]
        extra: studio_runtime_schema::raw_json::RawExtensions,
    },
    AccountSettlement {
        cursor: Option<u64>,
        reason: String,
        #[serde(rename = "previousError")]
        previous_error: RawJson,
        #[serde(rename = "previousNativeFailureHold")]
        previous_native_failure_hold: crate::records::PreviousNativeFailureHold,
        #[serde(flatten)]
        extra: studio_runtime_schema::raw_json::RawExtensions,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviousNativeFailureHold {
    pub present: bool,
    pub value: RawJson,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StoredStartOutcomeHold {
    Known(StartOutcomeHold),
    Unrecognized(RawJson),
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveExecutionView {
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub run_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub attempt_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    @extensions pub extra,
}
}
impl ActiveExecutionView {
    pub fn run_id_text(&self) -> Option<&str> {
        self.run_id
            .value()
            .and_then(StoredTransferValue::known)
            .map(String::as_str)
    }
    pub fn attempt_id_text(&self) -> Option<&str> {
        self.attempt_id
            .value()
            .and_then(StoredTransferValue::known)
            .map(String::as_str)
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for StoredTransferValue<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawJson::deserialize(deserializer)?;

        if let Ok(known) = studio_json::from_str::<T>(raw.0.as_str()) {
            return Ok(Self::Known(known));
        }

        Ok(Self::Unreadable(raw))
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for StoredTransferObject<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawJson::deserialize(deserializer)?;
        if raw.0.as_str().trim_start().starts_with('{')
            && let Ok(known) = studio_json::from_str::<T>(raw.0.as_str())
        {
            return Ok(Self::Known(known));
        }
        Ok(Self::Unreadable(raw))
    }
}

impl<'de> Deserialize<'de> for StoredWorkerDefaults {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawJson::deserialize(deserializer)?;
        if raw.0.as_str().trim_start().starts_with('{')
            && let Ok(known) = studio_json::from_str::<crate::transfer_record::TransferWorkerDefaults>(
                raw.0.as_str(),
            )
        {
            return Ok(Self::Known(known));
        }
        Ok(Self::Unreadable(raw))
    }
}

impl<'de> Deserialize<'de> for StoredTitleJob {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawJson::deserialize(deserializer)?;

        if let Ok(known) = studio_json::from_str::<Box<TitleJob>>(raw.0.as_str()) {
            return Ok(Self::Job(known));
        }

        Ok(Self::Unrecognized(raw))
    }
}

impl<'de> Deserialize<'de> for StoredStartOutcomeHold {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawJson::deserialize(deserializer)?;

        if let Ok(known) = studio_json::from_str::<StartOutcomeHold>(raw.0.as_str()) {
            return Ok(Self::Known(known));
        }

        Ok(Self::Unrecognized(raw))
    }
}

/// Read-only legacy selector; never stored beside the original evidence.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerDefaultsSelection {
    pub model: Option<StoredTransferValue<String>>,
    pub effort: Option<StoredTransferValue<String>>,
    pub fast_mode: Option<StoredTransferValue<bool>>,
    pub daybreak_enabled: Option<StoredTransferValue<bool>>,
    pub delegation_depth: Option<StoredTransferValue<u64>>,
}

impl<T> StoredTransferValue<T> {
    pub fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unreadable(_) => None,
        }
    }
}

impl StoredWorkerDefaults {
    pub fn legacy_selection(&self) -> Option<WorkerDefaultsSelection> {
        let Self::Unreadable(raw) = self else {
            return None;
        };
        raw.0
            .as_str()
            .trim_start()
            .starts_with('{')
            .then(|| studio_json::from_str(raw.0.as_str()).ok())
            .flatten()
    }
}

impl<T> StoredTransferObject<T> {
    pub fn legacy_reason(&self) -> Option<StoredTransferValue<String>> {
        #[derive(Deserialize)]
        struct Reason {
            reason: Option<StoredTransferValue<String>>,
        }
        let Self::Unreadable(raw) = self else {
            return None;
        };
        if !raw.0.as_str().trim_start().starts_with('{') {
            return None;
        }
        studio_json::from_str::<Reason>(raw.0.as_str()).ok()?.reason
    }
}

impl<'de> Deserialize<'de> for StartOutcomeHold {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Stage {
            Held,
            Recovery,
            AccountSettlement,
        }
        #[derive(Deserialize)]
        struct Tag {
            stage: Stage,
        }
        let raw = RawJson::deserialize(deserializer)?;
        let tag: Tag = studio_json::from_str(raw.0.as_str()).map_err(serde::de::Error::custom)?;
        let mut decoder = studio_json::Deserializer::from_str(raw.0.as_str());
        match tag.stage {
            Stage::Held => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Held {
                    stage: serde::de::IgnoredAny,
                    operation_id: Option<String>,
                    connection_generation: Option<u64>,
                    reason: Option<String>,
                    attempt_id: Option<String>,
                }
                let (held, extra) =
                    studio_runtime_schema::raw_json::deserialize_with_raw_extensions::<_, Held>(
                        &mut decoder,
                    )
                    .map_err(serde::de::Error::custom)?;
                let _ = held.stage;
                Ok(Self::Held {
                    operation_id: held.operation_id,
                    connection_generation: held.connection_generation,
                    reason: held.reason,
                    attempt_id: held.attempt_id,
                    extra,
                })
            }
            Stage::Recovery => {
                #[derive(Deserialize)]
                struct Recovery {
                    stage: serde::de::IgnoredAny,
                    epoch: i64,
                    reason: String,
                }
                let (recovery, extra) = studio_runtime_schema::raw_json::deserialize_with_raw_extensions::<_, Recovery>(&mut decoder).map_err(serde::de::Error::custom)?;
                let _ = recovery.stage;
                Ok(Self::Recovery {
                    epoch: recovery.epoch,
                    reason: recovery.reason,
                    extra,
                })
            }
            Stage::AccountSettlement => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Settlement {
                    stage: serde::de::IgnoredAny,
                    cursor: Option<u64>,
                    reason: String,
                    previous_error: RawJson,
                    previous_native_failure_hold: PreviousNativeFailureHold,
                }
                let (settlement, extra) = studio_runtime_schema::raw_json::deserialize_with_raw_extensions::<_, Settlement>(&mut decoder).map_err(serde::de::Error::custom)?;
                let _ = settlement.stage;
                Ok(Self::AccountSettlement {
                    cursor: settlement.cursor,
                    reason: settlement.reason,
                    previous_error: settlement.previous_error,
                    previous_native_failure_hold: settlement.previous_native_failure_hold,
                    extra,
                })
            }
        }
    }
}

impl NativeRelease {
    pub fn thread_id_selector(
        &self,
    ) -> crate::StoreResult<studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>>
    {
        match self.extra.get("threadId") {
            None => Ok(studio_runtime_schema::queue_command::Field::Missing),
            Some(raw) => Ok(studio_json::from_str(raw.as_str())?),
        }
    }

    pub fn close_thread(
        &mut self,
        supplied: &studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
        at: f64,
        agent_id: &str,
    ) -> crate::StoreResult<bool> {
        use studio_runtime_schema::queue_command::Field;
        let stored = self.thread_id_selector()?;
        if [&stored, supplied]
            .into_iter()
            .any(|selector| matches!(selector, Field::Value(StoredTransferValue::Unreadable(_))))
        {
            return Err(crate::StoreError::InvalidSyncObjectField {
                scope: "runtime_agents".into(),
                id: agent_id.into(),
                pointer: "/nativeRelease/threadId".into(),
                reason: "non-string native release thread selector".into(),
            });
        }
        if &stored != supplied {
            return Ok(false);
        }
        self.extra
            .insert("closedAt".into(), studio_json::to_raw_value(&at)?);
        self.extra
            .insert("resetPending".into(), studio_json::to_raw_value(&false)?);
        Ok(true)
    }
}

impl AgentRecord {
    pub fn context_repair_wait_active(&self) -> bool {
        let raw = studio_json::to_raw_value(&self.context_repair_wait)
            .expect("retained recovery marker serializes as valid JSON");
        crate::schema::raw_truthy(Some(&raw))
    }

    pub fn moved_to_active(&self) -> bool {
        crate::schema::raw_truthy(self.moved_to.value().map(|raw| &raw.0))
    }
    pub fn move_import_pending_active(&self) -> bool {
        crate::schema::raw_truthy(self.move_import_pending.value().map(|raw| &raw.0))
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionRecoverySelectors {
    #[serde(default)]
    pub turn_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(default)]
    pub account_key: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
    #[serde(default)]
    pub thread_id: studio_runtime_schema::queue_command::Field<StoredTransferValue<String>>,
}

impl
    StoredTransferObject<
        crate::json_record_codec::ExactJson<
            studio_runtime_schema::recovery_record::ConnectionRecoveryFields,
        >,
    >
{
    pub(crate) fn connection_recovery_selectors(
        &self,
    ) -> Result<ConnectionRecoverySelectors, studio_json::Error> {
        let raw = match self {
            Self::Known(exact) => exact.raw(),
            Self::Unreadable(raw) => raw.0.as_str(),
        };
        if !raw.trim_start().starts_with('{') {
            return Ok(ConnectionRecoverySelectors::default());
        }
        let mut members: studio_runtime_schema::raw_json::RawExtensions =
            studio_json::from_str(raw)?;
        Ok(ConnectionRecoverySelectors {
            turn_id: members
                .remove("turnId")
                .map(|raw| studio_json::from_str(raw.as_str()))
                .transpose()?
                .unwrap_or_default(),
            account_key: members
                .remove("accountKey")
                .map(|raw| studio_json::from_str(raw.as_str()))
                .transpose()?
                .unwrap_or_default(),
            thread_id: members
                .remove("threadId")
                .map(|raw| studio_json::from_str(raw.as_str()))
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

/// Single timestamp contract for agent persistence, in seconds.
pub(crate) fn valid_deleted_at(value: f64) -> bool {
    value.is_finite() && value != 0.0
}
pub(crate) fn deserialize_deleted_at<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<f64>, D::Error> {
    let value = Option::<f64>::deserialize(d)?;
    if value.is_none_or(valid_deleted_at) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "deletedAt must be a nonzero finite timestamp",
        ))
    }
}
fn serialize_deleted_at<S: serde::Serializer>(
    value: &Option<f64>,
    s: S,
) -> Result<S::Ok, S::Error> {
    if value.is_none_or(valid_deleted_at) {
        value.serialize(s)
    } else {
        Err(serde::ser::Error::custom(
            "deletedAt must be a nonzero finite timestamp",
        ))
    }
}
