//! Nested agent wire contracts.
use crate::queue_command::Field;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentOverview {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub task: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub task_truncated: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub result: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub result_truncated: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub result_file: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub result_turn_id: Field<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AgentSource {
    #[default]
    #[serde(rename = "managed")]
    Managed,
    #[serde(rename = "registered")]
    Registered,
    #[serde(rename = "app-server")]
    AppServer,
    #[serde(rename = "orchestrator-reference")]
    OrchestratorReference,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentNaming {
    #[default]
    Automatic,
    PendingTitle,
    Manual,
}

impl AgentNaming {
    pub fn set_needs_title(&mut self, title: Option<bool>) {
        if *self != Self::Manual {
            *self = if title == Some(true) {
                Self::PendingTitle
            } else {
                Self::Automatic
            };
        }
    }

    pub fn needs_title(&self) -> Option<bool> {
        Some(*self == Self::PendingTitle)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentEntityDtoKind {
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentEntityDtoEnvironment {
    Host,
    Linux,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentEntityDtoExecutionMode {
    Vm,
    Native,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentEntityDtoWorktreePreparation {
    Waiting,
    Preparing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AgentEntityDtoWorktree {
    Boolean(bool),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AgentEntityDtoError {
    NativeProviderError(crate::provider_errors::NativeProviderError),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Multi,
    Single,
}

// Stored unknown statuses are addressed by work-1793547226242324480-6.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Unknown,
    Idle,
    Queued,
    Starting,
    Running,
    Waiting,
    Approval,
    Completed,
    Interrupted,
    Failed,
    Blocked,
    Paused,
    Parked,
    Abandoned,
    #[serde(rename = "capacity-retry")]
    CapacityRetry,
    WaitingOnMonitor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AgentNativeStatusValue {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "notLoaded")]
    NotLoaded,
    #[serde(rename = "unsubscribed")]
    Unsubscribed,
    #[serde(rename = "notSubscribed")]
    NotSubscribed,
}

/// Context repair wait fields rendered in native notices.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentEntityContextRepairWaitDto {
    pub scope: String,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MovePointerDto {
    pub server: String,
    #[serde(rename = "move")]
    pub move_id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub agent_id: Field<String>,
}

fn required_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

fn optional_nonnull<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// Maximum delegation depth for worker defaults.
pub type WorkerDelegationDepth = crate::bounded_integer::BoundedInteger<1, 3>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerDefaultsDto {
    #[serde(deserialize_with = "required_nullable")]
    pub model: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub effort: Option<String>,
    pub fast_mode: bool,
    #[serde(
        default,
        deserialize_with = "optional_nonnull",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub daybreak_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<crate::account_key::AccountKey>")]
    pub account_key: Field<crate::account_key::AccountKey>,
    #[serde(
        default,
        deserialize_with = "optional_nonnull",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "WorkerDelegationDepth")]
    pub delegation_depth: Option<WorkerDelegationDepth>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub cyber_access_program: Field<String>,
}

pub(crate) fn schemas() -> [(&'static str, serde_json::Value); 9] {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    let mut overview = AgentOverview::json_schema(&mut generator).to_value();
    overview["title"] = "AgentOverview".into();
    // Preserve the published OpenAPI spelling of optional nullable properties.
    for (name, title) in [
        ("result", "Result"),
        ("resultFile", "Resultfile"),
        ("resultTruncated", "Resulttruncated"),
        ("resultTurnId", "Resultturnid"),
        ("task", "Task"),
        ("taskTruncated", "Tasktruncated"),
    ] {
        let kind = overview["properties"][name]["type"][0].clone();
        let variants = if kind == "string" {
            serde_json::json!([{"type":"null"},{"type":kind}])
        } else {
            serde_json::json!([{"type":kind},{"type":"null"}])
        };
        overview["properties"][name] =
            serde_json::json!({"anyOf":variants,"default":null,"title":title});
    }
    let mut source = AgentSource::json_schema(&mut generator).to_value();
    source["title"] = "AgentSource".into();
    let naming = AgentNaming::json_schema(&mut generator).to_value();
    let mut mode = AgentMode::json_schema(&mut generator).to_value();
    mode["title"] = "AgentMode".into();
    let mut status = AgentStatus::json_schema(&mut generator).to_value();
    status["title"] = "AgentStatus".into();
    let mut native_status = AgentNativeStatusValue::json_schema(&mut generator).to_value();
    native_status["title"] = "AgentNativeStatusValue".into();
    let mut repair = AgentEntityContextRepairWaitDto::json_schema(&mut generator).to_value();
    repair["title"] = "AgentEntityContextRepairWaitDto".into();
    repair["properties"]["scope"]["title"] = "Scope".into();
    repair["properties"]["error"]["title"] = "Error".into();
    let mut pointer = MovePointerDto::json_schema(&mut generator).to_value();
    pointer["title"] = "MovePointerDto".into();
    pointer["properties"]["server"]["title"] = "Server".into();
    pointer["properties"]["move"]["title"] = "Move".into();
    let agent_id_type = pointer["properties"]["agentId"]["type"][0].clone();
    pointer["properties"]["agentId"] = serde_json::json!({"anyOf":[{"type":"null"},{"type":agent_id_type}],"default":null,"title":"Agentid"});
    let mut defaults = WorkerDefaultsDto::json_schema(&mut generator).to_value();
    defaults["title"] = "WorkerDefaultsDto".into();
    for (name, title) in [
        ("model", "Model"),
        ("effort", "Effort"),
        ("accountKey", "Accountkey"),
        ("cyberAccessProgram", "Cyberaccessprogram"),
    ] {
        let property = &mut defaults["properties"][name];
        let mut value = property.clone();
        if let Some(types) = value["type"].as_array_mut() {
            types.retain(|kind| kind != "null");
            if types.len() == 1 {
                value["type"] = types[0].clone();
            }
        }
        *property = serde_json::json!({"anyOf":[{"type":"null"},value],"title":title});
        if name == "accountKey" || name == "cyberAccessProgram" {
            property["default"] = serde_json::Value::Null;
        }
    }
    for (name, title, default) in [
        ("fastMode", "Fastmode", None),
        (
            "daybreakEnabled",
            "Daybreakenabled",
            Some(serde_json::json!(false)),
        ),
        (
            "delegationDepth",
            "Delegationdepth",
            Some(serde_json::json!(1)),
        ),
    ] {
        let property = &mut defaults["properties"][name];
        property["title"] = title.into();
        if let Some(default) = default {
            property["default"] = default;
        }
    }
    [
        ("AgentOverview", overview),
        ("AgentSource", source),
        ("AgentNaming", naming),
        ("AgentMode", mode),
        ("AgentStatus", status),
        ("AgentNativeStatusValue", native_status),
        ("AgentEntityContextRepairWaitDto", repair),
        ("MovePointerDto", pointer),
        ("WorkerDefaultsDto", defaults),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overview_preserves_missing_null_and_values() {
        for value in [
            json!({}),
            json!({"task":null,"resultTruncated":false}),
            json!({"task":"work","resultFile":"path"}),
        ] {
            let overview: AgentOverview = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(overview).unwrap(), value);
        }
        assert!(serde_json::from_value::<AgentOverview>(json!({"extra":true})).is_err());
    }

    #[test]
    fn worker_defaults_reject_invalid_keys_and_missing_or_null_required_fields() {
        let valid = json!({"model":null,"effort":null,"fastMode":false});
        assert!(serde_json::from_value::<WorkerDefaultsDto>(valid.clone()).is_ok());
        for invalid in [
            json!({"effort":null,"fastMode":false}),
            json!({"model":null,"effort":null,"fastMode":null}),
            json!({"model":null,"effort":null,"fastMode":false,"accountKey":""}),
            json!({"model":null,"effort":null,"fastMode":false,"daybreakEnabled":null}),
            json!({"model":null,"effort":null,"fastMode":false,"delegationDepth":4}),
        ] {
            assert!(serde_json::from_value::<WorkerDefaultsDto>(invalid).is_err());
        }
    }

    #[test]
    fn published_components_match_owners() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        for (name, schema) in schemas() {
            let mut published = document["components"]["schemas"][name].clone();
            if name == "WorkerDefaultsDto" {
                published["properties"]["accountKey"]["anyOf"][1]["minLength"] = 1.into();
                published["properties"]["accountKey"]["anyOf"][1]["maxLength"] =
                    crate::account_key::ACCOUNT_KEY_MAX_CHARS.into();
            }
            assert_eq!(published, schema, "{name}");
        }
    }
}
