//! Discovery consumes injected status/probe observations. It never invokes
//! Tailscale, probes a host, or restores a revoked credential.
use futures_util::future::BoxFuture;
use std::collections::BTreeSet;
use std::sync::Arc;

use rusqlite::params;
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;
use studio_runtime_schema::{
    multi_server_access::{AccessClientKind, AccessClientStatus, PersistedAccessClient},
    multi_server_protocol::NonNegativeNumber,
    queue_command::Field as PresentField,
};
use studio_runtime_store::RuntimeStore;

use crate::multi_server_auth::{
    AccessError, MAX_OBSERVATION_BYTES, discovery_identity, origin_host, refused, stored_client,
    text,
};
use crate::multi_server_pairing::read_record;

pub const MAX_CANDIDATES: usize = 64;
pub const DISCOVERY_PORTS: [Option<u16>; 2] = [None, Some(8443)];
const MAX_DISCOVERED: i64 = 256;

/// Typed stored peer used by the discovery driver.
#[derive(serde::Serialize)]
#[serde(transparent)]
pub struct StoredPeer(PersistedAccessClient);

impl StoredPeer {
    pub fn from_raw(raw: &str) -> Option<Self> {
        serde_json::from_str::<studio_runtime_schema::http_ingress::Object<PersistedAccessClient>>(
            raw,
        )
        .ok()
        .map(|value| Self(value.0))
    }
    pub fn origin(&self) -> &str {
        self.0.origin.value().map_or("", String::as_str)
    }
    pub fn public_key(&self) -> &str {
        self.0.public_key.as_str()
    }
    pub fn is_paired(&self) -> bool {
        self.0.status == AccessClientStatus::Paired
    }
    pub fn is_revoked(&self) -> bool {
        self.0.status == AccessClientStatus::Revoked
    }
    pub fn created(&self) -> &NonNegativeNumber {
        &self.0.created
    }
    pub fn record(&self) -> &PersistedAccessClient {
        &self.0
    }
    pub fn mark_unreachable(&mut self) {
        self.0.status = AccessClientStatus::Unreachable;
    }
    pub fn set_label(&mut self, label: String) -> Result<(), &'static str> {
        self.0.label = label.try_into().map_err(|_| "invalid label")?;
        Ok(())
    }
}

// Tailscale observations can contain malformed individual peer fields. Those
// fields have always been treated as absent, while malformed tags fail closed.
#[derive(Deserialize)]
#[serde(untagged)]
enum Field<T> {
    Valid(T),
    Other(IgnoredAny),
}
impl<T> Field<T> {
    fn value(&self) -> Option<&T> {
        match self {
            Self::Valid(value) => Some(value),
            Self::Other(_) => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Tags {
    Null(()),
    List(Vec<IgnoredAny>),
    Other(IgnoredAny),
}
impl Tags {
    fn tagged(&self) -> bool {
        match self {
            Self::Null(()) => false,
            Self::List(tags) => !tags.is_empty(),
            Self::Other(_) => true,
        }
    }
}

#[derive(Deserialize)]
struct DiscoveryNode {
    #[serde(rename = "UserID")]
    user: Option<Field<u64>>,
    #[serde(rename = "DNSName")]
    dns: Option<Field<String>>,
    #[serde(rename = "Online")]
    online: Option<Field<bool>>,
    #[serde(rename = "Tags")]
    tags: Option<Tags>,
}
impl DiscoveryNode {
    fn from_object(value: &Value) -> Option<Self> {
        value.as_object()?;
        Self::deserialize(value).ok()
    }
    fn tagged(&self) -> bool {
        self.tags.as_ref().is_some_and(Tags::tagged)
    }
}

/// Host-owned discovery adapter. The HTTP and service layers pass the
/// persisted server identity and public Serve origin; tests inject a fake
/// adapter and never consult the machine's Tailscale installation.
pub trait ServerDiscoveryPort: Send + Sync {
    fn discover<'a>(
        &'a self,
        store: Arc<RuntimeStore>,
        identity: &'a Value,
        public_origin: &'a str,
    ) -> BoxFuture<'a, Result<Value, AccessError>>;
}

#[derive(Clone)]
pub struct DiscoverySnapshot {
    owner: String,
    local_id: String,
    candidates: BTreeSet<String>,
}
impl DiscoverySnapshot {
    /// Checked constructor from a Tailscale status observation. The owner login
    /// and candidates come from the same observation, never caller headers.
    pub fn from_status(
        status: &Value,
        local_id: &str,
        local_origin: &str,
    ) -> Result<Self, AccessError> {
        origin_host(local_origin)?;
        let invalid = || {
            refused(
                503,
                "identity_unavailable",
                "The Tailscale peers are unavailable",
            )
        };
        if status.to_string().len() > MAX_OBSERVATION_BYTES {
            return Err(invalid());
        }
        let node = status
            .get("Self")
            .and_then(DiscoveryNode::from_object)
            .ok_or_else(invalid)?;
        let peers = status
            .get("Peer")
            .and_then(Value::as_object)
            .ok_or_else(invalid)?;
        let user = node
            .user
            .as_ref()
            .and_then(Field::value)
            .copied()
            .filter(|v| *v > 0)
            .ok_or_else(invalid)?;
        if text(status, "BackendState") != "Running" || node.tagged() {
            return Err(invalid());
        }
        let owner = status
            .get("User")
            .and_then(|v| v.get(user.to_string()))
            .map(|v| text(v, "LoginName"))
            .filter(|v| !v.trim().is_empty() && v.len() <= 254)
            .ok_or_else(invalid)?
            .trim()
            .to_lowercase();
        let mut candidates = BTreeSet::new();
        for (is_local, peer) in std::iter::once((true, Some(node))).chain(
            peers
                .values()
                .map(|v| (false, DiscoveryNode::from_object(v))),
        ) {
            let Some(peer) = peer else { continue };
            if (!is_local && peer.online.as_ref().and_then(Field::value) != Some(&true))
                || peer.tagged()
                || peer.user.as_ref().and_then(Field::value) != Some(&user)
            {
                continue;
            }
            let hostname = peer
                .dns
                .as_ref()
                .and_then(Field::value)
                .map_or("", String::as_str)
                .trim_end_matches('.')
                .to_lowercase();
            for port in DISCOVERY_PORTS {
                let authority =
                    port.map_or_else(|| hostname.clone(), |port| format!("{hostname}:{port}"));
                let origin = format!("https://{authority}");
                if origin != local_origin && origin_host(&origin).is_ok() {
                    candidates.insert(origin);
                }
            }
        }
        Ok(Self {
            owner,
            local_id: local_id.to_owned(),
            candidates,
        })
    }
    pub fn candidates(&self) -> Vec<&str> {
        self.candidates
            .iter()
            .take(MAX_CANDIDATES)
            .map(String::as_str)
            .collect()
    }
    pub fn has_candidate(&self, origin: &str) -> bool {
        self.candidates.contains(origin)
    }
    pub fn owner(&self) -> &str {
        &self.owner
    }
}
pub struct DiscoveryDecision {
    record: Option<PersistedAccessClient>,
    auto_pair: bool,
}
impl DiscoveryDecision {
    pub fn record(&self) -> Option<&PersistedAccessClient> {
        self.record.as_ref()
    }
    pub fn auto_pair(&self) -> bool {
        self.auto_pair
    }
}

/// A signed status refresh must already have been applied to `peer` by the
/// observer for existing paired servers; unsigned probes cannot rename them.
pub fn decide_probe(
    snapshot: &DiscoverySnapshot,
    origin: &str,
    peer: &Value,
    previous: Option<&str>,
    client: Option<&str>,
    auto_pair: bool,
    now: i64,
) -> Result<DiscoveryDecision, AccessError> {
    let identity = discovery_identity(peer)?;
    if !snapshot.candidates.contains(origin)
        || identity.origin.as_deref().unwrap_or_default() != origin
    {
        return Err(refused(
            403,
            "identity_mismatch",
            "The peer identity is invalid",
        ));
    }
    let id = identity.server_id.as_str();
    let previous = previous
        .map(|r| stored_client("runtime_access_discovered", id, r))
        .transpose()?;
    let client = client
        .map(|r| stored_client("runtime_access_clients", id, r))
        .transpose()?;
    if id == snapshot.local_id
        || identity
            .tailscale_user
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase()
            != snapshot.owner
        || client.as_ref().is_some_and(|v| {
            v.public_key.as_str() != identity.public_key
                || v.origin.value().map(String::as_str) != Some(origin)
        })
    {
        return Ok(DiscoveryDecision {
            record: None,
            auto_pair: false,
        });
    }
    let label = client
        .as_ref()
        .filter(|v| v.status == AccessClientStatus::Paired)
        .map(|v| v.label.clone())
        .map(Ok)
        .unwrap_or_else(|| identity.label.clone().try_into())
        .map_err(|_| refused(400, "invalid_peer", "The peer identity is invalid"))?;
    let created = previous
        .as_ref()
        .map(|v| v.created.clone())
        .map(Ok)
        .unwrap_or_else(|| NonNegativeNumber::try_from(serde_json::Number::from(now)))
        .map_err(|_| refused(400, "invalid_peer", "The peer identity is invalid"))?;
    let invalid = || refused(400, "invalid_peer", "The peer identity is invalid");
    let record = PersistedAccessClient {
        id: id.to_owned().try_into().map_err(|_| invalid())?,
        client_id: id.to_owned().try_into().map_err(|_| invalid())?,
        server_id: PresentField::Value(id.to_owned()),
        kind: AccessClientKind::Server,
        label,
        origin: PresentField::Value(origin.to_owned()),
        public_key: identity
            .public_key
            .clone()
            .try_into()
            .map_err(|_| invalid())?,
        tailscale_user: PresentField::Value(snapshot.owner.clone()),
        status: AccessClientStatus::Discovered,
        created,
        last_access: PresentField::Null,
        revoked: PresentField::Null,
        last_seen: PresentField::Value(now.into()),
        auto_pair: PresentField::Value(identity.auto_pair),
        reachability: PresentField::Missing,
        alias: PresentField::Missing,
        replaced_by: PresentField::Missing,
        grant: PresentField::Missing,
        pairing_request_id: PresentField::Missing,
        pairing_hash: PresentField::Missing,
        pairing_receipt: PresentField::Missing,
    };
    Ok(DiscoveryDecision {
        record: Some(record),
        auto_pair: client.is_none() && auto_pair && identity.auto_pair,
    })
}

/// Apply one bounded observation. Auto pairing is returned as a proposal for
/// the host driver, never performed by an unsigned discovery observation.
pub async fn record_probe(
    store: &RuntimeStore,
    snapshot: &DiscoverySnapshot,
    origin: &str,
    peer: &Value,
    now: i64,
) -> Result<DiscoveryDecision, AccessError> {
    let snapshot = snapshot.clone();
    let origin = origin.to_owned();
    let peer = peer.clone();
    store.transaction_with(move |tx| {
        let id=text(&peer,"serverId");
        let previous=read_record(tx,"runtime_access_discovered",id)?;
        let client=read_record(tx,"runtime_access_clients",id)?;
        let enabled:bool=tx.query_row("SELECT auto_pair FROM runtime_access_settings WHERE id=1",[],|r|r.get(0))?;
        let decision=decide_probe(&snapshot,&origin,&peer,previous.as_deref(),client.as_deref(),enabled,now)?;
        if let Some(record)=&decision.record {
            let raw=serde_json::to_string(record).map_err(|_| refused(500,"invalid_peer","The peer identity is invalid"))?;
            tx.execute("INSERT INTO runtime_access_discovered(id,record) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET record=excluded.record",params![id,raw])?;
            tx.execute("DELETE FROM runtime_access_discovered WHERE id IN (SELECT id FROM runtime_access_discovered ORDER BY json_extract(record,'$.lastSeen') DESC,id LIMIT -1 OFFSET ?1)",[MAX_DISCOVERED])?;
        }
        Ok(decision)
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn discovery_tolerates_bad_peer_fields_and_rejects_tags() {
        let status = json!({
            "BackendState": "Running",
            "Self": {"UserID": 1, "DNSName": "local.test.ts.net."},
            "User": {"1": {"LoginName": "owner@example.test"}},
            "Peer": {
                "valid": {"UserID": 1, "DNSName": "peer.test.ts.net.", "Online": true, "Tags": []},
                "bad-online": {"UserID": 1, "DNSName": "bad.test.ts.net", "Online": "true"},
                "bad-tags": {"UserID": 1, "DNSName": "tagged.test.ts.net", "Online": true, "Tags": {}},
                "array": [1, "array.test.ts.net", true]
            }
        });
        let snapshot =
            DiscoverySnapshot::from_status(&status, "local", "https://local.test.ts.net").unwrap();
        assert_eq!(
            snapshot.candidates(),
            vec![
                "https://local.test.ts.net:8443",
                "https://peer.test.ts.net",
                "https://peer.test.ts.net:8443"
            ]
        );
        let mut invalid = status;
        invalid["Self"] = json!([1, "local.test.ts.net"]);
        assert!(
            DiscoverySnapshot::from_status(&invalid, "local", "https://local.test.ts.net").is_err()
        );
    }
}
