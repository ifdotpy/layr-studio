//! Project pairing across two in-process stores. The loopback transport answers the signed
//! pairing request with the inviting store's real inbound handler.

use super::{FederationError, accept_project_peer_with, manage_with_origin, project_grants_view};
use crate::federation_delivery::{
    FederationExchange, FederationTransport, FederationTransportResponse,
};
use crate::federation_effects::{self, FederationAction, OriginObservation, SignedRequest};
use crate::remote_transport::RemoteTransportError;
use futures_util::future::BoxFuture;
use std::sync::Arc;
use studio_json::{Value, json};
use studio_runtime_schema::federation_ingress::{FederationInvitation, FederationRequest};
use studio_runtime_store::{RuntimeStore, StoreError};

const ORIGIN_A: &str = "https://a.example.ts.net";
const ORIGIN_B: &str = "https://b.example.ts.net";

/// Delivers each signed request to the `remote` store, as the Serve path would.
struct Loopback {
    remote: Arc<RuntimeStore>,
}

impl FederationTransport for Loopback {
    fn exchange<'a>(
        &'a self,
        exchange: FederationExchange,
    ) -> BoxFuture<'a, crate::remote_transport::RemoteTransportResult<FederationTransportResponse>>
    {
        Box::pin(async move {
            let header = |name: &str| -> String {
                exchange
                    .headers
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value.clone())
                    .expect("signed header")
            };
            let peer_id = header("x-studio-client");
            let timestamp: i64 = header("x-studio-timestamp").parse().expect("time");
            let nonce = header("x-studio-nonce");
            let signature = header("x-studio-signature");
            let server_id = header("x-studio-server");
            let request_id = header("x-studio-request-id");
            let answer = federation_effects::handle(
                Arc::clone(&self.remote),
                SignedRequest {
                    action: FederationAction::Pair,
                    peer_id: &peer_id,
                    timestamp,
                    nonce: &nonce,
                    signature: &signature,
                    raw: &exchange.body,
                    server_id: &server_id,
                    request_id: &request_id,
                },
                OriginObservation::from_trusted_serve(None),
                timestamp as f64,
            )
            .await
            .map_err(|_| RemoteTransportError::InvalidInput)?;
            Ok(FederationTransportResponse {
                body: studio_json::to_vec(&answer).expect("answer JSON"),
                link_advert: None,
            })
        })
    }
}

/// A store with federation enabled and one project row.
async fn server(project: &str) -> (Arc<RuntimeStore>, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("isolated state");
    let store = Arc::new(
        RuntimeStore::open(Some(root.path()))
            .await
            .expect("runtime store opens"),
    );
    manage(
        &store,
        json!({"action": "set_enabled", "enabled": true}),
        None,
    )
    .await
    .expect("federation enabled");
    let project = project.to_owned();
    store
        .transaction_with(move |tx| {
            tx.execute(
                "INSERT INTO runtime_projects(id,record) VALUES(?1,'{}')",
                [&project],
            )
            .map_err(StoreError::from)?;
            Ok::<_, StoreError>(())
        })
        .await
        .expect("project row");
    (store, root)
}

async fn manage(
    store: &Arc<RuntimeStore>,
    body: Value,
    origin: Option<&str>,
) -> Result<Value, FederationError> {
    let request: FederationRequest =
        studio_json::from_str(&body.to_string()).expect("request parses");
    manage_with_origin(Arc::clone(store), &request, origin).await
}

/// The state id and public key PEM of a server.
async fn identity(store: &Arc<RuntimeStore>) -> (String, String) {
    store
        .read_async("runtime-services/federation/tests", |tx| {
            tx.query_row(
                "SELECT json_extract(record,'$.stateId'), json_extract(record,'$.publicKey') FROM runtime_federation_identity WHERE id='local'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(StoreError::from)
        })
        .await
        .expect("identity row")
}

async fn invite(server: &Arc<RuntimeStore>, project: &str) -> FederationInvitation {
    let created = manage(
        server,
        json!({"action": "create_project_invite", "project_id": project}),
        Some(ORIGIN_A),
    )
    .await
    .expect("project invitation");
    studio_json::from_str(&created["invitation"].to_string()).expect("invitation parses")
}

async fn accept(
    accepting: &Arc<RuntimeStore>,
    invitation: &FederationInvitation,
    project: &str,
    remote: &Arc<RuntimeStore>,
) -> Result<Value, FederationError> {
    accept_project_peer_with(
        Arc::clone(accepting),
        invitation,
        project,
        Some(ORIGIN_B),
        &Loopback {
            remote: Arc::clone(remote),
        },
    )
    .await
}

#[tokio::test]
async fn project_invitation_names_the_project_with_a_fifteen_minute_expiry() {
    let (a, _root) = server("project-a").await;
    let created = manage(
        &a,
        json!({"action": "create_project_invite", "project_id": "project-a"}),
        Some(ORIGIN_A),
    )
    .await
    .expect("project invitation");
    let invitation = &created["invitation"];
    assert_eq!(invitation["scope"], "project");
    assert_eq!(invitation["projectId"], "project-a");
    assert_eq!(invitation["origin"], ORIGIN_A);
    let expires = created["expires"].as_i64().expect("expiry");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    assert!(
        (now + 890..=now + 900).contains(&expires),
        "expires {expires}"
    );
}

#[tokio::test]
async fn project_invitation_is_one_use_for_other_peers_and_refused_after_expiry() {
    let (a, _a_root) = server("project-a").await;
    let (b, _b_root) = server("project-b").await;
    let (c, _c_root) = server("project-c").await;
    let invitation = invite(&a, "project-a").await;

    accept(&b, &invitation, "project-b", &a)
        .await
        .expect("first peer accepts");
    // The same peer may retry with the same invitation.
    accept(&b, &invitation, "project-b", &a)
        .await
        .expect("retry by the same peer");
    // Another server cannot use the used invitation.
    assert!(accept(&c, &invitation, "project-c", &a).await.is_err());

    let expired = invite(&a, "project-a").await;
    a.transaction_with({
        let invite_id = expired.invite_id.clone();
        move |tx| {
            tx.execute(
                "UPDATE runtime_federation_invites SET record=json_set(record,'$.expires',1) WHERE id=?1",
                [&invite_id],
            )
            .map_err(StoreError::from)?;
            Ok::<_, StoreError>(())
        }
    })
    .await
    .expect("expire the invitation");
    assert!(accept(&c, &expired, "project-c", &a).await.is_err());
}

#[tokio::test]
async fn accept_writes_one_grant_on_each_side_and_lists_the_fingerprint() {
    let (a, _a_root) = server("project-a").await;
    let (b, _b_root) = server("project-b").await;
    let invitation = invite(&a, "project-a").await;
    let (a_state, a_key) = identity(&a).await;

    accept(&b, &invitation, "project-b", &a)
        .await
        .expect("accept writes the grant");
    let (b_state, b_key) = identity(&b).await;

    let b_view = project_grants_view(&b, "project-b")
        .await
        .expect("list on b");
    let b_grants = b_view["grants"].as_array().expect("grants");
    assert_eq!(b_grants.len(), 1);
    assert_eq!(b_grants[0]["peerStateId"], a_state);
    assert_eq!(b_grants[0]["remoteProject"], "project-a");
    assert_eq!(b_grants[0]["capabilities"], json!(["LeadMessages"]));
    assert_eq!(
        b_grants[0]["fingerprint"],
        studio_runtime_store::sha256_hex(a_key.as_bytes())
    );

    let a_view = project_grants_view(&a, "project-a")
        .await
        .expect("list on a");
    let a_grants = a_view["grants"].as_array().expect("grants");
    assert_eq!(a_grants.len(), 1);
    assert_eq!(a_grants[0]["peerStateId"], b_state);
    assert_eq!(a_grants[0]["remoteProject"], "project-b");
    assert_eq!(
        a_grants[0]["fingerprint"],
        studio_runtime_store::sha256_hex(b_key.as_bytes())
    );
}

#[tokio::test]
async fn revoke_is_final_and_refuses_a_later_pairing_request() {
    let (a, _a_root) = server("project-a").await;
    let (b, _b_root) = server("project-b").await;
    let invitation = invite(&a, "project-a").await;
    accept(&b, &invitation, "project-b", &a)
        .await
        .expect("accept writes the grant");
    let a_grant = project_grants_view(&a, "project-a")
        .await
        .expect("list on a")["grants"][0]["grantId"]
        .as_str()
        .expect("grant id")
        .to_owned();

    let after_revoke = manage(
        &a,
        json!({"action": "revoke_project_grant", "grant_id": a_grant}),
        None,
    )
    .await
    .expect("revoke");
    assert_eq!(after_revoke["grants"], json!([]));
    assert!(
        manage(
            &a,
            json!({"action": "revoke_project_grant", "grant_id": a_grant}),
            None,
        )
        .await
        .is_err(),
        "a second revoke is refused"
    );
    // The same invitation no longer pairs on the revoked side.
    assert!(accept(&b, &invitation, "project-b", &a).await.is_err());
    // Revocation is local: the accepting side keeps its own grant until it revokes it.
    assert_eq!(
        project_grants_view(&b, "project-b")
            .await
            .expect("list on b")["grants"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

/// Peer team `team` of the local project `project-b`, as its document is built at read time.
async fn peer_team_payload(store: &Arc<RuntimeStore>) -> String {
    store
        .read_async("runtime-services/federation/tests", |tx| {
            crate::sync_policy::peer_team_document(tx, "team")?
                .map(|value| value.as_str().to_owned())
                .ok_or(StoreError::RecordContract("the team is not visible"))
        })
        .await
        .expect("peer team entity")
}

#[tokio::test]
async fn accept_shows_the_peer_lead_in_the_team_and_revoke_removes_it() {
    let (a, _a_root) = server("project-a").await;
    let (b, _b_root) = server("project-b").await;
    let invitation = invite(&a, "project-a").await;
    // No paired access client of `a` exists: the peer lead comes from the grant label alone.
    b.transaction_with(move |tx| {
        for id in ["a1", "a2"] {
            tx.execute(
                "INSERT INTO runtime_agents(id,record) VALUES(?1,?2)",
                (id, json!({"compactions": 0, "id": id, "rootId": id, "epoch": 1, "status": "idle", "name": id, "provider": "codex", "model": "gpt-6", "prompt": "Start", "cwd": "/project-b", "accountKey": "default", "isLead": true, "autoWake": false}).to_string()),
            )
            .map_err(StoreError::from)?;
        }
        tx.execute(
            "UPDATE runtime_projects SET record=?1 WHERE id='project-b'",
            [json!({"recordKind": "live", "id": "project-b", "path": "/project-b", "peerTeams": [{"id": "team", "name": "Team", "members": ["a1", "a2"]}]}).to_string()],
        )
        .map_err(StoreError::from)?;
        Ok::<_, StoreError>(())
    })
    .await
    .expect("paired server and team fixtures");
    accept(&b, &invitation, "project-b", &a)
        .await
        .expect("accept writes the grant");
    let grant = format!("\"grant\":\"{}\"", invitation.invite_id);
    assert!(
        peer_team_payload(&b)
            .await
            .contains("\"label\":\"Studio server\""),
        "the label comes from the grant"
    );
    assert!(
        peer_team_payload(&b).await.contains(&grant),
        "peer lead appears"
    );

    let grant_id = invitation.invite_id.clone();
    let view = manage(
        &b,
        json!({"action": "revoke_project_grant", "grant_id": grant_id}),
        None,
    )
    .await
    .expect("revoke");
    assert_eq!(view["grants"], json!([]));
    assert!(
        !peer_team_payload(&b).await.contains(&grant),
        "peer lead removed"
    );
}
