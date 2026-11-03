//! Edge check of a request from a paired project of another server (decision
//! 2026-11-01, "Authorization"). The decision uses the peer key and the project grant
//! only. The network path (address, Tailscale identity, origin) is not an input, so a
//! request from outside the tailnet gets the same answer as one from inside it.

use std::sync::Arc;

use studio_runtime_store::project_grants;
use studio_runtime_store::{RuntimeStore, StoreError};

use crate::principal::{CommandClass, Principal, access};

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Unknown key, a revoked grant, or a project outside the grant.
    NoGrant,
    /// The request signature does not verify with the granted key.
    BadSignature,
    /// The grant has no row that allows this command class.
    NotAllowed,
    /// The grant could not be read.
    Store,
}

/// Build the `ProjectPeer` principal for one request, or refuse it. `verify` checks the
/// request signature with the public key stored in the grant.
pub async fn authorize(
    store: Arc<RuntimeStore>,
    peer: String,
    project: String,
    class: CommandClass,
    verify: impl FnOnce(&str) -> bool + Send + 'static,
) -> Result<Principal, Refusal> {
    store
        .read_async("runtime-services/project_peer", move |tx| {
            let rows = project_grants::active_rows(tx, &peer, &project)?;
            let Some(first) = rows.first() else {
                return Ok::<_, StoreError>(Err(Refusal::NoGrant));
            };
            if !verify(&first.peer_public_key) {
                return Ok(Err(Refusal::BadSignature));
            }
            let principal = Principal::ProjectPeer {
                peer,
                project,
                capabilities: rows.iter().map(|row| row.capability).collect(),
            };
            if access(&principal, class).passes_edge() {
                Ok(Ok(principal))
            } else {
                Ok(Err(Refusal::NotAllowed))
            }
        })
        .await
        .map_err(|_| Refusal::Store)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_runtime_store::project_grants::{Capability, GrantRow};

    const KEY: &str = "PEM-PEER-1";

    async fn store_with_grant(revoke: bool) -> (Arc<RuntimeStore>, tempfile::TempDir) {
        let root = tempfile::tempdir().expect("isolated state");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store opens"),
        );
        store
            .transaction_with(move |tx| {
                project_grants::insert_rows(
                    tx,
                    &[GrantRow {
                        grant_id: "g1".into(),
                        peer: "peer-1".into(),
                        peer_public_key: KEY.into(),
                        local_project: "proj-a".into(),
                        remote_project: "proj-b".into(),
                        capability: Capability::LeadMessages,
                        created_at: 100,
                        revoked_at: None,
                        peer_label: "Server B".into(),
                    }],
                )?;
                if revoke {
                    project_grants::revoke(tx, "g1", 200)?;
                }
                Ok::<_, StoreError>(())
            })
            .await
            .expect("grant written");
        (store, root)
    }

    async fn check(
        store: &Arc<RuntimeStore>,
        peer: &str,
        project: &str,
        class: CommandClass,
    ) -> Result<Principal, Refusal> {
        authorize(
            Arc::clone(store),
            peer.into(),
            project.into(),
            class,
            |key| key == KEY,
        )
        .await
    }

    #[tokio::test]
    async fn valid_grant_is_accepted_for_lead_messages_without_any_network_identity() {
        let (store, _root) = store_with_grant(false).await;
        // No origin, Tailscale identity or address is passed: the decision has no such input.
        let principal = check(&store, "peer-1", "proj-a", CommandClass::ChatControl)
            .await
            .expect("lead message is allowed");
        assert_eq!(
            principal,
            Principal::ProjectPeer {
                peer: "peer-1".into(),
                project: "proj-a".into(),
                capabilities: [Capability::LeadMessages].into_iter().collect(),
            }
        );
    }

    #[tokio::test]
    async fn every_other_class_is_refused_for_a_project_peer() {
        let (store, _root) = store_with_grant(false).await;
        for class in [
            CommandClass::Read,
            CommandClass::Spawn,
            CommandClass::Host,
            CommandClass::OwnerOnly,
            CommandClass::Federation,
        ] {
            assert_eq!(
                check(&store, "peer-1", "proj-a", class).await,
                Err(Refusal::NotAllowed),
                "{class:?}"
            );
        }
    }

    #[tokio::test]
    async fn unknown_key_and_other_project_are_refused() {
        let (store, _root) = store_with_grant(false).await;
        assert_eq!(
            check(&store, "peer-2", "proj-a", CommandClass::ChatControl).await,
            Err(Refusal::NoGrant)
        );
        assert_eq!(
            check(&store, "peer-1", "proj-c", CommandClass::ChatControl).await,
            Err(Refusal::NoGrant)
        );
    }

    #[tokio::test]
    async fn wrong_signature_is_refused() {
        let (store, _root) = store_with_grant(false).await;
        let refused = authorize(
            Arc::clone(&store),
            "peer-1".into(),
            "proj-a".into(),
            CommandClass::ChatControl,
            |_| false,
        )
        .await;
        assert_eq!(refused, Err(Refusal::BadSignature));
    }

    #[tokio::test]
    async fn revoked_grant_is_refused_and_stays_refused() {
        let (store, _root) = store_with_grant(true).await;
        assert_eq!(
            check(&store, "peer-1", "proj-a", CommandClass::ChatControl).await,
            Err(Refusal::NoGrant)
        );
    }
}
