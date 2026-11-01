//! Known Codex error classifications. The generated vendor leaf owns their
//! spellings and payloads; parity rejects a changed known set. This is a tag
//! projection, not a replacement for the lossless provider error JSON record.
use serde::de::value::{Error, StrDeserializer};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

// Reserved for the complete original provider value; marks the normalized wire form.
const STUDIO_RAW_EVIDENCE: &str = "studioRawEvidence";

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
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl PartialEq for NativeProviderError {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message
            && self.code == other.code
            && self.codex_error_info == other.codex_error_info
            && self.claude_failure == other.claude_failure
            && self.extra == other.extra
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
    Structured(BTreeMap<String, CodexErrorPayload>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CodexErrorPayload {
    Detail(CodexErrorDetail),
    Other(Value),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexErrorDetail {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_kind: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub fn native_provider_error_schema() -> Value {
    let mut settings = schemars::generate::SchemaSettings::openapi3();
    settings.inline_subschemas = true;
    let mut schema = settings
        .into_generator()
        .into_root_schema_for::<NativeProviderError>()
        .to_value();
    schema["properties"]["claudeFailure"] = json!({"$ref":"#/components/schemas/ClaudeFailure"});
    schema["additionalProperties"] = json!({"$ref":"#/components/schemas/JsonValue"});
    schema["properties"]["codexErrorInfo"] = json!({"$ref":"#/components/schemas/CodexErrorInfo"});
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
        let original = Value::deserialize(deserializer)?;
        if let Value::String(text) = &original
            && readable_message(&original, 0).as_deref() == Some(text.as_str())
        {
            return Ok(Self::Message(text.clone()));
        }
        if original.get(STUDIO_RAW_EVIDENCE).is_some()
            && let Ok(canonical) = NativeProviderError::deserialize(original.clone())
        {
            return Ok(Self::Provider(Box::new(canonical)));
        }
        let mut fields = original.as_object().cloned().unwrap_or_default();
        let message = readable_message(&original, 0).unwrap_or_default();
        let code = fields
            .remove("code")
            .and_then(|value| match value {
                Value::String(value) => Some(value),
                Value::Number(value) => Some(value.to_string()),
                _ => None,
            })
            .or_else(|| {
                [
                    "Invalid prompt: we've limited access to this content for safety reasons.",
                    "This content was flagged for possible biological risk.",
                ]
                .iter()
                .any(|prefix| message.starts_with(prefix))
                .then(|| "bio_policy".to_owned())
            });
        let codex_error_info = fields
            .remove("codexErrorInfo")
            .and_then(|value| CodexErrorInfo::deserialize(value).ok());
        let claude_failure = fields
            .remove("claudeFailure")
            .and_then(|value| crate::claude_failure::ClaudeFailure::deserialize(value).ok());
        let raw = |value: Value| serde_json::value::RawValue::from_string(value.to_string());
        let data = fields
            .remove("data")
            .filter(|v| !v.is_null())
            .map(raw)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let additional_details = fields
            .remove("additionalDetails")
            .filter(|v| !v.is_null())
            .map(raw)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        fields.remove("message");
        fields.insert(STUDIO_RAW_EVIDENCE.into(), original);
        Ok(Self::Provider(Box::new(NativeProviderError {
            message,
            code,
            codex_error_info,
            claude_failure,
            data,
            additional_details,
            extra: fields.into_iter().collect(),
        })))
    }
}

fn readable_message(value: &Value, depth: usize) -> Option<String> {
    if depth >= 8 {
        return None;
    }
    match value {
        Value::Object(fields) => fields
            .get("error")
            .and_then(|v| readable_message(v, depth + 1))
            .or_else(|| {
                fields.get("message").and_then(|v| {
                    readable_message(v, depth + 1).or_else(|| v.is_object().then(|| v.to_string()))
                })
            }),
        Value::String(text) => {
            if text.len() <= 65_536 {
                if let Ok(parsed) = serde_json::from_str::<Value>(text)
                    && parsed != *value
                    && let Some(message) = readable_message(&parsed, depth + 1)
                {
                    return Some(message);
                }
                let (mut start, mut level, mut quoted, mut escaped) = (None, 0usize, false, false);
                for (index, ch) in text.char_indices() {
                    if start.is_none() {
                        if ch == '{' {
                            start = Some(index);
                            level = 1;
                        }
                        continue;
                    }
                    if quoted {
                        if escaped {
                            escaped = false;
                        } else if ch == '\\' {
                            escaped = true;
                        } else if ch == '"' {
                            quoted = false;
                        }
                    } else if ch == '"' {
                        quoted = true;
                    } else if ch == '{' {
                        level += 1;
                    } else if ch == '}' {
                        level -= 1;
                        if level == 0
                            && let Some(begin) = start.take()
                            && let Ok(parsed) = serde_json::from_str::<Value>(&text[begin..=index])
                            && let Some(message) = readable_message(&parsed, depth + 1)
                        {
                            return Some(message);
                        }
                    }
                }
            }
            Some(text.clone())
        }
        _ => None,
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
            Self::Provider(error) => error
                .extra
                .get(STUDIO_RAW_EVIDENCE)
                .and_then(Value::as_str)
                .map(|_| error.message.as_str()),
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

    /// Decode receipts through the same path as persisted errors.
    pub fn from_receipt(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
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
    fn plain_studio_disconnect_remains_a_string_after_round_trip() {
        let raw = json!(crate::shared_values::PROVIDER_DISCONNECTED_TURN_ERROR);
        let decoded: AgentError = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), raw);
    }

    #[test]
    fn provider_raw_evidence_key_remains_provider_data() {
        let raw = json!({"message":"failure","rawEvidence":{"provider":42},"extra":null});
        let error: AgentError = serde_json::from_value(raw.clone()).unwrap();
        let value = serde_json::to_value(error).unwrap();
        assert_eq!(value.get("rawEvidence"), raw.get("rawEvidence"));
        assert_eq!(value.get("studioRawEvidence"), Some(&raw));
    }

    #[test]
    fn typed_error_deserialization_keeps_original_and_ignores_embedded_classification() {
        for raw in [
            json!({"message":"failed", "code":{"number":42}, "data":null, "additionalDetails":null, "vendor":null}),
            json!({"message":r#"HTTP 500: {"error":{"message":"Failed","codexErrorInfo":"misalignmentPolicyViolation"}}"#}),
            json!(r#""Service unavailable""#),
        ] {
            let error: AgentError = serde_json::from_value(raw.clone()).unwrap();
            assert_eq!(error.known_kind(), None);
            let normalized = serde_json::to_value(error).unwrap();
            assert_eq!(normalized.get("studioRawEvidence"), Some(&raw));
        }
    }

    #[test]
    fn provider_receipt_preserves_unrecognized_failure_and_nested_collisions() {
        for receipt in [
            json!({"message":"failure", "claudeFailure":{"kind":"future", "opaque":[1,null]}}),
            json!({"message":{"message":"inner", "code":"inner", "detail":1}, "code":"outer"}),
        ] {
            let normalized =
                serde_json::to_value(AgentError::from_receipt(receipt.clone()).unwrap()).unwrap();
            assert_eq!(normalized.get("studioRawEvidence"), Some(&receipt));
            let decoded: AgentError = serde_json::from_value(normalized.clone()).unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), normalized);
        }
    }

    #[test]
    fn legacy_renderer_raw_cases_are_normalized_at_ingress() {
        let nested = r#"{"error":{"message":"Service unavailable","code":"unavailable"}}"#;
        let nested_notification = json!({"error":{"message":nested}}).to_string();
        for raw in [
            nested.to_owned(),
            format!("```json\n{nested}\n```"),
            format!("HTTP 503: {nested} (request abc)"),
            nested_notification,
            format!("HTTP error {{\"requestId\":\"abc\"}} then {nested}"),
        ] {
            let direct =
                serde_json::to_value(AgentError::from_receipt(json!(raw)).unwrap()).unwrap();
            assert_eq!(direct.get("message"), Some(&json!("Service unavailable")));
            assert_eq!(direct.get("code"), None);
            assert_eq!(direct.get("studioRawEvidence"), Some(&json!(raw)));
            let receipt = json!({"message":raw,"codexErrorInfo":"httpConnectionFailed"});
            let wrapped =
                serde_json::to_value(AgentError::from_receipt(receipt.clone()).unwrap()).unwrap();
            assert_eq!(wrapped.get("message"), Some(&json!("Service unavailable")));
            assert_eq!(wrapped.get("code"), None);
            assert_eq!(
                wrapped.get("codexErrorInfo"),
                Some(&json!("httpConnectionFailed"))
            );
            assert_eq!(wrapped.get("studioRawEvidence"), Some(&receipt));
        }
        let rpc = r#"{"message":"RPC rejected","code":-32600}"#;
        assert_eq!(
            serde_json::to_value(AgentError::from_receipt(json!(rpc)).unwrap()).unwrap(),
            json!({"message":"RPC rejected","studioRawEvidence":rpc})
        );
        let quoted = r#""Service unavailable""#;
        assert_eq!(
            serde_json::to_value(AgentError::from_receipt(json!(quoted)).unwrap()).unwrap(),
            json!({"message":"Service unavailable","studioRawEvidence":quoted})
        );
        let nested = json!({"error":{"message":"Nested failure"}});
        assert_eq!(
            serde_json::to_value(AgentError::from_receipt(nested).unwrap()).unwrap(),
            json!({"message":"Nested failure","error":{"message":"Nested failure"},"studioRawEvidence":{"error":{"message":"Nested failure"}}})
        );
        for raw in [
            "Error instance",
            r#"HTTP 503: {"error":{"message":"Unavailable"}}"#,
        ] {
            let actual = AgentError::from_receipt(json!({"message":raw})).unwrap();
            assert_eq!(
                actual.display_text(),
                if raw == "Error instance" {
                    raw
                } else {
                    "Unavailable"
                }
            );
        }
        let raw = json!({"message":"  ","codexErrorInfo":"serverOverloaded"}).to_string();
        let actual = serde_json::to_value(AgentError::from_receipt(json!(raw)).unwrap()).unwrap();
        assert_eq!(actual, json!({"message":"  ","studioRawEvidence":raw}));
        let actual = serde_json::to_value(
            AgentError::from_receipt(json!({"data":{"codexErrorInfo":"serverOverloaded"}}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            actual,
            json!({"message":"","data":{"codexErrorInfo":"serverOverloaded"},"studioRawEvidence":{"data":{"codexErrorInfo":"serverOverloaded"}}})
        );
    }

    #[test]
    fn legacy_renderer_remaining_raw_cases_have_exact_normalized_fields() {
        let biological = [
            "Invalid prompt: we've limited access to this content for safety reasons. More context",
            "This content was flagged for possible biological risk.",
        ];
        for message in biological {
            let actual = serde_json::to_value(
                AgentError::from_receipt(json!({"message":message,"codexErrorInfo":"other"}))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(actual.get("message"), Some(&json!(message)));
            assert_eq!(actual.get("code"), Some(&json!("bio_policy")));
            assert_eq!(actual.get("codexErrorInfo"), Some(&json!("other")));
        }
        for raw in [
            r#"HTTP 400: {"error":{"code":"bio_policy","message":"Restricted"}} tail"#,
            "```json\n{\"error\":{\"code\":\"bio_policy\",\"message\":\"Restricted\"}}\n```",
        ] {
            let receipt = json!({"message":raw,"codexErrorInfo":"other"});
            let actual =
                serde_json::to_value(AgentError::from_receipt(receipt.clone()).unwrap()).unwrap();
            assert_eq!(actual.get("message"), Some(&json!("Restricted")));
            assert_eq!(actual.get("code"), None);
            assert_eq!(actual.get("codexErrorInfo"), Some(&json!("other")));
            assert_eq!(actual.get("studioRawEvidence"), Some(&receipt));
        }
        for raw in [
            r#"HTTP 500: {"message":"bio_policy mentioned in a log"}"#,
            r#"HTTP 500: {"error":{"message":"Failed"},"metadata":{"code":"bio_policy"}}"#,
            r#"HTTP 500: {"error":{"message":"Failed","codexErrorInfo":"misalignmentPolicyViolation"}}"#,
            r#"{"message":"Failed","metadata":{"error":{"code":"bio_policy"}}}"#,
        ] {
            let actual = serde_json::to_value(
                AgentError::from_receipt(json!({"message":raw,"codexErrorInfo":"other"})).unwrap(),
            )
            .unwrap();
            assert_eq!(actual.get("codexErrorInfo"), Some(&json!("other")));
            assert_eq!(actual.get("code"), None);
        }
        for raw in [
            r#"HTTP failure {"requestId":"abc"} tail"#,
            r#"{"requestId":"abc"}"#,
            r#"HTTP failure {"error":{"message":null}} tail"#,
            r#"HTTP failure {"error":{"message":"unterminated}"#,
        ] {
            assert_eq!(
                AgentError::from_receipt(json!(raw)).unwrap().display_text(),
                raw
            );
        }
        let excessive = format!("HTTP failure {}", "{".repeat(70_000));
        assert_eq!(
            AgentError::from_receipt(json!(excessive))
                .unwrap()
                .display_text(),
            excessive
        );
        let receipt =
            json!({"message":r#"{"error":{"message":"<img src=x onerror=\"alert(1)\"> {text}"}}"#});
        assert_eq!(
            AgentError::from_receipt(receipt).unwrap().display_text(),
            r#"<img src=x onerror="alert(1)"> {text}"#
        );
        let disk = serde_json::to_value(
            AgentError::from_receipt(json!({"message":"Disk is full.","data":{"diskFull":true}}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            disk,
            json!({"message":"Disk is full.","data":{"diskFull":true},"studioRawEvidence":{"message":"Disk is full.","data":{"diskFull":true}}})
        );
    }

    #[test]
    fn provider_errors_preserve_extensions_and_normalize_encoded_messages() {
        for info in [
            json!({"futureLimit":{"opaque":[1,null]}}),
            json!({"futureLimit":[1,null]}),
        ] {
            let receipt = json!({"error":{"message":"no", "vendorNested":true}, "codexErrorInfo":info, "vendorExtension":[1,null]});
            let normalized =
                serde_json::to_value(AgentError::from_receipt(receipt.clone()).unwrap()).unwrap();
            assert_eq!(
                normalized.get("codexErrorInfo"),
                receipt.get("codexErrorInfo")
            );
            assert_eq!(
                normalized.get("vendorExtension"),
                receipt.get("vendorExtension")
            );
            assert_eq!(normalized.get("error"), receipt.get("error"));
        }
        let encoded = json!(r#"{"error":{"message":"Write failed","data":{"diskFull":true}}}"#);
        let normalized =
            serde_json::to_value(AgentError::from_receipt(encoded.clone()).unwrap()).unwrap();
        assert_eq!(normalized.get("message"), Some(&json!("Write failed")));
        assert_eq!(normalized.get("studioRawEvidence"), Some(&encoded));
        assert_eq!(normalized.get("codexErrorInfo"), None);
        for receipt in [
            json!(r#"HTTP 503: {"error":{"message":"Unavailable"}}"#),
            json!({"message":r#"{"error":{"message":"Unavailable"}}"#}),
        ] {
            let error = AgentError::from_receipt(receipt).unwrap();
            assert_eq!(error.display_text(), "Unavailable");
        }
        let error = AgentError::from_receipt(
            json!({"message":"overloaded","data":{"codexErrorInfo":"serverOverloaded"}}),
        )
        .unwrap();
        assert_eq!(error.known_kind(), None);
    }

    #[test]
    fn provider_receipt_normalizes_and_keeps_details() {
        let receipt = json!({"error":{"message":{"detail":"no"},"code":429},"codexErrorInfo":"serverOverloaded","data":{"trace":"x"},"additionalDetails":"details","discarded":true});
        let error = AgentError::from_receipt(receipt.clone()).unwrap();
        let AgentError::Provider(error) = error else {
            panic!("provider error")
        };
        assert_eq!(error.message, r#"{"detail":"no"}"#);
        assert_eq!(error.code, None);
        assert_eq!(error.extra.get("studioRawEvidence"), Some(&receipt));
        assert_eq!(error.data.as_deref().unwrap().get(), r#"{"trace":"x"}"#);
        assert!(serde_json::to_string(&error).unwrap().contains("discarded"));
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
