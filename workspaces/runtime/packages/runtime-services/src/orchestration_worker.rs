//! Typed worker and event payloads for paired-server delivery.
use serde::{Deserialize, Serialize};
use studio_runtime_schema::queue_command::Field;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventPayload {
    pub link: String,
    pub parent_epoch: i64,
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputPayload {
    pub link: String,
    pub parent_epoch: i64,
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub anchor: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub worker: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub epoch: Field<i64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub control_epoch: Field<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StopPayload {
    pub link: String,
    pub worker: String,
    pub control_epoch: i64,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reason: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FoldersPayload {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cwd: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocationInfoPayload {
    pub cwd: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocationMatchesPayload {
    pub origin: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cwd: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    bound(deserialize = "T: Deserialize<'de>")
)]
pub struct WorkerCall<T> {
    pub link: String,
    pub worker: String,
    pub epoch: i64,
    pub parent_epoch: i64,
    #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
    pub args: T,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerMessage {
    pub target: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub importance: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub progress_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub progress_version: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery: Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MoveSend {
    pub agent_id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub delivery: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub request_id: Field<String>,
}

#[derive(Clone, Debug)]
pub enum WorkerAction {
    Message(WorkerCall<WorkerMessage>),
    MoveSend(WorkerCall<MoveSend>),
}

impl WorkerAction {
    pub fn identity(&self) -> (&str, &str, i64, i64) {
        match self {
            Self::Message(call) => (&call.link, &call.worker, call.epoch, call.parent_epoch),
            Self::MoveSend(call) => (&call.link, &call.worker, call.epoch, call.parent_epoch),
        }
    }
}
