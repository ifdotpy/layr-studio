//! Layr copy progress published to clients. Counts use the named unit.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CopyPhase {
    Sizing,
    Staging,
    ImportingCommits,
    Materializing,
    RestoringAttributes,
    WarmingCaches,
    Transferring,
    CreatingLine,
    Archiving,
    Uploading,
    ImportingProject,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CopyUnit {
    Files,
    Bytes,
    Commits,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CopyProgress {
    pub phase: CopyPhase,
    #[schemars(range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub done: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub total: Option<u64>,
    pub unit: CopyUnit,
}
