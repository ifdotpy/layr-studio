//! Strict retained agent markers, owned by their current production writer shapes.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParkReceipt {
    pub event: String,
    pub epoch: i64,
    pub sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelledPark {
    pub event: String,
    pub epoch: i64,
    pub sequence: i64,
    pub cancelled_at_epoch: i64,
}

impl ParkReceipt {
    pub fn cancelled(self, epoch: i64) -> CancelledPark {
        CancelledPark {
            event: self.event,
            epoch: self.epoch,
            sequence: self.sequence,
            cancelled_at_epoch: epoch,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualRecoveryDeliveryHold {
    pub epoch: i64,
    pub attempt_id: Option<String>,
    pub account_key: String,
    pub thread_id: Option<String>,
    pub turn_id: String,
    pub generation: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayrTurnSave {
    Pending,
    Saved {
        #[serde(rename = "stateId")]
        state_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredNativeReview {
    pub target: studio_runtime_schema::review::ReviewTarget,
    pub resolved_base: Option<studio_runtime_schema::review::ResolvedBase>,
    pub source_cwd: String,
    pub request_id: String,
    pub actor_id: String,
    pub args: studio_runtime_schema::creation_intent::CreationIntent,
    pub status: NativeReviewStatus,
    pub response: NativeReviewResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeReviewStatus {
    OrdinaryTurn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeReviewResponse {
    pub request_id: String,
    pub agent_id: String,
    pub status: crate::AgentStatus,
    pub review_model: String,
    pub review_effort: Option<String>,
    pub agents: Vec<NativeReviewAgentBrief>,
    pub delivery: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeReviewAgentBrief {
    pub id: String,
    pub name: String,
    pub status: crate::AgentStatus,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub fast_mode: Option<bool>,
}

/// Last unsubmitted workspace wait; its timestamp is seconds, not an attempt creation time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LastWorkspaceWait {
    pub attempt_id: String,
    pub events: Vec<String>,
    pub blockers: [(); 0],
    pub at: f64,
}

/// One account's bounded authentication-proof fanout cursor; proof time is seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthResumeProofCursor {
    pub proof_at: f64,
    pub after_id: String,
}
