//! Named queue/execution payload views. LazyJson retains admitted malformed history;
//! typed decoding is explicit and never reconstructs the original stored evidence.
use serde::{Deserialize, Serialize};
use studio_json::OwnedRawJson;
use studio_runtime_schema::{persistence::*, queue_command::Field, raw_json::RawJson};

// Known and unknown duplicate members retain the last value, as historical JSON
// object readers did. Only the temporary typed view is normalized; stored bytes stay raw.
macro_rules! payload {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            $(#[serde(default)] pub $field: $ty,)*
            #[serde(flatten)]
            pub extensions: studio_runtime_schema::raw_json::RawExtensions,
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Fields { $(#[serde(default)] $field: $ty,)* }
                let raw = studio_runtime_schema::raw_json::RawExtensions::deserialize(d)?;
                let normalized = studio_json::to_string(&raw).map_err(serde::de::Error::custom)?;
                let mut decoder = studio_json::Deserializer::from_str(&normalized);
                let (fields, extensions): (Fields, _) =
                    studio_runtime_schema::raw_json::deserialize_with_raw_extensions(&mut decoder)
                        .map_err(serde::de::Error::custom)?;
                Ok(Self { $($field: fields.$field,)* extensions })
            }
        }
    };
}

/// Asset metadata is historically unconstrained JSON, including scalar/null entries.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
pub struct EventAssetReference(pub OwnedRawJson);

/// Timing keys are open and their values preserve the producer's numeric spellings.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
pub struct EventTiming(pub std::collections::BTreeMap<String, OwnedRawJson>);

payload!(EventMetadataPayload {
    assets: Option<Vec<EventAssetReference>>,
    delivery: Field<studio_runtime_schema::event_metadata::StoredDelivery>,
    requested_delivery: Field<studio_runtime_schema::event_metadata::StoredDelivery>,
    timing: Field<EventTiming>,
    transcript_item_id: Field<String>,
});
payload!(NativeNoticePayload {
    id: Option<String>, account_key: Option<String>, connection_id: Option<String>,
    message: Option<String>, details: Field<RawJson>, at: Option<studio_json::Number>,
});
payload!(ExecutionRunPayload {
    id: Option<String>, agent: Option<String>, account_key: Option<String>,
    epoch: Option<studio_runtime_schema::AgentCounter>, created: Option<studio_json::Number>,
    status: Option<ExecutionStatus>, thread_id: Field<String>, turn_id: Field<String>,
    first_attempt_id: Field<RawJson>, root_attempt_id: Field<RawJson>, latest_attempt_id: Field<String>,
});
payload!(ExecutionNodePayload {
    id: Option<String>, run_id: Option<String>, agent_id: Option<String>,
    kind: Option<String>, status: Option<ExecutionNodeStatus>,
});
payload!(ExecutionEffectPayload {
    id: Option<String>, run_id: Option<String>, kind: Option<ExecutionEffectKind>,
    reference_id: Option<String>, attempt_id: Field<String>, request_id: Field<String>,
    status: Option<ExecutionEffectStatus>, outcome: Field<RawJson>, result_reference: Field<String>,
});
payload!(ExecutionAttemptPayload {
    id: Option<String>, run_id: Option<String>, epoch: Option<studio_runtime_schema::AgentCounter>,
    events: Option<Vec<String>>, submission: Option<AttemptSubmission>, turn_status: Option<ExecutionStatus>,
    input_identity: Field<RawJson>, claim_result: Field<RawJson>,
    provider_submission: Field<RawJson>, terminal_repair: Field<RawJson>,
});
payload!(ToolRequestPayload {
    id: Option<String>, kind: Option<ToolRequestKind>, agent: Option<String>,
    stage: Option<ToolRequestStatus>, status: Option<ToolRequestStatus>,
    admitted: Field<crate::request_results::AdmittedRequest>, payload: Field<RawJson>,
    origin_key: Field<String>, updated: Option<studio_json::Number>,
});

/// Legacy TEXT vocabularies use their existing SQL codecs, without changing the
/// permanent INTEGER-code contract of newly declared DbEnum owners.
pub struct LegacyTextCodec<T>(std::marker::PhantomData<T>);
impl<T: rusqlite::types::FromSql + rusqlite::types::ToSql> studio_db_api::Codec<T>
    for LegacyTextCodec<T>
{
    type Stored = String;
    type Encoded<'a>
        = &'a T
    where
        T: 'a;
    fn encode<'a>(_: &rusqlite::Connection, value: &'a T) -> rusqlite::Result<&'a T> {
        Ok(value)
    }
    fn decode(_: &rusqlite::Connection, value: String) -> rusqlite::Result<T> {
        T::column_result(rusqlite::types::ValueRef::Text(value.as_bytes())).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    }
}
impl<T: rusqlite::types::FromSql + rusqlite::types::ToSql> studio_db_api::Codec<Option<T>>
    for LegacyTextCodec<T>
{
    type Stored = Option<String>;
    type Encoded<'a>
        = &'a Option<T>
    where
        T: 'a;
    fn encode<'a>(
        _: &rusqlite::Connection,
        value: &'a Option<T>,
    ) -> rusqlite::Result<&'a Option<T>> {
        Ok(value)
    }
    fn decode(
        connection: &rusqlite::Connection,
        value: Option<String>,
    ) -> rusqlite::Result<Option<T>> {
        value
            .map(|text| <Self as studio_db_api::Codec<T>>::decode(connection, text))
            .transpose()
    }
}
