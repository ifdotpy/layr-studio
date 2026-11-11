use serde::{Deserialize, Serialize, Serializer, ser::SerializeMap};
use std::borrow::Cow;
use studio_json::{Map, Value};
use thiserror::Error;

/// Largest accepted UTF-8 JSON payload, excluding its LF or CRLF delimiter.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
/// Maximum wire bytes occupied by a frame's LF or CRLF delimiter.
pub const MAX_FRAME_DELIMITER_BYTES: usize = 2;
/// Reserved bytes for the transport stamp member, including its key and comma.
pub(crate) const MAX_BOUNDARY_METADATA_BYTES: usize = 256;

/// Count payload bytes in a complete wire frame, excluding LF or CRLF when present.
#[must_use]
pub fn frame_payload_bytes(frame: &[u8]) -> usize {
    frame
        .strip_suffix(b"\n")
        .map_or(frame, |without_lf| {
            without_lf.strip_suffix(b"\r").unwrap_or(without_lf)
        })
        .len()
}

/// Count payload bytes in a partial transport line, deferring a trailing CR until the next read.
#[must_use]
pub fn frame_prefix_payload_bytes(prefix: &[u8]) -> usize {
    prefix.strip_suffix(b"\r").unwrap_or(prefix).len()
}

pub use studio_runtime_schema::provider_input::RequestId;

/// Define the single closed set of method names understood by this crate.
/// Unknown methods are kept losslessly by [`Frame::UnknownMethod`].
macro_rules! define_methods {
    ($( $variant:ident => $wire_name:literal, )+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Method {
            $( $variant, )+
        }

        impl Method {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire_name, )+
                }
            }

            fn from_str(value: &str) -> Option<Self> {
                match value {
                    $( $wire_name => Some(Self::$variant), )+
                    _ => None,
                }
            }

            /// Convert a method name from a wire frame when it belongs to the known set.
            #[must_use]
            pub fn from_wire(value: &str) -> Option<Self> {
                Self::from_str(value)
            }

            /// Return every method name the runtime currently understands.
            #[must_use]
            pub const fn all() -> &'static [Self] {
                &[$(Self::$variant),+]
            }
        }
    };
}

impl Serialize for Method {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Method {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::from_wire(&value).ok_or_else(|| serde::de::Error::custom("unknown provider method"))
    }
}

macro_rules! runtime_methods {
    ($( $variant:ident => $wire_name:literal, )+) => {
        define_methods! {
            $( $variant => $wire_name, )+
    Ack => "ack",
    ThreadTurnsItemsList => "thread/turns/items/list",
    ThreadRollback => "thread/rollback",
    ThreadCommandExecutionBackground => "thread/commandExecution/background",
    ClaudeState => "claude/state",
    ClaudeRepairReport => "claude/repairReport",
    ClaudeFileBackups => "claude/fileBackups",
    ClaudeSettings => "claude/settings",
    ClaudeActivity => "claude/activity",
    ClaudeStopTask => "claude/stopTask",
    ClaudeShellPolicy => "claude/shellPolicy",
    ClaudeDiagnostics => "claude/diagnostics",
    ClaudeMoveVersions => "claude/moveVersions",
    McpServerReconnect => "mcpServer/reconnect",
    OperationStatus => "operationStatus",
    BrowserAnswer => "browser.answer",
    BrowserCallback => "browser.callback",
    BrowserClose => "browser.close",
    BrowserWait => "browser.wait",
    ClaudeCommands => "claude/commands",
    ClaudeMoveExport => "claude/moveExport",
    ClaudeMoveImport => "claude/moveImport",
    ClaudeMoveIdentity => "claude/moveIdentity",
    ClaudeMovePreflight => "claude/movePreflight",
    ClaudeMoveProof => "claude/moveProof",
    Detach => "detach",
    EventRead => "eventRead",
    Exec => "exec",
    Generate => "generate",
    Health => "health",
    Info => "info",
    LayrHealth => "layr.health",
    Next => "next",
    Open => "open",
    ProjectEnsure => "project.ensure",
    ProjectExec => "project.exec",
    ProjectImport => "project.import",
    ProviderList => "provider.list",
    Replay => "replay",
    ResponseStatus => "responseStatus",
    RetireCommand => "retireCommand",
    SecretsList => "secrets.list",
    SecretsRemove => "secrets.remove",
    SecretsSet => "secrets.set",
    ShareConfigure => "share.configure",
    ShareName => "share.name",
    ShareStatus => "share.status",
    Sign => "sign",
    Status => "status",
    SyncMacApply => "sync.mac.apply",
    SyncMacHandoff => "sync.mac.handoff",
    SyncMacHashes => "sync.mac.hashes",
    SyncMacHead => "sync.mac.head",
    SyncMacRead => "sync.mac.read",
    SyncMacWorktree => "sync.mac.worktree",
    VaultConfigure => "vault.configure",
    VaultPush => "vault.push",
    VaultStatus => "vault.status",
    UploadBegin => "upload.begin",
    UploadChunk => "upload.chunk",
    UploadCommit => "upload.commit",
    Verify => "verify",
    Write => "write",
    ThreadIdleCompactionUpdated => "thread/idleCompaction/updated",
    ThreadShellPolicyUpdated => "thread/shellPolicy/updated",
    ProviderGenerationStarted => "provider/generationStarted",
        }
    };
}
studio_codex_protocol::codex_methods!(runtime_methods);

/// Provider response payload. Error bodies are kept as JSON because provider
/// error fields vary by CLI release.
#[derive(Clone, Debug, PartialEq)]
pub enum ResponseBody {
    /// A Codex result awaiting the request book's method correlation.
    Uncorrelated(studio_json::OwnedRawJson),
    CodexResult(studio_codex_protocol::ClientRequestResult),
    CodexServerResult(studio_codex_protocol::ServerRequestResult),
    CodexError(studio_codex_protocol::JSONRPCErrorError),
    CodexUnparsedResult {
        method: studio_codex_protocol::ClientRequestMethod,
        result: studio_json::OwnedRawJson,
    },
    Result(Value),
    Error(Value),
}

/// A recognized request frame. `fields` retains extension fields other than
/// `id`, `method`, and `params`.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestFrame {
    pub id: RequestId,
    pub method: Method,
    pub params: Option<Value>,
    pub fields: Map<String, Value>,
}

/// A recognized notification frame without a request ID.
#[derive(Clone, Debug, PartialEq)]
pub struct NotificationFrame {
    pub method: Method,
    pub params: Option<Value>,
    pub fields: Map<String, Value>,
}

/// An unrecognized method, retaining the complete JSON object unchanged in
/// meaning so callers can forward newer provider messages.
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownMethodFrame {
    pub method: String,
    pub fields: Map<String, Value>,
}

/// One decoded JSON-RPC line.
#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    /// A known method with an incompatible payload; never typed authority.
    KnownMethodUnparsed(UnknownMethodFrame),
    CodexClientRequest(Box<studio_codex_protocol::ClientRequest>),
    CodexServerRequest(Box<studio_codex_protocol::ServerRequest>),
    CodexNotification(Box<studio_codex_protocol::ServerNotification>),
    CodexClientNotification(Box<studio_codex_protocol::ClientNotification>),
    Request(RequestFrame),
    Notification(NotificationFrame),
    UnknownMethod(UnknownMethodFrame),
    Response {
        id: RequestId,
        body: ResponseBody,
        fields: Map<String, Value>,
    },
}

/// The decoded contents of one supervisor stdout journal entry.
#[derive(Debug, PartialEq)]
pub enum DecodedJournalPayload {
    Frame(Frame),
    /// The payload was not a protocol frame. Its bytes and decoding error stay
    /// attached to the journal cursor so a caller can settle or expose it.
    RawStdout {
        bytes: Vec<u8>,
        error: CodecError,
    },
}

/// A decoded stdout record, retaining the journal identity on both paths.
#[derive(Debug, PartialEq)]
pub struct DecodedJournalEntry {
    pub cursor: u64,
    pub generation: u64,
    pub payload: DecodedJournalPayload,
    pub boundary_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
}

#[derive(Debug, Error, PartialEq)]
pub enum CodecError {
    #[error("invalid JSON frame: {0}")]
    MalformedJson(String),
    #[error("invalid protocol frame: {0}")]
    MalformedFrame(&'static str),
    #[error("protocol frame exceeds {limit} bytes")]
    FrameTooLarge { limit: usize },
    #[error("cannot serialize protocol frame: {0}")]
    Serialization(String),
}

/// One provider wire object. Serde splits the envelope from the extension
/// members while it parses, so a frame is built from a single JSON tree with
/// no member clones or removals.
#[derive(Deserialize)]
struct WireObject<'a> {
    #[serde(borrow)]
    method: Option<Cow<'a, str>>,
    id: Option<RequestId>,
    #[serde(default, deserialize_with = "present")]
    params: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    result: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    error: Option<Value>,
    /// Transport metadata is consumed before constructing a provider Frame.
    #[serde(rename = "_studioBoundaryAt")]
    boundary_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

/// Keep an explicit JSON `null` member as `Some(Value::Null)`; only an absent
/// member is `None`, so a frame re-encodes to the object it was decoded from.
fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

impl WireObject<'_> {
    fn into_frame(self) -> Result<Frame, CodecError> {
        let Self {
            method,
            id,
            mut params,
            mut result,
            error,
            boundary_at: _,
            mut fields,
        } = self;
        let mut keep = |key: &str, value: Option<Value>| {
            if let Some(value) = value {
                fields.insert(key.to_owned(), value);
            }
        };
        let Some(method) = method else {
            if let Some(result) = result.as_mut() {
                sanitize_failure_payload(result);
            }
            let id = id.ok_or(CodecError::MalformedFrame(
                "expected a method or response id",
            ))?;
            keep("params", params);
            let body = match (error, result) {
                (Some(error), None) => ResponseBody::Error(error),
                (None, Some(result)) => ResponseBody::Result(result),
                _ => {
                    return Err(CodecError::MalformedFrame(
                        "response requires exactly one of result or error",
                    ));
                }
            };
            return Ok(Frame::Response { id, body, fields });
        };
        keep("result", result);
        keep("error", error);
        let Some(known) = Method::from_str(&method) else {
            keep("params", params);
            keep("id", id.map(Value::from));
            keep("method", Some(Value::String(method.clone().into_owned())));
            return Ok(Frame::UnknownMethod(UnknownMethodFrame {
                method: method.into_owned(),
                fields,
            }));
        };
        if id.is_none()
            && matches!(known, Method::TurnCompleted | Method::ProtocolError)
            && let Some(params) = params.as_mut()
        {
            sanitize_failure_payload(params);
        }
        Ok(match id {
            Some(id) => Frame::Request(RequestFrame {
                id,
                method: known,
                params,
                fields,
            }),
            None => Frame::Notification(NotificationFrame {
                method: known,
                params,
                fields,
            }),
        })
    }
}

/// Decode a supervisor event payload that arrived already parsed with the
/// same wire schema as stdout lines, without re-encoding it. Only a malformed
/// payload is encoded, to keep its bytes for the journal.
pub fn decode_journal_value(cursor: u64, generation: u64, payload: &Value) -> DecodedJournalEntry {
    let decoded = json_payload_bytes(payload)
        .and_then(|bytes| check_journal_size(payload, bytes))
        .and_then(|()| journal_value_frame(payload));
    let (payload, boundary_at) = match decoded {
        Ok((frame, stamp)) => (DecodedJournalPayload::Frame(frame), stamp),
        Err(error) => raw_journal_payload(studio_json::to_vec(payload).unwrap_or_default(), error),
    };
    DecodedJournalEntry {
        cursor,
        generation,
        payload,
        boundary_at,
    }
}

fn journal_value_frame(
    payload: &Value,
) -> Result<
    (
        Frame,
        Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
    ),
    CodecError,
> {
    let wire = WireObject::deserialize(payload)
        .map_err(|error| CodecError::MalformedJson(error.to_string()))?;
    let stamp = wire.boundary_at;
    wire.into_frame().map(|frame| (frame, stamp))
}

/// Count encoded bytes without allocating a second payload buffer.
fn json_payload_bytes(value: &Value) -> Result<usize, CodecError> {
    #[derive(Default)]
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count::default();
    studio_json::to_writer(&mut count, value).map_err(serialize_error)?;
    Ok(count.0)
}

fn check_journal_size(payload: &Value, wire_bytes: usize) -> Result<(), CodecError> {
    let name = studio_runtime_schema::provider_timing::BOUNDARY_STAMP_FIELD;
    let metadata_bytes = payload
        .as_object()
        .and_then(|object| object.get(name).map(|value| (object.len(), value)))
        .map(|(members, value)| {
            json_payload_bytes(value).map(|bytes| bytes + name.len() + 3 + usize::from(members > 1))
        })
        .transpose()?
        .unwrap_or_default();
    check_journal_byte_sizes(wire_bytes, metadata_bytes)
}

pub(crate) fn check_journal_byte_sizes(
    wire_bytes: usize,
    metadata_bytes: usize,
) -> Result<(), CodecError> {
    if metadata_bytes > MAX_BOUNDARY_METADATA_BYTES
        || wire_bytes > MAX_FRAME_BYTES + MAX_BOUNDARY_METADATA_BYTES
        || wire_bytes.saturating_sub(metadata_bytes) > MAX_FRAME_BYTES
    {
        return Err(CodecError::FrameTooLarge {
            limit: MAX_FRAME_BYTES,
        });
    }
    Ok(())
}

/// Decode a complete supervisor stdout journal record without losing its
/// cursor when its payload is not valid protocol JSON.
pub fn decode_journal_entry(cursor: u64, generation: u64, bytes: Vec<u8>) -> DecodedJournalEntry {
    let decoded = decode_frame_with_metadata(&bytes);
    let (payload, boundary_at) = match decoded {
        Ok((frame, stamp)) => (DecodedJournalPayload::Frame(frame), stamp),
        Err(error) => raw_journal_payload(bytes, error),
    };
    DecodedJournalEntry {
        cursor,
        generation,
        payload,
        boundary_at,
    }
}

/// Decode a complete stdout record by borrowing its buffer. Malformed bytes are
/// copied into the returned journal entry; valid frame bytes are not retained.
pub fn decode_journal_entry_borrowed(
    cursor: u64,
    generation: u64,
    bytes: &[u8],
) -> DecodedJournalEntry {
    let (payload, boundary_at) = match decode_frame_with_metadata(bytes) {
        Ok((frame, stamp)) => (DecodedJournalPayload::Frame(frame), stamp),
        Err(error) => raw_journal_payload(bytes.to_vec(), error),
    };
    DecodedJournalEntry {
        cursor,
        generation,
        payload,
        boundary_at,
    }
}

/// A frame and its transport boundary stamp, decoded together.
pub type DecodedFrame = (
    Frame,
    Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
);

/// Provider-selected ingress, retaining transport metadata and malformed bytes.
pub fn decode_journal_with(
    cursor: u64,
    generation: u64,
    bytes: &[u8],
    decode: fn(&[u8]) -> Result<DecodedFrame, CodecError>,
) -> DecodedJournalEntry {
    let result = decode(bytes);
    let (payload, boundary_at) = match result {
        Ok((frame, stamp)) => (DecodedJournalPayload::Frame(frame), stamp),
        Err(error) => raw_journal_payload(bytes.to_vec(), error),
    };
    DecodedJournalEntry {
        cursor,
        generation,
        payload,
        boundary_at,
    }
}

/// Generic helper/Claude decoder. Codex callers select `decode_codex_frame`.
pub fn decode_frame(bytes: &[u8]) -> Result<Frame, CodecError> {
    decode_frame_with_metadata(bytes).map(|(frame, _)| frame)
}

/// A supervisor can timestamp a JSON object that is not a valid provider frame.
/// Strip only the new transport field on that fallback too. Legacy raw bytes
/// without the field remain byte-for-byte unchanged.
fn raw_journal_payload(
    mut bytes: Vec<u8>,
    error: CodecError,
) -> (
    DecodedJournalPayload,
    Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
) {
    let mut boundary_at = None;
    if let Ok(mut object) = studio_json::from_slice::<Map<String, Value>>(&bytes)
        && let Some(stamp) =
            object.remove(studio_runtime_schema::provider_timing::BOUNDARY_STAMP_FIELD)
    {
        boundary_at = studio_json::from_value(stamp).ok();
        bytes = studio_json::to_vec(&object).expect("parsed provider JSON serializes");
    }
    (
        DecodedJournalPayload::RawStdout { bytes, error },
        boundary_at,
    )
}

pub fn decode_frame_with_metadata(
    bytes: &[u8],
) -> Result<
    (
        Frame,
        Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
    ),
    CodecError,
> {
    let mut line = bytes;
    if let Some(without_newline) = line.strip_suffix(b"\n") {
        line = without_newline;
        if let Some(without_carriage_return) = line.strip_suffix(b"\r") {
            line = without_carriage_return;
        }
    }
    let payload_bytes = frame_payload_bytes(bytes);
    if payload_bytes > MAX_FRAME_BYTES + MAX_BOUNDARY_METADATA_BYTES {
        Err(CodecError::FrameTooLarge {
            limit: MAX_FRAME_BYTES,
        })
    } else {
        if payload_bytes > MAX_FRAME_BYTES {
            // Only near-limit records need this allocation-free extra pass.
            // Measure the stamp's actual bytes, including noncanonical numbers.
            #[derive(Deserialize)]
            struct Envelope<'a> {
                #[serde(borrow, rename = "_studioBoundaryAt")]
                stamp: Option<studio_json::RawJson<'a>>,
            }
            let envelope: Envelope<'_> = studio_json::from_slice(line)
                .map_err(|error| CodecError::MalformedJson(error.to_string()))?;
            let metadata_bytes = envelope.stamp.map_or(0, |stamp| {
                stamp.as_str().len()
                    + studio_runtime_schema::provider_timing::BOUNDARY_STAMP_FIELD.len()
                    + 4
            });
            check_journal_byte_sizes(payload_bytes, metadata_bytes)?;
        }
        let wire = studio_json::from_slice::<WireObject<'_>>(line)
            .map_err(|error| CodecError::MalformedJson(error.to_string()))?;
        let stamp = wire.boundary_at;
        wire.into_frame().map(|frame| (frame, stamp))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod boundary_metadata_tests {
    use super::*;

    #[test]
    fn boundary_metadata_does_not_consume_provider_frame_budget() {
        use studio_runtime_schema::provider_timing::BOUNDARY_STAMP_FIELD;
        for provider_bytes in [
            MAX_FRAME_BYTES - 20,
            MAX_FRAME_BYTES,
            MAX_FRAME_BYTES + 1,
            MAX_FRAME_BYTES + MAX_BOUNDARY_METADATA_BYTES,
        ] {
            let mut value = studio_json::json!({"id":"r","result":{"pad":""}});
            let overhead = studio_json::to_vec(&value).unwrap().len();
            value["result"]["pad"] = Value::String("x".repeat(provider_bytes - overhead));
            value[BOUNDARY_STAMP_FIELD] = studio_json::json!({"bootId":"00000000-0000-0000-0000-000000000001","unixSeconds":10.,"monotonicNs":42});
            let bytes = studio_json::to_vec(&value).unwrap();
            assert!(bytes.len() > MAX_FRAME_BYTES);
            for decoder in 0..3 {
                let decoded = match decoder {
                    0 => decode_journal_value(1, 7, &value),
                    1 => decode_journal_entry(1, 7, bytes.clone()),
                    _ => decode_journal_entry_borrowed(1, 7, &bytes),
                };
                if provider_bytes <= MAX_FRAME_BYTES {
                    assert!(
                        matches!(decoded.payload, DecodedJournalPayload::Frame(_)),
                        "decoder={decoder}, provider_bytes={provider_bytes}"
                    );
                } else {
                    assert!(
                        matches!(
                            decoded.payload,
                            DecodedJournalPayload::RawStdout {
                                error: CodecError::FrameTooLarge { .. },
                                ..
                            }
                        ),
                        "decoder={decoder}"
                    );
                }
            }
        }
    }

    #[test]
    fn malformed_boundary_metadata_is_extracted_without_changing_raw_provider_bytes() {
        use studio_runtime_schema::provider_timing::{BOUNDARY_STAMP_FIELD, BoundaryStamp};
        let stamp = BoundaryStamp {
            boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
            unix_seconds: 10.,
            monotonic_ns: 42,
        };
        for plain in [
            studio_json::json!({"method":17}),
            studio_json::json!({"id":null,"result":{}}),
            studio_json::json!({"method":"turn/started","id":false}),
        ] {
            let raw = studio_json::to_vec(&plain).unwrap();
            let mut tagged = plain.clone();
            tagged[BOUNDARY_STAMP_FIELD] = studio_json::json!(stamp);
            let bytes = studio_json::to_vec(&tagged).unwrap();
            for decoded in [
                decode_journal_value(1, 7, &tagged),
                decode_journal_entry(1, 7, bytes.clone()),
                decode_journal_entry_borrowed(1, 7, &bytes),
            ] {
                assert_eq!(decoded.boundary_at, Some(stamp));
                let DecodedJournalPayload::RawStdout { bytes, .. } = decoded.payload else {
                    panic!("invalid frame")
                };
                assert_eq!(bytes, raw);
            }
        }
    }

    #[test]
    fn supervisor_boundary_metadata_is_removed_before_frame_construction() {
        use studio_runtime_schema::provider_timing::{BOUNDARY_STAMP_FIELD, BoundaryStamp};
        let stamp = BoundaryStamp {
            boot_id: Some("00000000-0000-0000-0000-000000000001".parse().unwrap()),
            unix_seconds: 10.,
            monotonic_ns: 42,
        };
        for mut wire in [
            studio_json::json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"turn"}},"extension":{"keep":true}}),
            studio_json::json!({"id":"request","result":{"turn":{"id":"turn"}}}),
            studio_json::json!({"method":"future/event","params":{"text":"keep"},"extension":{"keep":true}}),
        ] {
            let plain = decode_journal_value(1, 7, &wire);
            wire[BOUNDARY_STAMP_FIELD] = studio_json::json!(stamp);
            let bytes = studio_json::to_vec(&wire).unwrap();
            for tagged in [
                decode_journal_value(1, 7, &wire),
                decode_journal_entry(1, 7, bytes.clone()),
                decode_journal_entry_borrowed(1, 7, &bytes),
            ] {
                assert_eq!(tagged.boundary_at, Some(stamp));
                assert_eq!(tagged.payload, plain.payload);
                let (DecodedJournalPayload::Frame(tagged), DecodedJournalPayload::Frame(plain)) =
                    (&tagged.payload, &plain.payload)
                else {
                    panic!("valid frames")
                };
                assert_eq!(encode_line(tagged).unwrap(), encode_line(plain).unwrap());
            }
        }
    }
}

fn serialize_error(error: studio_json::Error) -> CodecError {
    CodecError::Serialization(error.to_string())
}

/// Serialize a frame with the provider stdio newline delimiter.
pub fn encode_line(frame: &Frame) -> Result<Vec<u8>, CodecError> {
    let mut output = Vec::new();
    encode_line_into(frame, &mut output)?;
    Ok(output)
}

/// Serialize into a caller-owned buffer, retaining its allocation for reuse.
pub fn encode_line_into(frame: &Frame, output: &mut Vec<u8>) -> Result<(), CodecError> {
    output.clear();
    let view = FrameView::new(frame);
    if let Err(error) = studio_json::to_writer(&mut *output, &view) {
        output.clear();
        return Err(serialize_error(error));
    }
    if output.len() > MAX_FRAME_BYTES {
        output.clear();
        return Err(CodecError::FrameTooLarge {
            limit: MAX_FRAME_BYTES,
        });
    }
    output.push(b'\n');
    Ok(())
}

#[derive(Clone, Copy)]
enum FrameValue<'a> {
    Json(&'a Value),
    Id(&'a RequestId),
    Text(&'a str),
    Raw(&'a studio_json::OwnedRawJson),
    CodexResult(&'a studio_codex_protocol::ClientRequestResult),
    CodexServerResult(&'a studio_codex_protocol::ServerRequestResult),
    CodexError(&'a studio_codex_protocol::JSONRPCErrorError),
}

impl Serialize for FrameValue<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Json(value) => value.serialize(serializer),
            Self::Id(value) => value.serialize(serializer),
            Self::Text(value) => serializer.serialize_str(value),
            Self::Raw(value) => value.serialize(serializer),
            Self::CodexResult(value) => {
                crate::codex_codec::ResultPayload(value).serialize(serializer)
            }
            Self::CodexError(value) => value.serialize(serializer),
            Self::CodexServerResult(value) => {
                crate::codex_codec::ServerResultPayload(value).serialize(serializer)
            }
        }
    }
}

enum FrameView<'a> {
    Entries(Vec<(&'a str, FrameValue<'a>)>),
    ClientRequest(&'a studio_codex_protocol::ClientRequest),
    ServerRequest(&'a studio_codex_protocol::ServerRequest),
    Notification(&'a studio_codex_protocol::ServerNotification),
    ClientNotification(&'a studio_codex_protocol::ClientNotification),
}

impl<'a> FrameView<'a> {
    fn new(frame: &'a Frame) -> Self {
        let (fields, overrides): (&Map<String, Value>, Vec<(&str, FrameValue<'a>)>) = match frame {
            Frame::CodexClientRequest(request) => return Self::ClientRequest(request),
            Frame::CodexServerRequest(request) => return Self::ServerRequest(request),
            Frame::CodexNotification(notification) => return Self::Notification(notification),
            Frame::CodexClientNotification(notification) => {
                return Self::ClientNotification(notification);
            }
            Frame::Request(method) => {
                let mut overrides = vec![
                    ("id", FrameValue::Id(&method.id)),
                    ("method", FrameValue::Text(method.method.as_str())),
                ];
                if let Some(params) = &method.params {
                    overrides.push(("params", FrameValue::Json(params)));
                }
                (&method.fields, overrides)
            }
            Frame::Notification(method) => {
                let mut overrides = vec![("method", FrameValue::Text(method.method.as_str()))];
                if let Some(params) = &method.params {
                    overrides.push(("params", FrameValue::Json(params)));
                }
                (&method.fields, overrides)
            }
            Frame::UnknownMethod(unknown) | Frame::KnownMethodUnparsed(unknown) => {
                (&unknown.fields, Vec::new())
            }
            Frame::Response { id, body, fields } => {
                let mut overrides = vec![("id", FrameValue::Id(id))];
                match body {
                    ResponseBody::CodexUnparsedResult { result, .. } => {
                        overrides.push(("result", FrameValue::Raw(result)))
                    }
                    ResponseBody::Uncorrelated(value) => {
                        overrides.push(("result", FrameValue::Raw(value)))
                    }
                    ResponseBody::CodexResult(value) => {
                        overrides.push(("result", FrameValue::CodexResult(value)))
                    }
                    ResponseBody::CodexError(value) => {
                        overrides.push(("error", FrameValue::CodexError(value)))
                    }
                    ResponseBody::CodexServerResult(value) => {
                        overrides.push(("result", FrameValue::CodexServerResult(value)))
                    }
                    ResponseBody::Result(value) => {
                        overrides.push(("result", FrameValue::Json(value)))
                    }
                    ResponseBody::Error(value) => {
                        overrides.push(("error", FrameValue::Json(value)))
                    }
                }
                (fields, overrides)
            }
        };
        let mut entries = fields
            .iter()
            .map(|(key, value)| (key.as_str(), FrameValue::Json(value)))
            .collect::<Vec<_>>();
        for (key, value) in overrides {
            match entries.binary_search_by(|(existing, _)| existing.cmp(&key)) {
                Ok(index) => entries[index].1 = value,
                Err(index) => entries.insert(index, (key, value)),
            }
        }
        Self::Entries(entries)
    }
}

impl Serialize for FrameView<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::ClientRequest(value) => value.serialize(serializer),
            Self::ServerRequest(value) => value.serialize(serializer),
            Self::Notification(value) => value.serialize(serializer),
            Self::ClientNotification(value) => value.serialize(serializer),
            Self::Entries(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl Serialize for Frame {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FrameView::new(self).serialize(serializer)
    }
}

/// Validate provider initialization parameters before opening a child process.
/// The request ID used here is the shortest wire-valid ID; `SessionState`
/// validates the final generated ID again before it emits the initialize write.
pub fn validate_initialize_params(params: &Value) -> Result<(), CodecError> {
    encode_line(&Frame::Request(RequestFrame {
        id: RequestId::Number(1),
        method: Method::Initialize,
        params: Some(params.clone()),
        fields: Map::new(),
    }))
    .map(|_| ())
}

/// Only the bounded Claude extension is normalized; native policy fields stay unchanged.
fn sanitize_failure_payload(payload: &mut Value) {
    for pointer in ["/error", "/turn/error"] {
        if let Some(error) = payload.pointer_mut(pointer) {
            studio_runtime_schema::claude_failure::sanitize_error(error);
        }
    }
    for pointer in ["/turns", "/thread/turns"] {
        if let Some(turns) = payload.pointer_mut(pointer).and_then(Value::as_array_mut) {
            for turn in turns {
                if let Some(error) = turn.get_mut("error") {
                    studio_runtime_schema::claude_failure::sanitize_error(error);
                }
            }
        }
    }
}

/// Vendor method classification of an incoming Codex frame. Unknown names keep
/// their original frame through the same compatibility accessor as known names.
/// Payload migration deliberately stays separate: this classification does not
/// deserialize a second JSON tree on the stdout hot path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodexIncomingMethod {
    Notification(studio_codex_protocol::ServerNotificationMethod),
    Request(studio_codex_protocol::ServerRequestMethod),
    Unknown { method: String },
}

impl CodexIncomingMethod {
    #[must_use]
    pub fn disposition(&self) -> Option<crate::codex_disposition::Disposition> {
        match self {
            Self::Notification(method) => {
                Some(crate::codex_disposition::server_notification(*method))
            }
            Self::Request(method) => Some(crate::codex_disposition::server_request(*method)),
            Self::Unknown { method: _ } => None,
        }
    }
}

impl Frame {
    /// Visit only generated open-union fallbacks; known fields stay strongly typed.
    pub fn visit_unknown_variants(&self, visitor: &mut dyn FnMut(&'static str, &str)) {
        use studio_codex_protocol::VisitUnknown;
        match self {
            Self::CodexClientRequest(value) => value.visit_unknowns(visitor),
            Self::CodexServerRequest(value) => value.visit_unknowns(visitor),
            Self::CodexNotification(value) => value.visit_unknowns(visitor),
            Self::CodexClientNotification(value) => value.visit_unknowns(visitor),
            Self::Response { body, .. } => match body {
                ResponseBody::CodexResult(value) => value.visit_unknowns(visitor),
                ResponseBody::CodexServerResult(value) => value.visit_unknowns(visitor),
                ResponseBody::CodexError(_)
                | ResponseBody::CodexUnparsedResult { .. }
                | ResponseBody::Uncorrelated(_)
                | ResponseBody::Result(_)
                | ResponseBody::Error(_) => {}
            },
            Self::Request(_)
            | Self::Notification(_)
            | Self::UnknownMethod(_)
            | Self::KnownMethodUnparsed(_) => {}
        }
    }

    /// Wire identity, shared by transport receipts and request correlation.
    #[must_use]
    pub fn id(&self) -> Option<RequestId> {
        match self {
            Self::CodexClientRequest(request) => crate::codex_request_id(request.id()).ok(),
            Self::CodexServerRequest(request) => crate::codex_request_id(request.id()).ok(),
            Self::Request(request) => Some(request.id.clone()),
            Self::Response { id, .. } => Some(id.clone()),
            Self::UnknownMethod(frame) | Self::KnownMethodUnparsed(frame) => frame
                .fields
                .get("id")
                .and_then(|id| RequestId::deserialize(id).ok()),
            Self::Notification(_)
            | Self::CodexNotification(_)
            | Self::CodexClientNotification(_) => None,
        }
    }

    /// Method identity without projecting typed payloads through JSON.
    #[must_use]
    pub fn method(&self) -> Option<&str> {
        match self {
            Self::CodexClientRequest(request) => Some(request.method().as_str()),
            Self::CodexServerRequest(request) => Some(request.method().as_str()),
            Self::CodexNotification(notification) => Some(notification.method().as_str()),
            Self::CodexClientNotification(notification) => Some(notification.method().as_str()),
            Self::Request(request) => Some(request.method.as_str()),
            Self::Notification(notification) => Some(notification.method.as_str()),
            Self::UnknownMethod(unknown) | Self::KnownMethodUnparsed(unknown) => {
                Some(&unknown.method)
            }
            Self::Response { .. } => None,
        }
    }

    /// Compatibility view for consumers awaiting typed payload migration.
    #[must_use]
    pub fn method_and_params(&self) -> Option<(&str, &Value)> {
        static NULL: Value = Value::Null;
        match self {
            Self::CodexClientRequest(_)
            | Self::CodexServerRequest(_)
            | Self::CodexNotification(_)
            | Self::CodexClientNotification(_) => None,
            Self::Notification(frame) => Some((
                frame.method.as_str(),
                frame.params.as_ref().unwrap_or(&NULL),
            )),
            Self::Request(frame) => Some((
                frame.method.as_str(),
                frame.params.as_ref().unwrap_or(&NULL),
            )),
            Self::UnknownMethod(frame) | Self::KnownMethodUnparsed(frame) => {
                Some((&frame.method, frame.fields.get("params").unwrap_or(&NULL)))
            }
            Self::Response {
                id: _,
                body: _,
                fields: _,
            } => None,
        }
    }

    /// Classify once at a Codex ingress. The enclosing reader owns provider
    /// identity; the Claude bridge's extensions do not enter this decoder.
    #[must_use]
    pub fn codex_incoming_method(&self) -> Option<CodexIncomingMethod> {
        match self {
            Self::CodexNotification(notification) => {
                return Some(CodexIncomingMethod::Notification(notification.method()));
            }
            Self::CodexServerRequest(request) => {
                return Some(CodexIncomingMethod::Request(request.method()));
            }
            Self::CodexClientRequest(_) | Self::CodexClientNotification(_) => return None,
            Self::Request(_)
            | Self::Notification(_)
            | Self::UnknownMethod(_)
            | Self::KnownMethodUnparsed(_)
            | Self::Response { .. } => {}
        }
        let (method, _) = self.method_and_params()?;
        let is_request = match self {
            Self::Request(_) => true,
            Self::Notification(_) => false,
            Self::UnknownMethod(frame) | Self::KnownMethodUnparsed(frame) => {
                frame.fields.contains_key("id")
            }
            Self::Response {
                id: _,
                body: _,
                fields: _,
            } => return None,
            Self::CodexClientRequest(_)
            | Self::CodexServerRequest(_)
            | Self::CodexNotification(_)
            | Self::CodexClientNotification(_) => return None,
        };
        Some(if is_request {
            studio_codex_protocol::ServerRequestMethod::from_wire(method)
                .map(CodexIncomingMethod::Request)
                .unwrap_or_else(|| CodexIncomingMethod::Unknown {
                    method: method.to_owned(),
                })
        } else {
            studio_codex_protocol::ServerNotificationMethod::from_wire(method)
                .map(CodexIncomingMethod::Notification)
                .unwrap_or_else(|| CodexIncomingMethod::Unknown {
                    method: method.to_owned(),
                })
        })
    }
}

#[cfg(test)]
mod codex_protocol_tests {
    use super::*;
    use studio_codex_protocol::{ServerNotification, ServerNotificationMethod, ServerRequest};
    use studio_json::json;

    #[test]
    fn codex_known_unknown_and_malformed_notification_boundary() {
        let known = json!({"method":"thread/name/updated","params":{"threadId":"thread","threadName":"name"}});
        let decoded = decode_journal_value(1, 1, &known);
        let DecodedJournalPayload::Frame(frame) = decoded.payload else {
            panic!("known notification frame")
        };
        assert_eq!(
            frame.codex_incoming_method(),
            Some(CodexIncomingMethod::Notification(
                ServerNotificationMethod::ThreadNameUpdated
            ))
        );
        assert_eq!(
            frame.method_and_params(),
            Some(("thread/name/updated", &known["params"]))
        );
        assert!(ServerNotification::deserialize(&known).is_ok());
        let unknown = json!({"method":"future/notification","params":{"opaque":[1,2]}});
        let DecodedJournalPayload::Frame(frame) = decode_journal_value(2, 1, &unknown).payload
        else {
            panic!("unknown notification retained")
        };
        assert_eq!(
            frame.codex_incoming_method(),
            Some(CodexIncomingMethod::Unknown {
                method: "future/notification".into()
            })
        );
        assert_eq!(
            frame.method_and_params(),
            Some(("future/notification", &unknown["params"]))
        );
        assert_eq!(
            studio_json::from_slice::<Value>(&encode_line(&frame).unwrap()).unwrap(),
            unknown
        );
        let malformed =
            json!({"method":"thread/name/updated","params":{"threadId":123,"threadName":"name"}});
        assert!(ServerNotification::deserialize(&malformed).is_err());
        assert!(
            ServerNotification::deserialize(json!({"method":"thread/name/updated","params":{}}))
                .is_err()
        );
    }

    #[test]
    fn codex_vendor_union_keeps_common_fields_and_optional_null_presence() {
        let valid = json!({"id":1,"method":"mcpServer/elicitation/request","params":{"mode":"openaiForm","message":"Confirm","requestedSchema":{},"serverName":"mcp","threadId":"thread","turnId":null}});
        assert!(ServerRequest::deserialize(&valid).is_ok());
        let mut missing = valid.clone();
        missing["params"]
            .as_object_mut()
            .unwrap()
            .remove("serverName");
        assert!(ServerRequest::deserialize(&missing).is_err());
        let mut unknown = valid;
        unknown["params"]["mode"] = json!("futureForm");
        assert!(ServerRequest::deserialize(&unknown).is_ok());
        assert!(
            studio_codex_protocol::ThreadStatus::deserialize(json!({"type":"active"})).is_err()
        );
        assert!(
            studio_codex_protocol::ThreadStatus::deserialize(
                json!({"type":"active","activeFlags":["futureFlag"]})
            )
            .is_err()
        );
    }

    #[test]
    fn codex_server_request_decodes_typed_params_and_result_contract() {
        let value = json!({"id":17,"method":"currentTime/read","params":{"threadId":"thread"}});
        assert!(ServerRequest::deserialize(&value).is_ok());
        let DecodedJournalPayload::Frame(frame) = decode_journal_value(1, 1, &value).payload else {
            panic!("request frame")
        };
        assert_eq!(
            frame.codex_incoming_method(),
            Some(CodexIncomingMethod::Request(
                studio_codex_protocol::ServerRequestMethod::CurrentTimeRead
            ))
        );
        assert_eq!(
            frame.codex_incoming_method().unwrap().disposition(),
            Some(crate::codex_disposition::Disposition::Handled)
        );
    }
}
