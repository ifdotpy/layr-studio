//! The pairing keys of the link. The iroh endpoint uses the server's Ed25519 pairing key, so
//! the endpoint id is the paired public key. iroh checks the peer key in the handshake, and
//! `pairing` (in the hooks) accepts only the keys of the paired servers.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use iroh::endpoint::{
    AfterHandshakeOutcome, BeforeConnectOutcome, Connection, EndpointHooks, VarInt,
};
use iroh::{EndpointAddr, PublicKey, SecretKey};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{PrivateKeyDer, SubjectPublicKeyInfoDer};

/// The ALPN of the peer request framing (docs/peer-link.md).
pub const ALPN: &[u8] = b"studio-peer/1";

/// Close code of a connection with a key that is not paired. It lies in the range that the
/// backend reads as "the peer ran nothing" (`inbound`/`outbound`).
pub const REFUSED: u32 = 0x100;

/// The keys of the paired servers, shared with the link and the hooks. The backend replaces the
/// set when a pairing changes.
pub type PairedKeys = Arc<RwLock<HashSet<PublicKey>>>;

/// DER SubjectPublicKeyInfo of a PEM public key, as the pairing records store it.
pub fn spki(pem: &str) -> Result<Vec<u8>, String> {
    SubjectPublicKeyInfoDer::from_pem_slice(pem.as_bytes())
        .map(|key| key.as_ref().to_vec())
        .map_err(|_| "The public key is not a PEM SubjectPublicKeyInfo".to_owned())
}

/// The iroh endpoint id of an Ed25519 SubjectPublicKeyInfo: its last 32 bytes.
pub fn endpoint_id(spki: &[u8]) -> Result<PublicKey, String> {
    const PREFIX: [u8; 12] = [
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    let key: [u8; 32] = spki
        .strip_prefix(&PREFIX)
        .and_then(|key| key.try_into().ok())
        .ok_or_else(|| "The public key is not Ed25519".to_owned())?;
    PublicKey::from_bytes(&key).map_err(|_| "The public key is not Ed25519".to_owned())
}

/// The endpoint secret key from the PKCS#8 PEM of the pairing key. It must match the public key.
pub fn secret_key(private_pem: &str, public_pem: &str) -> Result<SecretKey, String> {
    const PREFIX: [u8; 16] = [
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    let private = PrivateKeyDer::from_pem_slice(private_pem.as_bytes())
        .map_err(|_| "The private key is not PEM PKCS#8".to_owned())?;
    let seed: Option<[u8; 32]> = if let PrivateKeyDer::Pkcs8(der) = &private {
        der.secret_pkcs8_der()
            .strip_prefix(&PREFIX)
            .and_then(|seed| seed.try_into().ok())
    } else {
        None
    };
    let seed = seed.ok_or_else(|| "The private key is not an Ed25519 key".to_owned())?;
    let secret = SecretKey::from_bytes(&seed);
    if secret.public() != endpoint_id(&spki(public_pem)?)? {
        return Err("The public key does not belong to the private key".to_owned());
    }
    Ok(secret)
}

/// Refuses every connection, in either direction, with a key that is not paired. The check runs
/// before the dialer sends a stream and after the handshake of an accepted connection.
#[derive(Debug)]
pub struct Pairing {
    pub keys: PairedKeys,
}

impl Pairing {
    fn paired(&self, id: &PublicKey) -> bool {
        self.keys
            .read()
            .map(|keys| keys.contains(id))
            .unwrap_or(false)
    }
}

impl EndpointHooks for Pairing {
    async fn before_connect(&self, remote: &EndpointAddr, _alpn: &[u8]) -> BeforeConnectOutcome {
        if self.paired(&remote.id) {
            BeforeConnectOutcome::Accept
        } else {
            BeforeConnectOutcome::Reject
        }
    }

    async fn after_handshake(&self, connection: &Connection) -> AfterHandshakeOutcome {
        if self.paired(&connection.remote_id()) {
            AfterHandshakeOutcome::Accept
        } else {
            AfterHandshakeOutcome::Reject {
                error_code: VarInt::from_u32(REFUSED),
                reason: b"not paired".to_vec(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh pairing key as the backend writes it: PKCS#8 and SPKI PEM.
    fn pem_pair() -> (String, String) {
        let secret = SecretKey::generate();
        let mut private = vec![
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
            0x04, 0x20,
        ];
        private.extend_from_slice(&secret.to_bytes());
        let mut public = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        public.extend_from_slice(secret.public().as_bytes());
        (pem("PRIVATE KEY", &private), pem("PUBLIC KEY", &public))
    }

    fn pem(label: &str, der: &[u8]) -> String {
        use base64::Engine;
        let body = base64::engine::general_purpose::STANDARD.encode(der);
        format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")
    }

    #[test]
    fn the_pairing_key_is_the_endpoint_identity() {
        let (private, public) = pem_pair();
        let secret = secret_key(&private, &public).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            endpoint_id(&spki(&public).unwrap_or_default()).ok(),
            Some(secret.public())
        );
    }

    #[test]
    fn a_public_key_of_another_pair_is_refused() {
        let (private, _) = pem_pair();
        let (_, other) = pem_pair();
        assert!(secret_key(&private, &other).is_err());
    }
}
