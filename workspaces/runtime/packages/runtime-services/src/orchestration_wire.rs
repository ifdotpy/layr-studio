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
    MoveStatus(Value),
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
    ExecEvent(Value),
    Projects(Value),
    #[serde(rename = "projects.changed")]
    ProjectsChanged(Value),
    Folders(
        #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
        FoldersPayload,
    ),
    Git(Value),
    Export(Value),
    Chunk(Value),
    ExecRead(Value),
    ExecInput(Value),
    ExecCancel(Value),
    Release(Value),
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
    Task(Value),
    Complaint(Value),
    Context(Value),
    Directory(Value),
    ChatRead(Value),
    MoveManage(Value),
    MoveInterrupt(Value),
    MoveSpawn(Value),
    Exec(Value),
    ExecReceipt(Value),
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

impl OrchestrationCommand {
    pub(crate) fn remaining(kind: ServerOperationKind, payload: Value) -> Option<Self> {
        match kind {
            ServerOperationKind::MoveStatus => Some(Self::MoveStatus(payload)),
            ServerOperationKind::ExecEvent => Some(Self::ExecEvent(payload)),
            ServerOperationKind::Projects => Some(Self::Projects(payload)),
            ServerOperationKind::ProjectsChanged => Some(Self::ProjectsChanged(payload)),
            ServerOperationKind::Git => Some(Self::Git(payload)),
            ServerOperationKind::Export => Some(Self::Export(payload)),
            ServerOperationKind::Chunk => Some(Self::Chunk(payload)),
            ServerOperationKind::ExecRead => Some(Self::ExecRead(payload)),
            ServerOperationKind::ExecInput => Some(Self::ExecInput(payload)),
            ServerOperationKind::ExecCancel => Some(Self::ExecCancel(payload)),
            ServerOperationKind::Release => Some(Self::Release(payload)),
            ServerOperationKind::UpdateOffer => Some(Self::UpdateOffer(payload)),
            ServerOperationKind::UpdateExport => Some(Self::UpdateExport(payload)),
            ServerOperationKind::UpdateChunk => Some(Self::UpdateChunk(payload)),
            ServerOperationKind::UpdateRelease => Some(Self::UpdateRelease(payload)),
            ServerOperationKind::BackupPut => Some(Self::BackupPut(payload)),
            ServerOperationKind::Task => Some(Self::Task(payload)),
            ServerOperationKind::Complaint => Some(Self::Complaint(payload)),
            ServerOperationKind::Context => Some(Self::Context(payload)),
            ServerOperationKind::Directory => Some(Self::Directory(payload)),
            ServerOperationKind::ChatRead => Some(Self::ChatRead(payload)),
            ServerOperationKind::MoveManage => Some(Self::MoveManage(payload)),
            ServerOperationKind::MoveInterrupt => Some(Self::MoveInterrupt(payload)),
            ServerOperationKind::MoveSpawn => Some(Self::MoveSpawn(payload)),
            ServerOperationKind::Exec => Some(Self::Exec(payload)),
            ServerOperationKind::ExecReceipt => Some(Self::ExecReceipt(payload)),
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
            | ServerOperationKind::RemoveLocation => None,
        }
    }

    pub fn kind(&self) -> ServerOperationKind {
        match self {
            Self::Event(_) => ServerOperationKind::Event,
            Self::Input(_) => ServerOperationKind::Input,
            Self::Message(_) => ServerOperationKind::Message,
            Self::MoveSend(_) => ServerOperationKind::MoveSend,
            Self::MoveStatus(_) => ServerOperationKind::MoveStatus,
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
            Self::Task(_) => ServerOperationKind::Task,
            Self::Complaint(_) => ServerOperationKind::Complaint,
            Self::Context(_) => ServerOperationKind::Context,
            Self::Directory(_) => ServerOperationKind::Directory,
            Self::ChatRead(_) => ServerOperationKind::ChatRead,
            Self::MoveManage(_) => ServerOperationKind::MoveManage,
            Self::MoveInterrupt(_) => ServerOperationKind::MoveInterrupt,
            Self::MoveSpawn(_) => ServerOperationKind::MoveSpawn,
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
            Self::MoveStatus(value) => Some(value),
            Self::ExecEvent(value) => Some(value),
            Self::Projects(value) => Some(value),
            Self::ProjectsChanged(value) => Some(value),
            Self::Git(value) => Some(value),
            Self::Export(value) => Some(value),
            Self::Chunk(value) => Some(value),
            Self::ExecRead(value) => Some(value),
            Self::ExecInput(value) => Some(value),
            Self::ExecCancel(value) => Some(value),
            Self::Release(value) => Some(value),
            Self::UpdateOffer(value) => Some(value),
            Self::UpdateExport(value) => Some(value),
            Self::UpdateChunk(value) => Some(value),
            Self::UpdateRelease(value) => Some(value),
            Self::BackupPut(value) => Some(value),
            Self::Task(value) => Some(value),
            Self::Complaint(value) => Some(value),
            Self::Context(value) => Some(value),
            Self::Directory(value) => Some(value),
            Self::ChatRead(value) => Some(value),
            Self::MoveManage(value) => Some(value),
            Self::MoveInterrupt(value) => Some(value),
            Self::MoveSpawn(value) => Some(value),
            Self::Exec(value) => Some(value),
            Self::ExecReceipt(value) => Some(value),
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
            | Self::RemoveLocation(_) => None,
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
