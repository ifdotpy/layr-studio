//! Wire owner of the one command path: `POST /api/commands`.
//!
//! Every write action of a person, an agent, or a peer server is one `CommandRequest`.
//! `Command` is internally tagged by `kind`; the OpenAPI schema `CommandRequest` mirrors it
//! and the web client consumes the generated TypeScript form.
//! Tracking: https://github.com/ifdotpy/layr-studio/issues/630

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::DeliveryMoment;
use crate::bounded_integer::BoundedInteger;
use crate::bounded_text::BoundedText;
use crate::continuation_snapshot::BoundedVec;

/// A command response whose request identity can no longer be replayed safely.
#[derive(Clone, Copy, Debug, Serialize, schemars::JsonSchema, strum::VariantNames)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CommandErrorCode {
    ReceiptExpired,
}

/// Maximum length of a client id. Unit: characters.
pub const COMMAND_CLIENT_ID_MAX_CHARS: usize = 128;
/// Largest response row payload. Larger rows are left to the stream. Unit: bytes.
pub const COMMAND_ROW_MAX_BYTES: usize = 262_144;
/// Maximum attachments of one message. Unit: attachments.
pub const COMMAND_SEND_MAX_ASSETS: usize = 8;
/// Longest command target. Unit: Unicode code points.
pub const COMMAND_TARGET_MAX_CHARS: usize = 200;

/// An HTTP id or a trusted provider tool-call id. Tool ids never deserialize from HTTP.
#[derive(Clone, Debug, PartialEq, Eq)]
enum CommandIdKind {
    Wire(String),
    Tool(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandId(CommandIdKind);

impl CommandId {
    pub fn tool(value: String) -> Self {
        Self(CommandIdKind::Tool(value))
    }
    pub fn as_str(&self) -> &str {
        match &self.0 {
            CommandIdKind::Wire(value) | CommandIdKind::Tool(value) => value,
        }
    }
}
impl Serialize for CommandId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for CommandId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.len() == 36
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c) || c == b'-')
        {
            Ok(Self(CommandIdKind::Wire(value)))
        } else {
            Err(serde::de::Error::custom("invalid command id"))
        }
    }
}
impl JsonSchema for CommandId {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CommandId".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string","pattern":"^[a-f0-9-]{36}$"})
    }
}

/// A command message identity, which must be nonempty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct MessageId(String);
impl MessageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for MessageId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err("empty message id")
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> Deserialize<'de> for MessageId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}
impl JsonSchema for MessageId {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "MessageId".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string","minLength":1})
    }
}

/// One command. `id` is a UUIDv7 made by the caller. `client` names the numbering client
/// (a device install, an agent, a peer link) and `number` is that client's command counter,
/// starting at 1. The server keeps the highest applied number for each client.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandRequest {
    pub id: CommandId,
    pub client: BoundedText<1, COMMAND_CLIENT_ID_MAX_CHARS>,
    pub number: BoundedInteger<1, { crate::shared_values::MAX_SAFE_INTEGER as u64 }>,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Create a user message. The command id is the message id.
    Send {
        target: BoundedText<1, COMMAND_TARGET_MAX_CHARS>,
        body: SendBody,
    },
    /// Delete a queued message and return its text. `target` is the chat; `body.message`
    /// is the message id.
    Withdraw {
        target: BoundedText<1, COMMAND_TARGET_MAX_CHARS>,
        body: WithdrawBody,
    },
    /// Stop the turn of a chat. `target` is the chat.
    Stop {
        target: BoundedText<1, COMMAND_TARGET_MAX_CHARS>,
        body: StopBody,
    },
}

impl Command {
    /// The wire name of the command, also the operation part of its request identity.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Send { .. } => "send",
            Self::Withdraw { .. } => "withdraw",
            Self::Stop { .. } => "stop",
        }
    }

    /// The chat that the command acts on.
    pub fn target(&self) -> &str {
        match self {
            Self::Send { target, .. }
            | Self::Withdraw { target, .. }
            | Self::Stop { target, .. } => target.as_str(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SendBody {
    pub text: BoundedText<0, { crate::shared_values::MAX_MANAGED_MESSAGE_CODEPOINTS }>,
    #[serde(default)]
    pub assets: BoundedVec<String, COMMAND_SEND_MAX_ASSETS>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::delivery::compatibility::option"
    )]
    #[schemars(with = "Option<String>")]
    pub delivery: Option<DeliveryMoment>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WithdrawBody {
    pub message: MessageId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StopBody {
    #[serde(default = "default_true")]
    pub descendants: bool,
}

fn default_true() -> bool {
    true
}

/// A rejected command is an HTTP error. Only an applied command has a response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Applied,
}

/// The one response form, on the first run and on a replay. `ref_*` and `rev` are exact.
/// `rows` are read when the response is built and can be newer than after the first run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommandResponse {
    pub status: CommandStatus,
    pub ref_kind: String,
    pub ref_id: String,
    pub rev: i64,
    /// Highest `seq` among the rows of the entities that the command changed. A client that
    /// holds a row with `seq` at or above this value needs no pending layer.
    pub seq: i64,
    pub rows: Vec<CommandRow>,
}

/// One entity row with its stream `seq`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommandRow {
    pub collection: String,
    pub id: String,
    pub seq: i64,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_flat_and_tagged_by_kind() {
        let wire = serde_json::json!({
            "id": "0192f3a0-0000-7000-8000-000000000001",
            "client": "web-1",
            "number": 3,
            "kind": "withdraw",
            "target": "chat",
            "body": {"message": "m"}
        });
        let request: CommandRequest = serde_json::from_value(wire.clone()).expect("parse");
        assert_eq!(request.command.name(), "withdraw");
        assert_eq!(request.command.target(), "chat");
        assert_eq!(serde_json::to_value(&request).expect("encode"), wire);
    }

    #[test]
    fn unknown_kind_and_unknown_body_field_are_refused() {
        let base = serde_json::json!({"id":"0192f3a0-0000-7000-8000-000000000001","client":"c","number":1,"target":"t"});
        let mut unknown_kind = base.clone();
        unknown_kind["kind"] = "delete_everything".into();
        unknown_kind["body"] = serde_json::json!({});
        assert!(serde_json::from_value::<CommandRequest>(unknown_kind).is_err());
        let mut extra = base;
        extra["kind"] = "stop".into();
        extra["body"] = serde_json::json!({"extra": true});
        assert!(serde_json::from_value::<CommandRequest>(extra).is_err());
        let extra = serde_json::json!({"id":"0192f3a0-0000-7000-8000-000000000001","client":"c","number":1,"kind":"stop","target":"t","body":{},"extra":true});
        assert!(serde_json::from_value::<CommandRequest>(extra).is_err());
    }

    #[test]
    fn stop_descendants_default_to_true() {
        let request: CommandRequest = serde_json::from_str(
            r#"{"id":"0192f3a0-0000-7000-8000-000000000001","client":"c","number":1,"kind":"stop","target":"t","body":{}}"#,
        )
        .expect("parse");
        assert_eq!(
            request.command,
            Command::Stop {
                target: "t".to_owned().try_into().expect("target"),
                body: StopBody { descendants: true }
            }
        );
    }
}
