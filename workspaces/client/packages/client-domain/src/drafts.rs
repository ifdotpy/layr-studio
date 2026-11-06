//! Draft branch rules ported from web/src/sync/{conflicts,drafts}.ts.
use crate::MAX_SAFE_INTEGER;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use studio_json::Value;

/// Largest draft text, measured in Unicode code points.
pub const MAX_DRAFT_TEXT_CODEPOINTS: usize = 100_000;
/// Largest stored branch identity, measured in UTF-8 bytes.
pub const MAX_DRAFT_ID_BYTES: usize = 300;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct DraftVersion {
    pub id: String,
    pub session: String,
    pub device: String,
    #[schemars(length(max = MAX_DRAFT_TEXT_CODEPOINTS))]
    pub text: String,
    // Historical payloads are read without rewriting their stored bytes. The
    // generated client type stays strict for newly authored drafts.
    #[serde(default, deserialize_with = "legacy_updated")]
    #[schemars(schema_with = "safe_integer_schema")]
    pub updated: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(schema_with = "seen_schema")]
    pub seen: BTreeMap<String, u64>,
    // Historical extras survive decoding, merging, and pending push retries.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extra: BTreeMap<String, Value>,
}

fn safe_integer_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema = <u64>::json_schema(generator);
    schema.insert("maximum".into(), MAX_SAFE_INTEGER.into());
    schema
}

fn seen_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema = <BTreeMap<String, u64>>::json_schema(generator);
    schema.insert(
        "additionalProperties".into(),
        safe_integer_schema(generator).to_value(),
    );
    schema
}

fn legacy_updated<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Timestamp {
        Integer(u64),
        Fraction(f64),
    }
    match Option::<Timestamp>::deserialize(decoder)? {
        None => Ok(0),
        Some(Timestamp::Integer(value)) if value <= MAX_SAFE_INTEGER as u64 => Ok(value),
        Some(Timestamp::Fraction(value)) if value >= 0.0 && value <= MAX_SAFE_INTEGER as f64 => {
            Ok(value.floor() as u64)
        }
        _ => Err(serde::de::Error::custom("Invalid draft timestamp.")),
    }
}

/// Push validation keeps historical extensions until persisted browser/native
/// pending rows have a migration. Generated new-draft types remain closed.
pub fn schema() -> Schema {
    let mut schema = schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<DraftVersion>();
    schema.insert("additionalProperties".into(), true.into());
    schema
}

impl DraftVersion {
    /// Decode historical stored payloads without changing the original document.
    pub fn from_stored_value(id: &str, value: Value) -> Result<Self, String> {
        let Value::Object(mut fields) = value else {
            return Err("Invalid draft document.".into());
        };
        fields
            .entry("id")
            .or_insert_with(|| Value::String(id.into()));
        let version: Self =
            studio_json::from_value(Value::Object(fields)).map_err(|error| error.to_string())?;
        version.validate()?;
        if version.id != id {
            return Err("Invalid draft document.".into());
        }
        Ok(version)
    }

    /// A chat has one draft: its id is the chat id (`session`). `device` names the
    /// device that wrote it last.
    pub fn validate(&self) -> Result<(), String> {
        if self.device.is_empty()
            || self.session.is_empty()
            || self.id != self.session
            || self.id.len() > MAX_DRAFT_ID_BYTES
            || self.text.chars().count() > MAX_DRAFT_TEXT_CODEPOINTS
            || self.updated > MAX_SAFE_INTEGER as u64
            || self.seen.values().any(|n| *n > MAX_SAFE_INTEGER as u64)
        {
            return Err("Invalid draft identity or version.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct DraftView {
    pub drafts: BTreeMap<String, String>,
    pub conflicts: Vec<DraftVersion>,
}

/// Committed draft view and its native workspace metadata.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftSnapshot {
    #[serde(flatten)]
    pub view: DraftView,
    pub workspace_id: String,
    pub revision: u64,
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, session: &str) -> DraftVersion {
        DraftVersion {
            id: id.into(),
            session: session.into(),
            device: "phone".into(),
            text: "text".into(),
            updated: 1,
            seen: BTreeMap::new(),
            alternatives: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn a_draft_is_named_by_its_chat() {
        assert!(version("chat", "chat").validate().is_ok());
        for id in ["phone:chat", "phone:tab:chat", "other"] {
            assert!(version(id, "chat").validate().is_err(), "{id}");
        }
    }
}
