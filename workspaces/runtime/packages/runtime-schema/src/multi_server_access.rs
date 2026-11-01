//! Strict persisted identities for paired and discovered peers.
use crate::{
    bounded_text::BoundedText, multi_server_protocol::NonNegativeNumber, queue_command::Field,
};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedAccessClient {
    pub id: BoundedText<1, CLIENT_ID_MAX_CHARS>,
    pub client_id: BoundedText<1, CLIENT_ID_MAX_CHARS>,
    pub kind: AccessClientKind,
    pub label: BoundedText<1, CLIENT_LABEL_MAX_CHARS>,
    pub created: NonNegativeNumber,
    pub public_key: BoundedText<0, CLIENT_PUBLIC_KEY_MAX_CHARS>,
    pub status: AccessClientStatus,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub server_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub origin: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub tailscale_user: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub last_access: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub revoked: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub last_seen: Field<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub auto_pair: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub reachability: Field<Reachability>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub alias: Field<crate::multi_server_ingress::ServerAlias>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub replaced_by: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub grant: Field<AccessGrant>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub pairing_request_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub pairing_hash: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
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
