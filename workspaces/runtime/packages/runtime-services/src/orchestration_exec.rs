//! Typed paired-server command inputs. Their serialized bytes define durable
//! fingerprints; retries from before this format may be refused as conflicts.
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{SeqAccess, Visitor},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum CommandInput {
    Shell(String),
    Argv(Vec<String>),
}
impl<'de> Deserialize<'de> for CommandInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CommandVisitor;
        impl<'de> Visitor<'de> for CommandVisitor {
            type Value = CommandInput;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a command string or array of strings")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(CommandInput::Shell(value.to_owned()))
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(CommandInput::Shell(value))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut argv = Vec::new();
                while let Some(value) = seq.next_element()? {
                    argv.push(value);
                }
                Ok(CommandInput::Argv(argv))
            }
        }
        deserializer.deserialize_any(CommandVisitor)
    }
}
fn default_timeout() -> u64 {
    120
}
fn default_output_limit() -> u64 {
    256 * 1024
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecRequest {
    pub actor: String,
    pub cwd: String,
    pub command: CommandInput,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_output_limit")]
    pub output_limit: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadRequest {
    pub actor: String,
    pub handle: String,
    #[serde(default)]
    pub stdout_offset: u64,
    #[serde(default)]
    pub stderr_offset: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputRequest {
    pub actor: String,
    pub handle: String,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub input: Option<String>,
    #[serde(default)]
    pub close_stdin: bool,
}

fn deserialize_present<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CancelRequest {
    pub actor: String,
    pub handle: String,
}

#[derive(Clone, Debug)]
pub enum ControlRequest {
    Input(InputRequest),
    Cancel(CancelRequest),
}

#[derive(Clone, Debug)]
pub enum CommandOperation {
    Start(ExecRequest),
    Read(ReadRequest),
    Control(ControlRequest),
}

/// The stored compact form is an effect-free receipt probe, never a command.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactedExec {
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub actor: studio_runtime_schema::queue_command::Field<String>,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum ExecPayload {
    Start(ExecRequest),
    Compacted(CompactedExec),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecFields {
    #[serde(default)]
    actor: studio_runtime_schema::queue_command::Field<String>,
    #[serde(default)]
    cwd: studio_runtime_schema::queue_command::Field<String>,
    #[serde(default)]
    command: studio_runtime_schema::queue_command::Field<CommandInput>,
    #[serde(default)]
    env: studio_runtime_schema::queue_command::Field<BTreeMap<String, String>>,
    #[serde(default)]
    timeout: studio_runtime_schema::queue_command::Field<u64>,
    #[serde(default)]
    output_limit: studio_runtime_schema::queue_command::Field<u64>,
    #[serde(default, rename = "receiptDigest")]
    receipt_digest: studio_runtime_schema::queue_command::Field<String>,
}
impl<'de> Deserialize<'de> for ExecPayload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        use studio_runtime_schema::queue_command::Field;
        let fields = ExecFields::deserialize(deserializer)?;
        if let Field::Value(digest) = fields.receipt_digest {
            if !fields.cwd.is_missing()
                || !fields.command.is_missing()
                || !fields.env.is_missing()
                || !fields.timeout.is_missing()
                || !fields.output_limit.is_missing()
            {
                return Err(D::Error::custom(
                    "compact receipt cannot contain command inputs",
                ));
            }
            return Ok(Self::Compacted(CompactedExec {
                actor: fields.actor,
                receipt_digest: digest,
            }));
        }
        if !fields.receipt_digest.is_missing() {
            return Err(D::Error::custom("invalid receipt digest"));
        }
        let actor = match fields.actor {
            Field::Value(v) => v,
            Field::Missing | Field::Null => return Err(D::Error::missing_field("actor")),
        };
        let cwd = match fields.cwd {
            Field::Value(v) => v,
            Field::Missing | Field::Null => return Err(D::Error::missing_field("cwd")),
        };
        let command = match fields.command {
            Field::Value(v) => v,
            Field::Missing | Field::Null => return Err(D::Error::missing_field("command")),
        };
        let env = match fields.env {
            Field::Value(v) => v,
            Field::Missing => BTreeMap::new(),
            Field::Null => return Err(D::Error::custom("invalid env")),
        };
        let timeout = match fields.timeout {
            Field::Value(v) => v,
            Field::Missing => default_timeout(),
            Field::Null => return Err(D::Error::custom("invalid timeout")),
        };
        let output_limit = match fields.output_limit {
            Field::Value(v) => v,
            Field::Missing => default_output_limit(),
            Field::Null => return Err(D::Error::custom("invalid output limit")),
        };
        Ok(Self::Start(ExecRequest {
            actor,
            cwd,
            command,
            env,
            timeout,
            output_limit,
        }))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptRequest {
    pub handle: String,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub actor: studio_runtime_schema::queue_command::Field<String>,
    pub digest: String,
}

#[cfg(test)]
mod tests {
    use super::{ExecPayload, InputRequest};
    use crate::orchestration_wire::OrchestrationRequest;

    #[test]
    fn compact_receipt_is_distinct_and_rejects_command_fields() {
        let compact: ExecPayload =
            serde_json::from_str(r#"{"actor":"lead","receiptDigest":"abc"}"#).unwrap();
        assert!(matches!(compact, ExecPayload::Compacted(_)));
        assert!(
            serde_json::from_str::<ExecPayload>(
                r#"{"actor":"lead","receiptDigest":"abc","cwd":"/tmp"}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<ExecPayload>(
                r#"{"actor":"lead","receiptDigest":"abc","unexpected":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn close_stdin_without_input_round_trips_and_null_is_refused() {
        let input: InputRequest =
            serde_json::from_str(r#"{"actor":"a","handle":"h","close_stdin":true}"#).unwrap();
        let bytes = serde_json::to_vec(&input).unwrap();
        let decoded: InputRequest = serde_json::from_slice(&bytes).unwrap();
        assert!(decoded.close_stdin && decoded.input.is_none());
        assert!(
            serde_json::from_str::<InputRequest>(
                r#"{"actor":"a","handle":"h","input":null,"close_stdin":true}"#
            )
            .is_err()
        );
    }

    #[test]
    fn equivalent_command_requests_have_identical_typed_signatures() {
        let left: OrchestrationRequest = serde_json::from_str(r#"{"requestId":"r","operation":[],"action":"exec","payload":{"actor":"a","cwd":"/p","command":["true"],"env":{"B":"2","A":"1"}}}"#).unwrap();
        let right: OrchestrationRequest = serde_json::from_str(r#"{"action":"exec","operation":[],"requestId":"r","payload":{"command":["true"],"cwd":"/p","actor":"a","env":{"A":"1","B":"2"},"timeout":120,"output_limit":262144}}"#).unwrap();
        let left = serde_json::to_vec(&left).unwrap();
        let right = serde_json::to_vec(&right).unwrap();
        assert_eq!(left, right);
        assert_eq!(
            crate::multi_server_auth::digest(&left),
            crate::multi_server_auth::digest(&right)
        );
    }
}

/// Strict view of the server-exec record carried in a completion notice.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletionRecord {
    pub handle: String,
    pub status: String,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub server_id: studio_runtime_schema::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub started_at: studio_runtime_schema::queue_command::Field<f64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub finished_at: studio_runtime_schema::queue_command::Field<f64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub exit_code: studio_runtime_schema::queue_command::Field<i64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub signal: studio_runtime_schema::queue_command::Field<i64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub duration: studio_runtime_schema::queue_command::Field<f64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub timed_out: studio_runtime_schema::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub output_limit: studio_runtime_schema::queue_command::Field<u64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub output_expires_at: studio_runtime_schema::queue_command::Field<f64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub stopped_descendants: studio_runtime_schema::queue_command::Field<u64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub cleanup_unknown_forks: studio_runtime_schema::queue_command::Field<u64>,
    #[serde(
        default,
        skip_serializing_if = "studio_runtime_schema::queue_command::Field::is_missing"
    )]
    pub error: studio_runtime_schema::queue_command::Field<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecEvent {
    pub actor: String,
    pub handle: String,
    #[serde(deserialize_with = "studio_runtime_schema::http_ingress::deserialize_object")]
    pub record: CompletionRecord,
}
