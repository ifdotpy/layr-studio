use studio_runtime_schema::persistence::{
    HoldStatus, IdleEpisodeOutcome, TitleStatus, WorkEffectStatus,
};

use super::{AgentIdentity, NullableAgentSource, evidence::*};

/// Independent source presence; there is no both-absent application row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentComponents {
    AgentOnly,
    ChatOnly,
    Both,
}

impl AgentComponents {
    pub fn from_presence(agent: bool, chat: bool) -> crate::StoreResult<Self> {
        match (agent, chat) {
            (true, false) => Ok(Self::AgentOnly),
            (false, true) => Ok(Self::ChatOnly),
            (true, true) => Ok(Self::Both),
            (false, false) => Err(crate::StoreError::RecordContract(
                "merged agent row has no source component",
            )),
        }
    }
}

/// Every generated selector in the supported runtime_agents source descriptor.
/// TEXT affinity has already been applied by SQLite before these cells are read.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "preserves independent generated source predicates for held evidence"
)]
pub struct HeldAgentSelectors {
    pub root_id: Option<String>,
    pub parent_id: Option<String>,
    pub account_key: String,
    pub provider: String,
    pub role: Option<String>,
    pub deleted_at: DeletedAtEvidence,
    pub cwd: Option<String>,
    pub workspace_operation: WorkspaceOperationEvidence,
    pub remote_worker: RemoteWorkerEvidence,
    pub remote_anchor: RemoteAnchorEvidence,
    pub native_failure_hold: NativeFailureHoldEvidence,
    pub steer_rejected_turn_id: Option<String>,
    pub native_tool_refresh_id: Option<String>,
    pub context_repair_phase: Option<String>,
    pub source_cleanup_phase: Option<String>,
    pub source_cleanup_account_key: Option<String>,
    pub source_cleanup_thread_id: Option<String>,
    pub has_disconnect_recovery: bool,
    pub disconnect_recovery_stage: Option<String>,
    pub has_restart_recovery: bool,
    pub restart_recovery_stage: Option<String>,
    pub has_workspace_cleanup_intent: bool,
    pub agent_archived: bool,
    pub review_archive_failed: bool,
    pub review_archive_marked: bool,
    pub review_archive_scheduled: bool,
    pub review_archive_next_at: ReviewArchiveNextAtEvidence,
    pub idle_episode_type: Option<String>,
    pub idle_episode_fire_at: IdleEpisodeFireAtEvidence,
    pub idle_episode_id: IdleEpisodeIdEvidence,
    pub legacy_unsettled: bool,
    pub title_status: Option<TitleStatus>,
    pub vm_launch_status: Option<WorkEffectStatus>,
    pub idle_episode_outcome: Option<IdleEpisodeOutcome>,
    pub provider_stream_generation: ProviderStreamGenerationEvidence,
    pub provider_stream_ack: ProviderStreamAckEvidence,
    pub account_transfer_id: AccountTransferIdEvidence,
    pub native_release_reset_pending: NativeReleaseResetPendingEvidence,
    pub is_lead: IsLeadEvidence,
    pub auth_resume_proof_pending: bool,
}

/// One bounded legacy-source read, before any overlay or component reconciliation.
/// Original bytes/selectors stay available to the migration evidence owner.
pub struct AgentSourceSnapshot {
    pub physical_id: Option<String>,
    pub source_rowid: i64,
    pub record: Box<str>,
    pub hold: HoldStatus,
    pub hold_incident: Option<String>,
    pub selectors: HeldAgentSelectors,
}

/// Admission of a source component, not a second stored AgentRecord format.
pub enum AdmittedAgentSource<'a> {
    Normal(Box<crate::AgentRecord>),
    Held(HeldAgentEvidence<'a>),
}

/// Quarantined source evidence never acquires fabricated normal fields.
pub struct HeldAgentEvidence<'a> {
    source: &'a AgentSourceSnapshot,
}

impl AgentSourceSnapshot {
    pub fn identity(&self) -> AgentIdentity {
        AgentIdentity::from_source(
            NullableAgentSource::Agent,
            self.physical_id.as_deref(),
            self.source_rowid,
        )
    }

    pub fn admit(&self) -> crate::StoreResult<AdmittedAgentSource<'_>> {
        match self.hold {
            HoldStatus::Invalid | HoldStatus::Unrecognized => {
                Ok(AdmittedAgentSource::Held(HeldAgentEvidence {
                    source: self,
                }))
            }
            HoldStatus::Clear => {
                let id = self
                    .physical_id
                    .as_deref()
                    .ok_or(crate::StoreError::RecordContract(
                        "NULL agent source identity requires held evidence",
                    ))?;
                // This owner checks the embedded key and deliberately does not merge
                // volatile data: effective-overlay reconciliation is a later operation.
                let record = crate::records::parse_agent_record(id, &self.record)?;
                if !crate::AgentStatus::VALUES.contains(&record.status.as_str()) {
                    return Err(crate::StoreError::RecordContract(
                        "unknown normal agent status",
                    ));
                }
                if record.account_key != self.selectors.account_key {
                    return Err(crate::StoreError::RecordContract(
                        "agent source account selector disagrees with the record",
                    ));
                }
                Ok(AdmittedAgentSource::Normal(Box::new(record)))
            }
        }
    }
}

impl HeldAgentEvidence<'_> {
    pub fn identity(&self) -> AgentIdentity {
        self.source.identity()
    }

    pub fn record_bytes(&self) -> &[u8] {
        self.source.record.as_bytes()
    }

    pub fn hold(&self) -> HoldStatus {
        self.source.hold
    }

    pub fn incident(&self) -> Option<&str> {
        self.source.hold_incident.as_deref()
    }

    pub fn selectors(&self) -> &HeldAgentSelectors {
        &self.source.selectors
    }
}
