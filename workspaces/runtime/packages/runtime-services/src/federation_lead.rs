//! Receive side of `LeadMessage` (decision 2026-11-01, "Leads in one team").
//!
//! A paired lead of another server sends text to the root lead of a local project. The
//! sender is authorized by its project grant (`ProjectPeer` with `LeadMessages`), not by a
//! federation peer or room. The text enters the receiving lead as untrusted input and
//! wakes it like a child report.

use std::collections::BTreeSet;
use std::sync::Arc;

use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use studio_json::{Value, json};
use studio_runtime_store::federation_messaging::{
    FederationMessagingRepository as Repo, FederationTable as Table,
};
use studio_runtime_store::project_grants::{self, Capability};
use studio_runtime_store::{AgentId, AgentRepository, RuntimeStore, StoreError};

use crate::event_write::{CallerAdmission, RequestKey, begin_request, settle_request};
use crate::federation_effects::{SignedRequest, consume_nonce, signed_response, snapshot};
use crate::federation_messaging::{
    self as core, FederationMessageError as Error, MessageReceipt, PROTOCOL, Result,
};
use crate::principal::{Access, CommandClass, Principal, Scope, access};
use crate::remote_transport as transport;

/// The event kind that the receiving root lead gets (an existing kind: `runtime_events.kind`
/// has a CHECK list). The text carries `source: peer_lead`; it is untrusted peer input.
pub(crate) const EVENT_KIND: &str = "agent_message";

/// True when the body carries a `lead-message` envelope. Only these take this path.
pub fn is_lead_envelope(body: &Value) -> bool {
    body["envelope"]["kind"] == "lead-message"
}

/// Receive one signed `lead-message` request. The request and the envelope are each
/// signed by the granted peer key. A replayed envelope returns the first receipt.
pub(crate) async fn receive(
    store: Arc<RuntimeStore>,
    request: &SignedRequest<'_>,
    body: Value,
    now: f64,
) -> Result<Value> {
    let identity = Arc::new(transport::load_federation_identity(&store).await?);
    let envelope = body["envelope"].clone();
    let message = core::checked_envelope(&envelope)?;
    if message.kind != core::MessageKind::LeadMessage {
        return Err(Error::Invalid("kind"));
    }
    if message.sender_server != request.peer_id {
        return Err(Error::Forbidden("sender identity mismatch"));
    }
    let signing = request.signing_bytes(identity.id())?;
    let request_signature = request.signature.to_owned();
    let request_nonce = request.nonce.to_owned();
    let request_timestamp = request.timestamp;
    let nonce_check = request.validate_nonce(now as i64);
    let peer = request.peer_id.to_owned();
    let local_id = identity.id().to_owned();
    let result = store
        .transaction_with(move |tx| {
            let rows = project_grants::active_rows(tx, &peer, &message.room)?;
            let Some(first) = rows.first() else {
                return Err(Error::Forbidden("no project grant"));
            };
            nonce_check?;
            transport::verify_signature(&first.peer_public_key, &signing, &request_signature)?;
            let capabilities: BTreeSet<Capability> =
                rows.iter().map(|row| row.capability).collect();
            let principal = Principal::ProjectPeer {
                peer: peer.clone(),
                project: message.room.clone(),
                capabilities,
            };
            if access(&principal, CommandClass::ChatControl) != Access::Scoped(Scope::PairedLead) {
                return Err(Error::Forbidden(
                    "lead messages need the LeadMessages grant",
                ));
            }
            transport::verify_signature(
                &first.peer_public_key,
                &transport::federation_message_signing_bytes(&envelope)?,
                &message.signature,
            )?;
            consume_nonce(tx, &peer, &request_nonce, request_timestamp, now as i64)?;
            consume_nonce(tx, &peer, &message.nonce, message.timestamp, now as i64)?;
            let hash = core::envelope_hash(&envelope)?;
            let raw = studio_runtime_store::multi_server_delivery::remote_result_key(
                "federation-lead-inbox",
                &[peer.as_str()],
                &message.id,
            );
            let admitted = match begin_request::<MessageReceipt>(
                tx,
                RequestKey::new(
                    "federation-lead-inbox",
                    &[peer.as_str()],
                    &message.request_id,
                )
                .raw(&raw),
                &hash,
                chrono::Utc::now().timestamp_millis(),
            )? {
                CallerAdmission::Fresh(admitted) => admitted,
                CallerAdmission::Replay(saved) => {
                    return Ok(saved);
                }
            };
            let grant = first.grant_id.clone();
            let lead = root_lead(tx, &message.room, &local_id)?
                .ok_or(Error::Forbidden("the project has no root lead"))?;
            let text = message.payload["text"]
                .as_str()
                .ok_or(Error::Invalid("payload.text"))?;
            let event = transport::canonical_json(&json!({
                "source": "peer_lead",
                "untrusted": true,
                "peerServer": peer,
                "grant": grant,
                "projectId": message.room,
                "text": text,
            }))?;
            crate::event_write::enqueue_recovery_event(
                tx,
                &lead,
                format!("peer-lead:{peer}:{}", message.id),
                EVENT_KIND,
                event,
                now,
            )?;
            let receipt = MessageReceipt {
                protocol: PROTOCOL as u32,
                message_id: message.id.clone(),
                accepted: true,
                room: message.room.clone(),
                sequence: None,
            };
            settle_request(tx, &admitted, None, &receipt)?;
            Ok(receipt)
        })
        .await?;
    signed_response(
        &identity,
        json!({"protocol": result.protocol, "messageId": result.message_id, "accepted": result.accepted, "room": result.room, "sequence": null}),
        now as i64,
    )
}

/// The accepted reply of one lead message. `id` is the outbox id of the message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadSendReply {
    pub id: String,
    pub grant: String,
}

/// Queue one lead message to the paired peer lead named by `grant`, in the caller's
/// transaction. Only the root lead of the grant's local project may send. The outbox row
/// is delivered later by the federation pump. A retry with the same `key` returns the
/// first reply and queues nothing.
pub(crate) fn send_in_transaction(
    tx: &Transaction<'_>,
    sender: &Value,
    grant: &str,
    text: &str,
    key: &str,
    now: f64,
) -> Result<LeadSendReply> {
    let root = sender["isLead"] == json!(true) && sender["parentId"].is_null();
    if !root {
        return Err(Error::Forbidden("only the root lead sends to a peer lead"));
    }
    let rows = project_grants::rows_of_grant(tx, grant)?;
    let first = rows
        .first()
        .ok_or(Error::Forbidden("the peer lead is unknown"))?;
    if first.revoked_at.is_some() {
        return Err(Error::Forbidden("the peer lead is no longer paired"));
    }
    if sender["projectId"] != json!(first.local_project) {
        return Err(Error::Forbidden("the peer lead belongs to another project"));
    }
    let text = crate::event_write::python_strip(text);
    let payload = json!({"text": text});
    core::validate("FederationLeadPayload", &payload, "payload")?;
    if key.len() > 4096 {
        return Err(Error::Invalid("request id"));
    }
    snapshot(tx, &first.peer, "", now)?.authorize(false)?;
    let state_id = local_state_id(tx)?;
    let namespace =
        uuid::Uuid::parse_str(&state_id).map_err(|_| Error::Invalid("identity.stateId"))?;
    let message_id = uuid::Uuid::new_v5(&namespace, key.as_bytes()).to_string();
    let sender_id = sender["id"].as_str().ok_or(Error::Invalid("agent.id"))?;
    let signature = crate::event_write::request_signature(&json!({
        "sender": sender_id,"grant": grant,"text": text,
    }));
    let admitted = match begin_request::<LeadSendReply>(
        tx,
        RequestKey::new("federation-lead-send", &[sender_id], key).raw(&message_id),
        &signature,
        chrono::Utc::now().timestamp_millis(),
    )? {
        CallerAdmission::Fresh(admitted) => admitted,
        CallerAdmission::Replay(saved) => return Ok(saved),
    };
    let immutable = json!({"protocol":PROTOCOL,"senderServer":state_id,"id":message_id,
        "kind":core::MessageKind::LeadMessage.as_str(),"room":first.remote_project,
        "payload":payload,"created":now});
    let delivery_signature = core::envelope_hash(&immutable)?;
    let peer = first.peer.as_str();
    let delivery_key = studio_runtime_store::multi_server_delivery::remote_result_key(
        "federation-outbox",
        &[peer],
        key,
    );
    let delivery_admission = match begin_request::<core::FederationDeliveryReply>(
        tx,
        RequestKey::new("federation-outbox", &[peer], key).raw(&delivery_key),
        &delivery_signature,
        chrono::Utc::now().timestamp_millis(),
    )? {
        CallerAdmission::Fresh(admitted) => admitted,
        CallerAdmission::Replay(_) => {
            return Err(Error::Conflict("message delivery already settled"));
        }
    };
    let item = json!({"id":message_id,"requestId":key,
        "kind":core::MessageKind::LeadMessage.as_str(),"room":first.remote_project,
        "payload":payload,"created":now,
        "status":studio_runtime_schema::persistence::FederationDeliveryState::Queued.as_str(),
        "attempts":0,"nextAt":0,"error":null,
        "admission":{"id":delivery_admission.id(),"signature":delivery_admission.signature(),
            "created":delivery_admission.created()}});
    let item = core::decide_queue(
        Repo::record(tx, Table::Outbox, &message_id, peer)?.as_ref(),
        item,
        Repo::queue_size(tx, peer)?,
    )?;
    Repo::put(tx, Table::Outbox, &message_id, peer, &item)?;
    let reply = LeadSendReply {
        id: message_id,
        grant: grant.to_owned(),
    };
    settle_request(tx, &admitted, Some(sender_id), &reply)?;
    Ok(reply)
}

/// Queue a message of the owner to the peer lead of `grant`. The sender is the root lead of
/// the grant's local project, the only lead that `send_in_transaction` accepts. `key` is the
/// command id, which dedupes the message.
pub(crate) fn lead_message_in_transaction(
    tx: &Transaction<'_>,
    grant: &str,
    text: &str,
    key: &str,
    now: f64,
) -> Result<LeadSendReply> {
    let rows = project_grants::rows_of_grant(tx, grant)?;
    let first = rows
        .first()
        .ok_or(Error::Forbidden("the peer lead is unknown"))?;
    let local_id = local_state_id(tx)?;
    let sender = root_lead(tx, &first.local_project, &local_id)?
        .ok_or(Error::Forbidden("the project has no root lead"))?
        .value();
    send_in_transaction(tx, &sender, grant, text, key, now)
}

/// The state id of this server's federation identity.
fn local_state_id(tx: &Transaction<'_>) -> Result<String> {
    tx.query_row(
        // Raw SQL: JSON selectors/projections are outside db_query.
        "SELECT json_extract(record,'$.stateId') FROM runtime_federation_identity WHERE id='local'",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(StoreError::from)?
    .ok_or(Error::Unavailable("federation-credentials/private-key"))
}

/// The root lead of a local project: a lead without a parent, bound to the project.
/// A row bound to another server (`projectServerId` set and not `local`) is not ours.
fn root_lead(
    tx: &Transaction<'_>,
    project: &str,
    local_id: &str,
) -> Result<Option<studio_runtime_store::AgentRecord>> {
    let id: Option<String> = tx
        .query_row(
            "SELECT id FROM runtime_agents WHERE deleted_at IS NULL \
             AND json_extract(record,'$.projectId')=?1 \
             AND json_extract(record,'$.isLead')=1 \
             AND json_extract(record,'$.parentId') IS NULL \
             AND (json_extract(record,'$.projectServerId') IS NULL OR json_extract(record,'$.projectServerId')=?2) \
             ORDER BY id LIMIT 1",
            params![project, local_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(studio_runtime_store::StoreError::from)?;
    match id {
        Some(id) => Ok(AgentRepository::get(tx, &AgentId::from(id.as_str()))?),
        None => Ok(None),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::federation_effects::{FederationAction, UnifiedFields};
    use crate::remote_transport::StoredMultiServerIdentity;
    use studio_runtime_store::project_grants::GrantRow;

    const LOCAL: &str = "11111111-1111-4111-8111-111111111111";
    const PEER: &str = "22222222-2222-4222-8222-222222222222";
    const NOW: f64 = 1_700_000_000.0;
    // Same key pairs as the federation corpus fixture.
    const LOCAL_PRIVATE: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIEwJtsrcWZOtVmTk9vLgk3Zl+8Q58joMUuCCp/nGwy/x\n-----END PRIVATE KEY-----\n";
    const LOCAL_PUBLIC: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEA04iOMwqCMPTabJroC0ZiAMqMk1I8hnJ3fDGB9jM+n9Q=\n-----END PUBLIC KEY-----\n";
    const PEER_PRIVATE: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIIqXlM698JXMmwsvhwSaoWuttKfmIUFYBr3RoCsa4eTc\n-----END PRIVATE KEY-----\n";
    const PEER_PUBLIC: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAGmnFFqggqjGtTZG67eDJH3j+m/LrPuPrNSLJyXG83Es=\n-----END PUBLIC KEY-----\n";

    fn peer_identity() -> transport::RemoteIdentity {
        StoredMultiServerIdentity {
            server_id: PEER.into(),
            label: "Peer".into(),
            private_key: PEER_PRIVATE.into(),
            public_key: PEER_PUBLIC.into(),
        }
        .signing_identity()
        .unwrap()
    }

    async fn store(name: &str) -> (tempfile::TempDir, Arc<RuntimeStore>) {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(
            RuntimeStore::open(Some(&root.path().join(name)))
                .await
                .unwrap(),
        );
        let local = json!({"stateId": LOCAL, "label": "Local",
            "privateKey": LOCAL_PRIVATE, "publicKey": LOCAL_PUBLIC});
        let lead = json!({"compactions":0,"accountKey":"default",
            "provider":studio_runtime_schema::ProviderKind::Codex,"id":"lead","rootId":"lead",
            "epoch":0,"status":"idle","name":"lead","autoWake":true,"isLead":true,
            "projectId":"proj","projectServerId":LOCAL});
        store
            .transaction(move |tx| {
                studio_runtime_store::receipt_schema::create(tx)?;
                studio_runtime_store::test_fixtures::canvas::seed_orchestration_settings(
                    tx,
                    r#"{"expired_before_ms":0}"#,
                )?;
                tx.execute(
                    "INSERT INTO runtime_federation_identity VALUES('local',?1)",
                    [local.to_string()],
                )?;
                tx.execute(
                    "INSERT INTO runtime_agents(id,record) VALUES('lead',?1)",
                    [lead.to_string()],
                )?;
                project_grants::insert_rows(
                    tx,
                    &[GrantRow {
                        grant_id: "g1".into(),
                        peer: PEER.into(),
                        peer_public_key: PEER_PUBLIC.into(),
                        local_project: "proj".into(),
                        remote_project: "remote-proj".into(),
                        capability: Capability::LeadMessages,
                        created_at: 1,
                        revoked_at: None,
                        peer_label: "Peer".into(),
                    }],
                )?;
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .unwrap();
        (root, store)
    }

    /// A UUIDv7 request id: the receiver reads its time to check expiry.
    fn test_request_id() -> String {
        static MS: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
        let ms = *MS.get_or_init(|| chrono::Utc::now().timestamp_millis());
        uuid::Uuid::from_u128(((ms as u128) << 80) | (7 << 76) | (2 << 62) | 1).to_string()
    }

    /// A `lead-message` body signed by the peer key, with its request signature.
    fn signed_body(id: &str, text: &str) -> (Value, Vec<u8>, String, String) {
        let identity = peer_identity();
        let mut envelope = json!({"protocol":PROTOCOL,"senderServer":PEER,"requestId":test_request_id(),
            "id":id,"kind":"lead-message","room":"proj","payload":{"text":text},
            "created":NOW,"timestamp":NOW as i64,"nonce":format!("envelope-nonce-{id}-123456")});
        envelope["signature"] =
            json!(identity.sign(&transport::federation_message_signing_bytes(&envelope).unwrap()));
        let body = json!({"envelope": envelope});
        let raw = studio_json::to_vec(&body).unwrap();
        let nonce = transport::new_nonce().unwrap();
        let unsigned = SignedRequest {
            action: FederationAction::Message,
            peer_id: PEER,
            timestamp: NOW as i64,
            nonce: &nonce,
            signature: "",
            raw: &raw,
            unified: Some(UnifiedFields {
                server_id: LOCAL,
                request_id: "request-1",
            }),
        };
        let signature = identity.sign(&unsigned.signing_bytes(LOCAL).unwrap());
        (body, raw, nonce, signature)
    }

    async fn deliver(store: &Arc<RuntimeStore>, id: &str, text: &str) -> Result<Value> {
        let (body, raw, nonce, signature) = signed_body(id, text);
        let request = SignedRequest {
            action: FederationAction::Message,
            peer_id: PEER,
            timestamp: NOW as i64,
            nonce: &nonce,
            signature: &signature,
            raw: &raw,
            unified: Some(UnifiedFields {
                server_id: LOCAL,
                request_id: "request-1",
            }),
        };
        receive(Arc::clone(store), &request, body, NOW).await
    }

    fn lead_events(store: &RuntimeStore) -> Vec<(String, String)> {
        let db = store.test_reader().unwrap();
        let mut query = db
            .prepare("SELECT kind,text FROM runtime_events WHERE agent='lead' AND kind=?1")
            .unwrap();
        query
            .query_map([EVENT_KIND], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    }

    #[tokio::test]
    async fn lead_message_reaches_and_wakes_the_root_lead() {
        let (_root, store) = store("reach").await;
        let receipt = deliver(&store, "m1", "hello lead").await.unwrap();
        assert_eq!(receipt["result"]["accepted"], json!(true));
        let events = lead_events(&store);
        assert_eq!(events.len(), 1);
        let text: Value = studio_json::from_str(&events[0].1).unwrap();
        assert_eq!(text["source"], "peer_lead");
        assert_eq!(text["untrusted"], json!(true));
        assert_eq!(text["text"], "hello lead");
    }

    #[tokio::test]
    async fn duplicate_delivery_adds_one_message() {
        let (_root, store) = store("duplicate").await;
        deliver(&store, "m2", "once").await.unwrap();
        // The same envelope nonce is refused as a replay before any write.
        let repeat = deliver(&store, "m2", "once").await;
        assert!(
            matches!(
                repeat,
                Err(Error::Forbidden("nonce replay or peer rate limit"))
            ),
            "{repeat:?}"
        );
        assert_eq!(lead_events(&store).len(), 1);
    }

    #[tokio::test]
    async fn a_revoked_grant_is_refused_before_any_write() {
        let (_root, store) = store("revoked").await;
        store
            .transaction(|tx| {
                project_grants::revoke(tx, "g1", 2)?;
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .unwrap();
        let error = deliver(&store, "m3", "blocked").await.unwrap_err();
        assert!(matches!(error, Error::Forbidden(_)), "{error:?}");
        assert!(lead_events(&store).is_empty());
    }

    #[tokio::test]
    async fn an_unknown_peer_without_a_grant_is_refused() {
        let (_root, store) = store("no-grant").await;
        store
            .transaction(|tx| {
                tx.execute("DELETE FROM project_grants", [])?;
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .unwrap();
        let error = deliver(&store, "m4", "blocked").await.unwrap_err();
        assert!(
            matches!(error, Error::Forbidden("no project grant")),
            "{error:?}"
        );
        assert!(lead_events(&store).is_empty());
    }
}
