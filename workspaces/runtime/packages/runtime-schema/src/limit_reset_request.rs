//! Typed reset-credit ingress. A request id always identifies the retry.

use crate::bounded_text::BoundedText;
use serde::{Deserialize, Deserializer, de::Error};
use uuid::Uuid;

/// Maximum reset account identity. Unit: Unicode code points.
pub const RESET_ACCOUNT_ID_MAX_CHARS: usize = 1024;
/// Maximum reset credit identity. Unit: Unicode code points.
pub const RESET_CREDIT_ID_MAX_CHARS: usize = 1024;
/// Length of a hyphenated reset request UUID. Unit: bytes.
pub const RESET_REQUEST_ID_WIRE_BYTES: usize = 36;
/// Account key used when a reset request omits one.
pub const DEFAULT_RESET_ACCOUNT_KEY: &str = "default";

#[derive(Clone, Debug)]
pub struct ResetRequestId(Uuid);

impl From<Uuid> for ResetRequestId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl ResetRequestId {
    pub fn canonical(&self) -> String {
        self.0.to_string()
    }
}

impl<'de> Deserialize<'de> for ResetRequestId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != RESET_REQUEST_ID_WIRE_BYTES
            || [8, 13, 18, 23]
                .iter()
                .any(|index| text.as_bytes()[*index] != b'-')
        {
            return Err(D::Error::custom("invalid reset request id"));
        }
        Uuid::parse_str(&text).map(Self).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetRequest {
    pub account_id: BoundedText<1, RESET_ACCOUNT_ID_MAX_CHARS>,
    #[serde(default, deserialize_with = "present_account_key")]
    pub account_key: Option<String>,
    pub credit_id: BoundedText<1, RESET_CREDIT_ID_MAX_CHARS>,
    pub request_id: ResetRequestId,
}

fn present_account_key<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::ResetRequest;
    use serde_json::json;

    #[test]
    fn reset_request_requires_bounded_identities_and_a_uuid() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).expect("OpenAPI");
        for (field, max) in [
            ("account_id", super::RESET_ACCOUNT_ID_MAX_CHARS),
            ("credit_id", super::RESET_CREDIT_ID_MAX_CHARS),
        ] {
            assert_eq!(
                schema.pointer(&format!(
                    "/components/schemas/ResetRequest/properties/{field}/maxLength"
                )),
                Some(&json!(max))
            );
        }
        for field in ["minLength", "maxLength"] {
            assert_eq!(
                schema.pointer(&format!(
                    "/components/schemas/ResetRequest/properties/request_id/{field}"
                )),
                Some(&json!(super::RESET_REQUEST_ID_WIRE_BYTES))
            );
        }
        assert_eq!(
            schema.pointer("/components/schemas/ResetRequest/properties/account_key/default"),
            Some(&json!(super::DEFAULT_RESET_ACCOUNT_KEY))
        );
        let valid = json!({"account_id":"a","credit_id":"c","request_id":"0192f3a0-0000-7000-8000-000000000001"});
        let parsed: ResetRequest = serde_json::from_value(valid.clone()).expect("valid request");
        assert!(parsed.account_key.is_none());
        for (field, value) in [
            ("account_id", json!("")),
            ("credit_id", json!("x".repeat(1025))),
            ("request_id", json!("0192f3a0000070008000000000000001")),
            ("extra", json!(true)),
        ] {
            let mut invalid = valid.clone();
            invalid[field] = value;
            assert!(serde_json::from_value::<ResetRequest>(invalid).is_err());
        }
    }
}
