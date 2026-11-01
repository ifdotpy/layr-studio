//! Cross-server request envelope. Unmigrated operation payloads keep their
//! existing representation until their own producer and consumer are typed.
use crate::orchestration_worker::{
    EventPayload, FoldersPayload, InputPayload, LocationInfoPayload, LocationMatchesPayload,
    MoveSend, StopPayload, WorkerCall, WorkerMessage,
};
use crate::project_locations::LocationPayload;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use studio_runtime_schema::persistence::ServerOperationKind;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrchestrationRequest {
    #[serde(deserialize_with = "request_id")]
    pub request_id: String,
    pub operation: Vec<String>,
    #[serde(flatten)]
    pub command: OrchestrationCommand,
}

type RequestId = studio_runtime_schema::bounded_text::BoundedText<1, 128>;

fn request_id<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    RequestId::deserialize(deserializer).map(|value| value.as_str().to_owned())
}

impl OrchestrationRequest {
    pub(crate) fn valid_request_id(&self) -> bool {
        RequestId::try_from(self.request_id.clone()).is_ok()
    }
}

impl OrchestrationRequest {
    pub(crate) fn remaining_value(&self) -> Option<Value> {
        self.command.remaining_payload().map(|payload| {
            serde_json::json!({
                "requestId": self.request_id,
                "operation": self.operation,
                "action": self.command.kind(),
                "payload": payload,
            })
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum OrchestrationCommand {
    Event(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        EventPayload,
    ),
    Input(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        InputPayload,
    ),
    Message(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        WorkerCall<WorkerMessage>,
    ),
    MoveSend(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        WorkerCall<MoveSend>,
    ),
    LocationInfo(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        LocationInfoPayload,
    ),
    LocationMatches(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        LocationMatchesPayload,
    ),
    Stop(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        StopPayload,
    ),
    ExecEvent(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::orchestration_exec::ExecEvent,
    ),
    Projects(Value),
    #[serde(rename = "projects.changed")]
    ProjectsChanged(Value),
    Folders(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        FoldersPayload,
    ),
    Git(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::server_git::GitReadRequest,
    ),
    Export(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::server_git::ExportRequest,
    ),
    Chunk(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::server_git::ChunkRequest,
    ),
    ExecRead(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::orchestration_exec::ReadRequest,
    ),
    ExecInput(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::orchestration_exec::InputRequest,
    ),
    ExecCancel(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::orchestration_exec::CancelRequest,
    ),
    Release(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::server_git::ReleaseRequest,
    ),
    #[serde(rename = "update.offer")]
    UpdateOffer(Value),
    #[serde(rename = "update.export")]
    UpdateExport(Value),
    #[serde(rename = "update.chunk")]
    UpdateChunk(Value),
    #[serde(rename = "update.release")]
    UpdateRelease(Value),
    #[serde(rename = "backup.put")]
    BackupPut(Value),
    Exec(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::orchestration_exec::ExecPayload,
    ),
    ExecReceipt(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        crate::orchestration_exec::ReceiptRequest,
    ),
    BrowserCallback(Value),
    AddLocation(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        LocationPayload,
    ),
    RemoveLocation(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        LocationPayload,
    ),
    MoveValidate(Value),
    MoveBegin(Value),
    DeviceTrust(Value),
}

/// Serialize only the action payload, retaining its original JSON presence.
pub(crate) struct CommandPayload<'a>(pub &'a OrchestrationCommand);
impl serde::Serialize for CommandPayload<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            OrchestrationCommand::Event(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::Input(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::Message(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::MoveSend(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::LocationInfo(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::LocationMatches(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::Stop(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::ExecEvent(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::Projects(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::ProjectsChanged(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::Folders(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::Git(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::Export(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::Chunk(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::ExecRead(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::ExecInput(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::ExecCancel(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::Release(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::UpdateOffer(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::UpdateExport(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::UpdateChunk(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::UpdateRelease(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::BackupPut(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::Exec(value) => serde::Serialize::serialize(value, serializer),
            OrchestrationCommand::ExecReceipt(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::BrowserCallback(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::AddLocation(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::RemoveLocation(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::MoveValidate(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::MoveBegin(value) => {
                serde::Serialize::serialize(value, serializer)
            }
            OrchestrationCommand::DeviceTrust(value) => {
                serde::Serialize::serialize(value, serializer)
            }
        }
    }
}

impl OrchestrationCommand {
    pub(crate) fn actor(&self) -> Option<&str> {
        if let Self::ExecEvent(value) = self {
            return Some(&value.actor);
        }
        if let Self::Exec(value) = self {
            return match value {
                crate::orchestration_exec::ExecPayload::Start(value) => Some(&value.actor),
                crate::orchestration_exec::ExecPayload::Compacted(value) => {
                    value.actor.value().map(String::as_str)
                }
            };
        }
        if let Self::ExecRead(value) = self {
            return Some(&value.actor);
        }
        if let Self::ExecInput(value) = self {
            return Some(&value.actor);
        }
        if let Self::ExecCancel(value) = self {
            return Some(&value.actor);
        }
        self.remaining_payload()
            .and_then(|value| value.get("actor"))
            .and_then(Value::as_str)
    }
    pub(crate) fn remaining(kind: ServerOperationKind, payload: Value) -> Option<Self> {
        match kind {
            ServerOperationKind::Projects => Some(Self::Projects(payload)),
            ServerOperationKind::ProjectsChanged => Some(Self::ProjectsChanged(payload)),
            ServerOperationKind::UpdateOffer => Some(Self::UpdateOffer(payload)),
            ServerOperationKind::UpdateExport => Some(Self::UpdateExport(payload)),
            ServerOperationKind::UpdateChunk => Some(Self::UpdateChunk(payload)),
            ServerOperationKind::UpdateRelease => Some(Self::UpdateRelease(payload)),
            ServerOperationKind::BackupPut => Some(Self::BackupPut(payload)),
            ServerOperationKind::BrowserCallback => Some(Self::BrowserCallback(payload)),
            ServerOperationKind::MoveValidate => Some(Self::MoveValidate(payload)),
            ServerOperationKind::MoveBegin => Some(Self::MoveBegin(payload)),
            ServerOperationKind::DeviceTrust => Some(Self::DeviceTrust(payload)),
            ServerOperationKind::Event
            | ServerOperationKind::Input
            | ServerOperationKind::Message
            | ServerOperationKind::MoveSend
            | ServerOperationKind::LocationInfo
            | ServerOperationKind::LocationMatches
            | ServerOperationKind::Stop
            | ServerOperationKind::Folders
            | ServerOperationKind::AddLocation
            | ServerOperationKind::RemoveLocation
            | ServerOperationKind::Git
            | ServerOperationKind::Export
            | ServerOperationKind::Chunk
            | ServerOperationKind::Release
            | ServerOperationKind::Exec
            | ServerOperationKind::ExecRead
            | ServerOperationKind::ExecInput
            | ServerOperationKind::ExecCancel
            | ServerOperationKind::ExecReceipt
            | ServerOperationKind::ExecEvent => None,
        }
    }

    pub fn kind(&self) -> ServerOperationKind {
        match self {
            Self::Event(_) => ServerOperationKind::Event,
            Self::Input(_) => ServerOperationKind::Input,
            Self::Message(_) => ServerOperationKind::Message,
            Self::MoveSend(_) => ServerOperationKind::MoveSend,
            Self::LocationInfo(_) => ServerOperationKind::LocationInfo,
            Self::LocationMatches(_) => ServerOperationKind::LocationMatches,
            Self::Stop(_) => ServerOperationKind::Stop,
            Self::ExecEvent(_) => ServerOperationKind::ExecEvent,
            Self::Projects(_) => ServerOperationKind::Projects,
            Self::ProjectsChanged(_) => ServerOperationKind::ProjectsChanged,
            Self::Folders(_) => ServerOperationKind::Folders,
            Self::Git(_) => ServerOperationKind::Git,
            Self::Export(_) => ServerOperationKind::Export,
            Self::Chunk(_) => ServerOperationKind::Chunk,
            Self::ExecRead(_) => ServerOperationKind::ExecRead,
            Self::ExecInput(_) => ServerOperationKind::ExecInput,
            Self::ExecCancel(_) => ServerOperationKind::ExecCancel,
            Self::Release(_) => ServerOperationKind::Release,
            Self::UpdateOffer(_) => ServerOperationKind::UpdateOffer,
            Self::UpdateExport(_) => ServerOperationKind::UpdateExport,
            Self::UpdateChunk(_) => ServerOperationKind::UpdateChunk,
            Self::UpdateRelease(_) => ServerOperationKind::UpdateRelease,
            Self::BackupPut(_) => ServerOperationKind::BackupPut,
            Self::Exec(_) => ServerOperationKind::Exec,
            Self::ExecReceipt(_) => ServerOperationKind::ExecReceipt,
            Self::BrowserCallback(_) => ServerOperationKind::BrowserCallback,
            Self::AddLocation(_) => ServerOperationKind::AddLocation,
            Self::RemoveLocation(_) => ServerOperationKind::RemoveLocation,
            Self::MoveValidate(_) => ServerOperationKind::MoveValidate,
            Self::MoveBegin(_) => ServerOperationKind::MoveBegin,
            Self::DeviceTrust(_) => ServerOperationKind::DeviceTrust,
        }
    }

    /// Only operations not yet migrated in this series retain dynamic payloads.
    pub(crate) fn remaining_payload(&self) -> Option<&Value> {
        match self {
            Self::Projects(value) => Some(value),
            Self::ProjectsChanged(value) => Some(value),
            Self::UpdateOffer(value) => Some(value),
            Self::UpdateExport(value) => Some(value),
            Self::UpdateChunk(value) => Some(value),
            Self::UpdateRelease(value) => Some(value),
            Self::BackupPut(value) => Some(value),
            Self::BrowserCallback(value) => Some(value),
            Self::MoveValidate(value) => Some(value),
            Self::MoveBegin(value) => Some(value),
            Self::DeviceTrust(value) => Some(value),
            Self::Event(_)
            | Self::Input(_)
            | Self::Message(_)
            | Self::MoveSend(_)
            | Self::LocationInfo(_)
            | Self::LocationMatches(_)
            | Self::Stop(_)
            | Self::Folders(_)
            | Self::AddLocation(_)
            | Self::RemoveLocation(_)
            | Self::Git(_)
            | Self::Export(_)
            | Self::Chunk(_)
            | Self::Release(_)
            | Self::Exec(_)
            | Self::ExecRead(_)
            | Self::ExecInput(_)
            | Self::ExecCancel(_)
            | Self::ExecReceipt(_)
            | Self::ExecEvent(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrchestrationRequest;
    use strum::VariantNames;

    #[test]
    fn envelope_requires_known_root_fields_and_action() {
        let body = r#"{"requestId":"r","operation":[],"action":"projects","payload":{}}"#;
        assert!(serde_json::from_str::<OrchestrationRequest>(body).is_ok());
        let extra =
            r#"{"requestId":"r","operation":[],"action":"projects","payload":{},"extra":1}"#;
        assert!(serde_json::from_str::<OrchestrationRequest>(extra).is_err());
        let unknown = r#"{"requestId":"r","operation":[],"action":"future","payload":{}}"#;
        assert!(serde_json::from_str::<OrchestrationRequest>(unknown).is_err());
        let array_payload =
            r#"{"requestId":"r","operation":[],"action":"stop","payload":["l","w",1]}"#;
        assert!(serde_json::from_str::<OrchestrationRequest>(array_payload).is_err());
        let array_args = r#"{"requestId":"r","operation":[],"action":"move_send","payload":{"link":"l","worker":"w","epoch":1,"parentEpoch":1,"args":["a","x"]}}"#;
        assert!(serde_json::from_str::<OrchestrationRequest>(array_args).is_err());
        let empty = r#"{"requestId":"","operation":[],"action":"projects","payload":{}}"#;
        assert!(serde_json::from_str::<OrchestrationRequest>(empty).is_err());
        let long = format!(
            r#"{{"requestId":"{}","operation":[],"action":"projects","payload":{{}}}}"#,
            "x".repeat(129)
        );
        assert!(serde_json::from_str::<OrchestrationRequest>(&long).is_err());
    }

    #[test]
    fn action_tags_match_the_shared_operation_owner() {
        for action in studio_runtime_schema::persistence::ServerOperationKind::VARIANTS {
            let payload = match *action {
                "exec" => r#"{"actor":"a","cwd":"/p","command":["true"]}"#,
                "exec_read" | "exec_input" | "exec_cancel" => r#"{"actor":"a","handle":"h"}"#,
                "exec_event" => {
                    r#"{"actor":"a","handle":"h","record":{"handle":"h","status":"completed"}}"#
                }
                "exec_receipt" => r#"{"actor":"a","handle":"h","digest":"d"}"#,
                "git" => r#"{"cwd":"/p","argv":["status","--porcelain=v1"]}"#,
                "export" => r#"{"cwd":"/p","branch":"main","known":[]}"#,
                "chunk" => r#"{"export":"x","offset":0}"#,
                "release" => r#"{"export":"x"}"#,
                "add_location" | "remove_location" => {
                    r#"{"project":"p","request":{"action":"add_location","path":"/p","project":"p","server":"s"}}"#
                }
                "event" | "input" => r#"{"link":"l","parentEpoch":1,"kind":"user","text":"hello"}"#,
                "location_info" => r#"{"cwd":"/p"}"#,
                "location_matches" => r#"{"origin":"x"}"#,
                "stop" => r#"{"link":"l","worker":"w","controlEpoch":1}"#,
                "message" => {
                    r#"{"link":"l","worker":"w","epoch":1,"parentEpoch":1,"args":{"target":"parent","text":"hi"}}"#
                }
                "move_send" => {
                    r#"{"link":"l","worker":"w","epoch":1,"parentEpoch":1,"args":{"agent_id":"a","text":"hi"}}"#
                }
                _ => "{}",
            };
            let body = format!(
                r#"{{"requestId":"r","operation":[],"action":"{action}","payload":{payload}}}"#
            );
            let request: OrchestrationRequest =
                serde_json::from_str(&body).expect("known operation");
            assert_eq!(request.command.kind().as_str(), *action);
        }
    }
}
