//! Strict persisted identities for paired and discovered peers.
use crate::{
    bounded_text::BoundedText, multi_server_protocol::NonNegativeNumber, queue_command::Field,
};
use serde::{Deserialize, Serialize};

/// The version of the multi-server access protocol on the wire.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct AccessProtocolVersion(u64);
impl AccessProtocolVersion {
    pub const fn current() -> Self {
        Self(crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION)
    }
}
impl TryFrom<u64> for AccessProtocolVersion {
    type Error = &'static str;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value == crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION {
            Ok(Self(value))
        } else {
            Err("unsupported access protocol")
        }
    }
}
impl From<AccessProtocolVersion> for u64 {
    fn from(value: AccessProtocolVersion) -> Self {
        value.0
    }
}
impl<'de> Deserialize<'de> for AccessProtocolVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(u64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
impl schemars::JsonSchema for AccessProtocolVersion {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AccessProtocolVersion".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"integer", "const":crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION, "minimum":crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION, "maximum":crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION})
    }
}

/// Maximum stored client identity, label and public-key lengths, in Unicode code points.
pub const CLIENT_ID_MAX_CHARS: usize = 128;
pub const CLIENT_LABEL_MAX_CHARS: usize = 80;
pub const CLIENT_PUBLIC_KEY_MAX_CHARS: usize = 4096;
pub const GRANT_FULL: &str = "full";
pub const GRANT_OBSERVE: &str = "observe";

/// Fields retained when older paired or discovered records are migrated.
pub const CLIENT_FIELDS: &[&str] = &[
    "id",
    "clientId",
    "serverId",
    "kind",
    "label",
    "origin",
    "publicKey",
    "tailscaleUser",
    "status",
    "created",
    "lastAccess",
    "revoked",
    "lastSeen",
    "autoPair",
    "reachability",
    "alias",
    "replacedBy",
    "grant",
    "pairingRequestId",
    "pairingHash",
    "pairingReceipt",
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AccessClientKind {
    Ui,
    Server,
}
impl AccessClientKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Server => "server",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AccessClientStatus {
    Paired,
    Revoked,
    Discovered,
    Unreachable,
}
impl AccessClientStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paired => "paired",
            Self::Revoked => "revoked",
            Self::Discovered => "discovered",
            Self::Unreachable => "unreachable",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Reachability {
    Reachable,
    Unreachable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccessGrant {
    Full,
    Observe,
}

/// The legacy stored pairing response and the current pairing reply.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePairReply {
    pub protocol: u64,
    pub server_id: String,
    pub client_id: String,
    pub label: String,
    pub origin: String,
    pub public_key: String,
    pub tailscale_user: String,
    pub paired: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(rename = "ServerIdentity")]
pub struct AccessIdentity {
    pub server_id: String,
    pub label: String,
    #[serde(deserialize_with = "required_nullable")]
    #[schemars(with = "RequiredNullableString")]
    pub origin: Option<String>,
    pub public_key: String,
    #[serde(deserialize_with = "required_nullable")]
    #[schemars(with = "RequiredNullableString")]
    pub tailscale_user: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResponse {
    #[serde(flatten)]
    pub identity: AccessIdentity,
    pub protocol: u64,
    pub auto_pair: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof: Option<String>,
}

fn required_nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

struct RequiredNullableString;
impl schemars::JsonSchema for RequiredNullableString {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "RequiredNullableString".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":["string", "null"]})
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InviteSummary {
    pub invite_id: String,
    /// Creation time in Unix seconds, including fractional seconds.
    pub created: NonNegativeNumber,
    /// Unix seconds, including fractional seconds.
    pub expires: NonNegativeNumber,
    pub status: crate::multi_server_protocol::InviteStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessInvitation {
    pub protocol: AccessProtocolVersion,
    #[schemars(length(min = 1, max = crate::multi_server_ingress::INVITE_ID_MAX_CODEPOINTS))]
    pub invite_id: String,
    #[schemars(length(min = 1, max = crate::multi_server_ingress::SERVER_ID_MAX_CODEPOINTS))]
    pub server_id: String,
    #[schemars(length(min = 1, max = crate::multi_server_ingress::INVITATION_LABEL_MAX_CODEPOINTS))]
    pub label: String,
    #[schemars(length(max = crate::multi_server_ingress::ORIGIN_MAX_CODEPOINTS))]
    pub origin: String,
    #[schemars(length(max = crate::multi_server_ingress::PUBLIC_KEY_MAX_CODEPOINTS))]
    pub public_key: String,
    #[schemars(length(min = 1, max = crate::multi_server_ingress::TAILSCALE_USER_MAX_CODEPOINTS))]
    pub tailscale_user: String,
    /// Expiry in Unix seconds, including fractional seconds.
    #[schemars(range(max = crate::shared_values::MAX_SAFE_INTEGER))]
    pub expires: NonNegativeNumber,
    #[schemars(length(min = 1, max = crate::multi_server_ingress::TOKEN_MAX_CODEPOINTS))]
    pub token: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvitationResponse {
    pub invitation: AccessInvitation,
    /// Expiry in Unix seconds, including fractional seconds.
    pub expires: NonNegativeNumber,
}

/// Regenerate HTTP response components from their Rust serialization owners.
pub(crate) fn refresh_openapi(openapi: &mut serde_json::Value) {
    let mut generator = schemars::generate::SchemaSettings::draft2020_12()
        .for_deserialize()
        .with(|settings| settings.definitions_path = "/components/schemas".into())
        .into_generator();
    generator.subschema_for::<AccessSnapshot>();
    generator.subschema_for::<InvitationResponse>();
    generator.subschema_for::<crate::multi_server_ingress::AccessInvitation>();
    for (name, schema) in generator.take_definitions(true) {
        openapi["components"]["schemas"][name] = schema;
    }
    openapi["components"]["schemas"]["DiscoveredServer"] =
        serde_json::json!({"$ref":"#/components/schemas/AccessClient"});
    openapi["components"]["schemas"]["AcceptInvite"]["properties"]["invitation"] =
        serde_json::json!({"$ref":"#/components/schemas/AccessInvitationRequest"});
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityAlertCode {
    PeerIdentityChanged,
    LocalIdentityChanged,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityAlert {
    pub code: IdentityAlertCode,
    pub old_server_id: String,
    pub new_server_id: String,
    pub origin: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessSettings {
    pub auto_pair: bool,
    #[serde(default = "default_auto_update")]
    pub auto_update: bool,
    #[serde(default)]
    pub aliases: std::collections::BTreeMap<String, String>,
}

fn default_auto_update() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessSnapshot {
    pub protocol: AccessProtocolVersion,
    pub identity: AccessIdentity,
    pub clients: Vec<PublicAccessClient>,
    pub servers: Vec<PublicAccessClient>,
    pub invites: Vec<InviteSummary>,
    pub settings: AccessSettings,
    #[serde(default)]
    pub identity_alerts: Vec<IdentityAlert>,
}

/// Public projection of a stored client. Missing required fields become null.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(rename = "AccessClient")]
pub struct PublicAccessClient {
    pub id: BoundedText<1, CLIENT_ID_MAX_CHARS>,
    pub client_id: BoundedText<1, CLIENT_ID_MAX_CHARS>,
    pub kind: AccessClientKind,
    pub label: BoundedText<1, CLIENT_LABEL_MAX_CHARS>,
    pub created: NonNegativeNumber,
    pub public_key: BoundedText<0, CLIENT_PUBLIC_KEY_MAX_CHARS>,
    pub status: AccessClientStatus,
    #[serde(deserialize_with = "required_nullable")]
    #[schemars(with = "RequiredNullableString")]
    pub server_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[schemars(with = "RequiredNullableString")]
    pub origin: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    #[schemars(with = "RequiredNullableString")]
    pub tailscale_user: Option<String>,
    #[serde(deserialize_with = "required_nullable_number")]
    #[schemars(with = "RequiredNullableNumber")]
    pub last_access: Option<serde_json::Number>,
    #[serde(deserialize_with = "required_nullable_number")]
    #[schemars(with = "RequiredNullableNumber")]
    pub revoked: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    pub last_seen: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub auto_pair: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Reachability>")]
    pub reachability: Field<Reachability>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub alias: Field<crate::multi_server_ingress::ServerAlias>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub replaced_by: Field<String>,
}
fn required_nullable_number<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<serde_json::Number>, D::Error> {
    Option::<serde_json::Number>::deserialize(d)
}
struct RequiredNullableNumber;
impl schemars::JsonSchema for RequiredNullableNumber {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "RequiredNullableNumber".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":["number", "null"]})
    }
}
impl From<PersistedAccessClient> for PublicAccessClient {
    fn from(c: PersistedAccessClient) -> Self {
        Self {
            id: c.id,
            client_id: c.client_id,
            kind: c.kind,
            label: c.label,
            created: c.created,
            public_key: c.public_key,
            status: c.status,
            server_id: c.server_id.value().cloned(),
            origin: c.origin.value().cloned(),
            tailscale_user: c.tailscale_user.value().cloned(),
            last_access: c.last_access.value().cloned(),
            revoked: c.revoked.value().cloned(),
            last_seen: c.last_seen,
            auto_pair: c.auto_pair,
            reachability: c.reachability,
            alias: c.alias,
            replaced_by: c.replaced_by,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedAccessClient {
    pub id: BoundedText<1, CLIENT_ID_MAX_CHARS>,
    pub client_id: BoundedText<1, CLIENT_ID_MAX_CHARS>,
    pub kind: AccessClientKind,
    pub label: BoundedText<1, CLIENT_LABEL_MAX_CHARS>,
    /// Creation time in Unix seconds, including fractional seconds.
    pub created: NonNegativeNumber,
    pub public_key: BoundedText<0, CLIENT_PUBLIC_KEY_MAX_CHARS>,
    pub status: AccessClientStatus,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub server_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub origin: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub tailscale_user: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    /// Last access in Unix seconds, including fractional seconds.
    pub last_access: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    /// Revocation time in Unix seconds, including fractional seconds.
    pub revoked: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<serde_json::Number>")]
    /// Discovery time in Unix seconds, including fractional seconds.
    pub last_seen: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub auto_pair: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Reachability>")]
    pub reachability: Field<Reachability>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub alias: Field<crate::multi_server_ingress::ServerAlias>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub replaced_by: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(skip)]
    pub grant: Field<AccessGrant>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(skip)]
    pub pairing_request_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(skip)]
    pub pairing_hash: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(skip)]
    pub pairing_receipt: Field<DevicePairReply>,
}

#[cfg(test)]
mod tests {
    use super::{AccessGrant, CLIENT_FIELDS, GRANT_FULL, GRANT_OBSERVE, PersistedAccessClient};

    #[test]
    fn strict_client_accepts_valid_grants_and_rejects_unknown_fields() {
        assert_eq!(
            serde_json::to_string(&AccessGrant::Full).unwrap(),
            format!("\"{GRANT_FULL}\"")
        );
        assert_eq!(
            serde_json::to_string(&AccessGrant::Observe).unwrap(),
            format!("\"{GRANT_OBSERVE}\"")
        );
        let prefix = r#"{"id":"peer","clientId":"client","kind":"server","label":"Peer","created":1,"publicKey":"key","status":"paired""#;
        for suffix in ["}", r#", "grant":null}"#, r#", "grant":"observe"}"#] {
            assert!(
                serde_json::from_str::<PersistedAccessClient>(&format!("{prefix}{suffix}")).is_ok()
            );
        }
        for suffix in [r#", "grant":"other"}"#, r#", "extra":true}"#] {
            assert!(
                serde_json::from_str::<PersistedAccessClient>(&format!("{prefix}{suffix}"))
                    .is_err()
            );
        }
    }

    #[test]
    fn migration_fields_match_the_record_owner() {
        use super::{AccessClientKind, AccessClientStatus};
        use crate::{multi_server_protocol::NonNegativeNumber, queue_command::Field};
        let record = PersistedAccessClient {
            id: "peer".to_owned().try_into().unwrap(),
            client_id: "peer".to_owned().try_into().unwrap(),
            kind: AccessClientKind::Server,
            label: "Peer".to_owned().try_into().unwrap(),
            created: NonNegativeNumber::try_from(serde_json::Number::from(1)).unwrap(),
            public_key: "key".to_owned().try_into().unwrap(),
            status: AccessClientStatus::Paired,
            server_id: Field::Null,
            origin: Field::Null,
            tailscale_user: Field::Null,
            last_access: Field::Null,
            revoked: Field::Null,
            last_seen: Field::Null,
            auto_pair: Field::Null,
            reachability: Field::Null,
            alias: Field::Null,
            replaced_by: Field::Null,
            grant: Field::Null,
            pairing_request_id: Field::Null,
            pairing_hash: Field::Null,
            pairing_receipt: Field::Null,
        };
        let serialized = serde_json::to_string(&record).unwrap();
        let object: std::collections::BTreeMap<String, serde::de::IgnoredAny> =
            serde_json::from_str(&serialized).unwrap();
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        let mut fields = CLIENT_FIELDS.to_vec();
        keys.sort_unstable();
        fields.sort_unstable();
        assert_eq!(keys, fields);
    }
}
