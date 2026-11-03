use crate::{
    error::{CSP, error_response, with_api_headers},
    middleware::trusted,
    schema::{API_SCHEMA_HASH_HEADER, api_schema_hash, path_matches as route_matches},
};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};

#[test]
fn generated_provider_identity_is_the_schema_owner() {
    for provider in studio_runtime_schema::ProviderKind::ALL {
        let generated: crate::generated::ProviderKind = provider;
        let owner: studio_runtime_schema::ProviderKind = generated;
        assert_eq!(owner, provider);
        assert_eq!(
            serde_json::to_value(generated).expect("generated provider serializes"),
            serde_json::to_value(provider).expect("owner serializes")
        );
    }
}

#[test]
fn route_template_matching_is_segment_exact() {
    assert!(route_matches("/api/agents/{id}", "/api/agents/a"));
    assert!(!route_matches("/api/agents/{id}", "/api/agents/a/b"));
}

#[test]
fn trust_requires_loopback_origin_and_constant_time_token_match() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4312"));
    assert!(trusted(
        &headers,
        false,
        "token",
        true,
        4312,
        &None,
        "/api/probe"
    ));
    assert!(!trusted(
        &headers,
        false,
        "token",
        false,
        4312,
        &None,
        "/api/probe"
    ));
    assert!(!trusted(
        &headers,
        true,
        "token",
        true,
        4312,
        &None,
        "/api/probe"
    ));
    headers.insert("x-canvas-token", HeaderValue::from_static("token"));
    assert!(trusted(
        &headers,
        true,
        "token",
        true,
        4312,
        &None,
        "/api/probe"
    ));
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://attacker.example"),
    );
    assert!(!trusted(
        &headers,
        false,
        "token",
        true,
        4312,
        &None,
        "/api/probe"
    ));
}

#[test]
fn forwarded_origins_require_the_configured_tailnet_proxy() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4312"));
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://studio.ts.net"),
    );
    headers.insert(
        "x-forwarded-host",
        HeaderValue::from_static("studio.ts.net"),
    );
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
    headers.insert("x-forwarded-for", HeaderValue::from_static("100.64.0.9"));
    let public_origin = Some("https://studio.ts.net".to_owned());
    assert!(trusted(
        &headers,
        false,
        "token",
        true,
        4312,
        &public_origin,
        "/api/probe"
    ));
    headers.insert("x-forwarded-for", HeaderValue::from_static("192.0.2.9"));
    assert!(!trusted(
        &headers,
        false,
        "token",
        true,
        4312,
        &public_origin,
        "/api/probe"
    ));
}

#[test]
fn signed_federation_writes_do_not_require_the_local_session_token() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4312"));
    assert!(trusted(
        &headers,
        true,
        "token",
        true,
        4312,
        &None,
        "/api/federation/v1/message"
    ));
}

#[test]
fn api_responses_include_schema_and_security_headers() {
    let response = with_api_headers(
        error_response(StatusCode::NOT_IMPLEMENTED, "Not implemented"),
        None,
    );
    assert_eq!(
        response.headers()[API_SCHEMA_HASH_HEADER],
        api_schema_hash()
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert_eq!(response.headers()["content-security-policy"], CSP);
}
// The current OpenAPI operation count is owned by the document and can change.

#[test]
#[allow(clippy::disallowed_methods)]
fn workspace_keeps_serde_json_maps_sorted() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../");
    let output = std::process::Command::new("cargo")
        .args([
            "tree",
            "--workspace",
            "-e",
            "normal,features",
            "-i",
            "serde_json",
        ])
        .current_dir(workspace)
        .output()
        .expect("cargo tree is available for the workspace feature guard");
    assert!(
        output.status.success(),
        "cargo tree --workspace -e features failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    // arbitrary_precision changes JSON numbers into a private object representation.
    // preserve_order changes serde_json::Map from sorted to insertion-ordered keys.
    assert!(
        !tree.contains("serde_json feature \"arbitrary_precision\"")
            && !tree.contains("serde_json feature \"preserve_order\""),
        "serde_json arbitrary_precision changes number meaning, and preserve_order changes Map order relied upon by canonical output in several crates; code needing insertion order must parse ordered pairs locally"
    );
}

#[test]
fn a_federation_route_gets_no_principal_from_the_edge() {
    use crate::middleware::edge_check;
    use studio_runtime_services::principal::{CommandClass, Grant, Principal};
    // A forged peer header is not read at the edge: the handler builds `Federated`
    // only after the signature check.
    assert_eq!(edge_check(None, CommandClass::Federation), Ok(None));
    // A signed device or server cannot use a federation route.
    let device = Principal::Device {
        id: "d".into(),
        grant: Grant::Full,
    };
    let peer = Principal::Peer { server: "s".into() };
    assert_eq!(edge_check(Some(&device), CommandClass::Federation), Err(()));
    assert_eq!(edge_check(Some(&peer), CommandClass::Federation), Err(()));
    // Other routes: no principal means the owner, which the table checks.
    assert_eq!(
        edge_check(None, CommandClass::OwnerOnly),
        Ok(Some(Principal::Owner))
    );
    assert_eq!(edge_check(Some(&peer), CommandClass::OwnerOnly), Err(()));
}

#[test]
fn project_peer_flag_admits_only_the_forwarded_source_outside_the_tailnet() {
    use crate::middleware::trusted_with;
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4312"));
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
    headers.insert(
        "x-forwarded-host",
        HeaderValue::from_static("studio.example.ts.net"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
    let origin = Some("https://studio.example.ts.net".to_owned());
    let path = "/api/federation/v1/message";
    assert!(!trusted_with(
        &headers, true, "token", true, 4312, &origin, path, false
    ));
    assert!(trusted_with(
        &headers, true, "token", true, 4312, &origin, path, true
    ));
}

#[tokio::test]
async fn a_lead_message_from_a_granted_peer_outside_the_tailnet_is_admitted() {
    use crate::middleware::project_peer_source;
    use studio_runtime_store::project_grants::{Capability, GrantRow, insert_rows};
    use studio_runtime_store::{RuntimeStore, StoreError};
    let root = tempfile::tempdir().expect("isolated state");
    let store = std::sync::Arc::new(
        RuntimeStore::open(Some(root.path()))
            .await
            .expect("runtime store opens"),
    );
    store
        .transaction_with(|tx| {
            insert_rows(
                tx,
                &[GrantRow {
                    grant_id: "g1".into(),
                    peer: "peer-1".into(),
                    peer_public_key: "PEM".into(),
                    local_project: "proj-a".into(),
                    remote_project: "proj-b".into(),
                    capability: Capability::LeadMessages,
                    created_at: 1,
                    revoked_at: None,
                }],
            )?;
            Ok::<_, StoreError>(())
        })
        .await
        .expect("grant written");
    let headers = |source: &'static str, peer: &'static str| {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static(source));
        headers.insert("x-studio-client", HeaderValue::from_static(peer));
        headers
    };
    let lead = "/api/federation/v1/message";
    let outside = "203.0.113.7";
    assert!(
        project_peer_source(
            Some(store.clone()),
            &headers(outside, "peer-1"),
            &axum::http::Method::POST,
            lead
        )
        .await
    );
    // A tailnet source is not an admission: the tailnet rule decides it.
    assert!(
        !project_peer_source(
            Some(store.clone()),
            &headers("100.75.160.63", "peer-1"),
            &axum::http::Method::POST,
            lead
        )
        .await
    );
    // Another route, another method, an unknown peer, or no store: refused.
    assert!(
        !project_peer_source(
            Some(store.clone()),
            &headers(outside, "peer-1"),
            &axum::http::Method::POST,
            "/api/federation/v1/pull"
        )
        .await
    );
    assert!(
        !project_peer_source(
            Some(store.clone()),
            &headers(outside, "peer-1"),
            &axum::http::Method::GET,
            lead
        )
        .await
    );
    assert!(
        !project_peer_source(
            Some(store.clone()),
            &headers(outside, "peer-2"),
            &axum::http::Method::POST,
            lead
        )
        .await
    );
    assert!(
        !project_peer_source(
            None,
            &headers(outside, "peer-1"),
            &axum::http::Method::POST,
            lead
        )
        .await
    );
}
