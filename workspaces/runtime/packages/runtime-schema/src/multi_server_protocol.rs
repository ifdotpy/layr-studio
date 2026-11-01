//! Multi-server access protocol identity. This file is the owner; the web copy
//! `multiServerProtocol.ts` is rendered by `studio-contracts`.

/// Version of the signed multi-server access protocol.
pub const ACCESS_PROTOCOL_VERSION: u64 = 1;
/// Domain separator prefixed to every signed multi-server message. Owned by `studio-request-signing`.
pub use studio_request_signing::MULTI_SERVER_SIGNING_DOMAIN;
/// Size of the random nonce that a client sends to prove a route's identity, in bytes.
pub const IDENTITY_PROOF_NONCE_BYTES: usize = 24;
/// File name of the remote access record in the state directory.
pub const REMOTE_ACCESS_FILE: &str = "remote-access.json";
/// Environment variable that overrides the public origin of this server.
pub const PUBLIC_ORIGIN_ENVIRONMENT: &str = "CODEX_CANVAS_PUBLIC_ORIGIN";
/// Maximum invitation ID and owner lengths, in Unicode code points.
pub const INVITE_ID_MAX_CHARS: usize = 128;
pub const INVITE_OWNER_MAX_CHARS: usize = 254;
/// Length of the lower-case SHA-256 token digest, in ASCII bytes.
pub const INVITE_TOKEN_HASH_BYTES: usize = 64;

/// The message a server signs to prove its identity for `nonce`. The nonce is
/// unpadded base64url of `IDENTITY_PROOF_NONCE_BYTES` bytes; any other value gives `None`.
pub fn identity_proof_message(server_id: &str, nonce: &str) -> Option<String> {
    let valid = nonce.len() == (IDENTITY_PROOF_NONCE_BYTES * 4).div_ceil(3)
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    valid.then(|| format!("{MULTI_SERVER_SIGNING_DOMAIN}\nidentity\n{server_id}\n{nonce}"))
}

/// A complete discovery response, checked while decoding.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryIdentity {
    pub server_id: String,
    pub label: String,
    #[serde(deserialize_with = "required_nullable")]
    pub origin: Option<String>,
    pub public_key: String,
    #[serde(deserialize_with = "required_nullable")]
    pub tailscale_user: Option<String>,
    pub auto_pair: bool,
    #[serde(rename = "protocol")]
    _protocol: AccessProtocol,
    #[serde(default, rename = "proof", deserialize_with = "optional_proof")]
    _proof: Option<String>,
}

struct AccessProtocol;
impl<'de> serde::Deserialize<'de> for AccessProtocol {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = <serde_json::Number as serde::Deserialize>::deserialize(deserializer)?;
        if number.as_f64() == Some(ACCESS_PROTOCOL_VERSION as f64) {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom("unsupported access protocol"))
        }
    }
}

fn required_nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    <Option<String> as serde::Deserialize>::deserialize(deserializer)
}

fn optional_proof<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    <String as serde::Deserialize>::deserialize(deserializer).map(Some)
}

/// A stored invitation. Legacy rows that do not decode are discarded.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedAccessInvite {
    pub invite_id: crate::bounded_text::BoundedText<1, INVITE_ID_MAX_CHARS>,
    pub status: InviteStatus,
    pub expires: NonNegativeNumber,
    pub token_hash: InviteTokenHash,
    pub tailscale_user: crate::bounded_text::BoundedText<1, INVITE_OWNER_MAX_CHARS>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    pub protocol: crate::queue_command::Field<u64>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    pub server_id: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    pub label: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    pub origin: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    pub public_key: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    pub created: crate::queue_command::Field<serde_json::Number>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    pub grant: crate::queue_command::Field<String>,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteStatus {
    Open,
    Used,
}
impl InviteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Used => "used",
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(transparent)]
pub struct NonNegativeNumber(serde_json::Number);
impl NonNegativeNumber {
    pub fn as_f64(&self) -> Option<f64> {
        self.0.as_f64()
    }
    pub fn as_i64(&self) -> Option<i64> {
        self.0.as_i64()
    }
}
impl TryFrom<serde_json::Number> for NonNegativeNumber {
    type Error = &'static str;
    fn try_from(value: serde_json::Number) -> Result<Self, Self::Error> {
        if value.as_f64().is_some_and(|number| number >= 0.0) {
            Ok(Self(value))
        } else {
            Err("negative invitation expiry")
        }
    }
}

impl<'de> serde::Deserialize<'de> for NonNegativeNumber {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <serde_json::Number as serde::Deserialize>::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(transparent)]
pub struct InviteTokenHash(String);
impl TryFrom<String> for InviteTokenHash {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == INVITE_TOKEN_HASH_BYTES
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err("invalid invitation token hash")
        }
    }
}
impl<'de> serde::Deserialize<'de> for InviteTokenHash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

impl PersistedAccessInvite {
    pub fn invite_id(&self) -> &str {
        self.invite_id.as_str()
    }
    pub fn is_open_at(&self, now: f64) -> bool {
        matches!(self.status, InviteStatus::Open)
            && self.expires.0.as_f64().is_some_and(|v| v >= now)
    }
    pub fn token_hash(&self) -> &str {
        &self.token_hash.0
    }
    pub fn tailscale_user(&self) -> &str {
        self.tailscale_user.as_str()
    }
    pub fn grant(&self) -> Option<&str> {
        self.grant.value().map(String::as_str)
    }
    pub fn mark_used(&mut self) {
        self.status = InviteStatus::Used;
    }
}

/// The signed transport mutation root is distinct from the deterministic message ID.
pub(crate) fn refresh_openapi(openapi: &mut serde_json::Value) {
    for name in ["FederationAck", "FederationAckResult"] {
        let ack = &mut openapi["components"]["schemas"][name];
        ack["properties"]["requestId"] = serde_json::json!({"type":"string","format":"uuid"});
        if let Some(required) = ack["required"].as_array_mut()
            && !required.iter().any(|field| field == "requestId")
        {
            required.push(serde_json::json!("requestId"));
        }
    }
    let schema = &mut openapi["components"]["schemas"]["SignedEnvelope"];
    schema["properties"]["requestId"] = serde_json::json!({"type":"string","format":"uuid","description":"UUIDv7 mutation root, issued once and retained across delivery retries."});
    if let Some(required) = schema["required"].as_array_mut()
        && !required.iter().any(|field| field == "requestId")
    {
        required.push(serde_json::json!("requestId"));
    }
    let operation = &mut openapi["components"]["schemas"]["ServerOperationRequest"];
    operation["properties"]["operation"] = serde_json::json!({"type":"array","items":{"type":"string"},"description":"Scoped suboperation tuple; empty for a single-step request."});
    if let Some(required) = operation["required"].as_array_mut()
        && !required.iter().any(|field| field == "operation")
    {
        required.push(serde_json::json!("operation"));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    #[test]
    fn signing_domain_names_the_protocol_version() {
        assert_eq!(
            super::MULTI_SERVER_SIGNING_DOMAIN,
            format!("studio-multi-server-v{}", super::ACCESS_PROTOCOL_VERSION)
        );
    }

    #[test]
    fn identity_proof_message_is_fixed_and_checks_the_nonce() {
        // The web copy (pairing.test.ts) asserts the same vector.
        let nonce = "A".repeat(32);
        assert_eq!(
            super::identity_proof_message("srv", &nonce).as_deref(),
            Some("studio-multi-server-v1\nidentity\nsrv\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        );
        assert_eq!(super::identity_proof_message("srv", "short"), None);
        assert_eq!(super::identity_proof_message("srv", &"+".repeat(32)), None);
    }

    #[test]
    fn discovery_view_tracks_schema_fields_and_valid_formats() {
        let document: Value = serde_json::from_slice(crate::OPENAPI).unwrap();
        let schema = &document["components"]["schemas"]["DiscoveryIdentity"];
        let expected = [
            "autoPair",
            "label",
            "origin",
            "proof",
            "protocol",
            "publicKey",
            "serverId",
            "tailscaleUser",
        ];
        let fields = schema["properties"].as_object().unwrap();
        assert_eq!(
            fields.keys().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
        let mut constraints = Value::Object(fields.clone());
        for field in constraints.as_object_mut().unwrap().values_mut() {
            let field = field.as_object_mut().unwrap();
            field.remove("title");
            field.remove("description");
        }
        assert_eq!(
            constraints,
            json!({
                "serverId":{"type":"string"}, "label":{"type":"string"},
                "origin":{"anyOf":[{"type":"null"},{"type":"string"}]},
                "publicKey":{"type":"string"},
                "tailscaleUser":{"anyOf":[{"type":"null"},{"type":"string"}]},
                "protocol":{"type":"integer","const":1,"minimum":1,"maximum":1},
                "autoPair":{"type":"boolean"},
                "proof":{"type":"string"}
            })
        );
        // `proof` is optional: only an answer to a nonce carries it.
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), expected.len() - 1);
        assert!(
            expected
                .iter()
                .filter(|name| **name != "proof")
                .all(|name| required.contains(&json!(name)))
        );
        assert_eq!(schema["additionalProperties"], false);
        let valid = json!({"serverId":"s", "label":"l", "origin":null, "publicKey":"k", "tailscaleUser":null, "protocol":1, "autoPair":true});
        for (field, replacement) in [
            ("serverId", json!(null)),
            ("label", json!(0)),
            ("origin", json!(false)),
            ("publicKey", json!([])),
            ("tailscaleUser", json!({})),
            ("protocol", json!(2)),
            ("autoPair", json!(1)),
        ] {
            let mut bad = valid.clone();
            bad[field] = replacement;
            assert!(
                !crate::validate("DiscoveryIdentity", &bad).unwrap(),
                "{field}"
            );
            assert!(
                serde_json::from_value::<crate::http_ingress::Object<super::DiscoveryIdentity>>(
                    bad
                )
                .is_err(),
                "{field}"
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                !crate::validate("DiscoveryIdentity", &missing).unwrap(),
                "missing {field}"
            );
            assert!(
                serde_json::from_value::<crate::http_ingress::Object<super::DiscoveryIdentity>>(
                    missing
                )
                .is_err(),
                "missing {field}"
            );
        }
        for protocol in [json!(1), json!(1.0)] {
            let mut input = valid.clone();
            input["protocol"] = protocol;
            assert!(crate::validate("DiscoveryIdentity", &input).unwrap());
            assert!(serde_json::from_value::<super::DiscoveryIdentity>(input).is_ok());
        }
        assert!(!crate::validate("DiscoveryIdentity", &json!([])).unwrap());
        let mut bad_proof = valid.clone();
        bad_proof["proof"] = json!(null);
        assert!(!crate::validate("DiscoveryIdentity", &bad_proof).unwrap());
        assert!(
            serde_json::from_value::<crate::http_ingress::Object<super::DiscoveryIdentity>>(
                bad_proof
            )
            .is_err()
        );
        let mut extra = valid;
        extra["unexpected"] = json!(true);
        assert!(!crate::validate("DiscoveryIdentity", &extra).unwrap());
        assert!(
            serde_json::from_value::<crate::http_ingress::Object<super::DiscoveryIdentity>>(extra)
                .is_err()
        );
    }

    #[test]
    fn invite_record_tracks_schema_and_rejects_unknown_fields() {
        let schema: Value =
            serde_json::from_str(include_str!("schemas/persisted-access-invite.schema.json"))
                .unwrap();
        let mut properties = schema["properties"].clone();
        for field in properties.as_object_mut().unwrap().values_mut() {
            field.as_object_mut().unwrap().remove("title");
        }
        assert_eq!(
            properties,
            json!({
                "inviteId":{"type":"string","minLength":1,"maxLength":128},
                "status":{"type":"string","enum":["open","used"]},
                "expires":{"type":"number","minimum":0},
                "tokenHash":{"type":"string","pattern":"^[0-9a-f]{64}$"},
                "tailscaleUser":{"type":"string","minLength":1,"maxLength":254},
                "protocol":{"type":["integer","null"],"minimum":0},
                "serverId":{"type":["string","null"]},
                "label":{"type":["string","null"]},
                "origin":{"type":["string","null"]},
                "publicKey":{"type":["string","null"]},
                "created":{"type":["number","null"]},
                "grant":{"type":["string","null"]}
            })
        );
        assert_eq!(
            schema["required"],
            json!([
                "inviteId",
                "status",
                "expires",
                "tokenHash",
                "tailscaleUser"
            ])
        );
        assert_eq!(schema["additionalProperties"], false);
        let valid = json!({"inviteId":"i", "status":"open", "expires":9007199254740993_u64, "tokenHash":"a".repeat(64), "tailscaleUser":"owner", "grant":"observe"});
        assert!(crate::validate("PersistedAccessInvite", &valid).unwrap());
        let mut invite: super::PersistedAccessInvite =
            serde::Deserialize::deserialize(&valid).unwrap();
        assert_eq!(serde_json::to_value(&invite).unwrap(), valid);
        invite.mark_used();
        let mut expected = valid;
        expected["status"] = json!("used");
        assert_eq!(serde_json::to_value(invite).unwrap(), expected);
        for (field, invalid) in [
            ("status", json!("unknown")),
            ("expires", json!(-1)),
            ("inviteId", json!("")),
            ("tokenHash", json!("bad")),
            ("tailscaleUser", json!("")),
        ] {
            let mut input = expected.clone();
            input[field] = invalid;
            assert!(
                <super::PersistedAccessInvite as serde::Deserialize>::deserialize(&input).is_err(),
                "{field}"
            );
        }
        let mut unknown = expected;
        unknown["extension"] = json!(true);
        assert!(
            <super::PersistedAccessInvite as serde::Deserialize>::deserialize(&unknown).is_err()
        );
    }
}
