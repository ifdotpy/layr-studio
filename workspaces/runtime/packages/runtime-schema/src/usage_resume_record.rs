//! The permissive stored usage-resume envelope. Decision and cancellation
//! callers apply their own stricter contracts to these historical records.
use crate::queue_command::Field;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use studio_json::{Number, Value};

#[derive(Clone, Debug, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "PersistedUsageResumeRecord")]
pub struct UsageResumeRecord {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub cause: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub status: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Number>")]
    pub failed_at: Field<Number>,
    // Persistence historically accepts any account evidence; recovery only
    // selects its string form after SQLite has applied the recovery predicate.
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Value")]
    pub account_key: Field<AccountIdentityEvidence>,
    #[serde(flatten)]
    #[schemars(with = "BTreeMap<String, Value>")]
    pub extra: BTreeMap<String, crate::raw_json::RawJson>,
}

/// Projection consumed by the shared raw-extension decoder.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageResumeFields {
    #[serde(default)]
    cause: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    status: Field<String>,
    #[serde(default)]
    failed_at: Field<Number>,
    #[serde(default)]
    account_key: Field<AccountIdentityEvidence>,
}
impl<'de> Deserialize<'de> for UsageResumeRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (fields, extra): (UsageResumeFields, _) =
            crate::raw_json::deserialize_with_raw_extensions(deserializer)?;
        Ok(Self {
            cause: fields.cause,
            status: fields.status,
            failed_at: fields.failed_at,
            account_key: fields.account_key,
            extra: extra
                .into_iter()
                .map(|(key, raw)| (key, crate::raw_json::RawJson(raw)))
                .collect(),
        })
    }
}

/// Arbitrary historical account evidence; only its string form can select an account.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountIdentityEvidence(pub crate::raw_json::RawJson);
impl AccountIdentityEvidence {
    pub fn account_key(&self) -> Option<String> {
        studio_json::from_str(self.0.0.as_str()).ok()
    }
}

pub(crate) fn schema() -> Value {
    schemars::schema_for!(UsageResumeRecord).to_value()
}

pub(crate) fn check_schema_parity() -> Result<(), String> {
    let stored: Value = studio_json::from_slice(include_bytes!(
        "schemas/persisted-usage-resume-record.schema.json"
    ))
    .map_err(|error| error.to_string())?;
    if stored != schema() {
        return Err("persisted usage resume schema must be generated from usage_resume_record::UsageResumeRecord".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    #[test]
    fn raw_usage_resume_keeps_last_members_and_integral_decimals() {
        let raw = r#"{"cause":"future","status":null,"failedAt":1.0,"accountKey":"acct","unknown":false,"unknown":{"n":2.00}}"#;
        let record: UsageResumeRecord = studio_json::from_str(raw).unwrap();
        assert_eq!(record.cause.value().map(String::as_str), Some("future"));
        assert!(matches!(record.status, Field::Null));
        assert_eq!(record.failed_at.value().unwrap().as_f64(), Some(1.0));
        assert_eq!(
            record.account_key.value().unwrap().account_key().as_deref(),
            Some("acct")
        );
        assert_eq!(record.extra["unknown"].0.as_str(), r#"{"n":2.00}"#);
    }

    #[test]
    fn persisted_schema_keeps_permissive_evidence_and_presence() {
        check_schema_parity().unwrap();
        for (value, valid) in [
            (json!({}), true),
            (
                json!({"status":null,"cause":"future","failedAt":1.0,"accountKey":[],"epoch":null,"taskClaims":{"future":true}}),
                true,
            ),
            (json!({"status":false}), false),
            (json!({"cause":1}), false),
            (json!({"failedAt":"1"}), false),
            (json!([]), false),
        ] {
            let parsed = UsageResumeRecord::deserialize(value.clone());
            assert_eq!(parsed.is_ok(), valid, "{value}");
            assert_eq!(
                crate::validate("PersistedUsageResumeRecord", &value).unwrap(),
                valid,
                "{value}"
            );
            if let Ok(record) = parsed {
                assert_eq!(json!(record), value);
            }
        }
    }
}
