//! Strict project context values published in sync views.
use crate::queue_command::Field;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Ready,
    Importing,
    NotImported,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReadiness {
    pub state: ReadinessState,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub project_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    Syncing,
    Synced,
    Waiting,
    Conflict,
    Paused,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSync {
    pub project_id: String,
    pub state: SyncState,
    pub enabled: bool,
    pub conflicts: Vec<String>,
    pub choices: Vec<String>,
    pub with_agent: Vec<String>,
    pub deletes:
        crate::rule_record::RuleInteger<0, { crate::shared_values::MAX_SAFE_INTEGER as u64 }>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reason: Field<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VaultState {
    Ok,
    Error,
    None,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectVault {
    pub project_id: String,
    pub state: VaultState,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub disk: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub last_push: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub prefix: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reason: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub site: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub unlocked: Field<bool>,
}
