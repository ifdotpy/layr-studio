//! The shared witness accepts the actual request emitted by a production owner.
//! Call this at a fixture port before recording or answering that request.
use studio_json::Value;
use studio_runtime_process::{
    OperationId,
    provider_seam::{PreparedWrite, ProviderCodec},
};
use studio_runtime_providers::{
    Frame, Method, ProviderKind, RequestFrame, RequestId, ResponseBody, SessionState,
};

#[track_caller]
pub fn request(method: Method, params: &Value) {
    // Derive reachability from the two wire owners, including bridge extensions
    // such as mcpServer/reconnect that have no provider namespace in their name.
    let codex =
        studio_runtime_providers::codex::ClientRequestMethod::from_wire(method.as_str()).is_some();
    let claude =
        studio_runtime_schema::provider_input::bridge::BridgeMethod::from_wire(method.as_str())
            .is_some();
    assert!(
        codex || claude,
        "internal-only request must use its non-vendor owner: {method:?}"
    );
    if let Some(native) =
        studio_runtime_providers::codex::ClientRequestMethod::from_wire(method.as_str())
    {
        use studio_runtime_providers::codex_disposition::{
            Disposition, HandlerKey, client_request,
        };
        assert_eq!(
            client_request(native),
            Disposition::Handled(HandlerKey::Sender(native)),
            "an actual registered Codex sender needs a Handled disposition: {method:?}"
        );
    }
    for (provider, reachable) in [(ProviderKind::Codex, codex), (ProviderKind::Claude, claude)] {
        if !reachable {
            continue;
        }
        let codec = match provider {
            ProviderKind::Codex => ProviderCodec::codex(),
            ProviderKind::Claude => ProviderCodec::default(),
        };
        let id = RequestId::String("internal-sender-witness".into());
        // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/model_catalog.rs#internal_sender_witness_claude_catalog_uses_retained_session_and_paginates_python_params
        let frame = Frame::Request(RequestFrame {
            id: id.clone(),
            method,
            params: Some(params.clone()),
            fields: Default::default(),
        });
        let frame = (codec.normalize)(&frame)
            .unwrap_or_else(|error| panic!("{provider:?} {method:?}: {error}; params={params}"));
        (codec.encode)(&frame)
            .unwrap_or_else(|error| panic!("{provider:?} {method:?}: {error}; params={params}"));
        PreparedWrite::new_with_codec(OperationId("witness".into()), &frame, codec)
            .expect("the production codec accepts the actual request");
        let (mut state, initial) = SessionState::new(
            1,
            provider.wire(),
            0,
            1000,
            studio_runtime_providers::initialize_params(
                studio_runtime_providers::ClientName::AccountSession,
            ),
        )
        .expect("session");
        let studio_runtime_providers::Action::Write(initial) = &initial[0] else {
            panic!("initialize write")
        };
        state
            .receive(Frame::Response {
                id: initial.id().expect("initialize id"),
                body: ResponseBody::Result(studio_json::json!({})),
                fields: Default::default(),
            })
            .expect("initialize receipt");
        state
            .mark_initialized_notification_written()
            .expect("ready");
        state
            .preflight_frame_with_id(
                id,
                &frame,
                params.get("threadId").and_then(Value::as_str),
                false,
            )
            .unwrap_or_else(|error| panic!("{provider:?} {method:?}: {error}; params={params}"));
    }
}
