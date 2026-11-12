//! Settings lifetime is explicit; persistent provider configuration is unsupported.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PERSISTENT_UNSUPPORTED_REASON: &str = "Studio never mutates provider config files.";

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingsScope {
    CurrentTurn {
        turn_id: crate::bounded_text::BoundedText<1, 200>,
    },
    #[default]
    Session,
    Persistent,
}

pub fn schema() -> studio_json::Value {
    schemars::generate::SchemaSettings::draft2020_12()
        .for_deserialize()
        .into_generator()
        .into_root_schema_for::<SettingsScope>()
        .to_value()
}
