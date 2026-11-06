//! Value-free browser performance report contract.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerfClientReport {
    pub window_ms: u64,
    pub long_tasks: PerfLongTasks,
    pub interactions: Vec<PerfInteraction>,
    pub renders: Vec<PerfRender>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerfLongTasks {
    pub count: u64,
    pub total_ms: f64,
    pub max_ms: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerfInteraction {
    #[serde(rename = "type")]
    pub kind: String,
    pub count: u64,
    pub max_duration_ms: f64,
    pub max_input_delay_ms: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerfRender {
    pub component: String,
    pub count: u64,
    pub total_ms: f64,
    pub max_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;

    #[test]
    fn generated_contract_covers_every_dto_field() {
        let document: studio_json::Value = studio_json::from_str(include_str!(
            "../../runtime-http/openapi.perf-reporting.json"
        ))
        .expect("generated performance contract");
        let mut generator = schemars::generate::SchemaSettings::openapi3().into_generator();
        for (name, schema) in [
            (
                "PerfClientReport",
                PerfClientReport::json_schema(&mut generator).to_value(),
            ),
            (
                "PerfLongTasks",
                PerfLongTasks::json_schema(&mut generator).to_value(),
            ),
            (
                "PerfInteraction",
                PerfInteraction::json_schema(&mut generator).to_value(),
            ),
            (
                "PerfRender",
                PerfRender::json_schema(&mut generator).to_value(),
            ),
        ] {
            let expected = &document["components"]["schemas"][name];
            let actual_fields = schema["properties"].as_object().expect("DTO properties");
            let generated_fields = expected["properties"]
                .as_object()
                .expect("contract properties");
            assert_eq!(
                actual_fields.keys().collect::<Vec<_>>(),
                generated_fields.keys().collect::<Vec<_>>(),
                "{name}"
            );
            for (field, actual) in actual_fields {
                let generated = &generated_fields[field];
                assert_eq!(actual.get("type"), generated.get("type"), "{name}.{field}");
                assert_eq!(actual.get("$ref"), generated.get("$ref"), "{name}.{field}");
                assert_eq!(
                    actual.pointer("/items/$ref"),
                    generated.pointer("/items/$ref"),
                    "{name}.{field}"
                );
            }
        }
    }
}
