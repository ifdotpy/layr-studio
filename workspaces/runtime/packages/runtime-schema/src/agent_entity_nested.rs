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

pub(crate) fn schemas() -> [(&'static str, serde_json::Value); 3] {
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
    [
        ("AgentOverview", overview),
        ("AgentSource", source),
        ("AgentNaming", naming),
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
    fn published_components_match_owners() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        for (name, schema) in schemas() {
            assert_eq!(document["components"]["schemas"][name], schema, "{name}");
        }
    }
}
