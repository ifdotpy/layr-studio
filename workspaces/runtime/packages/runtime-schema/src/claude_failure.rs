//! Claude failure metadata. The JSON schema owns allowed values and bounds.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeFailure {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_error_status: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_error_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_context_tokens: Option<u64>,
}

pub fn schema() -> Value {
    serde_json::from_str(include_str!("schemas/claude-failure.schema.json"))
        .expect("Claude failure schema")
}

impl ClaudeFailure {
    pub fn present(&mut self) {
        let max_output = (
            "Output token limit reached",
            "Claude reached the output token limit. Request a shorter answer.",
        );
        let cause = match self.api_error.as_deref().unwrap_or_default() {
            "tool_history_mismatch" => Some(("Tool history does not match", "The conversation's tool calls and results do not match. Rewind the conversation or start a new chat.")),
            "autocompact_thrashing" => Some(("Context fills again after compaction", "The context reaches its limit after repeated compaction. Remove large inputs or start a new chat.")),
            "media_removed" => Some(("Image or document removed", "Claude could not use an image or document. Later requests can omit it. Check the attachment before continuing.")),
            "provider_credentials" => Some(("Provider credentials failed", "The cloud provider refused the credentials or access. Check this account's sign-in and model access.")),
            "dlp_request_denied" => Some(("Organization policy refused the request", "The organization's data protection policy refused this request. Review the request before continuing.")),
            "safety_monitor_blocked" => Some(("Safety policy blocked the response", "Claude's safety monitor blocked this response. Review the request before continuing.")),
            "claude_code_version_too_old" => Some(("Claude Code update required", "Update Claude Code before continuing.")),
            "effort_requires_thinking" => Some(("Thinking is required", "The selected effort level requires thinking. Check the model settings.")),
            "advisor_incompatible" => Some(("Advisor model is incompatible", "The advisor cannot use the selected model. Check the model settings.")),
            "pdf_too_large" => Some(("PDF exceeds the limit", "The PDF exceeds the page or size limit. Use a smaller document.")),
            "pdf_password_protected" => Some(("PDF is password protected", "Remove the PDF password before attaching the document.")),
            "no_response" => Some(("No API response", "No response arrived before the deadline. Check the connection or gateway.")),
            "tls_untrusted_ca" => Some(("API certificate is not trusted", "Check the certificate authority for the API connection.")),
            "gateway_content_type" => Some(("Gateway response is incompatible", "The gateway changed the response format. Check the gateway settings.")),
            "gateway_signin_required" => Some(("Gateway sign-in required", "Sign in to the configured gateway before continuing.")),
            "gateway_session_expired" => Some(("Gateway session expired", "Refresh the gateway credentials before continuing.")),
            "api_key_auth_disabled" => Some(("API key sign-in is disabled", "The organization disabled API key authentication. Use an allowed sign-in method.")),
            "org_disabled_credential" => Some(("Credential organization is disabled", "The active API key belongs to a disabled organization. Check the account credentials.")),
            "invalid_credential_header" => Some(("Credential header is invalid", "A credential cannot be sent as an HTTP header. Check the account credentials.")),
            "model_requires_usage_credits" => Some(("Model requires usage credits", "Check this account's usage credits or select another model.")),
            "long_context_credits_required" => Some(("Long context requires usage credits", "Enable usage credits or reduce the context.")),
            "consent_unanswered" => Some(("Usage credit choice is unanswered", "Confirm the usage credit choice before continuing.")),
            "no_allowed_fallback" => Some(("No allowed fallback model", "The model policy allows no fallback. Check the allowed models.")),
            "model_substitution_disabled" => Some(("Model fallback is disabled", "Check the model fallback setting before continuing.")),
            "field_not_granted" => Some(("Request field is not allowed", "The organization cannot use a request field. Check the selected settings.")),
            "usage_limit_reached" => Some(("Claude usage limit reached", "Check this account's limit and reset time.")),
            "max_output_tokens" => Some(max_output),
            _ => None,
        }.or_else(|| match self.error.as_deref().unwrap_or_default() {
            "authentication_failed" => Some(("Claude sign-in failed", "Sign in again to this account.")),
            "cloud_credential_error" => Some(("Provider credentials failed", "Check the cloud provider credentials.")),
            "oauth_org_not_allowed" => Some(("Organization sign-in is not allowed", "Use an account in an allowed organization.")),
            "account_on_hold" => Some(("Claude account is on hold", "Check this account's status.")),
            "verification_required" => Some(("Account verification is required", "Complete this account's verification.")),
            "billing_error" => Some(("Claude billing failed", "Check this account's billing settings.")),
            "rate_limit" => Some(("Rate limit reached", "Check this account's limit and reset time.")),
            "overloaded" => Some(("Claude is at capacity", "Try again later or select another model.")),
            "invalid_request" => Some(("Claude request is invalid", "Check the request and model settings.")),
            "model_not_found" => Some(("Claude model is unavailable", "Check the selected model and account access.")),
            "server_error" => Some(("Claude server failed", "Review the last result before continuing.")),
            "max_output_tokens" => Some(max_output),
            _ => None,
        });
        let (title, description) = cause.unwrap_or((
            "Claude request failed",
            "Claude could not complete the request. Check the cause details before continuing.",
        ));
        self.title = Some(title.into());
        let counts = match (self.prompt_tokens, self.max_context_tokens) {
            (Some(prompt), Some(limit))
                if prompt <= crate::shared_values::MAX_SAFE_INTEGER as u64
                    && limit <= crate::shared_values::MAX_SAFE_INTEGER as u64 =>
            {
                format!(" Context: {prompt} tokens. Limit: {limit} tokens.")
            }
            _ => String::new(),
        };
        self.message = Some(format!("{description}{counts}"));
    }

    /// Drop unknown or invalid fields before retaining the typed record.
    pub fn from_value(value: &Value) -> Option<Self> {
        static FIELDS: OnceLock<BTreeMap<String, crate::Validator>> = OnceLock::new();
        let validators = FIELDS.get_or_init(|| {
            schema()["properties"]
                .as_object()
                .expect("properties")
                .iter()
                .map(|(key, rule)| {
                    (
                        key.clone(),
                        crate::Validator::new(rule).expect("field schema"),
                    )
                })
                .collect()
        });
        let clean: Map<String, Value> = value
            .as_object()?
            .iter()
            .filter(|(key, value)| {
                !matches!(key.as_str(), "title" | "message")
                    && validators
                        .get(*key)
                        .is_some_and(|validator| validator.is_valid(value))
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        if clean.is_empty() {
            return None;
        }
        let mut failure: Self = serde_json::from_value(Value::Object(clean)).ok()?;
        failure.present();
        Some(failure)
    }
}

/// Sanitize the extension independently of the existing error and policy fields.
pub fn sanitize_error(error: &mut Value) {
    let Some(fields) = error.as_object_mut() else {
        return;
    };
    if let Some(raw) = fields.remove("claudeFailure")
        && let Some(record) = ClaudeFailure::from_value(&raw)
    {
        fields.insert(
            "claudeFailure".into(),
            serde_json::to_value(record).expect("failure record"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_code_gets_generic_presentation_and_token_context() {
        let failure = ClaudeFailure::from_value(&json!({"apiErrorCode":"future_gate", "promptTokens":210001, "maxContextTokens":200000})).unwrap();
        assert_eq!(failure.title.as_deref(), Some("Claude request failed"));
        assert_eq!(
            failure.message.as_deref(),
            Some(
                "Claude could not complete the request. Check the cause details before continuing. Context: 210001 tokens. Limit: 200000 tokens."
            )
        );
    }

    #[test]
    fn stored_provider_error_gets_presentation_on_read() {
        let error: crate::provider_errors::AgentError = serde_json::from_str(
            r#"{"message":"failed","claudeFailure":{"apiError":"tool_history_mismatch"}}"#,
        )
        .unwrap();
        let encoded = serde_json::to_value(error).unwrap();
        assert_eq!(
            encoded["claudeFailure"]["title"],
            "Tool history does not match"
        );
        let replay: crate::provider_errors::AgentError =
            serde_json::from_str(&encoded.to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(replay).unwrap()["claudeFailure"]["title"],
            "Tool history does not match"
        );
    }

    #[test]
    fn schema_fields_match_the_typed_record_and_enforce_bounds() {
        let document = schema();
        let valid: Map<String, Value> = document["properties"]
            .as_object()
            .expect("schema properties")
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "title" | "message"))
            .map(|(key, rule)| {
                let sample = if let Some(values) = rule["enum"].as_array() {
                    values[0].clone()
                } else if rule["type"] == "integer" {
                    rule["maximum"].clone()
                } else {
                    json!("server_gate")
                };
                (key.clone(), sample)
            })
            .collect();
        let valid = Value::Object(valid);
        let failure = ClaudeFailure::from_value(&valid).expect("schema fields match Rust fields");
        assert!(failure.title.is_some());
        assert!(failure.message.is_some());
        for field in ["promptTokens", "maxContextTokens", "apiErrorStatus"] {
            for number in [
                json!(-1),
                json!(9007199254740992_u64),
                json!("123"),
                json!(1.5),
            ] {
                assert!(ClaudeFailure::from_value(&json!({field:number})).is_none());
            }
        }
        for code in ["Bearer secret", "sk-secret", &"x".repeat(65)] {
            assert!(ClaudeFailure::from_value(&json!({"apiErrorCode":code})).is_none());
        }
        assert!(
            ClaudeFailure::from_value(
                &json!({"message":"private","error_details":"secret","apiError":"new_unknown"})
            )
            .is_none()
        );
        assert!(ClaudeFailure::from_value(&json!([])).is_none());
    }
}
