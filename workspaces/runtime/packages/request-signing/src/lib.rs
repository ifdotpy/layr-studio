//! Owner of the multi-server request signature: the canonical bytes, the header names and
//! Ed25519 sign and verify. It does no I/O. Callers load keys and send or read requests.

#[cfg(feature = "ed25519")]
use base64::{Engine, engine::general_purpose::STANDARD};
#[cfg(feature = "ed25519")]
use ring::signature::{ED25519, Ed25519KeyPair, UnparsedPublicKey};
use sha2::{Digest, Sha256};

/// Domain separator prefixed to every signed multi-server message. The protocol version
/// (`ACCESS_PROTOCOL_VERSION` in runtime-schema) must match this name; runtime-schema tests it.
pub const MULTI_SERVER_SIGNING_DOMAIN: &str = "studio-multi-server-v1";

/// Size of the random nonce that a client sends to prove a route's identity, in bytes.
pub const IDENTITY_PROOF_NONCE_BYTES: usize = 24;

/// The message a server signs to prove its identity for `nonce`. The nonce is unpadded
/// base64url of `IDENTITY_PROOF_NONCE_BYTES` bytes; any other value gives `None`.
pub fn identity_proof_message(server_id: &str, nonce: &str) -> Option<String> {
    let valid = nonce.len() == (IDENTITY_PROOF_NONCE_BYTES * 4).div_ceil(3)
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    valid.then(|| format!("{MULTI_SERVER_SIGNING_DOMAIN}\nidentity\n{server_id}\n{nonce}"))
}

/// Header names of a signed multi-server request. HTTP header names are case-insensitive.
pub mod header {
    pub const CLIENT: &str = "X-Studio-Client";
    pub const SERVER: &str = "X-Studio-Server";
    pub const TIMESTAMP: &str = "X-Studio-Timestamp";
    pub const NONCE: &str = "X-Studio-Nonce";
    pub const REQUEST_ID: &str = "X-Studio-Request-Id";
    pub const SIGNATURE: &str = "X-Studio-Signature";
}

/// The fields a signature covers. The request body is hashed, not copied.
#[derive(Clone, Copy)]
pub struct SigningInput<'a> {
    pub method: &'a str,
    pub target: &'a str,
    pub server_id: &'a str,
    pub client_id: &'a str,
    pub timestamp: &'a str,
    pub nonce: &'a str,
    pub request_id: &'a str,
    pub body: &'a [u8],
}

/// A signature that does not match its request, or a public key that cannot verify.
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidSignature;

/// The canonical bytes: the domain, the method in upper case, the target, the server id, the
/// client id, the timestamp, the nonce, the request id, and the lower-case hex SHA-256 of the
/// body. Fields are joined by one newline.
pub fn signing_bytes(input: SigningInput<'_>) -> Vec<u8> {
    let method = input.method.to_ascii_uppercase();
    let digest = Sha256::digest(input.body)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    [
        MULTI_SERVER_SIGNING_DOMAIN,
        &method,
        input.target,
        input.server_id,
        input.client_id,
        input.timestamp,
        input.nonce,
        input.request_id,
        &digest,
    ]
    .join("\n")
    .into_bytes()
}

/// Sign `input` with the device key. The result is the base64 value of `header::SIGNATURE`.
#[cfg(feature = "ed25519")]
pub fn sign(key: &Ed25519KeyPair, input: SigningInput<'_>) -> String {
    STANDARD.encode(key.sign(&signing_bytes(input)).as_ref())
}

/// Verify the base64 `signature` of `input` with the raw 32-byte Ed25519 `public_key`.
#[cfg(feature = "ed25519")]
pub fn verify(
    public_key: &[u8],
    input: SigningInput<'_>,
    signature: &str,
) -> Result<(), InvalidSignature> {
    let signature = STANDARD.decode(signature).map_err(|_| InvalidSignature)?;
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(&signing_bytes(input), &signature)
        .map_err(|_| InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "ed25519")]
    // The RFC 8032 test seed that the runtime and client fixtures also use.
    const SEED: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];
    #[cfg(feature = "ed25519")]
    const PUBLIC: [u8; 32] = [
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07,
        0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07,
        0x51, 0x1a,
    ];
    // Signature of `fixed_request()` with the seed above. The runtime signer
    // (`multi_server_signed_headers`) and the client signer (`server_keys::signature`)
    // produced this exact value before the owner moved here.
    #[cfg(feature = "ed25519")]
    pub(crate) const GOLDEN_SIGNATURE: &str =
        "PfjrkGA/IwuIryihDJjgbfNY/NicDM3N3JicKu0yQMHvEE58pUjBK502a+n1/jr/ovXn4d9nwTi1Q6p1nfX2Dg==";

    fn fixed_request(body: &[u8]) -> SigningInput<'_> {
        SigningInput {
            method: "post",
            target: "/api/x?q=1",
            server_id: "server",
            client_id: "test-identity",
            timestamp: "1700000000",
            nonce: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            request_id: "request-1",
            body,
        }
    }

    #[cfg(feature = "ed25519")]
    fn key() -> Ed25519KeyPair {
        Ed25519KeyPair::from_seed_unchecked(&SEED).expect("fixed seed loads")
    }

    #[test]
    fn canonical_bytes_are_fixed() {
        // Body digest: SHA-256 of `{"a":1}`.
        assert_eq!(
            signing_bytes(fixed_request(b"{\"a\":1}")),
            b"studio-multi-server-v1\nPOST\n/api/x?q=1\nserver\ntest-identity\n1700000000\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\nrequest-1\n015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862"
        );
    }

    #[test]
    fn identity_proof_message_is_fixed_and_checks_the_nonce() {
        // The web test (pairing.test.ts) asserts the same vector.
        let nonce = "A".repeat(32);
        assert_eq!(
            identity_proof_message("srv", &nonce).as_deref(),
            Some("studio-multi-server-v1\nidentity\nsrv\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        );
        assert_eq!(identity_proof_message("srv", "short"), None);
        assert_eq!(identity_proof_message("srv", &"+".repeat(32)), None);
    }

    #[cfg(feature = "ed25519")]
    #[test]
    fn golden_signature_is_unchanged() {
        let body = b"{\"a\":1}";
        assert_eq!(sign(&key(), fixed_request(body)), GOLDEN_SIGNATURE);
    }

    #[cfg(feature = "ed25519")]
    #[test]
    fn signed_request_verifies_and_a_changed_body_is_refused() {
        let body = b"{\"a\":1}";
        let signature = sign(&key(), fixed_request(body));
        assert_eq!(verify(&PUBLIC, fixed_request(body), &signature), Ok(()));
        assert_eq!(
            verify(&PUBLIC, fixed_request(b"{\"a\":2}"), &signature),
            Err(InvalidSignature)
        );
        assert_eq!(
            verify(&PUBLIC, fixed_request(body), "not base64!"),
            Err(InvalidSignature)
        );
    }

    #[test]
    fn header_names_are_fixed() {
        assert_eq!(header::SIGNATURE, "X-Studio-Signature");
        assert_eq!(header::REQUEST_ID, "X-Studio-Request-Id");
    }
}
