//! Multi-server management and signed ingress bodies.
use crate::{bounded_integer::BoundedInteger, bounded_text::BoundedText, queue_command::Field};
use serde::{Deserialize, Serialize};

pub const REQUEST_ID_MAX_CODEPOINTS: usize = 128;
pub const SERVER_ID_MAX_CODEPOINTS: usize = 128;
pub const CLIENT_ID_MAX_CODEPOINTS: usize = 128;
pub const INVITE_ID_MAX_CODEPOINTS: usize = 128;
pub const TOKEN_MAX_CODEPOINTS: usize = 128;
pub const SERVER_LABEL_MAX_CODEPOINTS: usize = 80;
pub const PAIR_LABEL_MAX_CODEPOINTS: usize = 80;
pub const INVITATION_LABEL_MAX_CODEPOINTS: usize = 80;
pub const CREATE_INVITE_LABEL_MAX_CODEPOINTS: usize = 80;
pub const ORIGIN_MAX_CODEPOINTS: usize = 512;
pub const PUBLIC_KEY_MAX_CODEPOINTS: usize = 4096;
pub const TAILSCALE_USER_MAX_CODEPOINTS: usize = 254;
pub const ALIAS_MAX_CODEPOINTS: usize = 3;
const SERVER_LABEL_PATTERN: &str = r"^[^\s\p{C}](?:[^\s\p{C}]| +[^\s\p{C}])*$";

type RequestId = BoundedText<1, REQUEST_ID_MAX_CODEPOINTS>;
type ServerId = BoundedText<1, SERVER_ID_MAX_CODEPOINTS>;
type ClientId = BoundedText<1, CLIENT_ID_MAX_CODEPOINTS>;
type InviteId = BoundedText<1, INVITE_ID_MAX_CODEPOINTS>;
type Token = BoundedText<1, TOKEN_MAX_CODEPOINTS>;
type PairLabel = BoundedText<1, PAIR_LABEL_MAX_CODEPOINTS>;
type InvitationLabel = BoundedText<1, INVITATION_LABEL_MAX_CODEPOINTS>;
type CreateInviteLabel = BoundedText<1, CREATE_INVITE_LABEL_MAX_CODEPOINTS>;
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Origin(String);
impl Origin {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn set_normalized(&mut self, value: String) {
        self.0 = value;
    }
}
impl<'de> Deserialize<'de> for Origin {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        BoundedText::<0, ORIGIN_MAX_CODEPOINTS>::deserialize(d)
            .map(|value| Self(value.as_str().to_owned()))
    }
}
type PublicKey = BoundedText<0, PUBLIC_KEY_MAX_CODEPOINTS>;
type Version = BoundedInteger<
    { crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION as i64 },
    { crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION },
>;
type Expiry = BoundedInteger<0, { crate::shared_values::MAX_SAFE_INTEGER as u64 }>;

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct ServerLabel(String);
impl ServerLabel {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for ServerLabel {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let pattern =
            regex::Regex::new(SERVER_LABEL_PATTERN).map_err(|_| "invalid server label pattern")?;
        if value.chars().count() <= SERVER_LABEL_MAX_CODEPOINTS && pattern.is_match(&value) {
            Ok(Self(value))
        } else {
            Err("invalid server label")
        }
    }
}
impl<'de> Deserialize<'de> for ServerLabel {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct ServerAlias(String);
impl ServerAlias {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for ServerAlias {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if !value.is_empty()
            && value.len() <= ALIAS_MAX_CODEPOINTS
            && value.bytes().all(|b| b.is_ascii_uppercase())
        {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom("invalid server alias"))
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grant {
    Full,
    Observe,
}
impl Grant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Observe => "observe",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessInvitation {
    pub protocol: Version,
    pub invite_id: InviteId,
    pub token: Token,
    pub server_id: ServerId,
    pub label: InvitationLabel,
    pub origin: Origin,
    pub public_key: PublicKey,
    pub tailscale_user: BoundedText<1, TAILSCALE_USER_MAX_CODEPOINTS>,
    pub expires: Expiry,
}

/// Invite fingerprints use the typed JSON bytes. Invites recorded by older peers
/// may mismatch after upgrade; the peers must create a new invitation.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ManagementRequest {
    AcceptInvite {
        request_id: RequestId,
        #[serde(deserialize_with = "crate::http_ingress::deserialize_object")]
        invitation: AccessInvitation,
    },
    CreateInvite {
        request_id: RequestId,
        #[serde(default, skip_serializing_if = "Field::is_missing")]
        label: Field<CreateInviteLabel>,
        #[serde(default, skip_serializing_if = "Field::is_missing")]
        grant: Field<Grant>,
    },
    Discover {
        request_id: RequestId,
    },
    Revoke {
        request_id: RequestId,
        client_id: ClientId,
    },
    Settings {
        #[serde(default, skip_serializing_if = "Field::is_missing")]
        request_id: Field<RequestId>,
        #[serde(default, skip_serializing_if = "Field::is_missing")]
        auto_pair: Field<bool>,
        #[serde(default, skip_serializing_if = "Field::is_missing")]
        auto_update: Field<bool>,
    },
    Alias {
        request_id: RequestId,
        server_id: ServerId,
        alias: ServerAlias,
    },
    Name {
        request_id: RequestId,
        server_id: ServerId,
        label: ServerLabel,
    },
    UiInvite {
        request_id: RequestId,
        server_id: ServerId,
    },
    Unrevoke {
        request_id: RequestId,
        client_id: ClientId,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameServer {
    pub label: ServerLabel,
    pub request_id: RequestId,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PairKind {
    Ui,
    Server,
}
impl PairKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Server => "server",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevicePairRequest {
    pub protocol: Version,
    pub invite_id: InviteId,
    pub token: Token,
    pub client_id: ClientId,
    pub label: PairLabel,
    pub kind: PairKind,
    pub public_key: PublicKey,
    pub request_id: RequestId,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub origin: Field<Origin>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoPairRequest {
    pub protocol: Version,
    pub server_id: ServerId,
    pub label: PairLabel,
    pub origin: Origin,
    pub public_key: PublicKey,
    pub request_id: RequestId,
}

#[cfg(test)]
mod tests {
    use super::ServerLabel;
    use serde_json::json;

    #[test]
    fn request_limits_match_openapi() {
        use super::*;
        let doc: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        let schemas = &doc["components"]["schemas"];
        for schema in [
            "AcceptInvite",
            "CreateInvite",
            "DiscoverServers",
            "RevokeClient",
            "SetServerAlias",
            "SetServerName",
            "UiInvite",
            "UnrevokeServer",
            "DevicePairRequest",
            "AutoPairRequest",
            "RenameServer",
        ] {
            assert_eq!(
                schemas[schema]["properties"]["requestId"]["maxLength"], REQUEST_ID_MAX_CODEPOINTS,
                "{schema}"
            );
        }
        assert_eq!(
            schemas["SetAccessSettings"]["properties"]["requestId"]["anyOf"][0]["maxLength"],
            REQUEST_ID_MAX_CODEPOINTS
        );
        assert_eq!(
            schemas["SetServerName"]["properties"]["label"]["maxLength"],
            SERVER_LABEL_MAX_CODEPOINTS
        );
        assert_eq!(
            schemas["RenameServer"]["properties"]["label"]["maxLength"],
            SERVER_LABEL_MAX_CODEPOINTS
        );
        for schema in ["RenameServer", "SetServerName"] {
            assert_eq!(
                schemas[schema]["properties"]["label"]["pattern"],
                SERVER_LABEL_PATTERN
            );
        }
        for schema in ["DevicePairRequest", "AutoPairRequest"] {
            assert_eq!(
                schemas[schema]["properties"]["label"]["maxLength"],
                PAIR_LABEL_MAX_CODEPOINTS
            );
        }
        assert_eq!(
            schemas["AccessInvitation"]["properties"]["label"]["maxLength"],
            INVITATION_LABEL_MAX_CODEPOINTS
        );
        assert_eq!(
            schemas["CreateInvite"]["properties"]["label"]["anyOf"][0]["maxLength"],
            CREATE_INVITE_LABEL_MAX_CODEPOINTS
        );
        assert_eq!(
            schemas["SetServerAlias"]["properties"]["alias"]["maxLength"],
            ALIAS_MAX_CODEPOINTS
        );
        assert_eq!(
            schemas["AccessInvitation"]["properties"]["tailscaleUser"]["maxLength"],
            TAILSCALE_USER_MAX_CODEPOINTS
        );
        for schema in ["DevicePairRequest", "AutoPairRequest", "AccessInvitation"] {
            assert_eq!(
                schemas[schema]["properties"]["publicKey"]["maxLength"],
                PUBLIC_KEY_MAX_CODEPOINTS
            );
            assert_eq!(
                schemas[schema]["properties"]["protocol"]["const"],
                crate::multi_server_protocol::ACCESS_PROTOCOL_VERSION
            );
        }
        assert_eq!(
            schemas["DevicePairRequest"]["properties"]["origin"]["anyOf"][0]["maxLength"],
            ORIGIN_MAX_CODEPOINTS
        );
        for schema in ["AutoPairRequest", "AccessInvitation"] {
            assert_eq!(
                schemas[schema]["properties"]["origin"]["maxLength"],
                ORIGIN_MAX_CODEPOINTS
            );
        }
        for schema in [
            "AcceptInvite",
            "SetServerAlias",
            "SetServerName",
            "UiInvite",
            "AutoPairRequest",
            "AccessInvitation",
        ] {
            if schema != "AcceptInvite" {
                assert_eq!(
                    schemas[schema]["properties"]["serverId"]["maxLength"],
                    SERVER_ID_MAX_CODEPOINTS
                );
            }
        }
        for schema in ["DevicePairRequest", "RevokeClient", "UnrevokeServer"] {
            assert_eq!(
                schemas[schema]["properties"]["clientId"]["maxLength"],
                CLIENT_ID_MAX_CODEPOINTS
            );
        }
        for schema in ["DevicePairRequest", "AccessInvitation"] {
            assert_eq!(
                schemas[schema]["properties"]["inviteId"]["maxLength"],
                INVITE_ID_MAX_CODEPOINTS
            );
            assert_eq!(
                schemas[schema]["properties"]["token"]["maxLength"],
                TOKEN_MAX_CODEPOINTS
            );
        }
        assert_eq!(
            schemas["AccessInvitation"]["properties"]["expires"]["maximum"],
            crate::shared_values::MAX_SAFE_INTEGER
        );
    }

    #[test]
    fn invite_fingerprint_is_deterministic_and_preserves_null_presence() {
        use super::ManagementRequest;
        let parse = |value| serde_json::from_value::<ManagementRequest>(value).unwrap();
        let first = parse(json!({"action":"create_invite", "requestId":"r"}));
        let same = parse(json!({"requestId":"r", "action":"create_invite"}));
        let explicit_null = parse(json!({"action":"create_invite", "requestId":"r", "label":null}));
        let fingerprint = |body: &ManagementRequest| {
            let bytes = serde_json::to_vec(body).unwrap();
            use sha2::{Digest, Sha256};
            Sha256::digest(bytes)
        };
        assert_eq!(fingerprint(&first), fingerprint(&same));
        assert_ne!(fingerprint(&first), fingerprint(&explicit_null));
    }

    #[test]
    fn label_matches_existing_request_validator() {
        for label in [
            "Server",
            "A B",
            " Żółć ",
            "a\nb",
            "a\n",
            "a\tb",
            "a\u{200d}b",
            "a\u{e000}b",
            "🙂",
            "",
            "  ",
            "a  b",
            "a\u{a0}b",
        ] {
            let typed = serde_json::from_value::<ServerLabel>(json!(label)).is_ok();
            let old =
                crate::validate("RenameServer", &json!({"requestId":"r", "label":label})).unwrap();
            assert_eq!(typed, old, "{label:?}");
        }
    }
}

impl ManagementRequest {
    pub fn action(&self) -> &'static str {
        match self {
            Self::AcceptInvite { .. } => "accept_invite",
            Self::CreateInvite { .. } => "create_invite",
            Self::Discover { .. } => "discover",
            Self::Revoke { .. } => "revoke",
            Self::Settings { .. } => "settings",
            Self::Alias { .. } => "alias",
            Self::Name { .. } => "name",
            Self::UiInvite { .. } => "ui_invite",
            Self::Unrevoke { .. } => "unrevoke",
        }
    }
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Settings { request_id, .. } => request_id.value().map(|v| v.as_str()),
            Self::AcceptInvite { request_id, .. }
            | Self::CreateInvite { request_id, .. }
            | Self::Discover { request_id }
            | Self::Revoke { request_id, .. }
            | Self::Alias { request_id, .. }
            | Self::Name { request_id, .. }
            | Self::UiInvite { request_id, .. }
            | Self::Unrevoke { request_id, .. } => Some(request_id.as_str()),
        }
    }
}
