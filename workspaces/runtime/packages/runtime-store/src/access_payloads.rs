//! Named owners for access/federation JSON; unknown evidence stays encoded verbatim.
//! Existing access client/invite contracts are reused directly by access_tables.
use serde::{Deserialize, Serialize};
use studio_json::OwnedRawJson;
use studio_runtime_schema::persistence::{FederationAckState, FederationMessageKind};
use studio_runtime_schema::queue_command::Field;

pub type AccessClientPayload = studio_runtime_schema::http_ingress::Object<
    studio_runtime_schema::multi_server_access::PersistedAccessClient,
>;
pub type AccessInvitePayload = studio_runtime_schema::http_ingress::Object<
    studio_runtime_schema::multi_server_protocol::PersistedAccessInvite,
>;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FederationApprovalStatus {
    Approved,
    Pending,
    Revoked,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FederationRoomKind {
    Federated,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FederationParticipantRole {
    Lead,
    Agent,
}

/// Permission to disclose participant names in the federation room projection.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
pub struct ParticipantNameDisclosure(pub bool);

/// Permission to disclose participant status in the federation room projection.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
pub struct ParticipantStatusDisclosure(pub bool);

/// Auto-pair identity evidence is deliberately open, including non-string history.
/// Its comparison and missing/null rules remain owned by multi_server_discovery.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessAutoAttemptPayloadFields {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub protocol: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub server_id: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub label: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub origin: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub public_key: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub request_id: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub state: Field<OwnedRawJson>,
}

studio_runtime_schema::raw_json_payload!(pub struct AccessAutoAttemptPayload { pub fields: AccessAutoAttemptPayloadFields, pub extensions });

/// Signing fields read by remote_transport::StoredFederationIdentity.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FederationIdentityPayloadFields {
    pub state_id: String,
    pub label: String,
    pub private_key: String,
    pub public_key: String,
}

studio_runtime_schema::raw_json_payload!(pub struct FederationIdentityPayload { pub fields: FederationIdentityPayloadFields, pub extensions });

/// Fields written by federation::{create_invitation, create_project_invitation}.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FederationInvitePayloadFields {
    pub id: String,
    pub token_hash: String,
    pub created: f64,
    pub expires: f64,
    pub status: studio_runtime_schema::multi_server_protocol::InviteStatus,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub expected_user: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub scope: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub project_id: Field<String>,
}

studio_runtime_schema::raw_json_payload!(pub struct FederationInvitePayload { pub fields: FederationInvitePayloadFields, pub extensions });

/// The existing PersistedFederationPeer required selectors and open extensions.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FederationPeerPayloadFields {
    pub state_id: String,
    pub label: String,
    pub origin: String,
    pub public_key: String,
    pub status: FederationApprovalStatus,
    pub local_approved: bool,
    pub remote_approved: bool,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub whois_user: Field<String>,
}

studio_runtime_schema::raw_json_payload!(pub struct FederationPeerPayload { pub fields: FederationPeerPayloadFields, pub extensions });

/// The existing PersistedFederationRoom participant shape.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FederationParticipantPayload {
    pub id: String,
    pub role: FederationParticipantRole,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub status: Field<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FederationRoomPayloadFields {
    pub id: String,
    pub kind: FederationRoomKind,
    pub peer_id: String,
    pub peer_label: String,
    pub name: String,
    pub status: FederationApprovalStatus,
    pub local_members: Vec<String>,
    pub remote_members: Vec<FederationParticipantPayload>,
    pub local_approved: bool,
    pub remote_approved: bool,
    pub share_names: ParticipantNameDisclosure,
    pub share_status: ParticipantStatusDisclosure,
    pub created: f64,
    pub updated: f64,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub local_participants: Field<Vec<FederationParticipantPayload>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub sequence: Field<studio_runtime_schema::recovery_record::SafeInteger>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub remote_sequence: Field<studio_runtime_schema::recovery_record::SafeInteger>,
}

studio_runtime_schema::raw_json_payload!(pub struct FederationRoomPayload { pub fields: FederationRoomPayloadFields, pub extensions });

/// Receipt identity is the physical peer/message key, never the request UUID.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FederationInboxPayloadFields {
    pub hash: String,
    pub ack: FederationAckState,
    pub created: f64,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub kind: Field<FederationMessageKind>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub room: Field<String>,
}

studio_runtime_schema::raw_json_payload!(pub struct FederationInboxPayload { pub fields: FederationInboxPayloadFields, pub extensions });

/// Command snapshots written by server_exec::Job retain all execution evidence.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerExecPayloadFields {
    pub handle: String,
    pub server_id: String,
    pub status: String,
    pub started_at: f64,
    pub finished_at: Option<f64>,
    pub exit_code: Option<i64>,
    pub signal: Option<i64>,
    pub duration: Option<f64>,
    pub timed_out: bool,
    pub output_limit: u64,
    pub output_expires_at: f64,
}

studio_runtime_schema::raw_json_payload!(pub struct ServerExecPayload { pub fields: ServerExecPayloadFields, pub extensions });

/// Links have historical partial shapes; absent selectors remain absent.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLinkPayloadFields {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub side: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub server: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub parent: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub parent_epoch: Field<i64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub workers: Field<Vec<String>>,
}

studio_runtime_schema::raw_json_payload!(pub struct ServerLinkPayload { pub fields: ServerLinkPayloadFields, pub extensions });
