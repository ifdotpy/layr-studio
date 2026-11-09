//! Local, database-only federation state operations.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::OptionalExtension;
use std::collections::BTreeMap;
use std::sync::Arc;
use studio_json::{Value, json};
use studio_runtime_schema::federation_ingress::{
    FederationInvitation, FederationRequest, InvitationScope,
};
use studio_runtime_store::project_grants::{self, Capability, GrantRow};
use studio_runtime_store::{FederationRepository, RuntimeStore, StoreError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FederationError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("federation action refused: {0}")]
    Refused(String),
    #[error("local federation identity is required")]
    NeedsIdentity,
    #[error(transparent)]
    Credentials(#[from] crate::federation_credentials::CredentialError),
}

pub async fn snapshot(store: &RuntimeStore) -> Result<Value, FederationError> {
    Ok(store
        .read_async("runtime-services/federation", |transaction| {
            FederationRepository::snapshot(transaction)
        })
        .await?)
}

pub async fn manage_with_origin(
    store: Arc<RuntimeStore>,
    body: &FederationRequest,
    public_origin: Option<&str>,
) -> Result<Value, FederationError> {
    manage_with_origin_and_whoami(store, body, public_origin, None).await
}

pub async fn manage_with_origin_and_whoami(
    store: Arc<RuntimeStore>,
    body: &FederationRequest,
    public_origin: Option<&str>,
    whoami: Option<&Value>,
) -> Result<Value, FederationError> {
    match body {
        FederationRequest::CreateInvite {
            label,
            expected_user,
            ..
        } => {
            return create_invitation(
                store,
                label.as_ref().map(|v| v.as_str()),
                expected_user.as_ref().map(|v| v.as_str()),
                public_origin,
                whoami,
            )
            .await;
        }
        FederationRequest::AcceptPeer { invitation } => {
            return accept_peer(store, invitation, public_origin).await;
        }
        FederationRequest::CreateProjectInvite { project_id } => {
            return create_project_invitation(store, project_id, public_origin).await;
        }
        FederationRequest::AcceptProjectPeer {
            invitation,
            project_id,
        } => {
            return accept_project_peer(store, invitation, project_id, public_origin).await;
        }
        FederationRequest::ListProjectGrants { project_id } => {
            return project_grants_view(&store, project_id).await;
        }
        FederationRequest::RevokeProjectGrant { grant_id } => {
            return revoke_project_grant(store, grant_id).await;
        }
        FederationRequest::Status {}
        | FederationRequest::SetEnabled { .. }
        | FederationRequest::ApprovePeer { .. }
        | FederationRequest::RevokePeer { .. }
        | FederationRequest::RoomOptions { .. }
        | FederationRequest::CreateRoom { .. }
        | FederationRequest::ApproveRoom { .. } => {}
    }
    let body = body.clone();
    store
        .transaction_with(move |tx| {
            match &body {
                FederationRequest::SetEnabled { enabled } => {
                    FederationRepository::set_enabled(tx, *enabled)?
                }
                FederationRequest::ApprovePeer {
                    state_id,
                    accept_missing_whois,
                } => FederationRepository::approve_peer(tx, state_id, *accept_missing_whois)?,
                FederationRequest::RevokePeer { state_id } => {
                    FederationRepository::revoke_peer(tx, state_id)?
                }
                FederationRequest::RoomOptions {
                    room_id,
                    disclosure,
                } => FederationRepository::room_options(tx, room_id, disclosure)?,
                FederationRequest::CreateRoom { .. } | FederationRequest::ApproveRoom { .. } => {
                    FederationRepository::room_action(tx, &body)?
                }
                FederationRequest::Status {}
                | FederationRequest::CreateInvite { .. }
                | FederationRequest::AcceptPeer { .. }
                | FederationRequest::CreateProjectInvite { .. }
                | FederationRequest::AcceptProjectPeer { .. }
                | FederationRequest::ListProjectGrants { .. }
                | FederationRequest::RevokeProjectGrant { .. } => {
                    return Err(FederationError::Refused(
                        "unsupported local federation action".to_owned(),
                    ));
                }
            }
            FederationRepository::snapshot(tx).map_err(FederationError::from)
        })
        .await
        .map_err(|error| match error {
            FederationError::Store(StoreError::SchemaMismatch(message))
                if message.starts_with("missing local federation identity") =>
            {
                FederationError::NeedsIdentity
            }
            FederationError::Store(StoreError::SchemaMismatch(message))
                if !message.starts_with("invalid stored object") =>
            {
                FederationError::Refused(message)
            }
            other @ FederationError::Store(_)
            | other @ FederationError::Refused(_)
            | other @ FederationError::NeedsIdentity
            | other @ FederationError::Credentials(_) => other,
        })
}

async fn create_invitation(
    store: Arc<RuntimeStore>,
    label: Option<&str>,
    expected_user: Option<&str>,
    public_origin: Option<&str>,
    whoami: Option<&Value>,
) -> Result<Value, FederationError> {
    let origin = public_origin
        .and_then(|value| crate::remote_transport::validate_federation_origin(value).ok())
        .ok_or_else(|| {
            FederationError::Refused(
                "Configure Tailscale Serve before creating a pairing invitation".to_owned(),
            )
        })?;
    let enabled = store
        .read_async("runtime-services/federation", |tx| {
            let snapshot = FederationRepository::snapshot(tx)?;
            Ok::<_, StoreError>(snapshot["enabled"].as_bool() == Some(true))
        })
        .await?;
    if !enabled {
        return Err(FederationError::Refused(
            "Enable federation before creating an invitation".to_owned(),
        ));
    }

    let label = label.unwrap_or("Studio server").trim();
    if label.is_empty() || label.chars().count() > 80 {
        return Err(FederationError::Refused(
            "Server label must be 1 to 80 characters".to_owned(),
        ));
    }
    let now = unix_time()?;
    let lead_name = local_lead_name(Arc::clone(&store)).await?;
    let identity = crate::federation_credentials::Credentials::ensure_federation_identity(
        Arc::clone(&store),
        label,
        &lead_name,
        now,
    )
    .await?;
    let token = new_invitation_token()?;
    let invite_id = uuid::Uuid::new_v4().to_string();
    let expires = now as i64 + crate::federation_limits::INVITATION_TTL_SECONDS as i64;
    let expected_user = expected_user
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    let tailscale_user = whoami
        .and_then(|value| value.get("user"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let tailscale_status = if tailscale_user.is_some()
        && whoami
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            == Some("verified")
    {
        "verified"
    } else {
        "unavailable"
    };
    let record = json!({
        "id": invite_id,
        "tokenHash": studio_runtime_store::sha256_hex(token.as_bytes()),
        "created": now,
        "expires": expires,
        "status": "open",
        "expectedUser": expected_user,
    });
    let invite_id_for_write = invite_id.clone();
    let record_for_write = record.clone();
    let label_for_write = label.to_owned();
    let lead_name_for_write = lead_name.clone();
    let tailscale_user_for_write = tailscale_user.map(str::to_owned);
    let tailscale_status_for_write = tailscale_status.to_owned();
    store.transaction_with(move |tx| {
        tx.execute(
            // Raw SQL: JSON selectors/projections are outside db_query.
            "UPDATE runtime_federation_identity SET record=json_set(record,'$.label',?1,'$.leadName',?2,'$.tailscaleUser',?3,'$.tailscaleStatus',?4) WHERE id='local'",
            rusqlite::params![label_for_write, lead_name_for_write, tailscale_user_for_write, tailscale_status_for_write],
        )?;
        studio_runtime_store::access_tables::FederationInvitesRow::insert(tx, &invite_id_for_write, record_for_write.to_string())?;
        Ok::<_, StoreError>(())
    }).await?;
    let invitation = json!({
        "protocol": 1,
        "inviteId": invite_id,
        "token": token,
        "stateId": identity.id(),
        "label": label,
        "leadName": lead_name,
        "origin": origin,
        "publicKey": identity.public_key_pem(),
        "tailscaleUser": tailscale_user,
        "tailscaleStatus": tailscale_status,
        "expires": expires,
    });
    Ok(json!({
        "invitation": invitation,
        "expires": expires,
        "warning": "Share this invitation privately. It expires in 15 minutes and can be used once.",
    }))
}

async fn accept_peer(
    store: Arc<RuntimeStore>,
    invitation: &FederationInvitation,
    public_origin: Option<&str>,
) -> Result<Value, FederationError> {
    let invite_id = invitation.invite_id.as_str();
    let token = invitation.token.as_str();
    let state_id = invitation.state_id.as_str();
    let label = invitation.label.as_str();
    let public_key = invitation.public_key.as_str();
    let expires = invitation.expires.get();
    if invitation.scope.is_some() {
        return Err(FederationError::Refused(
            "This is a project invitation. Accept it from the project settings".to_owned(),
        ));
    }
    let origin = crate::remote_transport::validate_federation_origin(
        invitation.origin.as_deref().unwrap_or_default(),
    )
    .map_err(|_| {
        FederationError::Refused("Federation invitation has an invalid server URL".to_owned())
    })?;
    let now = unix_time()?;
    if invite_id.is_empty()
        || token.is_empty()
        || token.len() > 128
        || state_id.is_empty()
        || label.chars().count() > 80
        || public_key.is_empty()
        || public_key.len() > 4096
        || expires < now as i64
        || expires > now as i64 + crate::federation_limits::INVITATION_TTL_SECONDS as i64
    {
        return Err(FederationError::Refused(
            "Invalid or expired federation invitation".to_owned(),
        ));
    }
    uuid::Uuid::parse_str(state_id)
        .map_err(|_| FederationError::Refused("Peer state identity is invalid".to_owned()))?;
    let enabled = store.read_async("runtime-services/federation", |tx| {
        tx.query_row(
            "SELECT COALESCE(json_extract(record,'$.enabled'),0) FROM runtime_federation_settings WHERE id='global'",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(StoreError::from)
    })
    .await?;
    if !enabled {
        return Err(FederationError::Refused(
            "Enable federation before pairing".to_owned(),
        ));
    }
    let lead_name = local_lead_name(Arc::clone(&store)).await?;
    let local = crate::federation_credentials::Credentials::ensure_federation_identity(
        Arc::clone(&store),
        "Studio server",
        &lead_name,
        now,
    )
    .await?;
    if local.id() == state_id {
        return Err(FederationError::Refused(
            "Cannot pair a server with itself".to_owned(),
        ));
    }
    let local_origin = public_origin
        .and_then(|value| crate::remote_transport::validate_federation_origin(value).ok());
    let record = json!({
        "stateId": state_id,
        "label": label.trim(),
        "leadName": invitation.lead_name,
        "origin": origin,
        "publicKey": public_key,
        "inviteId": invite_id,
        "inviteToken": token,
        "localOrigin": local_origin,
        "localLeadName": lead_name,
        "status": "pending",
        "localApproved": true,
        "remoteApproved": false,
        "whoisStatus": "pending",
        "whoisUser": invitation.tailscale_user,
        "created": now,
        "updated": now,
        "protocol": crate::federation_limits::PROTOCOL_VERSION,
    });
    let state_id_for_write = state_id.to_owned();
    let invite_id_for_write = invite_id.to_owned();
    let token_for_write = token.to_owned();
    let public_key_for_write = public_key.to_owned();
    let origin_for_write = origin.clone();
    let record_for_write = record.clone();
    store
        .transaction_with(move |tx| {
            let existing: Option<String> = tx
                .query_row(
                    // Raw SQL: legacy record admission needs a typed-read equivalence proof.
                    "SELECT record FROM runtime_federation_peers WHERE id=?1",
                    [&state_id_for_write],
                    |row| row.get(0),
                )
                .optional()
                .map_err(StoreError::from)?;
            if let Some(existing) = existing {
                let existing: Value = studio_json::from_str(&existing).map_err(|_| {
                    StoreError::SchemaMismatch("invalid federation peer".to_owned())
                })?;
                if existing["inviteId"] != invite_id_for_write
                    || existing["inviteToken"] != token_for_write
                    || existing["publicKey"] != public_key_for_write
                    || existing["origin"] != origin_for_write
                {
                    return Err(FederationError::Refused(
                        "A different pairing invitation already exists for this server".to_owned(),
                    ));
                }
            } else {
                tx.execute(
                    // Raw SQL: legacy partial-record mutation keeps its existing admission.
                    "INSERT INTO runtime_federation_peers(id,record) VALUES(?1,?2)",
                    rusqlite::params![state_id_for_write, record_for_write.to_string()],
                )
                .map_err(StoreError::from)?;
            }
            Ok::<_, FederationError>(())
        })
        .await?;

    let transport = crate::federation_delivery::HttpsFederationTransport;
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    if crate::federation_delivery::pair_peer(
        Arc::clone(&store),
        state_id,
        now,
        &transport,
        Arc::new(cancelled),
    )
    .await
    .is_err()
    {
        let state_id = state_id.to_owned();
        store.transaction_with(move |tx| {
            tx.execute(
                // Raw SQL: JSON selectors/projections are outside db_query.
                "UPDATE runtime_federation_peers SET record=json_set(record,'$.lastError',?1) WHERE id=?2",
                rusqlite::params!["Connection unavailable; the pairing attempt will retry", state_id],
            )
            .map_err(StoreError::from)?;
            Ok::<_, StoreError>(())
        }).await?;
    }
    snapshot(&store).await
}

/// Create a one-use invitation for one project. The Serve origin is optional here: the
/// accepting server needs it, and it is refused there when missing.
async fn create_project_invitation(
    store: Arc<RuntimeStore>,
    project_id: &str,
    public_origin: Option<&str>,
) -> Result<Value, FederationError> {
    require_federation_enabled(&store).await?;
    require_local_project(&store, project_id).await?;
    let origin = public_origin
        .and_then(|value| crate::remote_transport::validate_federation_origin(value).ok());
    let now = unix_time()?;
    let lead_name = local_lead_name(Arc::clone(&store)).await?;
    let identity = crate::federation_credentials::Credentials::ensure_federation_identity(
        Arc::clone(&store),
        "Studio server",
        &lead_name,
        now,
    )
    .await?;
    let token = new_invitation_token()?;
    let invite_id = uuid::Uuid::new_v4().to_string();
    let expires = now as i64 + crate::federation_limits::INVITATION_TTL_SECONDS as i64;
    let record = json!({
        "id": invite_id,
        "tokenHash": studio_runtime_store::sha256_hex(token.as_bytes()),
        "created": now,
        "expires": expires,
        "status": "open",
        "expectedUser": null,
        "scope": "project",
        "projectId": project_id,
    });
    let invite_id_for_write = invite_id.clone();
    store
        .transaction_with(move |tx| {
            studio_runtime_store::access_tables::FederationInvitesRow::insert(
                tx,
                &invite_id_for_write,
                record.to_string(),
            )?;
            Ok::<_, StoreError>(())
        })
        .await?;
    Ok(json!({
        "invitation": {
            "protocol": 1,
            "inviteId": invite_id,
            "token": token,
            "stateId": identity.id(),
            "label": "Studio server",
            "leadName": lead_name,
            "origin": origin,
            "publicKey": identity.public_key_pem(),
            "tailscaleUser": null,
            "tailscaleStatus": "unavailable",
            "expires": expires,
            "scope": "project",
            "projectId": project_id,
        },
        "expires": expires,
        "warning": "Share this invitation privately. It expires in 15 minutes and can be used once.",
    }))
}

/// Accept a project invitation: send the signed pairing request to the inviting server,
/// then store this side of the grant. A retry with the same invitation is safe.
async fn accept_project_peer(
    store: Arc<RuntimeStore>,
    invitation: &FederationInvitation,
    project_id: &str,
    public_origin: Option<&str>,
) -> Result<Value, FederationError> {
    accept_project_peer_with(
        store,
        invitation,
        project_id,
        public_origin,
        &crate::federation_delivery::HttpsFederationTransport,
    )
    .await
}

async fn accept_project_peer_with(
    store: Arc<RuntimeStore>,
    invitation: &FederationInvitation,
    project_id: &str,
    public_origin: Option<&str>,
    transport: &impl crate::federation_delivery::FederationTransport,
) -> Result<Value, FederationError> {
    if invitation.scope != Some(InvitationScope::Project) {
        return Err(FederationError::Refused(
            "This is a server invitation. Accept it from the server pairing".to_owned(),
        ));
    }
    let remote_project = invitation
        .project_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| FederationError::Refused("Project invitation has no project".to_owned()))?;
    let origin = invitation
        .origin
        .as_deref()
        .and_then(|value| crate::remote_transport::validate_federation_origin(value).ok())
        .ok_or_else(|| {
            FederationError::Refused(
                "This invitation has no server URL. Ask the other owner to set up Tailscale Serve"
                    .to_owned(),
            )
        })?;
    let local_origin = public_origin
        .and_then(|value| crate::remote_transport::validate_federation_origin(value).ok())
        .ok_or_else(|| {
            FederationError::Refused(
                "Configure Tailscale Serve before accepting a project invitation".to_owned(),
            )
        })?;
    let now = unix_time()?;
    let state_id = invitation.state_id.as_str();
    if invitation.invite_id.is_empty()
        || invitation.token.is_empty()
        || invitation.token.len() > 128
        || invitation.public_key.is_empty()
        || invitation.public_key.len() > 4096
        || invitation.expires.get() < now as i64
        || invitation.expires.get()
            > now as i64 + crate::federation_limits::INVITATION_TTL_SECONDS as i64
    {
        return Err(FederationError::Refused(
            "Invalid or expired federation invitation".to_owned(),
        ));
    }
    uuid::Uuid::parse_str(state_id)
        .map_err(|_| FederationError::Refused("Peer state identity is invalid".to_owned()))?;
    require_federation_enabled(&store).await?;
    require_local_project(&store, project_id).await?;
    let lead_name = local_lead_name(Arc::clone(&store)).await?;
    let local = crate::federation_credentials::Credentials::ensure_federation_identity(
        Arc::clone(&store),
        "Studio server",
        &lead_name,
        now,
    )
    .await?;
    if local.id() == state_id {
        return Err(FederationError::Refused(
            "Cannot pair a server with itself".to_owned(),
        ));
    }
    let body = json!({
        "protocol": crate::federation_limits::PROTOCOL_VERSION,
        "inviteId": invitation.invite_id,
        "token": invitation.token,
        "stateId": local.id(),
        "label": "Studio server",
        "leadName": lead_name,
        "origin": local_origin,
        "publicKey": local.public_key_pem(),
        "tailscaleUser": null,
        "tailscaleStatus": "unavailable",
        "scope": "project",
        "projectId": remote_project,
        "peerProjectId": project_id,
    });
    let response = crate::federation_delivery::pair_project(
        Arc::clone(&store),
        transport,
        state_id,
        &origin,
        &invitation.public_key,
        body,
        now,
    )
    .await
    .map_err(|_| {
        FederationError::Refused(
            "The other server did not confirm. Accept again with the same invitation".to_owned(),
        )
    })?;
    if response["accepted"] != true
        || response["stateId"] != state_id
        || response["projectId"] != remote_project
    {
        return Err(FederationError::Refused(
            "The other server sent an invalid pairing answer".to_owned(),
        ));
    }
    let grant_id = invitation.invite_id.clone();
    let public_key = invitation.public_key.clone();
    let state_id_for_write = state_id.to_owned();
    let remote_project_for_write = remote_project.to_owned();
    let local_project = project_id.to_owned();
    let peer_label_for_write = invitation.label.trim().to_owned();
    store
        .transaction_with(move |tx| {
            if project_grants::grant_active(tx, &grant_id)?.is_none() {
                project_grants::insert_rows(
                    tx,
                    &[GrantRow {
                        grant_id,
                        peer: state_id_for_write,
                        peer_public_key: public_key,
                        local_project,
                        remote_project: remote_project_for_write,
                        capability: Capability::LeadMessages,
                        created_at: now as i64,
                        revoked_at: None,
                        peer_label: peer_label_for_write,
                    }],
                )?;
            }
            crate::sync_policy::publish_peer_teams(tx)?;
            Ok::<_, StoreError>(())
        })
        .await?;
    project_grants_view(&store, project_id).await
}

/// Revoke one project pairing on this side. Final: the peer link drops the key when no other
/// pairing uses it. Queued project messages do not exist yet, so none are dropped.
async fn revoke_project_grant(
    store: Arc<RuntimeStore>,
    grant_id: &str,
) -> Result<Value, FederationError> {
    let now = unix_time()? as i64;
    let grant_id_for_write = grant_id.to_owned();
    let project = store
        .transaction_with(move |tx| {
            let project: Option<String> = tx
                .query_row(
                    // Raw SQL: partial projections preserve the existing identity/permission boundary.
                    "SELECT local_project FROM project_grants WHERE grant_id=?1 LIMIT 1",
                    [&grant_id_for_write],
                    |row| row.get(0),
                )
                .optional()
                .map_err(StoreError::from)?;
            let project = project
                .ok_or_else(|| FederationError::Refused("Unknown project pairing".to_owned()))?;
            if project_grants::revoke(tx, &grant_id_for_write, now)? == 0 {
                return Err(FederationError::Refused(
                    "This project pairing is already revoked".to_owned(),
                ));
            }
            crate::sync_policy::publish_peer_teams(tx)?;
            Ok::<_, FederationError>(project)
        })
        .await?;
    project_grants_view(&store, &project).await
}

/// The active pairings of one local project, one entry per grant with its fingerprint.
async fn project_grants_view(
    store: &RuntimeStore,
    project_id: &str,
) -> Result<Value, FederationError> {
    let project_id = project_id.to_owned();
    let rows = store
        .read_async("runtime-services/federation/project-grants", move |tx| {
            Ok::<_, StoreError>(
                project_grants::all_rows(tx)?
                    .into_iter()
                    .filter(|row| row.local_project == project_id && row.revoked_at.is_none())
                    .collect::<Vec<_>>(),
            )
        })
        .await?;
    let mut grants: BTreeMap<String, Value> = BTreeMap::new();
    for row in rows {
        let capability = Value::from(row.capability.as_ref());
        let entry = grants.entry(row.grant_id.clone()).or_insert_with(|| {
            json!({
                "grantId": row.grant_id,
                "peerStateId": row.peer,
                "fingerprint": studio_runtime_store::sha256_hex(row.peer_public_key.as_bytes()),
                "localProject": row.local_project,
                "remoteProject": row.remote_project,
                "capabilities": [],
                "createdAt": row.created_at,
            })
        });
        if let Some(list) = entry["capabilities"].as_array_mut() {
            list.push(capability);
        }
    }
    Ok(json!({ "grants": grants.into_values().collect::<Vec<_>>() }))
}

async fn require_federation_enabled(store: &RuntimeStore) -> Result<(), FederationError> {
    let enabled = store
        .read_async("runtime-services/federation", |tx| {
            tx.query_row(
                "SELECT COALESCE(json_extract(record,'$.enabled'),0) FROM runtime_federation_settings WHERE id='global'",
                [],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map(|value| value.unwrap_or(false))
            .map_err(StoreError::from)
        })
        .await?;
    if enabled {
        Ok(())
    } else {
        Err(FederationError::Refused(
            "Enable federation before pairing a project".to_owned(),
        ))
    }
}

async fn require_local_project(
    store: &RuntimeStore,
    project_id: &str,
) -> Result<(), FederationError> {
    let project_id = project_id.to_owned();
    let exists = store
        .read_async("runtime-services/federation", move |tx| {
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM runtime_projects WHERE id=?1)",
                [&project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(StoreError::from)
        })
        .await?;
    if exists {
        Ok(())
    } else {
        Err(FederationError::Refused("Unknown project".to_owned()))
    }
}

fn new_invitation_token() -> Result<String, FederationError> {
    let mut token_bytes = [0_u8; 32];
    SystemRandom::new()
        .fill(&mut token_bytes)
        .map_err(|_| FederationError::Refused("Ed25519 operation failed".to_owned()))?;
    Ok(URL_SAFE_NO_PAD.encode(token_bytes))
}

async fn local_lead_name(store: Arc<RuntimeStore>) -> Result<String, FederationError> {
    let value = store
        .read_async("runtime-services/federation", |connection| {
            connection
                .query_row(
                    "SELECT json_extract(record,'$.name') FROM runtime_agents WHERE json_extract(record,'$.isLead')=1 AND (json_extract(record,'$.deletedAt') IS NULL OR json_extract(record,'$.deletedAt')=0) ORDER BY id LIMIT 1",
                    [],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map(|value| value.flatten())
                .map_err(StoreError::from)
        })
        .await?;
    Ok(value
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Lead".to_owned()))
}

fn unix_time() -> Result<f64, FederationError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .map_err(|_| FederationError::Refused("System clock is invalid".to_owned()))
}

#[cfg(test)]
#[path = "federation_project_tests.rs"]
mod project_tests;

#[cfg(test)]
mod tests {
    use super::manage_with_origin_and_whoami;
    use studio_json::json;

    #[tokio::test]
    async fn create_invite_uses_injected_tailscale_whoami_without_host_access() {
        let root = tempfile::tempdir().expect("isolated federation state");
        let store = std::sync::Arc::new(
            studio_runtime_store::RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store opens"),
        );
        manage_with_origin_and_whoami(
            std::sync::Arc::clone(&store),
            &studio_json::from_str(r#"{"action":"set_enabled","enabled":true}"#).unwrap(),
            None,
            None,
        )
        .await
        .expect("federation can be enabled locally");
        let whoami = json!({"status":"verified","user":"owner@example.test","node":"node-1"});
        let result = manage_with_origin_and_whoami(
            std::sync::Arc::clone(&store),
            &studio_json::from_str(r#"{"action":"create_invite","label":"Test server"}"#).unwrap(),
            Some("https://studio.example.ts.net"),
            Some(&whoami),
        )
        .await
        .expect("fake whoami invitation succeeds");
        assert_eq!(result["invitation"]["tailscaleUser"], "owner@example.test");
        assert_eq!(result["invitation"]["tailscaleStatus"], "verified");
        let identity = store
            .read_async("runtime-services/federation/tests", |tx| {
                tx.query_row(
                    "SELECT record FROM runtime_federation_identity WHERE id='local'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(studio_runtime_store::StoreError::from)
            })
            .await
            .expect("identity row reads");
        let identity: studio_json::Value = studio_json::from_str(&identity).expect("identity JSON");
        assert_eq!(identity["tailscaleUser"], "owner@example.test");
        assert_eq!(identity["tailscaleStatus"], "verified");
    }
}
