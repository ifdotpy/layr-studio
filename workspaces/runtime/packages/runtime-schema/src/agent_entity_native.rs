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
    #[schemars(with = "Option<String>")]
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub native_state: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub previous_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub read_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub restart_turn_status: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<crate::bounded_integer::BoundedInteger<0, 9007199254740991>>")]
    pub epoch: Field<SafeInteger>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeThreadBlockDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<crate::provider_errors::NativeProviderError>")]
    pub error: Field<crate::provider_errors::NativeProviderError>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTurnErrorDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<crate::provider_errors::NativeProviderError>")]
    pub error: Field<crate::provider_errors::NativeProviderError>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSafetyRetryDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub accepted_turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub model: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub new_thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub request_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub rpc_method: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    pub created: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    pub updated: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<crate::bounded_integer::BoundedInteger<0, 9007199254740991>>")]
    pub epoch: Field<SafeInteger>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<NativeSafetyStage>")]
    pub stage: Field<NativeSafetyStage>,
}

pub(crate) fn schemas() -> [(&'static str, serde_json::Value); 5] {
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
        schema["title"] = (*name).into();
        if let Some(properties) = schema.get_mut("properties").and_then(|p| p.as_object_mut()) {
            for (key, value) in properties {
                let title = format!("{}{}", key[..1].to_uppercase(), key[1..].to_lowercase());
                if key == "at" {
                    *value = serde_json::json!({"type":"number", "title":title});
                    continue;
                }
                let inner = if key == "error"
                    && (*name == "NativeThreadBlockDto" || *name == "NativeTurnErrorDto")
                {
                    serde_json::json!({"$ref":"#/components/schemas/NativeProviderError"})
                } else if key == "stage" {
                    serde_json::json!({"$ref":"#/components/schemas/NativeSafetyStage"})
                } else if key == "epoch" {
                    serde_json::json!({"type":"integer", "minimum":0, "maximum":crate::shared_values::MAX_SAFE_INTEGER})
                } else if key == "created" || key == "updated" {
                    serde_json::json!({"type":"number"})
                } else {
                    serde_json::json!({"type":"string"})
                };
                let reference =
                    key == "stage" || (key == "error" && *name != "NativeSafetyRetryDto");
                let variants = if reference || key == "epoch" {
                    serde_json::json!([inner, {"type":"null"}])
                } else {
                    serde_json::json!([{"type":"null"}, inner])
                };
                *value = serde_json::json!({"anyOf":variants,"default":null});
                if !reference {
                    value["title"] = title.into();
                }
            }
        }
    }
    schemas
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
        for (name, schema) in schemas() {
            assert_eq!(document["components"]["schemas"][name], schema, "{name}");
        }
    }
}
