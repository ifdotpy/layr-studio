//! Agent creation request fields retained for replay.
use crate::{claude_permissions::PermissionMode, queue_command::Field};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod review_state_tests {
    #[test]
    fn legacy_review_state_maps_to_typed_review() {
        let state = "0123456789abcdef0123456789abcdef";
        let intent: super::CreationIntent = studio_json::from_str(
            r#"{"role":"reviewer","review_state":"0123456789abcdef0123456789abcdef"}"#,
        )
        .unwrap();
        assert_eq!(
            intent.review,
            Some(crate::review::ReviewTarget::LayrState {
                state: crate::review::LayrStateId::try_from(state.to_owned()).unwrap(),
                base: crate::review::LayrBase::MergeBaseWithMain,
            })
        );
    }
    #[test]
    fn invalid_legacy_review_state_is_refused() {
        let value = studio_json::json!({"role":"reviewer","review_state":"invalid"});
        assert!(studio_json::from_value::<super::CreationIntent>(value).is_err());
    }
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

/// An explicitly selected effort overrides the inherited setting; null clears it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EffortSelection {
    #[default]
    Inherit,
    Clear,
    Set(String),
}

impl EffortSelection {
    pub fn is_inherit(&self) -> bool {
        matches!(self, Self::Inherit)
    }

    pub fn selected(&self) -> Option<&String> {
        match self {
            Self::Set(value) => Some(value),
            Self::Inherit | Self::Clear => None,
        }
    }
}

impl<'de> Deserialize<'de> for EffortSelection {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<String>::deserialize(deserializer)? {
            Some(value) => Self::Set(value),
            None => Self::Clear,
        })
    }
}

impl Serialize for EffortSelection {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.selected().serialize(serializer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Orchestrator,
    Implementer,
    Reviewer,
}

impl AgentRole {
    pub const fn inferred(native_review: bool, is_lead: bool) -> Self {
        if native_review {
            Self::Reviewer
        } else if is_lead {
            Self::Orchestrator
        } else {
            Self::Implementer
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Orchestrator => "orchestrator",
            Self::Implementer => "implementer",
            Self::Reviewer => "reviewer",
        }
    }
}

/// Select the chat exclusion list for the affected agent's role.
pub fn excluded_for<'a, T>(role: AgentRole, workers: &'a [T], reviewers: &'a [T]) -> &'a [T] {
    match role {
        AgentRole::Implementer => workers,
        AgentRole::Reviewer => reviewers,
        AgentRole::Orchestrator => &[],
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreationIntent {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub account_key: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub concurrency: Option<i64>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_excluded_account_keys: Option<Vec<crate::account_key::AccountKey>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer_excluded_account_keys: Option<Vec<crate::account_key::AccountKey>>,
    #[serde(
        rename = "layrStoreRoot",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub layr_store_root: Option<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub daybreak_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "EffortSelection::is_inherit")]
    pub effort: EffortSelection,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub fast_mode: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub id: Field<String>,
    #[serde(
        rename = "maxAgents",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_agents: Option<i64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub model: Field<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub parent: Field<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub permission_mode: Option<PermissionMode>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub previous: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub profile_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub project_folder: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub project_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub project_server_id: Field<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub prompt: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub reuse_empty: Option<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub review_defaults: Field<ReviewDefaults>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub role: Option<AgentRole>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub worker_defaults: Field<WorkerDefaults>,
    #[serde(
        rename = "workspaceMode",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub workspace_mode: Option<crate::records::WorkspaceMode>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub yolo_mode: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub base_ref: Option<String>,
    #[serde(
        default,
        alias = "review_state",
        deserialize_with = "review_target",
        skip_serializing_if = "Option::is_none"
    )]
    pub review: Option<crate::review::ReviewTarget>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub environment: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub server: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub task_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub workspace: Option<crate::records::WorkspaceMode>,
}

fn review_target<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<crate::review::ReviewTarget>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Input {
        Target(crate::review::ReviewTarget),
        Legacy(crate::review::LayrStateId),
    }
    Ok(Some(match Input::deserialize(d)? {
        Input::Target(target) => target,
        Input::Legacy(state) => crate::review::ReviewTarget::LayrState {
            state,
            base: crate::review::LayrBase::MergeBaseWithMain,
        },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerDefaults {
    #[serde(deserialize_with = "nullable")]
    pub model: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub effort: Option<String>,
    pub fast_mode: bool,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub daybreak_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    /// Historical request identity only; new requests must leave this missing.
    pub account_key: Field<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub delegation_depth: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDefaults {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    /// Historical request identity only; new requests must leave this missing.
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub model: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub effort: Field<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub fast_mode: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::{AgentRole, CreationIntent};

    #[test]
    fn stored_intents_preserve_missing_and_null_for_replay() {
        for raw in [
            r#"{"account_key":"default"}"#,
            r#"{"account_key":"default","daybreak_enabled":null,"effort":null,"fast_mode":null,"review_defaults":null,"worker_defaults":null}"#,
            r#"{"account_key":"default","effort":"high"}"#,
        ] {
            let stored: CreationIntent = studio_json::from_str(raw).unwrap();
            let encoded = studio_json::to_string(&stored).unwrap();
            assert_eq!(encoded, raw);
            let replay: CreationIntent = studio_json::from_str(&encoded).unwrap();
            assert_eq!(stored, replay);
        }
        let absent: CreationIntent = studio_json::from_str("{}").unwrap();
        let cleared: CreationIntent = studio_json::from_str(r#"{"effort":null}"#).unwrap();
        assert_ne!(absent, cleared);
    }

    #[test]
    fn historical_account_defaults_preserve_request_identity_bytes() {
        let raw = r#"{"review_defaults":{"account_key":"reviewer","model":"gpt-6-astra","effort":"high"},"worker_defaults":{"model":"gpt-6-luna","effort":"high","fast_mode":false,"account_key":"worker"}}"#;
        let intent: CreationIntent = studio_json::from_str(raw).unwrap();
        assert_eq!(studio_json::to_string(&intent).unwrap(), raw);
    }

    #[test]
    fn agent_role_matches_openapi_and_its_wire_names() {
        let openapi: studio_json::Value = studio_json::from_str(crate::OPENAPI_TEXT).unwrap();
        let roles = [
            AgentRole::Orchestrator,
            AgentRole::Implementer,
            AgentRole::Reviewer,
        ];
        let names: Vec<&str> = roles.iter().map(|role| role.as_str()).collect();
        assert_eq!(
            openapi["components"]["schemas"]["AgentRole-Input"]["enum"],
            studio_json::json!(names)
        );
        for role in roles {
            assert_eq!(
                studio_json::to_string(&role).unwrap(),
                format!("\"{}\"", role.as_str())
            );
        }
    }
}
