//! Retry and resume fields in the managed agent wire view.
use crate::AgentCounter;
use crate::continuation_snapshot::CapacityContinuationCause;
use crate::queue_command::Field;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An optional field that accepts a value when present, but not null.
#[derive(Clone, Debug)]
pub struct Present<T>(pub Option<T>);

impl<T> Default for Present<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T> Present<T> {
    pub fn is_missing(&self) -> bool {
        self.0.is_none()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Present<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(|value| Self(Some(value)))
    }
}

impl<T: Serialize> Serialize for Present<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<T: JsonSchema> JsonSchema for Present<T> {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        T::schema_name()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        T::json_schema(generator)
    }
}

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

pub(crate) fn normalize_nullable_counter(
    schema: &mut serde_json::Value,
    name: &str,
    label: &str,
    default: bool,
) -> Result<(), String> {
    use crate::agent_entity_nested::{object, property, title};
    let prop = property(schema, name)?;
    let mut value = prop.clone();
    let value_object = object(&mut value)?;
    let kind = value_object
        .get_mut("type")
        .ok_or_else(|| format!("{name} has no type"))?;
    if *kind != serde_json::json!(["integer", "null"]) {
        return Err(format!("{name} must be a nullable integer"));
    }
    *kind = "integer".into();
    *prop = serde_json::json!({"anyOf":[value,{"type":"null"}]});
    if default {
        object(prop)?.insert("default".into(), serde_json::Value::Null);
    }
    title(prop, label)
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextUsageDto {
    #[serde(deserialize_with = "nullable")]
    pub tokens: Option<AgentCounter>,
    #[serde(deserialize_with = "nullable")]
    pub window: Option<AgentCounter>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub at: Field<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadStateDto {
    pub thread_id: String,
    pub turn_id: String,
    pub read: bool,

    pub revision: AgentCounter,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Lead,
    Worker,
    Orchestrator,
    Agent,
    Implementer,
    Reviewer,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentNativeRelease {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub phase: Field<crate::persistence::NativeReleasePhase>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reset_pending: Field<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSafetyBufferingRecord {
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub account_key: Present<String>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub at: Present<f64>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub connection_id: Present<String>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub dismissed: Present<bool>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub faster_model: Present<String>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub response_started: Present<bool>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub show_buffering_ui: Present<bool>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub thread_id: Present<String>,
    #[serde(default, skip_serializing_if = "Present::is_missing")]
    pub turn_id: Present<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentStartAttempt {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub prepare_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub response_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub retired_events: Field<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(inline)]
pub enum ConversionActor {
    User,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConvertedFromLeadDto {
    pub request_id: String,
    pub by: ConversionActor,
    pub at: f64,
    pub old_root_id: String,
    pub root_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentEntityCapacityRetryDto {
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub thread_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub epoch: Option<AgentCounter>,
    #[serde(deserialize_with = "nullable")]
    pub account_key: Option<String>,
    pub status: String,
    pub updated_at: f64,
    #[serde(deserialize_with = "nullable")]
    pub due_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub accepted_turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cause: Field<CapacityContinuationCause>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub claimed_at: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reason: Field<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentEntityUsageResumeDto {
    pub id: String,
    pub status: String,
    pub cause: String,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<String>,
    pub updated_at: f64,
    #[serde(deserialize_with = "nullable")]
    pub planned_at: Option<f64>,
    #[serde(deserialize_with = "nullable")]
    pub due_at: Option<f64>,
}

pub(crate) fn schemas() -> Result<[(&'static str, serde_json::Value); 11], String> {
    use crate::agent_entity_nested::{nullable as normalize_nullable, object, property, title};
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .with(|settings| settings.definitions_path = "/components/schemas".into())
        .into_generator();
    let mut retry = AgentEntityCapacityRetryDto::json_schema(&mut generator).to_value();
    title(&mut retry, "AgentEntityCapacityRetryDto")?;
    object(&mut retry)?.insert(
        "description".into(),
        "Capacity retry state consumed by the entity-backed renderer.".into(),
    );
    for (name, label, default) in [
        ("acceptedTurnId", "Acceptedturnid", true),
        ("accountKey", "Accountkey", false),
        ("claimedAt", "Claimedat", true),
        ("dueAt", "Dueat", false),
        ("reason", "Reason", true),
        ("threadId", "Threadid", false),
    ] {
        normalize_nullable(&mut retry, name, label, default)?;
    }
    for (name, label) in [
        ("id", "Id"),
        ("status", "Status"),
        ("updatedAt", "Updatedat"),
    ] {
        let prop = property(&mut retry, name)?;
        title(prop, label)?;
        if name == "updatedAt" {
            if object(prop)?.remove("format") != Some(serde_json::json!("double")) {
                return Err("updatedAt must use the double number format".into());
            }
        }
    }
    normalize_nullable_counter(&mut retry, "epoch", "Epoch", false)?;
    let cause = property(&mut retry, "cause")?;
    let variants = object(cause)?
        .get_mut("anyOf")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("capacity retry cause must be nullable")?;
    if variants.len() != 2
        || variants
            .first()
            .and_then(|v| v.get("$ref"))
            .and_then(serde_json::Value::as_str)
            != Some("#/components/schemas/CapacityContinuationCause")
        || variants.get(1) != Some(&serde_json::json!({"type":"null"}))
    {
        return Err("capacity retry cause must reference its owner and null".into());
    }
    let reference = variants
        .first_mut()
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|v| v.get_mut("$ref"))
        .ok_or("missing capacity retry cause reference")?;
    *reference = "#/components/schemas/CapacityRetryCause".into();
    object(cause)?.insert("default".into(), serde_json::Value::Null);
    let mut retry_cause = CapacityContinuationCause::json_schema(&mut generator).to_value();
    title(&mut retry_cause, "CapacityRetryCause")?;
    let mut resume = AgentEntityUsageResumeDto::json_schema(&mut generator).to_value();
    title(&mut resume, "AgentEntityUsageResumeDto")?;
    object(&mut resume)?.insert(
        "description".into(),
        "Usage resume fields rendered in notices and recovery controls.".into(),
    );
    for (name, label) in [
        ("reason", "Reason"),
        ("plannedAt", "Plannedat"),
        ("dueAt", "Dueat"),
    ] {
        normalize_nullable(&mut resume, name, label, false)?;
    }
    for (name, label) in [
        ("id", "Id"),
        ("status", "Status"),
        ("cause", "Cause"),
        ("updatedAt", "Updatedat"),
    ] {
        let prop = property(&mut resume, name)?;
        title(prop, label)?;
        if name == "updatedAt" {
            if object(prop)?.remove("format") != Some(serde_json::json!("double")) {
                return Err("updatedAt must use the double number format".into());
            }
        }
    }
    let mut usage = ContextUsageDto::json_schema(&mut generator).to_value();
    title(&mut usage, "ContextUsageDto")?;
    normalize_nullable(&mut usage, "at", "At", true)?;
    for (name, label) in [("tokens", "Tokens"), ("window", "Window")] {
        normalize_nullable_counter(&mut usage, name, label, false)?;
    }
    let mut read = ReadStateDto::json_schema(&mut generator).to_value();
    title(&mut read, "ReadStateDto")?;
    for (name, label) in [
        ("threadId", "Threadid"),
        ("turnId", "Turnid"),
        ("read", "Read"),
        ("revision", "Revision"),
    ] {
        title(property(&mut read, name)?, label)?;
    }
    let mut role = AgentRole::json_schema(&mut generator).to_value();
    title(&mut role, "AgentRole")?;
    let mut release = AgentNativeRelease::json_schema(&mut generator).to_value();
    title(&mut release, "AgentNativeRelease")?;
    normalize_nullable(&mut release, "resetPending", "Resetpending", true)?;
    let phase = property(&mut release, "phase")?;
    let variants = object(phase)?
        .get_mut("anyOf")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("release phase must be nullable")?;
    if variants.len() != 2
        || variants
            .first()
            .and_then(|v| v.get("$ref"))
            .and_then(serde_json::Value::as_str)
            != Some("#/components/schemas/NativeReleasePhase")
        || variants.get(1) != Some(&serde_json::json!({"type":"null"}))
    {
        return Err("release phase must reference its owner and null".into());
    }
    object(phase)?.insert("default".into(), serde_json::Value::Null);
    let mut release_phase =
        crate::persistence::NativeReleasePhase::json_schema(&mut generator).to_value();
    title(&mut release_phase, "NativeReleasePhase")?;
    let entries = object(&mut release_phase)?
        .get_mut("enum")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("release phases must be an enum")?;
    if entries.get(2).and_then(serde_json::Value::as_str) != Some("unknown")
        || entries.get(3).and_then(serde_json::Value::as_str) != Some("released")
    {
        return Err("release phase order changed".into());
    }
    entries.swap(2, 3);
    let mut buffering = NativeSafetyBufferingRecord::json_schema(&mut generator).to_value();
    title(&mut buffering, "NativeSafetyBufferingRecord")?;
    for (name, label) in [
        ("accountKey", "Accountkey"),
        ("at", "At"),
        ("connectionId", "Connectionid"),
        ("dismissed", "Dismissed"),
        ("fasterModel", "Fastermodel"),
        ("responseStarted", "Responsestarted"),
        ("showBufferingUi", "Showbufferingui"),
        ("threadId", "Threadid"),
        ("turnId", "Turnid"),
    ] {
        let prop = property(&mut buffering, name)?;
        if name == "at" && object(prop)?.remove("format") != Some(serde_json::json!("double")) {
            return Err("buffering at must use double format".into());
        }
        title(prop, label)?;
    }
    let mut attempt = AgentStartAttempt::json_schema(&mut generator).to_value();
    title(&mut attempt, "AgentStartAttempt")?;
    for (name, label) in [
        ("prepareError", "Prepareerror"),
        ("responseError", "Responseerror"),
    ] {
        normalize_nullable(&mut attempt, name, label, true)?;
    }
    let prop = property(&mut attempt, "retiredEvents")?;
    let mut list = prop.clone();
    let list_object = object(&mut list)?;
    let kind = list_object
        .get_mut("type")
        .ok_or("retiredEvents missing type")?;
    if *kind != serde_json::json!(["array", "null"]) {
        return Err("retiredEvents must be a nullable array".into());
    }
    *kind = "array".into();
    *prop =
        serde_json::json!({"anyOf":[list,{"type":"null"}],"default":null,"title":"Retiredevents"});
    let mut converted = ConvertedFromLeadDto::json_schema(&mut generator).to_value();
    title(&mut converted, "ConvertedFromLeadDto")?;
    object(&mut converted)?.insert(
        "description".into(),
        "User conversion provenance persisted when a peer lead becomes a worker.".into(),
    );
    for (name, label) in [
        ("requestId", "Requestid"),
        ("by", "By"),
        ("at", "At"),
        ("oldRootId", "Oldrootid"),
        ("rootId", "Rootid"),
    ] {
        let prop = property(&mut converted, name)?;
        let obj = object(prop)?;
        if name == "at" && obj.remove("format") != Some(serde_json::json!("double")) {
            return Err("conversion at must use double format".into());
        }
        if name == "by" {
            let variants = obj
                .remove("enum")
                .and_then(|v| v.as_array().cloned())
                .ok_or("conversion actor must be an enum")?;
            if variants.len() != 1 {
                return Err("conversion actor must have one variant".into());
            }
            obj.insert(
                "const".into(),
                variants.into_iter().next().ok_or("missing actor")?,
            );
        }
        title(prop, label)?;
    }
    Ok([
        ("AgentEntityCapacityRetryDto", retry),
        ("CapacityRetryCause", retry_cause),
        ("AgentEntityUsageResumeDto", resume),
        ("ContextUsageDto", usage),
        ("ReadStateDto", read),
        ("AgentRole", role),
        ("AgentNativeRelease", release),
        ("NativeReleasePhase", release_phase),
        ("NativeSafetyBufferingRecord", buffering),
        ("AgentStartAttempt", attempt),
        ("ConvertedFromLeadDto", converted),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn schemas_match_published_components() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        for (name, schema) in schemas().unwrap() {
            assert_eq!(document["components"]["schemas"][name], schema, "{name}");
        }
    }
    #[test]
    fn retry_and_resume_enforce_presence_and_round_trip_nulls() {
        let retry = json!({"id":"r","threadId":null,"epoch":1.0,"accountKey":null,"status":"scheduled","updatedAt":1.0,"dueAt":null,"reason":null,"cause":"diskFull"});
        let parsed: AgentEntityCapacityRetryDto = serde_json::from_value(retry.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), retry);
        for bad in [
            json!({"id":"r"}),
            json!({"id":"r","threadId":null,"epoch":-1,"accountKey":null,"status":"scheduled","updatedAt":1.0,"dueAt":null}),
        ] {
            assert!(serde_json::from_value::<AgentEntityCapacityRetryDto>(bad).is_err());
        }
        let resume = json!({"id":"r","status":"waiting","cause":"limit","reason":null,"updatedAt":1.0,"plannedAt":null,"dueAt":null});
        let parsed: AgentEntityUsageResumeDto = serde_json::from_value(resume.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), resume);
        let mut missing = resume;
        missing.as_object_mut().unwrap().remove("reason");
        assert!(serde_json::from_value::<AgentEntityUsageResumeDto>(missing).is_err());
    }
    #[derive(JsonSchema)]
    struct ChangedReadState {
        read: String,
    }

    #[derive(JsonSchema)]
    #[serde(rename_all = "camelCase")]
    struct ChangedStartAttempt {
        prepare_error: Field<bool>,
    }

    #[test]
    fn read_field_change_changes_rendered_schema() {
        let witness = ChangedReadState { read: "x".into() };
        assert_eq!(witness.read, "x");
        let mut generator = schemars::generate::SchemaSettings::openapi3()
            .for_serialize()
            .into_generator();
        let changed = ChangedReadState::json_schema(&mut generator).to_value();
        let actual = ReadStateDto::json_schema(&mut generator).to_value();
        assert_ne!(changed["properties"]["read"], actual["properties"]["read"]);
    }

    #[test]
    fn attempt_field_change_changes_derived_schema() {
        let witness = ChangedStartAttempt {
            prepare_error: Field::Value(false),
        };
        assert!(witness.prepare_error.is_some());
        let mut generator = schemars::generate::SchemaSettings::openapi3()
            .for_serialize()
            .into_generator();
        let changed = ChangedStartAttempt::json_schema(&mut generator).to_value();
        let actual = AgentStartAttempt::json_schema(&mut generator).to_value();
        assert_ne!(
            changed.pointer("/properties/prepareError"),
            actual.pointer("/properties/prepareError")
        );
    }

    #[test]
    fn context_and_read_state_reject_missing_or_out_of_range_counters() {
        assert!(
            serde_json::from_value::<ContextUsageDto>(json!({"tokens": null, "window": null}))
                .is_ok()
        );
        assert!(serde_json::from_value::<ContextUsageDto>(json!({"tokens": null})).is_err());
        assert!(
            serde_json::from_value::<ReadStateDto>(
                json!({"threadId":"t", "turnId":"u", "read":true, "revision":-1})
            )
            .is_err()
        );
    }

    #[test]
    fn buffering_and_attempt_keep_their_distinct_null_rules() {
        let buffering: NativeSafetyBufferingRecord =
            serde_json::from_value(json!({"dismissed": false})).unwrap();
        assert_eq!(
            serde_json::to_value(buffering).unwrap(),
            json!({"dismissed": false})
        );
        assert!(
            serde_json::from_value::<NativeSafetyBufferingRecord>(json!({"dismissed": null}))
                .is_err()
        );
        let attempt: AgentStartAttempt =
            serde_json::from_value(json!({"retiredEvents": null})).unwrap();
        assert_eq!(
            serde_json::to_value(attempt).unwrap(),
            json!({"retiredEvents": null})
        );
        assert!(
            serde_json::from_value::<ConvertedFromLeadDto>(
                json!({"requestId":"r","by":"agent","at":1,"oldRootId":"old","rootId":"new"})
            )
            .is_err()
        );
    }
}
