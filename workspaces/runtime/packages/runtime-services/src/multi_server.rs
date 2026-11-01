//! Local multi-server access projections and state mutations.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::hmac;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use studio_runtime_schema::multi_server_protocol::{
    ACCESS_PROTOCOL_VERSION, PersistedAccessInvite,
};
use studio_runtime_schema::{
    multi_server_access::{AccessClientKind, AccessClientStatus, PersistedAccessClient},
    queue_command::Field,
};
use studio_runtime_store::{MultiServerRepository, RuntimeStore, StoreError};
use thiserror::Error;

const INVITATION_TTL_SECONDS: i64 = 15 * 60;

#[derive(Debug, Error)]
pub enum MultiServerError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("multi-server action refused: {0}")]
    Refused(String),
    #[error("multi-server request conflict: {0}")]
    Conflict(String),
    #[error("multi-server action requires a signed remote request")]
    NeedsSignedRequest,
}

pub async fn audit(store: &RuntimeStore) -> Result<Value, MultiServerError> {
    Ok(store
        .read_async("runtime-services/multi_server", |tx| {
            MultiServerRepository::audit(tx)
        })
        .await?)
}

pub async fn snapshot(store: &RuntimeStore, identity: &Value) -> Result<Value, MultiServerError> {
    let identity = identity.clone();
    Ok(store
        .transaction_with(move |tx| snapshot_in_transaction(tx, &identity, true))
        .await?)
}

/// Read a snapshot-shaped view without persisting default aliases.
/// Use for side-effect-free comparisons before authenticating signed writes.
pub async fn snapshot_for_comparison(
    store: &RuntimeStore,
    identity: &Value,
) -> Result<Value, MultiServerError> {
    let identity = identity.clone();
    Ok(store
        .transaction_with(move |tx| snapshot_in_transaction(tx, &identity, false))
        .await?)
}

fn snapshot_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    identity: &Value,
    persist_default_aliases: bool,
) -> Result<Value, StoreError> {
    let aliases = server_aliases(tx, identity, persist_default_aliases)?;
    let clients = project_records(tx, "runtime_access_clients")?;
    let discovered = project_records(tx, "runtime_access_discovered")?;
    let invites = tx
        .prepare_cached(
            "SELECT id,record FROM runtime_access_invites ORDER BY rowid DESC LIMIT 100",
        )?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .map(|r| {
            let (id, raw) = r?;
            let v = decode_invite(&id, &raw)?;
            let expires = v.expires.as_i64()
                .ok_or_else(|| invalid("runtime_access_invites", &id, "expires"))?;
            let created = v.created.value().and_then(serde_json::Number::as_i64)
                .ok_or_else(|| invalid("runtime_access_invites", &id, "created"))?;
            Ok(json!({"inviteId":v.invite_id(),"expires":expires,"created":created,"status":v.status.as_str()}))
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let auto_pair = tx
        .query_row(
            "SELECT auto_pair FROM runtime_access_settings WHERE id=1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(1)
        != 0;
    let auto_update = peer_auto_update(tx)?;
    let identity_alerts = identity_alerts(tx, identity, &clients, &discovered)?;
    let servers = server_list(&clients, discovered);

    Ok(json!({
        "protocol": ACCESS_PROTOCOL_VERSION,
        "identity": identity,
        "clients": clients,
        "servers": servers,
        "invites": invites,
        "settings": {"autoPair": auto_pair, "autoUpdate": auto_update, "aliases": aliases},
        "identityAlerts": identity_alerts,
    }))
}

/// Paired servers in the shape of Python `ServerTransport.servers()`: the
/// discovery-merged server list without unpaired or revoked rows.
pub fn paired_servers(tx: &rusqlite::Transaction<'_>) -> Result<Vec<Value>, StoreError> {
    // Python `MultiServerService._public` names every public field, null when absent.
    const PUBLIC: [&str; 14] = [
        "id",
        "clientId",
        "serverId",
        "label",
        "kind",
        "origin",
        "publicKey",
        "tailscaleUser",
        "status",
        "created",
        "lastAccess",
        "revoked",
        "alias",
        "replacedBy",
    ];
    let mut clients = project_records(tx, "runtime_access_clients")?;
    for client in clients
        .iter_mut()
        .filter(|client| client["kind"].as_str() == Some("server"))
    {
        if let Some(object) = client.as_object_mut() {
            object.retain(|key, _| PUBLIC.contains(&key.as_str()));
            for key in PUBLIC {
                object.entry(key).or_insert(Value::Null);
            }
        }
    }
    let discovered = project_records(tx, "runtime_access_discovered")?;
    Ok(server_list(&clients, discovered)
        .into_iter()
        .filter(|server| server["status"].as_str() == Some("paired"))
        .collect())
}

/// Paired and discovered servers by ID. A saved server takes its activity from its discovery
/// record. The rules are those of `ServerDiscovery.servers` in codex_server_discovery.py.
fn server_list(clients: &[Value], discovered: Vec<Value>) -> Vec<Value> {
    let mut known = discovered
        .into_iter()
        .map(|peer| (peer["id"].as_str().unwrap_or_default().to_owned(), peer))
        .collect::<BTreeMap<_, _>>();
    for client in clients
        .iter()
        .filter(|client| client["kind"].as_str() == Some("server"))
    {
        let id = client["id"].as_str().unwrap_or_default().to_owned();
        let found = known.get(&id);
        let reachability = match found {
            Some(peer) if peer["status"].as_str() == Some("unreachable") => "unreachable",
            Some(_) => "reachable",
            None => "unknown",
        };
        let mut server = client.clone();
        server["lastSeen"] = found.map_or(Value::Null, |peer| peer["lastSeen"].clone());
        server["autoPair"] = found.map_or(Value::Null, |peer| peer["autoPair"].clone());
        server["reachability"] = json!(reachability);
        known.insert(id, server);
    }
    known
        .into_values()
        .map(|mut peer| {
            if peer.get("reachability").is_none() {
                let unreachable = peer["status"].as_str() == Some("unreachable");
                peer["reachability"] = json!(if unreachable {
                    "unreachable"
                } else {
                    "reachable"
                });
            }
            peer
        })
        .collect()
}

/// The peer auto-update setting (docs/peer-updates.md). The Python backend creates its table;
/// without the table the setting has its contract default, on.
pub fn peer_auto_update(tx: &rusqlite::Transaction<'_>) -> Result<bool, StoreError> {
    Ok(tx
        .query_row(
            "SELECT auto_update FROM runtime_peer_update_settings WHERE id=1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .is_none_or(|value| value != 0))
}

fn table_exists(tx: &rusqlite::Transaction<'_>, table: &str) -> Result<bool, StoreError> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Identity changes that leave live references to a lost server. They are derived on each
/// read and never stored. See docs/multi-server.md, "Lost server identities".
fn identity_alerts(
    tx: &rusqlite::Transaction<'_>,
    identity: &Value,
    clients: &[Value],
    discovered: &[Value],
) -> Result<Vec<Value>, StoreError> {
    let id_of = |record: &Value| record["id"].as_str().unwrap_or_default().to_owned();
    let servers = clients
        .iter()
        .filter(|record| record["kind"].as_str() == Some("server"))
        .collect::<Vec<_>>();
    let replaced = servers
        .iter()
        .filter(|record| {
            record["replacedBy"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
        })
        .map(|record| id_of(record))
        .collect::<BTreeSet<_>>();
    let mut known = servers
        .iter()
        .map(|record| (id_of(record), *record))
        .collect::<BTreeMap<_, _>>();
    for record in discovered {
        known.entry(id_of(record)).or_insert(record);
    }
    let mut successors = servers
        .iter()
        .filter(|record| record["status"].as_str() == Some("paired"))
        .collect::<Vec<_>>();
    successors.sort_by_key(|record| id_of(record));
    let mut alerts = Vec::new();
    for successor in successors {
        let new = id_of(successor);
        let Some(origin) = successor["origin"]
            .as_str()
            .filter(|origin| !origin.is_empty())
        else {
            continue;
        };
        for (old, peer) in &known {
            if *old != new && !replaced.contains(old) && peer["origin"].as_str() == Some(origin) {
                alerts.push(json!({
                    "code": "peer_identity_changed",
                    "oldServerId": old,
                    "newServerId": new,
                    "origin": origin,
                    "detail": format!(
                        "{origin} now presents server {new}. References to server {old} remain unavailable."
                    ),
                }));
            }
        }
    }
    let local = identity["serverId"].as_str().unwrap_or_default();
    for old in previous_local_identities(tx, local)?.difference(&replaced) {
        alerts.push(json!({
            "code": "local_identity_changed",
            "oldServerId": old,
            "newServerId": local,
            "origin": identity["origin"],
            "detail": format!(
                "Local state still names this server {old}, but its current identity is {local}. References to the previous identity remain unavailable."
            ),
        }));
    }
    Ok(alerts)
}

/// Server IDs that local state uses for this server itself, other than its current ID: a
/// home-side link names this server as its home, and a project names its own server as its
/// home. The rules are those of `previous_local_identities` in codex_server_replacement.py.
fn previous_local_identities(
    tx: &rusqlite::Transaction<'_>,
    local: &str,
) -> Result<BTreeSet<String>, StoreError> {
    let queries = [
        (
            "runtime_server_links",
            "SELECT DISTINCT json_extract(record,'$.home') FROM runtime_server_links WHERE json_extract(record,'$.side')='home'",
        ),
        (
            "runtime_projects",
            "SELECT DISTINCT json_extract(record,'$.homeServerId') FROM runtime_projects",
        ),
    ];
    let mut found = BTreeSet::new();
    for (table, query) in queries {
        if !table_exists(tx, table)? {
            continue;
        }
        let mut statement = tx.prepare_cached(query)?;
        let rows = statement.query_map([], |row| row.get::<_, rusqlite::types::Value>(0))?;
        for value in rows {
            if let rusqlite::types::Value::Text(id) = value? {
                found.insert(id);
            }
        }
    }
    found.remove(local);
    Ok(found)
}

pub async fn manage(
    store: &RuntimeStore,
    identity: &Value,
    body: &studio_runtime_schema::multi_server_ingress::ManagementRequest,
    actor: &str,
) -> Result<Value, MultiServerError> {
    use studio_runtime_schema::multi_server_ingress::ManagementRequest;
    let identity = identity.clone();
    let body = body.clone();
    let actor = actor.to_owned();
    store.transaction_with(move |tx| -> Result<Value, MultiServerError> {
        match &body {
            ManagementRequest::Settings { auto_pair, .. } => { tx.execute("UPDATE runtime_access_settings SET auto_pair=?1 WHERE id=1",[i64::from(auto_pair.value().copied().unwrap_or(false))])?; audit_write(tx,"local",&actor,"settings")?; }
            ManagementRequest::Revoke { client_id, .. } => { let id=client_id.as_str(); let mut client=stored(tx,"runtime_access_clients",id)?.ok_or_else(||MultiServerError::Refused("client does not exist".into()))?; if client.status != AccessClientStatus::Revoked { client.status=AccessClientStatus::Revoked; client.revoked=Field::Value(serde_json::Number::from_f64(unix_time()?).ok_or_else(|| invalid("runtime_access_clients",id,"revoked"))?); tx.execute("UPDATE runtime_access_clients SET record=?1 WHERE id=?2",rusqlite::params![serde_json::to_string(&client).map_err(|_|invalid("runtime_access_clients",id,"record"))?,id])?; audit_write(tx,id,&actor,"revoke")?; } }
            ManagementRequest::Alias { server_id, request_id, alias } => {
                let server = server_id.as_str();
                let request = request_id.as_str();
                let alias_value = alias.clone();
                let alias = alias.as_str();
                if !valid_identifier(server) || !valid_identifier(request) {
                    return Err(MultiServerError::Refused("invalid server alias request".into()));
                }
                let local = server == "local" || server == identity["serverId"].as_str().unwrap_or_default();
                let target = if local { "local" } else { server };
                let prior=tx.query_row("SELECT server,alias FROM runtime_access_alias_requests WHERE id=?1",[&request],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?;
                if let Some((saved_target,saved_alias)) = prior {
                    if saved_target != target || saved_alias != alias {
                        return Err(MultiServerError::Conflict("request ID has different content".into()));
                    }
                } else {
                    let aliases = ensure_server_aliases(tx, &identity)?;
                    let local_id = identity["serverId"].as_str().unwrap_or_default();
                    if !local && !aliases.contains_key(server) {
                        return Err(MultiServerError::Refused("server does not exist".into()));
                    }
                    let collision = aliases.iter().any(|(id, existing)| {
                        existing == alias && id != target && !(local && id == local_id)
                    });
                    if collision {
                        return Err(MultiServerError::Conflict("server alias is already in use".into()));
                    }
                    if local {
                        tx.execute("INSERT INTO runtime_access_local_alias VALUES(1,?) ON CONFLICT(id) DO UPDATE SET alias=excluded.alias", [&alias])?;
                    } else {
                        let mut peer = stored(tx, "runtime_access_clients", server)?
                            .ok_or_else(|| MultiServerError::Refused("server does not exist".into()))?;
                        peer.alias = Field::Value(alias_value);
                        tx.execute("UPDATE runtime_access_clients SET record=?1 WHERE id=?2", rusqlite::params![serde_json::to_string(&peer).map_err(|_|invalid("runtime_access_clients",&server,"record"))?, server])?;
                    }
                    tx.execute("INSERT INTO runtime_access_alias_requests VALUES(?,?,?)", rusqlite::params![request,target,alias])?;
                    audit_write(tx,target,&actor,"alias")?;
                }
            }
            ManagementRequest::Unrevoke { request_id, client_id } => { let request = request_id.as_str(); let target = client_id.as_str(); let prior=tx.query_row("SELECT client FROM runtime_access_unrevokes WHERE id=?1",[&request],|r|r.get::<_,String>(0)).optional()?; if let Some(saved)=prior {if saved!=target{return Err(MultiServerError::Conflict("request ID selected another server".into()));}} else {let v=stored(tx,"runtime_access_clients",target)?.ok_or_else(||MultiServerError::Refused("select a revoked server".into()))?;if v.status!=AccessClientStatus::Revoked || v.kind!=AccessClientKind::Server{return Err(MultiServerError::Refused("select a revoked server".into()));}tx.execute("DELETE FROM runtime_access_clients WHERE id=?1",[&target])?;tx.execute("DELETE FROM runtime_access_auto_attempts WHERE id=?1",[&target])?;tx.execute("INSERT INTO runtime_access_unrevokes VALUES(?1,?2)",rusqlite::params![request,target])?;audit_write(tx,target,&actor,"unrevoke")?;} }
            ManagementRequest::AcceptInvite { .. } | ManagementRequest::CreateInvite { .. }
            | ManagementRequest::Discover { .. } | ManagementRequest::Name { .. }
            | ManagementRequest::UiInvite { .. } => return Err(MultiServerError::Refused(format!("unsupported local multi-server action: {}", body.action()))),
        }
        let clients=project_records(tx,"runtime_access_clients")?;
        let records=MultiServerRepository::audit(tx)?;
        let _=(clients,records);
        snapshot_in_transaction(tx, &identity, true).map_err(MultiServerError::from)
    }).await
}

/// Create a single-use signed pairing invitation. The token is deterministically
/// recoverable from the saved invite and request ID, so an exact retry returns
/// the same secret without replacing the durable invitation.
pub async fn create_invite(
    store: Arc<RuntimeStore>,
    identity: &Value,
    body: &studio_runtime_schema::multi_server_ingress::ManagementRequest,
    actor: &str,
    tailscale_user: Option<&str>,
) -> Result<Value, MultiServerError> {
    use studio_runtime_schema::multi_server_ingress::ManagementRequest;
    let ManagementRequest::CreateInvite {
        request_id,
        label,
        grant,
    } = body
    else {
        return Err(MultiServerError::Refused("Invalid pairing request".into()));
    };
    let request_id = request_id.as_str();
    if !valid_identifier(request_id) {
        return Err(MultiServerError::Refused("Invalid request ID".into()));
    }
    let origin = identity
        .get("origin")
        .and_then(Value::as_str)
        .and_then(|value| crate::remote_transport::validate_federation_origin(value).ok())
        .ok_or_else(|| {
            MultiServerError::Refused("Configure Tailscale Serve HTTPS before pairing".into())
        })?;
    let saved_label = identity
        .get("label")
        .and_then(Value::as_str)
        .ok_or_else(|| MultiServerError::Refused("The local server label is unavailable".into()))?;
    let label = match label.value() {
        None => saved_label,
        Some(value) => {
            let label = value.as_str().trim();
            if label.is_empty() || label.chars().count() > 80 {
                return Err(MultiServerError::Refused(
                    "Server label must be 1 to 80 characters".into(),
                ));
            }
            label
        }
    };
    let grant = grant.value().map(|value| value.as_str());
    let tailscale_user = tailscale_user
        .filter(|user| !user.is_empty() && user.len() <= 254)
        .ok_or_else(|| {
            MultiServerError::Refused("The Tailscale owner identity is unavailable".into())
        })?;
    let identity_root = store.root().to_path_buf();
    let stored = studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/multi_server",
        move || crate::remote_transport::read_multi_server_identity(&identity_root),
    )
    .await
    .map_err(|_| MultiServerError::Refused("The local server credentials are unavailable".into()))?
    .map_err(|_| {
        MultiServerError::Refused("The local server credentials are unavailable".into())
    })?;
    let fingerprint = invite_fingerprint(body)?;
    let request_id_for_read = request_id.to_owned();
    let prior = store
        .read_async("runtime-services/multi_server", move |tx| {
            let saved: Option<(String, String, String)> = tx
                .query_row(
                    "SELECT id,hash,record FROM runtime_access_invites WHERE request_id=?1",
                    [&request_id_for_read],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            saved
                .map(|(id, hash, raw)| decode_invite(&id, &raw).map(|record| (hash, record)))
                .transpose()
        })
        .await?;
    let invite = if let Some(invite) = prior {
        if invite.0 != fingerprint {
            return Err(MultiServerError::Conflict(
                "The request ID has different content".into(),
            ));
        }
        invite.1
    } else {
        let now = unix_time()?;
        let invite_id = uuid::Uuid::new_v4().simple().to_string();
        let token_data = format!("invite-v1\n{invite_id}\n{request_id}");
        let token = URL_SAFE_NO_PAD.encode(
            hmac::sign(
                &hmac::Key::new(hmac::HMAC_SHA256, stored.private_key.as_bytes()),
                token_data.as_bytes(),
            )
            .as_ref(),
        );
        use studio_runtime_schema::{multi_server_protocol::InviteStatus, queue_command::Field};
        let invalid_invite =
            |_| MultiServerError::Refused("Invalid local invitation identity".into());
        let optional_identity = |name: &str| {
            <Field<String> as serde::Deserialize>::deserialize(
                identity.get(name).unwrap_or(&Value::Null),
            )
            .map_err(|_| MultiServerError::Refused("Invalid local invitation identity".into()))
        };
        let invite = PersistedAccessInvite {
            protocol: Field::Value(ACCESS_PROTOCOL_VERSION),
            invite_id: invite_id.clone().try_into().map_err(invalid_invite)?,
            server_id: optional_identity("serverId")?,
            label: Field::Value(label.to_owned()),
            origin: Field::Value(origin.to_owned()),
            public_key: optional_identity("publicKey")?,
            tailscale_user: tailscale_user
                .to_owned()
                .try_into()
                .map_err(invalid_invite)?,
            expires: serde_json::Number::from(now as i64 + INVITATION_TTL_SECONDS)
                .try_into()
                .map_err(invalid_invite)?,
            created: Field::Value(serde_json::Number::from_f64(now).ok_or_else(|| {
                MultiServerError::Refused("Invalid local invitation time".into())
            })?),
            status: InviteStatus::Open,
            token_hash: studio_runtime_store::sha256_hex(token.as_bytes())
                .try_into()
                .map_err(invalid_invite)?,
            grant: grant.map_or(Field::Missing, |value| Field::Value(value.to_owned())),
        };
        let invite_id = invite_id.clone();
        let request_id = request_id.to_owned();
        let fingerprint = fingerprint.clone();
        let invite_for_write = invite.clone();
        let actor = actor.to_owned();
        store
            .transaction_with(move |tx| {
                tx.execute(
                "INSERT INTO runtime_access_invites(id,request_id,hash,record) VALUES(?1,?2,?3,?4)",
                rusqlite::params![invite_id, request_id, fingerprint, serde_json::to_string(&invite_for_write)?],
            )?;
                audit_write(tx, "local", &actor, "create_invite")?;
                Ok::<_, StoreError>(())
            })
            .await?;
        invite
    };
    let invite_id = invite.invite_id();
    let token_data = format!("invite-v1\n{invite_id}\n{request_id}");
    let token = URL_SAFE_NO_PAD.encode(
        hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, stored.private_key.as_bytes()),
            token_data.as_bytes(),
        )
        .as_ref(),
    );
    let invitation = json!({
        "protocol": invite.protocol, "inviteId": invite.invite_id,
        "serverId": invite.server_id, "label": invite.label,
        "origin": invite.origin, "publicKey": invite.public_key,
        "tailscaleUser": invite.tailscale_user, "expires": invite.expires,
        "token": token,
    });
    Ok(json!({
        "invitation": invitation,
        "expires": invite.expires
    }))
}

/// Preserve the canonical JSON bytes used by stored invite fingerprints before
/// request bodies became typed, including omitted fields and explicit nulls.
fn invite_fingerprint(
    body: &studio_runtime_schema::multi_server_ingress::ManagementRequest,
) -> Result<String, MultiServerError> {
    let value = serde::Serialize::serialize(body, serde_json::value::Serializer)
        .map_err(|_| MultiServerError::Refused("Invalid pairing request".into()))?;
    let canonical = crate::remote_transport::canonical_json(&value)
        .map_err(|_| MultiServerError::Refused("Invalid pairing request".into()))?;
    Ok(studio_runtime_store::sha256_hex(canonical.as_bytes()))
}

fn project_records(tx: &rusqlite::Transaction<'_>, table: &str) -> Result<Vec<Value>, StoreError> {
    let q = format!("SELECT id,record FROM {table} ORDER BY id");
    let mut st = tx.prepare_cached(&q)?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.map(|row| {
        let (id, raw) = row?;
        let client = decode_client(table, &id, &raw)?;
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), json!(client.id));
        obj.insert("clientId".into(), json!(client.client_id));
        obj.insert("kind".into(), json!(client.kind));
        obj.insert("label".into(), json!(client.label));
        obj.insert("created".into(), json!(client.created));
        obj.insert("publicKey".into(), json!(client.public_key));
        obj.insert("status".into(), json!(client.status));
        macro_rules! optional {
            ($name:literal, $field:expr) => {
                match $field {
                    Field::Missing => {}
                    Field::Null => {
                        obj.insert($name.into(), Value::Null);
                    }
                    Field::Value(value) => {
                        obj.insert($name.into(), json!(value));
                    }
                }
            };
        }
        optional!("serverId", client.server_id);
        optional!("origin", client.origin);
        optional!("tailscaleUser", client.tailscale_user);
        optional!("lastAccess", client.last_access);
        optional!("revoked", client.revoked);
        optional!("lastSeen", client.last_seen);
        optional!("autoPair", client.auto_pair);
        optional!("reachability", client.reachability);
        optional!("alias", client.alias);
        optional!("replacedBy", client.replaced_by);
        Ok(Value::Object(obj))
    })
    .collect()
}
fn ensure_server_aliases(
    tx: &rusqlite::Transaction<'_>,
    identity: &Value,
) -> Result<BTreeMap<String, String>, StoreError> {
    server_aliases(tx, identity, true)
}

fn server_aliases(
    tx: &rusqlite::Transaction<'_>,
    identity: &Value,
    persist_default_aliases: bool,
) -> Result<BTreeMap<String, String>, StoreError> {
    let local_alias = tx
        .query_row(
            "SELECT alias FROM runtime_access_local_alias WHERE id=1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| "LOC".to_owned());
    let mut clients = project_records(tx, "runtime_access_clients")?;
    let mut aliases = BTreeMap::from([
        ("local".to_owned(), local_alias.to_owned()),
        (
            identity["serverId"].as_str().unwrap_or_default().to_owned(),
            local_alias.to_owned(),
        ),
    ]);
    let mut used = BTreeSet::from([local_alias.to_owned()]);
    for client in clients
        .iter()
        .filter(|client| client.get("kind").and_then(Value::as_str) == Some("server"))
    {
        if let Some(alias) = client.get("alias").and_then(Value::as_str) {
            used.insert(alias.to_owned());
        }
    }
    for client in &mut clients {
        if client.get("kind").and_then(Value::as_str) != Some("server") {
            continue;
        }
        let Some(id) = client.get("id").and_then(Value::as_str).map(str::to_owned) else {
            continue;
        };
        let alias = if let Some(alias) = client.get("alias").and_then(Value::as_str) {
            alias.to_owned()
        } else {
            let alias = default_server_alias(client, &mut used);
            client["alias"] = json!(alias);
            if persist_default_aliases {
                // Set only the alias: the stored record keeps the fields that the projection omits.
                tx.execute(
                    "UPDATE runtime_access_clients SET record=json_set(record,'$.alias',?1) WHERE id=?2",
                    rusqlite::params![alias, id],
                )?;
            }
            alias
        };
        aliases.insert(id, alias);
    }
    Ok(aliases)
}

fn default_server_alias(server: &Value, used: &mut BTreeSet<String>) -> String {
    let hostname = server
        .get("origin")
        .and_then(Value::as_str)
        .and_then(|origin| url::Url::parse(origin).ok())
        .and_then(|origin| origin.host_str().map(str::to_owned))
        .unwrap_or_default();
    let preferred = [
        hostname.split('.').next().unwrap_or_default(),
        server["label"].as_str().unwrap_or_default(),
    ]
    .into_iter()
    .map(|value| {
        value
            .chars()
            .filter(char::is_ascii_alphabetic)
            .take(3)
            .collect::<String>()
            .to_ascii_uppercase()
    })
    .find(|value| !value.is_empty())
    .unwrap_or_else(|| "SRV".to_owned());
    if used.insert(preferred.clone()) {
        return preferred;
    }
    for number in 0..26usize.pow(3) {
        let alias = [number / 676, (number / 26) % 26, number % 26]
            .into_iter()
            .map(|letter| char::from(b'A' + letter as u8))
            .collect::<String>();
        if used.insert(alias.clone()) {
            return alias;
        }
    }
    "SRV".to_owned()
}

fn stored(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    id: &str,
) -> Result<Option<PersistedAccessClient>, StoreError> {
    tx.query_row(
        &format!("SELECT record FROM {table} WHERE id=?1"),
        [id],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|raw| decode_client(table, id, &raw))
    .transpose()
}
fn decode_client(table: &str, id: &str, raw: &str) -> Result<PersistedAccessClient, StoreError> {
    let client = serde_json::from_str::<
        studio_runtime_schema::http_ingress::Object<PersistedAccessClient>,
    >(raw)
    .map_err(|_| invalid(table, id, "record"))?
    .0;
    Ok(client)
}
fn decode_invite(id: &str, raw: &str) -> Result<PersistedAccessInvite, StoreError> {
    let invite = serde_json::from_str::<
        studio_runtime_schema::http_ingress::Object<PersistedAccessInvite>,
    >(raw)
    .map_err(|_| invalid("runtime_access_invites", id, "record"))?
    .0;
    if invite.invite_id() != id {
        return Err(invalid("runtime_access_invites", id, "inviteId"));
    }
    Ok(invite)
}
fn invalid(t: &str, id: &str, field: &str) -> StoreError {
    StoreError::SchemaMismatch(format!("invalid stored object {t}/{id}: {field}"))
}
fn audit_write(
    tx: &rusqlite::Transaction<'_>,
    client: &str,
    actor: &str,
    action: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO runtime_access_audit(client,actor,action,created) VALUES(?1,?2,?3,?4)",
        rusqlite::params![client, actor, action, unix_time()?],
    )?;
    Ok(())
}
fn unix_time() -> Result<f64, StoreError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| StoreError::SchemaMismatch(e.to_string()))?
        .as_secs_f64())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::MultiServerError;
    use crate::federation_credentials::Credentials;
    use serde::Deserialize;
    use serde_json::json;
    use std::sync::Arc;
    use studio_runtime_store::RuntimeStore;

    async fn create_invite(
        store: Arc<RuntimeStore>,
        identity: &serde_json::Value,
        body: &serde_json::Value,
        actor: &str,
        user: Option<&str>,
    ) -> Result<serde_json::Value, MultiServerError> {
        let request =
            studio_runtime_schema::multi_server_ingress::ManagementRequest::deserialize(body)
                .map_err(|_| MultiServerError::Refused("invalid test request".into()))?;
        super::create_invite(store, identity, &request, actor, user).await
    }
    async fn manage(
        store: &RuntimeStore,
        identity: &serde_json::Value,
        body: &serde_json::Value,
        actor: &str,
    ) -> Result<serde_json::Value, MultiServerError> {
        let request =
            studio_runtime_schema::multi_server_ingress::ManagementRequest::deserialize(body)
                .map_err(|_| MultiServerError::Refused("invalid test request".into()))?;
        super::manage(store, identity, &request, actor).await
    }

    #[test]
    fn stored_invite_fingerprint_matches_legacy_canonical_json() {
        use studio_runtime_schema::multi_server_ingress::ManagementRequest;

        let wire =
            r#"{"requestId":"stored-1","label":"Café","grant":"observe","action":"create_invite"}"#;
        let old_body: serde_json::Value = serde_json::from_str(wire).unwrap();
        let typed: ManagementRequest = serde_json::from_str(wire).unwrap();
        let old_bytes = crate::remote_transport::canonical_json(&old_body).unwrap();
        assert_eq!(
            old_bytes,
            r#"{"action":"create_invite","grant":"observe","label":"Café","requestId":"stored-1"}"#
        );
        let old_hash = studio_runtime_store::sha256_hex(old_bytes.as_bytes());
        assert_eq!(
            old_hash,
            "016cd5bb32c6b282facf16337fee8493688172cab5a6bfbb0f1d1fa4af3360ca"
        );
        assert_eq!(super::invite_fingerprint(&typed).unwrap(), old_hash);

        let missing = r#"{"action":"create_invite","requestId":"r"}"#;
        let null = r#"{"action":"create_invite","requestId":"r","label":null}"#;
        let missing_body: ManagementRequest = serde_json::from_str(missing).unwrap();
        let null_body: ManagementRequest = serde_json::from_str(null).unwrap();
        let serialized = serde_json::to_string(&missing_body).unwrap();
        let serialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            serialized,
            json!({"action":"create_invite","requestId":"r"})
        );
        for (wire, typed) in [(missing, &missing_body), (null, &null_body)] {
            let old: serde_json::Value = serde_json::from_str(wire).unwrap();
            let old = crate::remote_transport::canonical_json(&old).unwrap();
            assert_eq!(
                super::invite_fingerprint(typed).unwrap(),
                studio_runtime_store::sha256_hex(old.as_bytes())
            );
        }
        assert_ne!(
            super::invite_fingerprint(&missing_body).unwrap(),
            super::invite_fingerprint(&null_body).unwrap()
        );
    }

    #[tokio::test]
    async fn invitation_uses_injected_owner_identity_and_exact_retry_is_stable() {
        let root = tempfile::tempdir().expect("isolated multi-server state");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store opens"),
        );
        let credentials = Credentials::default();
        let identity = credentials
            .identity(Arc::clone(&store), Some("https://studio.example.ts.net"))
            .await
            .expect("fixture identity is created");
        let mut identity = identity;
        identity["label"] = json!("Saved server label");
        let request =
            json!({"action":"create_invite","requestId":"invite-test","label":"Test server"});
        let first = create_invite(
            Arc::clone(&store),
            &identity,
            &request,
            "local",
            Some("owner@example.test"),
        )
        .await
        .expect("fixture owner can create invitation");
        let retry = create_invite(
            Arc::clone(&store),
            &identity,
            &request,
            "local",
            Some("owner@example.test"),
        )
        .await
        .expect("exact invitation retry reuses durable invite");
        assert_eq!(first, retry);
        assert_eq!(first["invitation"]["tailscaleUser"], "owner@example.test");
        assert!(
            !first["invitation"]["token"]
                .as_str()
                .unwrap_or_default()
                .is_empty()
        );
        let stored_token_hash = store
            .read_async("runtime-services/multi_server/tests", |tx| {
                tx.query_row(
                    "SELECT record FROM runtime_access_invites WHERE request_id='invite-test'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(studio_runtime_store::StoreError::from)
            })
            .await
            .expect("stored invitation reads");
        let stored_token_hash: serde_json::Value =
            serde_json::from_str(&stored_token_hash).expect("stored invitation JSON");
        assert_eq!(
            data_encoding::HEXLOWER
                .decode(
                    stored_token_hash["tokenHash"]
                        .as_str()
                        .expect("stored token hash")
                        .as_bytes()
                )
                .expect("hexadecimal token hash"),
            ring::digest::digest(
                &ring::digest::SHA256,
                first["invitation"]["token"]
                    .as_str()
                    .expect("invitation token")
                    .as_bytes()
            )
            .as_ref()
        );
        let conflict = json!({"action":"create_invite","requestId":"invite-test","label":"Other"});
        assert!(
            create_invite(
                Arc::clone(&store),
                &identity,
                &conflict,
                "local",
                Some("owner@example.test"),
            )
            .await
            .is_err()
        );
        let count = store
            .read_async("runtime-services/multi_server/tests", |tx| {
                tx.query_row("SELECT count(*) FROM runtime_access_invites", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(studio_runtime_store::StoreError::from)
            })
            .await
            .expect("invite count reads");
        assert_eq!(count, 1);
        let default_label = create_invite(
            Arc::clone(&store),
            &identity,
            &json!({"action":"create_invite","requestId":"invite-default-label"}),
            "local",
            Some("owner@example.test"),
        )
        .await
        .expect("Python default label is used when label is omitted");
        assert_eq!(default_label["invitation"]["label"], identity["label"]);
        let null_label = create_invite(
            Arc::clone(&store),
            &identity,
            &json!({"action":"create_invite","requestId":"invite-null-label","label":null}),
            "local",
            Some("owner@example.test"),
        )
        .await
        .expect("null label uses the saved identity label");
        assert_eq!(null_label["invitation"]["label"], identity["label"]);
    }

    #[tokio::test]
    async fn peer_alias_mutation_is_local_idempotent_and_rejects_collisions() {
        let root = tempfile::tempdir().expect("isolated multi-server state");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store opens"),
        );
        let credentials = Credentials::default();
        let identity = credentials
            .identity(Arc::clone(&store), Some("https://local.example.ts.net"))
            .await
            .expect("fixture identity is created");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_access_clients(id,record) VALUES(?1,?2),(?3,?4)",
                    rusqlite::params![
                        "peer-a",
                        json!({"id":"peer-a","clientId":"peer-a","kind":"server","status":"paired","label":"Peer A","created":1,"publicKey":"peer-key-a","alias":"PEE"}).to_string(),
                        "peer-b",
                        json!({"id":"peer-b","clientId":"peer-b","kind":"server","status":"paired","label":"Peer B","created":1,"publicKey":"peer-key-b","alias":"OTH"}).to_string(),
                    ],
                )?;
                tx.execute(
                    "INSERT INTO runtime_access_clients(id,record) VALUES(?1,?2)",
                    rusqlite::params![
                        "peer-c",
                        json!({"id":"peer-c","clientId":"peer-c","kind":"server","status":"paired","label":"One","origin":"https://one.example.ts.net","created":1,"publicKey":"peer-key-c"}).to_string(),
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("paired peer fixtures insert");

        let request =
            json!({"action":"alias","serverId":"peer-a","alias":"NEW","requestId":"peer-alias"});
        let first = manage(&store, &identity, &request, "local")
            .await
            .expect("a paired peer alias updates local state");
        assert_eq!(first["settings"]["aliases"]["peer-a"], "NEW");
        assert_eq!(first["settings"]["aliases"]["peer-c"], "ONE");
        let retry = manage(&store, &identity, &request, "local")
            .await
            .expect("exact alias retry returns the saved state");
        assert_eq!(retry, first);

        let changed =
            json!({"action":"alias","serverId":"peer-a","alias":"TWO","requestId":"peer-alias"});
        assert!(matches!(
            manage(&store, &identity, &changed, "local").await,
            Err(super::MultiServerError::Conflict(_))
        ));
        let collision = json!({"action":"alias","serverId":"peer-a","alias":"OTH","requestId":"peer-alias-collision"});
        assert!(matches!(
            manage(&store, &identity, &collision, "local").await,
            Err(super::MultiServerError::Conflict(_))
        ));
        let default_collision = json!({"action":"alias","serverId":"peer-a","alias":"ONE","requestId":"peer-alias-default-collision"});
        assert!(matches!(
            manage(&store, &identity, &default_collision, "local").await,
            Err(super::MultiServerError::Conflict(_))
        ));
        let missing = json!({"action":"alias","serverId":"unknown-peer","alias":"NEW","requestId":"peer-alias-missing"});
        assert!(manage(&store, &identity, &missing, "local").await.is_err());

        let audit_count = store
            .read_async("runtime-services/multi_server/tests", |tx| {
                tx.query_row(
                    "SELECT count(*) FROM runtime_access_audit WHERE client='peer-a' AND action='alias'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(studio_runtime_store::StoreError::from)
            })
            .await
            .expect("alias audit count reads");
        assert_eq!(audit_count, 1);
    }

    #[tokio::test]
    async fn default_peer_alias_is_persisted_before_a_lexically_earlier_peer_arrives() {
        let root = tempfile::tempdir().expect("isolated multi-server state");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store opens"),
        );
        let identity = crate::federation_credentials::Credentials::default()
            .identity(Arc::clone(&store), Some("https://local.example.ts.net"))
            .await
            .expect("fixture identity is created");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_access_clients(id,record) VALUES(?1,?2)",
                    rusqlite::params![
                        "peer-b",
                        json!({"id":"peer-b","clientId":"peer-b","kind":"server","status":"paired","label":"Bob","created":1,"publicKey":"peer-key-b","origin":"https://bob.example.ts.net"}).to_string(),
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("first paired peer inserts");

        let first = super::snapshot(&store, &identity)
            .await
            .expect("snapshot seeds peer alias");
        assert_eq!(first["settings"]["aliases"]["peer-b"], "BOB");
        let saved: String = store
            .read_async("runtime-services/multi_server/tests", |tx| {
                tx.query_row(
                    "SELECT json_extract(record,'$.alias') FROM runtime_access_clients WHERE id='peer-b'",
                    [],
                    |row| row.get(0),
                )
                .map_err(studio_runtime_store::StoreError::from)
            })
            .await
            .expect("default alias is durably saved");
        assert_eq!(saved, "BOB");

        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_access_clients(id,record) VALUES(?1,?2)",
                    rusqlite::params![
                        "peer-a",
                        json!({"id":"peer-a","clientId":"peer-a","kind":"server","status":"paired","label":"Bob earlier","created":1,"publicKey":"peer-key-a","origin":"https://bob.example.ts.net"}).to_string(),
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("lexically earlier peer inserts");

        let later = super::snapshot(&store, &identity)
            .await
            .expect("snapshot preserves saved aliases");
        assert_eq!(later["settings"]["aliases"]["peer-b"], "BOB");
        assert_eq!(later["settings"]["aliases"]["peer-a"], "AAA");
    }

    #[tokio::test]
    async fn comparison_snapshot_does_not_persist_default_aliases() {
        let root = tempfile::tempdir().expect("isolated multi-server state");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store opens"),
        );
        let identity = crate::federation_credentials::Credentials::default()
            .identity(Arc::clone(&store), Some("https://local.example.ts.net"))
            .await
            .expect("fixture identity is created");
        store
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO runtime_access_clients(id,record) VALUES(?1,?2)",
                    rusqlite::params![
                        "peer-b",
                        json!({"id":"peer-b","clientId":"peer-b","kind":"server","status":"paired","label":"Bob","created":1,"publicKey":"peer-key-b","origin":"https://bob.example.ts.net"}).to_string(),
                    ],
                )?;
                Ok(())
            })
            .await
            .expect("fixture peer inserts");

        let snapshot = super::snapshot_for_comparison(&store, &identity)
            .await
            .expect("comparison snapshot computes default alias");
        assert_eq!(snapshot["settings"]["aliases"]["peer-b"], "BOB");
        let saved: Option<String> = store
            .read_async("runtime-services/multi_server/tests", |tx| {
                tx.query_row(
                    "SELECT json_extract(record,'$.alias') FROM runtime_access_clients WHERE id='peer-b'",
                    [],
                    |row| row.get(0),
                )
                .map_err(studio_runtime_store::StoreError::from)
            })
            .await
            .expect("alias field remains absent");
        assert_eq!(saved, None);
    }
}
