//! The pinned Codex protocol boundary. Results stay raw only until their
//! request method is known; application consumers receive the vendor union.
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use studio_codex_protocol as codex;
use studio_json::{Map, OwnedRawJson};

use crate::{CodecError, Frame, RequestId, ResponseBody, UnknownMethodFrame};

#[derive(Deserialize)]
struct Envelope<'a> {
    #[serde(borrow)]
    method: Option<Cow<'a, str>>,
    id: Option<RequestId>,
    #[serde(borrow, default, deserialize_with = "present_raw")]
    result: Option<studio_json::RawJson<'a>>,
    error: Option<codex::JSONRPCErrorError>,
    #[serde(borrow, default, deserialize_with = "present_raw")]
    params: Option<studio_json::RawJson<'a>>,
    #[serde(rename = "_studioBoundaryAt")]
    boundary_at: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
    #[serde(rename = "emittedAtMs")]
    emitted_at_ms: Option<i64>,
}

fn present_raw<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<studio_json::RawJson<'de>>, D::Error> {
    studio_json::RawJson::deserialize(deserializer).map(Some)
}

/// Decode one Codex frame using the pinned vendor types. An unknown method
/// is the only payload retained as extensible JSON.
pub fn decode_codex_frame(bytes: &[u8]) -> Result<Frame, CodecError> {
    decode_codex_frame_with_metadata(bytes).map(|(frame, _)| frame)
}

pub fn decode_codex_frame_with_metadata(bytes: &[u8]) -> Result<crate::DecodedFrame, CodecError> {
    let size = crate::frame_payload_bytes(bytes);
    if size > crate::MAX_FRAME_BYTES + crate::codec::MAX_BOUNDARY_METADATA_BYTES {
        return Err(CodecError::FrameTooLarge {
            limit: crate::MAX_FRAME_BYTES,
        });
    }
    let envelope: Envelope<'_> =
        studio_json::from_slice(bytes).map_err(|error| decode_error(&error))?;
    if size > crate::MAX_FRAME_BYTES {
        #[derive(Deserialize)]
        struct Stamp<'a> {
            #[serde(borrow, rename = "_studioBoundaryAt")]
            stamp: Option<studio_json::RawJson<'a>>,
        }
        let stamp: Stamp<'_> =
            studio_json::from_slice(bytes).map_err(|error| decode_error(&error))?;
        let metadata_bytes = stamp.stamp.as_ref().map_or(0, |stamp| {
            stamp.as_str().len()
                + studio_runtime_schema::provider_timing::BOUNDARY_STAMP_FIELD.len()
                + 4
        });
        crate::codec::check_journal_byte_sizes(size, metadata_bytes)?;
    }
    let stamp = envelope.boundary_at;
    let Some(method) = envelope.method.as_deref() else {
        let id = envelope.id.ok_or(CodecError::MalformedFrame(
            "expected a method or response id",
        ))?;
        let body = match (envelope.result, envelope.error) {
            (Some(result), None) => ResponseBody::Uncorrelated(result.to_owned()),
            (None, Some(error)) => ResponseBody::CodexError(error),
            _ => {
                return Err(CodecError::MalformedFrame(
                    "response requires exactly one of result or error",
                ));
            }
        };
        return Ok((
            Frame::Response {
                id,
                body,
                fields: Map::new(),
            },
            stamp,
        ));
    };
    let params = envelope.params.as_ref().map(studio_json::RawJson::as_str);
    if let Some(id) = &envelope.id {
        if let Some(method) = codex::ServerRequestMethod::from_wire(method) {
            let request =
                match codex::ServerRequest::from_params(vendor_request_id(id)?, method, params) {
                    Ok(request) => request,
                    Err(_) => return unparsed_known(bytes, method.as_str(), stamp),
                };
            return Ok((Frame::CodexServerRequest(Box::new(request)), stamp));
        }
        if let Some(method) = codex::ClientRequestMethod::from_wire(method) {
            let request =
                match codex::ClientRequest::from_params(vendor_request_id(id)?, method, params) {
                    Ok(request) => request,
                    Err(_) => return unparsed_known(bytes, method.as_str(), stamp),
                };
            return Ok((Frame::CodexClientRequest(Box::new(request)), stamp));
        }
    } else {
        if let Some(method) = codex::ServerNotificationMethod::from_wire(method) {
            let notification = match codex::ServerNotification::from_params(
                method,
                params,
                envelope.emitted_at_ms,
            ) {
                Ok(notification) => notification,
                Err(_) => return unparsed_known(bytes, method.as_str(), stamp),
            };
            return Ok((Frame::CodexNotification(Box::new(notification)), stamp));
        }
        if let Some(method) = codex::ClientNotificationMethod::from_wire(method) {
            let notification = match method {
                codex::ClientNotificationMethod::Initialized => {
                    codex::ClientNotification::Initialized
                }
            };
            return Ok((
                Frame::CodexClientNotification(Box::new(notification)),
                stamp,
            ));
        }
    }
    let fields = studio_json::from_slice(bytes).map_err(|error| decode_error(&error))?;
    Ok((
        Frame::UnknownMethod(UnknownMethodFrame {
            method: method.to_owned(),
            fields,
        }),
        stamp,
    ))
}

fn unparsed_known(
    bytes: &[u8],
    method: &str,
    stamp: Option<studio_runtime_schema::provider_timing::BoundaryStamp>,
) -> Result<crate::DecodedFrame, CodecError> {
    let mut fields: Map<String, studio_json::Value> =
        studio_json::from_slice(bytes).map_err(|error| decode_error(&error))?;
    fields.remove(studio_runtime_schema::provider_timing::BOUNDARY_STAMP_FIELD);
    Ok((
        Frame::KnownMethodUnparsed(UnknownMethodFrame {
            method: method.to_owned(),
            fields,
        }),
        stamp,
    ))
}

pub use decode_codex_frame as decode_codex_line;

/// Native compatibility probes keep their permissive notification admission,
/// but correlate successful results to the requested vendor DTO immediately.
pub fn decode_probe_response(
    line: &[u8],
    expected: u64,
    method: codex::ClientRequestMethod,
) -> Result<Option<codex::ClientRequestResult>, CodecError> {
    let wire: std::collections::BTreeMap<String, OwnedRawJson> =
        studio_json::from_slice(line).map_err(|error| decode_error(&error))?;
    if wire.contains_key("method") {
        return if wire.contains_key("id") {
            Err(CodecError::MalformedFrame("unexpected incoming request"))
        } else {
            Ok(None)
        };
    }
    let id = wire
        .get("id")
        .ok_or(CodecError::MalformedFrame("unexpected probe response"))?;
    let id: RequestId = studio_json::from_str(id.as_str()).map_err(|error| decode_error(&error))?;
    if id != RequestId::Number(expected) || wire.contains_key("error") {
        return Err(CodecError::MalformedFrame("unexpected probe response"));
    }
    let result = wire
        .get("result")
        .ok_or(CodecError::MalformedFrame("missing probe result"))?;
    codex::ClientRequestResult::decode(method, result.as_str())
        .map(Some)
        .map_err(|error| decode_error(&error))
}

/// Convert shared request preparation at the adapter boundary, before admission.
pub fn normalize_codex_frame(frame: &Frame) -> Result<Frame, CodecError> {
    match frame {
        Frame::Request(request) => {
            let id = vendor_request_id(&request.id)?;
            let params = request.params.clone().unwrap_or(studio_json::Value::Null);
            let decoded = if let Some(method) =
                codex::ServerRequestMethod::from_wire(request.method.as_str())
            {
                codex::ServerRequest::from_deserializer(id, method, params)
                    .map(|request| Frame::CodexServerRequest(Box::new(request)))
            } else if let Some(method) =
                codex::ClientRequestMethod::from_wire(request.method.as_str())
            {
                codex::ClientRequest::from_deserializer(id, method, params)
                    .map(|request| Frame::CodexClientRequest(Box::new(request)))
            } else {
                return Err(CodecError::MalformedFrame("not a Codex request method"));
            };
            Ok(match decoded {
                Ok(frame) => frame,
                Err(_) => {
                    let studio_json::Value::Object(fields) = studio_json::json!(frame) else {
                        unreachable!("request object");
                    };
                    Frame::KnownMethodUnparsed(UnknownMethodFrame {
                        method: request.method.as_str().into(),
                        fields,
                    })
                }
            })
        }
        Frame::Notification(notification) => {
            if notification.method != crate::Method::Initialized {
                return Err(CodecError::MalformedFrame(
                    "not a Codex client notification",
                ));
            }
            Ok(Frame::CodexClientNotification(Box::new(
                codex::ClientNotification::Initialized,
            )))
        }
        Frame::CodexClientRequest(_)
        | Frame::CodexServerRequest(_)
        | Frame::CodexNotification(_)
        | Frame::CodexClientNotification(_)
        | Frame::Response { .. }
        | Frame::UnknownMethod(_)
        | Frame::KnownMethodUnparsed(_) => Ok(frame.clone()),
    }
}

/// Codex egress cannot accidentally take the bridge's dynamic payload path.
pub fn encode_codex_line(frame: &Frame) -> Result<Vec<u8>, CodecError> {
    match frame {
        Frame::CodexClientRequest(_)
        | Frame::CodexServerRequest(_)
        | Frame::CodexNotification(_)
        | Frame::CodexClientNotification(_)
        | Frame::UnknownMethod(_) => crate::encode_line(frame),
        Frame::Response { body, .. } => match body {
            ResponseBody::CodexResult(_)
            | ResponseBody::CodexServerResult(_)
            | ResponseBody::CodexError(_)
            | ResponseBody::Uncorrelated(_) => crate::encode_line(frame),
            ResponseBody::Result(_)
            | ResponseBody::Error(_)
            | ResponseBody::CodexUnparsedResult { .. } => Err(CodecError::MalformedFrame(
                "Codex response requires a vendor payload",
            )),
        },
        Frame::Request(_) | Frame::Notification(_) | Frame::KnownMethodUnparsed(_) => Err(
            CodecError::MalformedFrame("Codex method requires a vendor payload"),
        ),
    }
}

pub(crate) fn decode_error(error: &studio_json::Error) -> CodecError {
    CodecError::MalformedJson(error.to_string())
}

/// Associate a previously opaque result with its exact registered method.
pub fn correlate_codex_result(
    method: crate::Method,
    raw: &OwnedRawJson,
) -> Result<codex::ClientRequestResult, CodecError> {
    let method = codex::ClientRequestMethod::from_wire(method.as_str()).ok_or(
        CodecError::MalformedFrame("response method is not a Codex client request"),
    )?;
    codex::ClientRequestResult::decode(method, raw.as_str()).map_err(|error| decode_error(&error))
}

/// Serialize a result without the method tag used by the vendor's Rust union.
pub(crate) struct ResultPayload<'a>(pub &'a codex::ClientRequestResult);
impl Serialize for ResultPayload<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_payload(serializer)
    }
}

pub(crate) struct ServerResultPayload<'a>(pub &'a codex::ServerRequestResult);
impl Serialize for ServerResultPayload<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_payload(serializer)
    }
}

pub fn codex_request_id(id: &codex::RequestId) -> Result<RequestId, CodecError> {
    match id {
        codex::RequestId::Variant1(id) => Ok(RequestId::String((**id).clone())),
        codex::RequestId::Variant2(id) => u64::try_from(**id)
            .map(RequestId::Number)
            .map_err(|_| CodecError::MalformedFrame("negative Codex request id")),
    }
}

pub fn vendor_request_id(id: &RequestId) -> Result<codex::RequestId, CodecError> {
    match id {
        RequestId::String(id) => Ok(codex::RequestId::Variant1(Box::new(id.clone()))),
        RequestId::Number(id) => i64::try_from(*id)
            .map(|id| codex::RequestId::Variant2(Box::new(id)))
            .map_err(|_| {
                CodecError::MalformedFrame("Codex request id exceeds signed integer range")
            }),
    }
}
