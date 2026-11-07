//! Server-owned chat indicator, including persisted unread receipt precedence.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatOperationalKind {
    Answer,
    Error,
    Working,
    Unread,
    Paused,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChatOperationalStatus {
    pub kind: ChatOperationalKind,
    pub label: String,
}

pub(crate) fn schemas() -> [(&'static str, studio_json::Value); 2] {
    let mut generator = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .into_generator();
    [
        (
            "ChatOperationalKind",
            ChatOperationalKind::json_schema(&mut generator).to_value(),
        ),
        (
            "ChatOperationalStatus",
            ChatOperationalStatus::json_schema(&mut generator).to_value(),
        ),
    ]
}
