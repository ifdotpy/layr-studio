//! Known Codex error classifications. The generated vendor leaf owns their
//! spellings and payloads; parity rejects a changed known set. This is a tag
//! projection, not a replacement for the lossless provider error JSON record.
use serde::de::value::{Error, StrDeserializer};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The provider-owned portion of an agent error that Studio displays or classifies.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NativeProviderError {
    #[serde(default)]
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_error_info: Option<CodexErrorInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<serde_json::Value>")]
    pub claude_failure: Option<crate::claude_failure::ClaudeFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<serde_json::Value>")]
    pub data: Option<Box<serde_json::value::RawValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<serde_json::Value>")]
    pub additional_details: Option<Box<serde_json::value::RawValue>>,
}

impl PartialEq for NativeProviderError {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message
            && self.code == other.code
            && self.codex_error_info == other.codex_error_info
            && self.claude_failure == other.claude_failure
            && self.data.as_deref().map(|value| value.get())
                == other.data.as_deref().map(|value| value.get())
            && self.additional_details.as_deref().map(|value| value.get())
                == other.additional_details.as_deref().map(|value| value.get())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CodexErrorInfo {
    Text(String),
    Structured(BTreeMap<String, CodexErrorDetail>),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexErrorDetail {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_kind: Option<String>,
}

pub fn native_provider_error_schema() -> Value {
    let mut settings = schemars::generate::SchemaSettings::openapi3();
    settings.inline_subschemas = true;
    let mut schema = settings
        .into_generator()
        .into_root_schema_for::<NativeProviderError>()
        .to_value();
    schema["properties"]["claudeFailure"] = json!({"$ref":"#/components/schemas/ClaudeFailure"});
    schema["properties"]["data"] = json!({"$ref":"#/components/schemas/JsonValue"});
    schema["properties"]["additionalDetails"] = json!({"$ref":"#/components/schemas/JsonValue"});
    schema
}

#[derive(Clone, Debug, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum AgentError {
    Message(String),
    Provider(Box<NativeProviderError>),
}

impl<'de> Deserialize<'de> for AgentError {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
        if raw.get().starts_with('"') {
            serde_json::from_str(raw.get())
                .map(Self::Message)
                .map_err(serde::de::Error::custom)
        } else {
            serde_json::from_str(raw.get())
                .map(Box::new)
                .map(Self::Provider)
                .map_err(serde::de::Error::custom)
        }
    }
}

impl AgentError {
    pub fn display_text(&self) -> &str {
        match self {
            Self::Message(message) => message,
            Self::Provider(error) => &error.message,
        }
    }

    pub fn diagnostic_text(&self) -> String {
        match self {
            Self::Message(message) => message.clone(),
            // Provider error fields contain only validated JSON values and strings.
            Self::Provider(error) => serde_json::to_string(error).expect("valid provider error"),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Message(message) => Some(message),
            Self::Provider(_) => None,
        }
    }

    pub fn known_kind(&self) -> Option<KnownCodexErrorKind> {
        let Self::Provider(error) = self else {
            return None;
        };
        let kind = match error.codex_error_info.as_ref()? {
            CodexErrorInfo::Text(kind) => kind.as_str(),
            CodexErrorInfo::Structured(fields) if fields.len() == 1 => {
                fields.keys().next()?.as_str()
            }
            CodexErrorInfo::Structured(_) => return None,
        };
        serde::Deserialize::deserialize(StrDeserializer::<Error>::new(kind)).ok()
    }

    /// Normalize a provider receipt at its existing JSON ingress boundary.
    pub fn from_receipt(value: Value) -> Result<Self, serde_json::Error> {
        let mut fields = match value {
            Value::String(message) => return Ok(Self::Message(message)),
            Value::Object(fields) => fields,
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => {
                return Err(serde::de::Error::custom(
                    "expected an error string or object",
                ));
            }
        };
        match fields.remove("error") {
            Some(Value::Object(mut nested)) => {
                for key in ["message", "code"] {
                    if !fields.contains_key(key)
                        && let Some(value) = nested.remove(key)
                    {
                        fields.insert(key.to_owned(), value);
                    }
                }
            }
            Some(Value::String(message)) if !fields.contains_key("message") => {
                fields.insert("message".into(), Value::String(message));
            }
            _ => {}
        }
        let message = match fields.remove("message") {
            Some(Value::String(message)) => message,
            Some(Value::Object(message)) => Value::Object(message).to_string(),
            _ => String::new(),
        };
        let code = match fields.remove("code") {
            Some(Value::String(code)) => Some(code),
            Some(Value::Number(code)) if code.is_i64() || code.is_u64() => Some(code.to_string()),
            _ => None,
        };
        let codex_error_info = fields
            .remove("codexErrorInfo")
            .filter(|v| !v.is_null())
            .map(CodexErrorInfo::deserialize)
            .transpose()?;
        let claude_failure = fields
            .remove("claudeFailure")
            .filter(|v| !v.is_null())
            .map(crate::claude_failure::ClaudeFailure::deserialize)
            .transpose()?;
        let raw = |value: Value| serde_json::value::RawValue::from_string(value.to_string());
        let data = fields
            .remove("data")
            .filter(|v| !v.is_null())
            .map(raw)
            .transpose()?;
        let additional_details = fields
            .remove("additionalDetails")
            .filter(|v| !v.is_null())
            .map(raw)
            .transpose()?;
        Ok(Self::Provider(Box::new(NativeProviderError {
            message,
            code,
            codex_error_info,
            claude_failure,
            data,
            additional_details,
        })))
    }
}

pub const CODEX_ERROR_SCHEMA: &str = include_str!("schemas/codex/CodexErrorInfo.json");

pub use crate::codex_generated::KnownCodexErrorKind;

/// External and historical records may carry a string or a single-key object.
/// Never choose an arbitrary key from an ambiguous provider extension object.
pub fn raw_error_kind(error: &Value) -> Option<&str> {
    let info = error.get("codexErrorInfo")?;
    info.as_str().or_else(|| {
        let mut keys = info.as_object()?.keys();
        let key = keys.next()?;
        keys.next().is_none().then_some(key.as_str())
    })
}

/// Classify known tags only. Unknown external records remain with their caller;
/// this function never rewrites the original payload or invents an enum variant.
pub fn known_error_kind(error: &Value) -> Option<KnownCodexErrorKind> {
    serde::Deserialize::deserialize(StrDeserializer::<Error>::new(raw_error_kind(error)?)).ok()
}

/// The vendor's thread-scoped precaution stop, shared by admission and recovery.
pub fn is_misalignment_policy_violation(error: &Value) -> bool {
    known_error_kind(error) == Some(KnownCodexErrorKind::MisalignmentPolicyViolation)
}

fn without_annotations(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "$schema" | "$comment" | "title" | "description"
                    )
                })
                .map(|(key, value)| (key.clone(), without_annotations(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(without_annotations).collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
}

/// Compatibility is scoped to the consumed error leaf, including its open
/// future-value branch and structured payloads. Documentation changes are inert.
pub fn codex_error_schema_compatible(notification: &Value) -> bool {
    let Ok(mut owner) = serde_json::from_str::<Value>(CODEX_ERROR_SCHEMA) else {
        return false;
    };
    let Some(candidate) = notification.pointer("/definitions/CodexErrorInfo") else {
        return false;
    };
    let Some(turn_kind) = notification.pointer("/definitions/NonSteerableTurnKind") else {
        return false;
    };
    let Some(definitions) = owner
        .as_object_mut()
        .and_then(|object| object.remove("definitions"))
    else {
        return false;
    };
    without_annotations(&owner) == without_annotations(candidate)
        && definitions
            .get("NonSteerableTurnKind")
            .is_some_and(|expected| without_annotations(expected) == without_annotations(turn_kind))
}

/// Vendor error payload types plus the closed classification projection, for
/// the existing TypeScript renderer. No complete vendor protocol is imported.
pub fn schemas() -> Result<Value, String> {
    let mut leaf: Value =
        serde_json::from_str(CODEX_ERROR_SCHEMA).map_err(|error| error.to_string())?;
    let definitions = leaf
        .as_object_mut()
        .ok_or("Codex error schema must be an object")?
        .remove("definitions")
        .ok_or("Codex error schema is missing definitions")?;
    // JSON Schema leaves object properties open by default. Spell this out for
    // openapi-typescript, whose default object rendering would erase extensions.
    let future = leaf["anyOf"]
        .as_array_mut()
        .and_then(|branches| branches.last_mut())
        .ok_or("Codex error schema is missing its future-value branch")?;
    if future["type"] != json!(["string", "object"]) {
        return Err("Codex error future-value branch changed".into());
    }
    future["additionalProperties"] = json!(true);
    let document = json!({
        "CodexErrorInfo": leaf,
        "NonSteerableTurnKind": definitions["NonSteerableTurnKind"],
        "KnownCodexErrorKind": schemars::schema_for!(KnownCodexErrorKind),
    });
    serde_json::from_str(
        &document
            .to_string()
            .replace("#/definitions/", "#/components/schemas/"),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_provider_error_contract_matches_owner() {
        let document: Value = serde_json::from_slice(crate::OPENAPI).unwrap();
        assert_eq!(
            document["components"]["schemas"]["NativeProviderError"],
            native_provider_error_schema()
        );
    }

    #[test]
    fn provider_receipt_normalizes_and_keeps_details() {
        let receipt = json!({"error":{"message":{"detail":"no"},"code":429},"codexErrorInfo":"serverOverloaded","data":{"trace":"x"},"additionalDetails":"details","discarded":true});
        let error = AgentError::from_receipt(receipt).unwrap();
        let AgentError::Provider(error) = error else {
            panic!("provider error")
        };
        assert_eq!(error.message, r#"{"detail":"no"}"#);
        assert_eq!(error.code.as_deref(), Some("429"));
        assert_eq!(error.data.as_deref().unwrap().get(), r#"{"trace":"x"}"#);
        assert!(!serde_json::to_string(&error).unwrap().contains("discarded"));
    }

    #[test]
    fn known_error_classification_matches_vendor_leaf() {
        let leaf: Value = serde_json::from_str(CODEX_ERROR_SCHEMA).expect("vendor leaf");
        let mut known = Vec::new();
        for branch in leaf["anyOf"].as_array().expect("vendor variants") {
            if let Some(values) = branch["enum"].as_array() {
                known.extend(values.iter().cloned());
            } else if let Some(properties) = branch["properties"].as_object() {
                known.extend(properties.keys().map(|name| json!(name)));
            }
        }
        let mirror = schemars::schema_for!(KnownCodexErrorKind).to_value();
        let mut mirror = mirror["enum"]
            .as_array()
            .expect("closed classification")
            .clone();
        known.sort_by_key(Value::to_string);
        mirror.sort_by_key(Value::to_string);
        assert_eq!(
            mirror, known,
            "CodexErrorInfo.json owns every known classification"
        );
        for kind in known {
            assert!(known_error_kind(&json!({"codexErrorInfo":kind})).is_some());
        }
    }

    #[test]
    fn unknown_and_ambiguous_error_records_remain_lossless() {
        for info in [
            json!("futureCode"),
            json!({"futureCode":{"opaque":[1,null]}}),
            json!({"serverOverloaded":{},"futureCode":{}}),
            json!(["serverOverloaded"]),
        ] {
            let error = json!({"codexErrorInfo":info,"message":"failure","extension":{"id":42}});
            let original = error.clone();
            assert_eq!(known_error_kind(&error), None);
            assert_eq!(error, original);
        }
        assert_eq!(
            known_error_kind(&json!({"codexErrorInfo":{"tooManyDenials":{}}})),
            Some(KnownCodexErrorKind::TooManyDenials)
        );
    }

    #[test]
    fn error_leaf_compatibility_rejects_variant_and_payload_drift() {
        let leaf: Value = serde_json::from_str(CODEX_ERROR_SCHEMA).expect("vendor leaf");
        let mut error = leaf.clone();
        error
            .as_object_mut()
            .expect("leaf object")
            .remove("definitions");
        let source = json!({"definitions":{"CodexErrorInfo":error,"NonSteerableTurnKind":leaf["definitions"]["NonSteerableTurnKind"]}});
        assert!(codex_error_schema_compatible(&source));
        let mut documented = source.clone();
        documented["definitions"]["CodexErrorInfo"]["description"] = json!("new documentation");
        assert!(codex_error_schema_compatible(&documented));
        for pointer in [
            "/definitions/CodexErrorInfo/anyOf/0/enum/0",
            "/definitions/CodexErrorInfo/anyOf/1/properties/httpConnectionFailed/properties/httpStatusCode/type",
            "/definitions/NonSteerableTurnKind/enum/0",
        ] {
            let mut changed = source.clone();
            *changed.pointer_mut(pointer).expect("consumed contract") = json!("drift");
            assert!(!codex_error_schema_compatible(&changed), "{pointer}");
        }
    }
}
