//! Closed current-writer context repair journal, shared by the current repair and its history.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum ContextRepair {
    Preparing(PreparingRepair),
    Submitted(SubmittedRepair),
    Unknown(UnknownRepair),
    Unchanged(UnchangedRepair),
    Completed(CompletedRepair),
    Failed(FailedRepair),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FailedRepair {
    BeforeFork(Box<FailedPreparingRepair>),
    AfterFork(Box<FailedSubmittedRepair>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparingRepair {
    pub id: String,
    pub root_request_id: String,
    pub agent: String,
    pub source: RepairSource,
    /// Creation timestamp in seconds.
    pub created: f64,
    pub checked_event_ids: Vec<String>,
    pub compactions: studio_runtime_schema::AgentCounter,
    pub start_attempt_snapshot: Option<Box<crate::StartAttemptView>>,
    pub connection_id: String,
    pub connection_generation: studio_runtime_schema::AgentCounter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_terminal_receipt: Option<NativeTerminalReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_input_proof: Option<HistoricalInputProof>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmittedRepair {
    pub id: String,
    pub root_request_id: String,
    pub agent: String,
    pub source: RepairSource,
    /// Creation timestamp in seconds.
    pub created: f64,
    pub checked_event_ids: Vec<String>,
    pub compactions: studio_runtime_schema::AgentCounter,
    pub start_attempt_snapshot: Option<Box<crate::StartAttemptView>>,
    pub connection_id: String,
    pub connection_generation: studio_runtime_schema::AgentCounter,
    pub native_terminal_receipt: NativeTerminalReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_input_proof: Option<HistoricalInputProof>,
    pub rpc_method: ForkMethod,
    pub fork_operation: String,
    pub snapshot: RepairSnapshot,
    pub fork_params: ForkParams,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnknownRepair {
    pub id: String,
    pub root_request_id: String,
    pub agent: String,
    pub source: RepairSource,
    /// Creation timestamp in seconds.
    pub created: f64,
    pub checked_event_ids: Vec<String>,
    pub compactions: studio_runtime_schema::AgentCounter,
    pub start_attempt_snapshot: Option<Box<crate::StartAttemptView>>,
    pub connection_id: String,
    pub connection_generation: studio_runtime_schema::AgentCounter,
    pub native_terminal_receipt: NativeTerminalReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_input_proof: Option<HistoricalInputProof>,
    pub rpc_method: ForkMethod,
    pub fork_operation: String,
    pub snapshot: RepairSnapshot,
    pub fork_params: ForkParams,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnchangedRepair {
    pub id: String,
    pub root_request_id: String,
    pub agent: String,
    pub source: RepairSource,
    /// Creation timestamp in seconds.
    pub created: f64,
    pub checked_event_ids: Vec<String>,
    pub compactions: studio_runtime_schema::AgentCounter,
    pub start_attempt_snapshot: Option<Box<crate::StartAttemptView>>,
    pub connection_id: String,
    pub connection_generation: studio_runtime_schema::AgentCounter,
    pub native_terminal_receipt: NativeTerminalReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_input_proof: Option<HistoricalInputProof>,
    pub snapshot: RepairSnapshot,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletedRepair {
    pub id: String,
    pub root_request_id: String,
    pub agent: String,
    pub source: RepairSource,
    /// Creation timestamp in seconds.
    pub created: f64,
    pub checked_event_ids: Vec<String>,
    pub compactions: studio_runtime_schema::AgentCounter,
    pub start_attempt_snapshot: Option<Box<crate::StartAttemptView>>,
    pub connection_id: String,
    pub connection_generation: studio_runtime_schema::AgentCounter,
    pub native_terminal_receipt: NativeTerminalReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_input_proof: Option<HistoricalInputProof>,
    pub rpc_method: ForkMethod,
    pub fork_operation: String,
    pub snapshot: RepairSnapshot,
    pub fork_params: ForkParams,
    pub new_thread_id: String,
    pub repaired_event_ids: Vec<String>,
    pub source_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FailedPreparingRepair {
    pub id: String,
    pub root_request_id: String,
    pub agent: String,
    pub source: RepairSource,
    /// Creation timestamp in seconds.
    pub created: f64,
    pub checked_event_ids: Vec<String>,
    pub compactions: studio_runtime_schema::AgentCounter,
    pub start_attempt_snapshot: Option<Box<crate::StartAttemptView>>,
    pub connection_id: String,
    pub connection_generation: studio_runtime_schema::AgentCounter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_terminal_receipt: Option<NativeTerminalReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_input_proof: Option<HistoricalInputProof>,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FailedSubmittedRepair {
    pub id: String,
    pub root_request_id: String,
    pub agent: String,
    pub source: RepairSource,
    /// Creation timestamp in seconds.
    pub created: f64,
    pub checked_event_ids: Vec<String>,
    pub compactions: studio_runtime_schema::AgentCounter,
    pub start_attempt_snapshot: Option<Box<crate::StartAttemptView>>,
    pub connection_id: String,
    pub connection_generation: studio_runtime_schema::AgentCounter,
    pub native_terminal_receipt: NativeTerminalReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub historical_input_proof: Option<HistoricalInputProof>,
    pub rpc_method: ForkMethod,
    pub fork_operation: String,
    pub snapshot: RepairSnapshot,
    pub fork_params: ForkParams,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepairSource {
    pub id: String,
    pub epoch: i64,
    pub account_key: String,
    pub thread_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTerminalReceipt {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub status: Option<String>,
    pub native_state: NativeRepairState,
    pub started_at: Option<f64>,
    pub completed_at: Option<f64>,
    pub inherited_empty: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoricalInputProof {
    pub events: Vec<HistoricalInput>,
    pub terminal_turn_id: Option<String>,
    pub terminal_completed_at: Option<f64>,
    pub source: RepairSource,
    pub delivery_outcome: UnknownDelivery,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoricalInput {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub created: f64,
    pub turn_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ForkParams {
    pub thread_id: String,
    pub path: String,
    pub exclude_turns: bool,
    pub defer_goal_continuation: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepairSnapshot {
    pub source_path: String,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub source_file_identity: SourceFileIdentity,
    pub import_thread_id: String,
    pub copy_path: Option<String>,
    pub copy_sha256: Option<String>,
    pub copy_bytes: Option<u64>,
    pub event_ids: Vec<String>,
    pub terminal_turn_id: Option<String>,
    pub terminal_status: Option<String>,
    pub terminal_completed_at: Option<f64>,
    pub inherited_empty: bool,
    pub saved_bytes: u64,
    pub ancestry: Vec<RepairAncestry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// The repair writer first constructs a JSON value, which admits these identities as u64.
pub struct SourceFileIdentity {
    pub device: u64,
    pub inode: u64,
    pub bytes: u64,
    pub modified_ns: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepairAncestry {
    pub path: String,
    pub thread_id: String,
    pub end_byte_offset: u64,
    pub end_ordinal_exclusive: Option<u64>,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ForkMethod {
    #[serde(rename = "thread/fork")]
    ThreadFork,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeRepairState {
    Idle,
    NotLoaded,
    SystemError,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownDelivery {
    Unknown,
}

#[cfg(any(test, feature = "test-support"))]
impl ContextRepair {
    /// A complete writer-shaped repair for consumer fixtures; never used by production writers.
    pub fn test_fixture(phase: &str) -> Self {
        let mut repair = studio_json::json!({
            "id":"repair", "rootRequestId":"repair", "agent":"agent",
            "phase":phase, "created":1.0, "checkedEventIds":[], "compactions":0,
            "source":{"id":"agent","epoch":0,"accountKey":"default","threadId":"thread"},
            "startAttemptSnapshot":null,"connectionId":"connection","connectionGeneration":1
        });
        if phase != "preparing" && phase != "failed" {
            repair["nativeTerminalReceipt"] = studio_json::json!({
                "threadId":"thread","turnId":"turn","status":"completed","nativeState":"idle",
                "startedAt":1.0,"completedAt":2.0,"inheritedEmpty":false
            });
            repair["snapshot"] = studio_json::json!({
                "sourcePath":"/source","sourceSha256":"source","sourceBytes":1,
                "sourceFileIdentity":{"device":1,"inode":2,"bytes":1,"modifiedNs":3},
                "importThreadId":"import-thread","copyPath":"/copy","copySha256":"copy","copyBytes":1,
                "eventIds":[],"terminalTurnId":"turn","terminalStatus":"completed",
                "terminalCompletedAt":2.0,"inheritedEmpty":false,"savedBytes":0,"ancestry":[]
            });
        }
        if matches!(phase, "submitted" | "unknown" | "completed") {
            repair["rpcMethod"] = studio_json::json!("thread/fork");
            repair["forkOperation"] = studio_json::json!("fork");
            repair["forkParams"] = studio_json::json!({
                "threadId":"import-thread","path":"/copy","excludeTurns":true,"deferGoalContinuation":true
            });
        }
        if phase == "completed" {
            repair["newThreadId"] = studio_json::json!("new-thread");
            repair["repairedEventIds"] = studio_json::json!([]);
            repair["sourceSha256"] = studio_json::json!("source");
        }
        if phase == "failed" {
            repair["error"] = studio_json::json!("failed before fork");
        }
        Self::deserialize(repair).expect("complete current-writer repair fixture")
    }
}

#[cfg(test)]
mod tests {
    use super::ContextRepair;
    use serde::Deserialize;

    #[test]
    fn context_repair_writer_shapes_are_closed_and_phase_specific() {
        for phase in [
            "preparing",
            "submitted",
            "unknown",
            "unchanged",
            "completed",
            "failed",
        ] {
            let repair = ContextRepair::test_fixture(phase);
            let encoded = studio_json::to_string(&repair).unwrap();
            assert_eq!(
                studio_json::from_str::<ContextRepair>(&encoded).unwrap(),
                repair
            );
            let mut value =
                serde::Serialize::serialize(&repair, studio_json::value::Serializer).unwrap();
            value["future"] = studio_json::json!(true);
            assert!(ContextRepair::deserialize(value).is_err(), "{phase}");
            let mut value =
                serde::Serialize::serialize(&repair, studio_json::value::Serializer).unwrap();
            value.as_object_mut().unwrap().remove("rootRequestId");
            assert!(ContextRepair::deserialize(value).is_err(), "{phase}");
        }
        for phase in ["ready", "other"] {
            assert!(ContextRepair::deserialize(studio_json::json!({"phase":phase})).is_err());
        }
    }
}
