//! Canonical opaque JSON evidence and extension tokens.
use crate::OwnedRawJson as RawValue;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawJson(pub RawValue);
impl PartialEq for RawJson {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}
impl schemars::JsonSchema for RawJson {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> Cow<'static, str> {
        "RawJson".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({})
    }
}

/// Unknown object members, retained as their original JSON tokens.
pub type RawExtensions = std::collections::BTreeMap<String, crate::OwnedRawJson>;
