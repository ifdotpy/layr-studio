//! Native agent connection and retry wire records.
use crate::{queue_command::Field, recovery_record::SafeInteger};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NativeSafetyStage {
    Turns,
    Items,
    Interrupt,
    VerifyTurns,
    VerifyItems,
    Fork,
    Start,
    Unknown,
    Running,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionCheckDto {
    pub at: serde_json::Number,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_state: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub previous_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub read_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub restart_turn_status: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub epoch: Field<SafeInteger>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeThreadBlockDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<crate::provider_errors::NativeProviderError>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTurnErrorDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<crate::provider_errors::NativeProviderError>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSafetyRetryDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub accepted_turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub model: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub new_thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub request_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub rpc_method: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub created: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub updated: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub epoch: Field<SafeInteger>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub stage: Field<NativeSafetyStage>,
}

pub(crate) fn schemas() -> Result<[(&'static str, serde_json::Value); 5], String> {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    let mut schemas = [
        (
            "ConnectionCheckDto",
            ConnectionCheckDto::json_schema(&mut generator).to_value(),
        ),
        (
            "NativeThreadBlockDto",
            NativeThreadBlockDto::json_schema(&mut generator).to_value(),
        ),
        (
            "NativeTurnErrorDto",
            NativeTurnErrorDto::json_schema(&mut generator).to_value(),
        ),
        (
            "NativeSafetyRetryDto",
            NativeSafetyRetryDto::json_schema(&mut generator).to_value(),
        ),
        (
            "NativeSafetyStage",
            NativeSafetyStage::json_schema(&mut generator).to_value(),
        ),
    ];
    for (name, schema) in &mut schemas {
        let schema_object = schema
            .as_object_mut()
            .ok_or_else(|| format!("{name} must be an object"))?;
        schema_object.insert("title".into(), (*name).into());
        if *name == "NativeSafetyStage" {
            continue;
        }
        let properties = schema_object
            .get_mut("properties")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| format!("{name} must have properties"))?;
        for (key, value) in properties {
            let mut chars = key.chars();
            let first = chars
                .next()
                .ok_or_else(|| format!("{name} has an empty property"))?;
            let label = format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase());
            let property = value
                .as_object_mut()
                .ok_or_else(|| format!("{name}.{key} must be an object"))?;
            if *name == "ConnectionCheckDto" && key == "at" {
                property.insert("title".into(), label.into());
                continue;
            }
            if let Some(variants) = property.get("anyOf") {
                let variants = variants
                    .as_array()
                    .ok_or_else(|| format!("{name}.{key} anyOf must be an array"))?;
                if variants.len() != 2
                    || variants[0]
                        .get("$ref")
                        .and_then(serde_json::Value::as_str)
                        .is_none()
                    || variants[1].get("type").and_then(serde_json::Value::as_str) != Some("null")
                {
                    return Err(format!("{name}.{key} must be a nullable reference"));
                }
                property.insert("default".into(), serde_json::Value::Null);
                continue;
            }
            let kinds = property
                .get("type")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| format!("{name}.{key} must have nullable type"))?;
            if kinds.len() != 2 || kinds[1] != "null" || !kinds[0].is_string() {
                return Err(format!("{name}.{key} must have one type and null"));
            }
            let kind = kinds[0].clone();
            property.insert("type".into(), kind.clone());
            let inner = value.clone();
            let variants = if kind == "integer" {
                serde_json::json!([inner,{"type":"null"}])
            } else {
                serde_json::json!([{"type":"null"},inner])
            };
            *value = serde_json::json!({"anyOf":variants,"default":null,"title":label});
        }
    }
    Ok(schemas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn records_preserve_missing_null_and_validate_epochs() {
        for value in [
            json!({"at":1}),
            json!({"at":1.5,"epoch":2.0,"threadId":null}),
        ] {
            let dto: ConnectionCheckDto = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(dto).unwrap(), value);
        }
        for value in [
            json!({}),
            json!({"at":1,"epoch":-1}),
            json!({"at":1,"epoch":1.5}),
            json!({"at":1,"extra":true}),
        ] {
            assert!(serde_json::from_value::<ConnectionCheckDto>(value).is_err());
        }
        let value = json!({"stage":"verify_turns","error":null,"created":1.5});
        let dto: NativeSafetyRetryDto = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(dto).unwrap(), value);
        assert!(
            serde_json::from_value::<NativeSafetyRetryDto>(json!({"stage":"invalid"})).is_err()
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
