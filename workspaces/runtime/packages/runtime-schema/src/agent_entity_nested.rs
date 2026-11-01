//! Nested agent wire contracts.
use crate::queue_command::Field;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentOverview {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub task: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub task_truncated: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub result: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub result_truncated: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub result_file: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NativeStatusPhase {
    Safety,
    Retrying,
    Auth,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AgentNativeStatusError {
    NativeProviderError(crate::provider_errors::NativeProviderError),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentNativeStatus {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub at: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<AgentNativeStatusError>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub message: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub phase: Field<NativeStatusPhase>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_id: Field<String>,
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
    pub agent_id: Field<String>,
}

fn required_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerDefaultsDto {
    #[serde(deserialize_with = "required_nullable")]
    pub model: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub effort: Option<String>,
    pub fast_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daybreak_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation_depth: Option<crate::bounded_integer::BoundedInteger<1, 3>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cyber_access_program: Field<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionSettingsDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cyber_access_program: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub daybreak_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub effort: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub fast_mode: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub model: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_effort: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub updated_at: Field<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCleanupSummaryDto {
    pub phase: String,
    #[serde(deserialize_with = "required_nullable")]
    pub reason: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActiveToolDto {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentActivity {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub at: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub phase: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub tools: Field<Vec<ActiveToolDto>>,
}

pub(crate) fn object(
    value: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| "agent schema must be an object".into())
}

pub(crate) fn property<'a>(
    schema: &'a mut serde_json::Value,
    name: &str,
) -> Result<&'a mut serde_json::Value, String> {
    object(schema)?
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|properties| properties.get_mut(name))
        .ok_or_else(|| format!("missing agent schema property {name}"))
}

pub(crate) fn title(schema: &mut serde_json::Value, name: &str) -> Result<(), String> {
    object(schema)?.insert("title".into(), name.into());
    Ok(())
}

// Keep the established OpenAPI nullable spelling while retaining schemars bounds.
pub(crate) fn nullable(
    schema: &mut serde_json::Value,
    name: &str,
    label: &str,
    default: bool,
) -> Result<(), String> {
    let prop = property(schema, name)?;
    let mut value = prop.clone();
    let value_object = object(&mut value)?;
    let types = value_object
        .get("type")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{name} must have nullable type"))?;
    if types.len() != 2 || types[1] != "null" || !types[0].is_string() {
        return Err(format!("{name} must have one type and null"));
    }
    let kind = types[0].clone();
    value_object.insert("type".into(), kind.clone());
    if kind == "number" {
        match value_object.remove("format") {
            Some(serde_json::Value::String(format)) if format == "double" => {}
            _ => return Err(format!("{name} must use the double number format")),
        }
    }
    let variants = if kind == "boolean" || kind == "array" {
        serde_json::json!([value,{"type":"null"}])
    } else {
        serde_json::json!([{"type":"null"},value])
    };
    *prop = serde_json::json!({"anyOf":variants,"title":label});
    if default {
        object(prop)?.insert("default".into(), serde_json::Value::Null);
    }
    Ok(())
}

fn inline_nullable_reference(
    schema: &mut serde_json::Value,
    definitions: &serde_json::Map<String, serde_json::Value>,
    name: &str,
    label: &str,
    flatten: bool,
) -> Result<(), String> {
    let prop = property(schema, name)?;
    let obj = object(prop)?;
    let variants = obj
        .get("anyOf")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{name} must be a nullable reference"))?;
    if variants.len() != 2
        || variants[1].get("type").and_then(serde_json::Value::as_str) != Some("null")
    {
        return Err(format!("{name} must be a nullable reference"));
    }
    let reference = variants[0]
        .get("$ref")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| value.strip_prefix("#/components/schemas/"))
        .ok_or_else(|| format!("{name} must reference a generated definition"))?;
    let definition = definitions
        .get(reference)
        .cloned()
        .ok_or_else(|| format!("missing generated definition {reference}"))?;
    let variants = if flatten {
        let definition_object = definition
            .as_object()
            .ok_or("union definition must be an object")?;
        if definition_object.len() != 1 {
            return Err("union definition has unexpected constraints".into());
        }
        let mut union = definition_object
            .get("anyOf")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .ok_or("union definition must have variants")?;
        if union.len() != 2 {
            return Err("union definition must have two variants".into());
        }
        union.insert(1, serde_json::json!({"type":"null"}));
        union
    } else {
        vec![definition, serde_json::json!({"type":"null"})]
    };
    obj.insert("anyOf".into(), variants.into());
    obj.insert("default".into(), serde_json::Value::Null);
    obj.insert("title".into(), label.into());
    Ok(())
}

pub(crate) fn schemas() -> Result<[(&'static str, serde_json::Value); 14], String> {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    let mut overview = AgentOverview::json_schema(&mut generator).to_value();
    title(&mut overview, "AgentOverview")?;
    for (name, label) in [
        ("result", "Result"),
        ("resultFile", "Resultfile"),
        ("resultTruncated", "Resulttruncated"),
        ("resultTurnId", "Resultturnid"),
        ("task", "Task"),
        ("taskTruncated", "Tasktruncated"),
    ] {
        nullable(&mut overview, name, label, true)?;
    }
    let mut source = AgentSource::json_schema(&mut generator).to_value();
    title(&mut source, "AgentSource")?;
    let naming = AgentNaming::json_schema(&mut generator).to_value();
    let mut mode = AgentMode::json_schema(&mut generator).to_value();
    title(&mut mode, "AgentMode")?;
    let mut status = AgentStatus::json_schema(&mut generator).to_value();
    title(&mut status, "AgentStatus")?;
    let mut native_status = AgentNativeStatusValue::json_schema(&mut generator).to_value();
    title(&mut native_status, "AgentNativeStatusValue")?;
    let mut native_status_record = AgentNativeStatus::json_schema(&mut generator).to_value();
    title(&mut native_status_record, "AgentNativeStatus")?;
    for (name, label) in [("at", "At"), ("message", "Message"), ("turnId", "Turnid")] {
        nullable(&mut native_status_record, name, label, true)?;
    }
    inline_nullable_reference(
        &mut native_status_record,
        generator.definitions(),
        "error",
        "Error",
        true,
    )?;
    inline_nullable_reference(
        &mut native_status_record,
        generator.definitions(),
        "phase",
        "Phase",
        false,
    )?;
    let mut repair = AgentEntityContextRepairWaitDto::json_schema(&mut generator).to_value();
    title(&mut repair, "AgentEntityContextRepairWaitDto")?;
    for (name, label) in [("scope", "Scope"), ("error", "Error")] {
        title(property(&mut repair, name)?, label)?;
    }
    let mut pointer = MovePointerDto::json_schema(&mut generator).to_value();
    title(&mut pointer, "MovePointerDto")?;
    for (name, label) in [("server", "Server"), ("move", "Move")] {
        title(property(&mut pointer, name)?, label)?;
    }
    nullable(&mut pointer, "agentId", "Agentid", true)?;
    let mut defaults = WorkerDefaultsDto::json_schema(&mut generator).to_value();
    title(&mut defaults, "WorkerDefaultsDto")?;
    for (name, label, default) in [
        ("model", "Model", false),
        ("effort", "Effort", false),
        ("accountKey", "Accountkey", true),
        ("cyberAccessProgram", "Cyberaccessprogram", true),
    ] {
        nullable(&mut defaults, name, label, default)?;
    }
    for (name, label, default) in [
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
        let prop = property(&mut defaults, name)?;
        title(prop, label)?;
        if let Some(default) = default {
            object(prop)?.insert("default".into(), default);
        }
    }
    let mut settings = ExecutionSettingsDto::json_schema(&mut generator).to_value();
    title(&mut settings, "ExecutionSettingsDto")?;
    for (name, label) in [
        ("accountKey", "Accountkey"),
        ("cyberAccessProgram", "Cyberaccessprogram"),
        ("daybreakEnabled", "Daybreakenabled"),
        ("effort", "Effort"),
        ("fastMode", "Fastmode"),
        ("model", "Model"),
        ("nativeEffort", "Nativeeffort"),
        ("updatedAt", "Updatedat"),
    ] {
        nullable(&mut settings, name, label, true)?;
    }
    let mut cleanup = WorkspaceCleanupSummaryDto::json_schema(&mut generator).to_value();
    for name in ["reason", "path"] {
        let prop = property(&mut cleanup, name)?;
        let prop_object = object(prop)?;
        let types = prop_object
            .get("type")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{name} must have nullable type"))?;
        if types.len() != 2 || types[1] != "null" {
            return Err(format!("{name} must have one type and null"));
        }
        let kind = types[0].clone();
        let mut inner = prop.clone();
        object(&mut inner)?.insert("type".into(), kind);
        *prop = serde_json::json!({"anyOf":[inner,{"type":"null"}]});
    }
    let mut tool = ActiveToolDto::json_schema(&mut generator).to_value();
    title(&mut tool, "ActiveToolDto")?;
    for (name, label) in [("id", "Id"), ("type", "Type"), ("name", "Name")] {
        title(property(&mut tool, name)?, label)?;
    }
    let mut activity = AgentActivity::json_schema(&mut generator).to_value();
    title(&mut activity, "AgentActivity")?;
    nullable(&mut activity, "at", "At", true)?;
    nullable(&mut activity, "phase", "Phase", true)?;
    nullable(&mut activity, "tools", "Tools", true)?;
    Ok([
        ("AgentOverview", overview),
        ("AgentSource", source),
        ("AgentNaming", naming),
        ("AgentMode", mode),
        ("AgentStatus", status),
        ("AgentNativeStatusValue", native_status),
        ("AgentNativeStatus", native_status_record),
        ("AgentEntityContextRepairWaitDto", repair),
        ("MovePointerDto", pointer),
        ("WorkerDefaultsDto", defaults),
        ("ExecutionSettingsDto", settings),
        ("WorkspaceCleanupSummaryDto", cleanup),
        ("ActiveToolDto", tool),
        ("AgentActivity", activity),
    ])
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
    fn worker_defaults_preserve_keys_and_reject_missing_or_null_required_fields() {
        let valid = json!({"model":null,"effort":null,"fastMode":false});
        assert!(serde_json::from_value::<WorkerDefaultsDto>(valid.clone()).is_ok());
        let null_options = json!({"model":null,"effort":null,"fastMode":false,"daybreakEnabled":null,"delegationDepth":null});
        let defaults: WorkerDefaultsDto = serde_json::from_value(null_options).unwrap();
        assert_eq!(serde_json::to_value(defaults).unwrap(), valid);
        for invalid in [
            json!({"effort":null,"fastMode":false}),
            json!({"model":null,"effort":null,"fastMode":null}),
            json!({"model":null,"effort":null,"fastMode":false,"delegationDepth":4}),
        ] {
            assert!(serde_json::from_value::<WorkerDefaultsDto>(invalid).is_err());
        }
        for key in [String::new(), "x".repeat(201)] {
            let value = json!({"model":null,"effort":null,"fastMode":false,"accountKey":key});
            let defaults: WorkerDefaultsDto = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(defaults).unwrap(), value);
        }
    }

    #[test]
    fn execution_settings_preserves_presence_and_rejects_unknown_fields() {
        for value in [
            json!({}),
            json!({"model":null,"fastMode":false,"updatedAt":1.5}),
        ] {
            let settings: ExecutionSettingsDto = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(settings).unwrap(), value);
        }
        assert!(serde_json::from_value::<ExecutionSettingsDto>(json!({"extra":true})).is_err());
    }

    #[test]
    fn native_status_preserves_optional_fields_and_rejects_unknown_phase() {
        let value = json!({"phase":"safety","error":"failed","at":null});
        let status: AgentNativeStatus = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(status).unwrap(), value);
        assert!(serde_json::from_value::<AgentNativeStatus>(json!({"phase":"unknown"})).is_err());
    }

    #[test]
    fn native_status_schema_rejects_a_changed_error_field() {
        #[derive(JsonSchema)]
        struct ChangedStatus {
            error: Field<String>,
        }
        assert!(
            ChangedStatus {
                error: Field::Missing
            }
            .error
            .is_missing()
        );
        let mut generator = schemars::generate::SchemaSettings::openapi3()
            .for_serialize()
            .into_generator();
        let mut changed = ChangedStatus::json_schema(&mut generator).to_value();
        assert!(
            inline_nullable_reference(
                &mut changed,
                generator.definitions(),
                "error",
                "Error",
                true
            )
            .is_err()
        );
    }

    #[test]
    fn published_native_status_error_matches_serde() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        let root = json!({"$ref":"#/components/schemas/AgentNativeStatus","components":document["components"]});
        let validator = jsonschema::validator_for(&root).unwrap();
        for (value, valid) in [
            (json!({"error":{"message":"failed"}}), true),
            (json!({"error":7}), false),
        ] {
            assert_eq!(validator.is_valid(&value), valid);
            assert_eq!(
                serde_json::from_value::<AgentNativeStatus>(value).is_ok(),
                valid
            );
        }
    }

    #[test]
    fn activity_schema_preserves_a_changed_tool_item_type() {
        #[derive(JsonSchema)]
        struct ChangedActivity {
            tools: Field<Vec<String>>,
        }
        assert!(
            ChangedActivity {
                tools: Field::Missing
            }
            .tools
            .is_missing()
        );
        let mut generator = schemars::generate::SchemaSettings::openapi3()
            .for_serialize()
            .into_generator();
        let mut changed = ChangedActivity::json_schema(&mut generator).to_value();
        nullable(&mut changed, "tools", "Tools", true).unwrap();
        let published: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        assert_ne!(
            changed["properties"]["tools"],
            published["components"]["schemas"]["AgentActivity"]["properties"]["tools"]
        );
    }

    #[test]
    fn published_components_match_owners() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        for (name, schema) in schemas().unwrap() {
            assert_eq!(document["components"]["schemas"][name], schema, "{name}");
        }
    }
}
