//! Synchronous provider wire framing and per-connection request bookkeeping.
//!
//! Provider sessions are pure state machines; a thin caller owns split byte I/O.

mod callback_queue_limit;
mod codec;
mod codex_codec;
pub use codex_codec::{
    codex_request_id, correlate_codex_result, decode_codex_frame, decode_codex_frame_with_metadata,
    decode_codex_line, decode_probe_response, encode_codex_line, normalize_codex_frame,
    vendor_request_id,
};
pub use studio_codex_protocol as codex;
pub mod codex_disposition;
mod connection;
mod initialize;
mod provider_kind;
mod session;
pub mod supervisor_protocol;

pub use callback_queue_limit::CALLBACK_QUEUE_LIMIT;
pub use codec::{
    CodecError, CodexIncomingMethod, DecodedFrame, DecodedJournalEntry, DecodedJournalPayload,
    Frame, MAX_FRAME_BYTES, MAX_FRAME_DELIMITER_BYTES, Method, NotificationFrame, RequestFrame,
    RequestId, ResponseBody, UnknownMethodFrame, decode_frame, decode_frame_with_metadata,
    decode_journal_entry, decode_journal_entry_borrowed, decode_journal_value, decode_journal_with,
    encode_line, encode_line_into, frame_payload_bytes, frame_prefix_payload_bytes,
    validate_initialize_params,
};
pub use connection::{
    Action, Inbound, InitializationStatus, Lifecycle, RequestOutcome, RequestTicket, SessionError,
    SessionState,
};
pub use initialize::{ClientName, codex_initialize_params, initialize_params};
pub use provider_kind::{
    ProviderCapability, ProviderKind, RequestIdScheme, UnknownProvider, WireProfile,
    stop_terminals_method,
};
pub use session::{BookError, PendingBook, RequestState, ResponseMatch};

pub use studio_runtime_schema::provider_diagnostics;

pub use studio_runtime_schema::supervisor_contract;
