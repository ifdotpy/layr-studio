//! Wire transcript rows: declared renderer fields plus opaque provider extensions.
use crate::queue_command::Field;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use studio_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NumberOrString {
    Number(studio_json::Number),
    Text(String),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectOrString {
    Object(BTreeMap<String, Value>),
    Text(String),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NativeNotice {
    Info,
    Warning,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderUnknown {
    pub kind: String,
    pub raw: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Runtime role and display text are required; omitted and null facts stay distinct.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct TranscriptMessageRecord {
    #[serde(flatten)]
    pub fields: TranscriptMessageFields,
    #[serde(flatten)]
    #[schemars(with = "BTreeMap<String, Value>")]
    pub extra: BTreeMap<String, Value>,
}

impl Serialize for TranscriptMessageRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Preserve the previous Value serializer's sorted order, including extensions.
        let mut value = studio_json::to_value(&self.fields).map_err(serde::ser::Error::custom)?;
        value
            .as_object_mut()
            .expect("transcript fields are an object")
            .extend(self.extra.clone());
        value.serialize(serializer)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptMessageFields {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_wide: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub agent: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "assets_schema")]
    pub assets: Field<Vec<crate::generated::TranscriptAsset>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "at_schema")]
    pub at: Field<NumberOrString>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub client_message_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cyber_access_program: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "delivery_status_schema")]
    pub delivery_status: Field<crate::delivery::MessageSendStatus>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "details_schema")]
    pub details: Field<Value>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "full_text_blob_schema")]
    pub full_text_blob: Field<crate::generated::TextBlobRef>,
    pub id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::non_null::option"
    )]
    #[schemars(schema_with = "idle_compaction_schema")]
    pub idle_compaction: Option<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "inputs_schema")]
    pub inputs: Field<Vec<crate::generated::TranscriptInput>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub kind: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub materialized: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "native_error_schema")]
    pub native_error: Field<Value>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "native_hook_schema")]
    pub native_hook: Field<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_hook_quiet: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "native_notice_schema")]
    pub native_notice: Field<NativeNotice>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "native_review_schema")]
    pub native_review: Field<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub observed_wait: Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::non_null::option"
    )]
    #[schemars(schema_with = "pending_schema")]
    pub pending: Option<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub phase: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "previous_error_schema")]
    pub previous_error: Field<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::non_null::option"
    )]
    #[schemars(schema_with = "provider_unknown_schema")]
    pub provider_unknown: Option<ProviderUnknown>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "reasoning_ms_schema")]
    pub reasoning_ms: Field<studio_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "reasoning_observed_at_schema")]
    pub reasoning_observed_at: Field<studio_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "reasoning_since_schema")]
    pub reasoning_since: Field<studio_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "requested_delivery_schema")]
    pub requested_delivery: Field<crate::delivery::DeliveryMoment>,
    pub role: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub streaming: Field<bool>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub thread_id: Field<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::non_null::option"
    )]
    #[schemars(schema_with = "title_schema")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub tool_status: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub truncated: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "turn_error_schema")]
    pub turn_error: Field<ObjectOrString>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_error_resolved: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_status: Field<String>,
}

fn assets_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"items":{"$ref":"#/components/schemas/TranscriptAsset"},"type":"array"},{"type":"null"}],"default":null,"title":"Assets"})
}
fn at_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"type":"null"},{"type":"number"},{"type":"string"}],"default":null,"title":"At"})
}
fn delivery_status_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"$ref":"#/components/schemas/MessageSendStatus"},{"type":"null"}]})
}
fn details_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"$ref":"#/components/schemas/JsonValue"},{"type":"null"}],"default":null})
}
fn full_text_blob_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"$ref":"#/components/schemas/TextBlobRef"},{"type":"null"}],"default":null,"title":"Fulltextblob"})
}
fn idle_compaction_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"description":"The notice confirms a completed idle compaction of a Claude session.","type":"boolean"})
}
fn inputs_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"items":{"$ref":"#/components/schemas/TranscriptInput"},"type":"array"},{"type":"null"}],"default":null,"title":"Inputs"})
}
fn native_error_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"$ref":"#/components/schemas/JsonValue"},{"type":"null"}],"default":null})
}
fn native_hook_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"additionalProperties":{"$ref":"#/components/schemas/JsonValue"},"type":"object"},{"type":"null"}],"default":null,"title":"Nativehook"})
}
fn native_notice_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"enum":["info","warning","error"],"type":"string"},{"type":"null"}],"default":null,"title":"Nativenotice"})
}
fn native_review_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"additionalProperties":{"$ref":"#/components/schemas/JsonValue"},"type":"object"},{"type":"null"}],"default":null,"title":"Nativereview"})
}
fn pending_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"default":false,"title":"Pending","type":"boolean"})
}
fn previous_error_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"$ref":"#/components/schemas/JsonValue"},{"type":"null"}],"default":null})
}
fn provider_unknown_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"description":"Set on a stored provider item whose kind Studio does not know. raw is the provider item as opaque JSON text.","properties":{"kind":{"type":"string"},"raw":{"type":"string"}},"required":["kind","raw"],"type":"object"})
}
fn reasoning_ms_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"type":"null"},{"type":"number"}],"default":null,"title":"Reasoningms"})
}
fn reasoning_observed_at_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"type":"null"},{"type":"number"}],"default":null,"title":"Reasoningobservedat"})
}
fn reasoning_since_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"type":"null"},{"type":"number"}],"default":null,"title":"Reasoningsince"})
}
fn requested_delivery_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"$ref":"#/components/schemas/DeliveryMoment"},{"type":"null"}],"default":null,"title":"Requesteddelivery"})
}
fn title_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"default":"","title":"Title","type":"string"})
}
fn turn_error_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"additionalProperties":{"$ref":"#/components/schemas/JsonValue"},"type":"object"},{"type":"null"},{"type":"string"}],"default":null,"title":"Turnerror"})
}

pub(crate) fn schema() -> Value {
    let mut value = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<TranscriptMessageRecord>()
        .to_value();
    value.as_object_mut().expect("schema").remove("$schema");
    value["additionalProperties"] = studio_json::json!({"$ref":"#/components/schemas/JsonValue"});
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rust_wire_transcript_schema_owns_the_openapi_component() {
        assert_eq!(
            schema(),
            studio_json::from_str::<Value>(crate::OPENAPI_TEXT).unwrap()["components"]["schemas"]["TranscriptMessageRecord"]
        );
    }
    #[test]
    fn existing_transcript_fixtures_round_trip_without_wire_changes() {
        fn check(value: &Value, count: &mut usize) {
            if value["id"].is_string() && value["role"].is_string() && value["text"].is_string() {
                let row: TranscriptMessageRecord =
                    studio_json::from_value(value.clone()).expect("fixture row");
                assert_eq!(
                    studio_json::to_vec(&row).expect("direct wire serializer"),
                    studio_json::to_vec(value).unwrap()
                );
                *count += 1;
            }
            match value {
                Value::Array(values) => values.iter().for_each(|value| check(value, count)),
                Value::Object(values) => values.values().for_each(|value| check(value, count)),
                _ => {}
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../runtime-services/tests/wave7_p13_transcript");
        let mut count = 0;
        check(
            &studio_json::json!({"aaa":1,"id":"m","middle":null,"role":"assistant","text":"x","zzz":{"value":true}}),
            &mut count,
        );
        for folder in ["default", "full"] {
            let fixture = std::fs::read_to_string(root.join(folder).join("cases.json"))
                .expect("existing fixture");
            check(
                &studio_json::from_str(&fixture).expect("JSON fixture"),
                &mut count,
            );
        }
        assert!(
            count > 100,
            "round trip must cover the existing transcript corpus: {count}"
        );
    }
}
