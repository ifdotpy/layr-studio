//! Browser bindings. Persistence and network I/O remain host adapters.
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

/// SHA-256 for browser content identities when Web Crypto is unavailable.
#[wasm_bindgen]
pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

#[wasm_bindgen]
pub fn parse_markdown(text: &str, streaming: bool) -> Result<String, JsValue> {
    studio_client_domain::markdown::parse(text, streaming)
        .and_then(|value| serde_json::to_string(&value).map_err(|error| error.to_string()))
        .map_err(|error| JsValue::from_str(&error))
}

/// The canonical bytes of one signed multi-server request. The owner is `studio-request-signing`;
/// the browser signs these bytes with its non-extractable device key.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn multi_server_signing_bytes(
    method: &str,
    target: &str,
    server_id: &str,
    client_id: &str,
    timestamp: &str,
    nonce: &str,
    request_id: &str,
    body: &[u8],
) -> Vec<u8> {
    studio_request_signing::signing_bytes(studio_request_signing::SigningInput {
        method,
        target,
        server_id,
        client_id,
        timestamp,
        nonce,
        request_id,
        body,
    })
}
