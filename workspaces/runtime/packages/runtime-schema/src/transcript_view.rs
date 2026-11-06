//! The transcript message as the HTTP API returns it (#630 item 145). These Rust types
//! own the `TranscriptMessageRecord`, `TranscriptItemResponse`, `TranscriptInput`,
//! `TranscriptAsset` and `TextBlobRef` schemas of `openapi.json`; `studio-contracts`
//! writes them from here, and the HTTP crate uses these types instead of copies
//! generated from a hand-written schema.
//!
//! A nullable field keeps the difference between an absent key and `null` (`Field`).
//! Keys that Studio does not name stay in `extra`, so a stored record passes through.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use studio_json::Value;

use crate::delivery::{DeliveryMoment, MessageSendStatus};
use crate::queue_command::Field;

/// Schema-only stand-ins: they name an `openapi.json` component that another owner
/// writes, so the generated schema refers to it by `$ref` and never copies it.
macro_rules! component_ref {
    ($($name:ident = $component:literal),+ $(,)?) => {$(
        struct $name;
        impl JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                $component.into()
            }
            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!(true)
            }
        }
    )+};
}
component_ref!(
    JsonValueRef = "JsonValue",
    DeliveryMomentRef = "DeliveryMoment"
);

/// The components this module owns, by their `openapi.json` names.
pub const OWNED_COMPONENTS: [&str; 5] = [
    "TranscriptMessageRecord",
    "TranscriptItemResponse",
    "TranscriptInput",
    "TranscriptAsset",
    "TextBlobRef",
];

/// The hash and size of a full text kept as a blob.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextBlobRef {
    pub hash: String,
    pub size: i64,
}

/// Path-free attachment metadata of a message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TranscriptAsset {
    pub id: String,
    pub agent: String,
    pub name: String,
    pub mime: String,
    pub image: bool,
    #[schemars(range(min = 0, max = 9_007_199_254_740_991_u64))]
    pub size: u64,
    pub hash: String,
    pub created: f64,
}

/// The severity of a provider notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(inline)]
pub enum NativeNotice {
    Info,
    Warning,
    Error,
}

/// A message time: seconds, or the text form of older records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[schemars(inline)]
pub enum TranscriptAt {
    Seconds(f64),
    Text(String),
}

/// A turn error: the provider object, or the text of older records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[schemars(inline)]
pub enum TurnErrorValue {
    Object(#[schemars(with = "BTreeMap<String, JsonValueRef>")] BTreeMap<String, Value>),
    Text(String),
}

/// A stored provider item of a kind Studio does not know; `raw` is its JSON text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(inline)]
pub struct ProviderUnknown {
    pub kind: String,
    pub raw: String,
}

/// One input of a merged user message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "TranscriptInput")]
pub struct TranscriptInput {
    pub kind: String,
    pub text: String,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<TranscriptAsset>")]
    pub assets: Option<Vec<TranscriptAsset>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub at: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub client_message_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery_status: Field<MessageSendStatus>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub materialized: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub pending: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<DeliveryMomentRef>")]
    pub requested_delivery: Field<DeliveryMoment>,
    #[serde(flatten)]
    #[schemars(with = "BTreeMap<String, JsonValueRef>")]
    pub extra: BTreeMap<String, Value>,
}

/// One transcript row: its runtime role and display text, with optional detail.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "TranscriptMessageRecord")]
pub struct TranscriptMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub account_wide: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub agent: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub assets: Field<Vec<TranscriptAsset>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub at: Field<TranscriptAt>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub client_message_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cyber_access_program: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery_error: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery_status: Field<MessageSendStatus>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<JsonValueRef>")]
    pub details: Field<Value>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub full_text_blob: Field<TextBlobRef>,
    /// The notice confirms a completed idle compaction of a Claude session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub idle_compaction: Option<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub inputs: Field<Vec<TranscriptInput>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub kind: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub materialized: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<JsonValueRef>")]
    pub native_error: Field<Value>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<BTreeMap<String, JsonValueRef>>")]
    pub native_hook: Field<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_hook_quiet: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub native_notice: Field<NativeNotice>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<BTreeMap<String, JsonValueRef>>")]
    pub native_review: Field<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub observed_wait: Field<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub pending: Option<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub phase: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<JsonValueRef>")]
    pub previous_error: Field<Value>,
    /// Set on a stored provider item whose kind Studio does not know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ProviderUnknown")]
    pub provider_unknown: Option<ProviderUnknown>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reasoning_ms: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reasoning_observed_at: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reasoning_since: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<DeliveryMomentRef>")]
    pub requested_delivery: Field<DeliveryMoment>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub source_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub streaming: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub thread_id: Field<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub tool_status: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub truncated: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_error: Field<TurnErrorValue>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_error_resolved: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub turn_status: Field<String>,
    #[serde(flatten)]
    #[schemars(with = "BTreeMap<String, JsonValueRef>")]
    pub extra: BTreeMap<String, Value>,
}

/// The schemas of the components this module owns, keyed by component name.
#[must_use]
pub fn schemas() -> BTreeMap<String, Value> {
    let mut generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.definitions_path = "/components/schemas".into())
        .into_generator();
    generator.subschema_for::<TranscriptMessage>();
    let definitions = generator.take_definitions(true);
    let mut owned: BTreeMap<String, Value> = definitions
        .into_iter()
        .filter(|(name, _)| OWNED_COMPONENTS.contains(&name.as_str()))
        .collect();
    owned.insert(
        "TranscriptItemResponse".into(),
        studio_json::json!({"$ref":"#/components/schemas/TranscriptMessageRecord"}),
    );
    owned
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    #[test]
    fn a_message_round_trips_and_keeps_absent_null_and_unknown_keys() {
        let raw = json!({
            "id":"a:1","role":"assistant","text":"hi","turnId":null,"at":3.5,
            "inputs":[{"kind":"user","text":"q","truncated":false,"requestedDelivery":"next_turn"}],
            "turnError":"old text","future":{"x":1}
        });
        let message: TranscriptMessage = studio_json::from_value(raw.clone()).expect("decodes");
        assert_eq!(message.turn_id, Field::Null);
        assert!(message.agent.is_missing());
        assert_eq!(message.extra["future"], json!({"x":1}));
        assert_eq!(studio_json::to_value(&message).expect("encodes"), raw);
    }

    #[test]
    fn owned_schemas_name_their_components_and_refer_to_the_others() {
        let schemas = schemas();
        assert_eq!(schemas.keys().map(String::as_str).collect::<Vec<_>>(), {
            let mut names = OWNED_COMPONENTS.to_vec();
            names.sort_unstable();
            names
        });
        let record = &schemas["TranscriptMessageRecord"];
        assert_eq!(record["required"], json!(["id", "role", "text"]));
        assert_eq!(
            record["properties"]["deliveryStatus"]["anyOf"][0]["$ref"],
            "#/components/schemas/MessageSendStatus"
        );
        assert_eq!(
            record["additionalProperties"]["$ref"],
            "#/components/schemas/JsonValue"
        );
    }
}
