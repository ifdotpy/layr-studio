//! Retry and resume fields in the managed agent wire view.
use crate::AgentCounter;
use crate::continuation_snapshot::CapacityContinuationCause;
use crate::queue_command::Field;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

fn nullable_counter(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"type":"integer","minimum":0,"maximum":crate::shared_values::MAX_SAFE_INTEGER},{"type":"null"}]})
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentEntityCapacityRetryDto {
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub thread_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    #[schemars(schema_with = "nullable_counter")]
    pub epoch: Option<AgentCounter>,
    #[serde(deserialize_with = "nullable")]
    pub account_key: Option<String>,
    pub status: String,
    pub updated_at: f64,
    #[serde(deserialize_with = "nullable")]
    pub due_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub accepted_turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<CapacityContinuationCause>")]
    pub cause: Field<CapacityContinuationCause>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<f64>")]
    pub claimed_at: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
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

pub(crate) fn schemas() -> Result<[(&'static str, serde_json::Value); 3], String> {
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
        ("epoch", "Epoch"),
    ] {
        let prop = property(&mut retry, name)?;
        title(prop, label)?;
        if name == "updatedAt" {
            if object(prop)?.remove("format") != Some(serde_json::json!("double")) {
                return Err("updatedAt must use the double number format".into());
            }
        }
    }
    let cause = property(&mut retry, "cause")?;
    let variants = object(cause)?
        .get_mut("anyOf")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("capacity retry cause must be nullable")?;
    if variants.len() != 2
        || variants[0]["$ref"] != "#/components/schemas/CapacityContinuationCause"
        || variants[1] != serde_json::json!({"type":"null"})
    {
        return Err("capacity retry cause must reference its owner and null".into());
    }
    variants[0]["$ref"] = "#/components/schemas/CapacityRetryCause".into();
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
    Ok([
        ("AgentEntityCapacityRetryDto", retry),
        ("CapacityRetryCause", retry_cause),
        ("AgentEntityUsageResumeDto", resume),
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
}
