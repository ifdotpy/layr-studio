use studio_runtime_providers::{Frame, decode_codex_line, encode_line};

#[test]
fn retained_codex_callbacks_keep_typed_and_unparsed_outcomes() {
    for (wire, parsed) in [
        (
            br#"{"id":7,"method":"currentTime/read","params":{"threadId":"thread"}}"#.as_slice(),
            true,
        ),
        (
            br#"{"id":7,"method":"currentTime/read","params":{"threadId":42}}"#.as_slice(),
            false,
        ),
    ] {
        let stored = studio_runtime_providers::decode_frame(wire).unwrap();
        let frame = studio_runtime_providers::normalize_codex_frame(&stored)
            .expect("retained callback revalidation");
        assert_eq!(frame.id(), stored.id());
        assert_eq!(frame.method(), stored.method());
        assert_eq!(matches!(frame, Frame::CodexServerRequest(_)), parsed);
        assert_eq!(matches!(frame, Frame::KnownMethodUnparsed(_)), !parsed);
    }
}

#[test]
fn codex_normalization_refuses_internal_studio_methods() {
    let frame = studio_runtime_providers::decode_frame(
        br#"{"id":7,"method":"operationStatus","params":{"operation":"same-operation"}}"#,
    )
    .unwrap();
    assert!(studio_runtime_providers::normalize_codex_frame(&frame).is_err());
}

#[test]
fn probe_rejects_vendor_field_type_changes() {
    let malformed = br#"{"id":1,"result":{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":42}}"#;
    assert!(
        studio_runtime_providers::decode_probe_response(
            malformed,
            1,
            studio_runtime_providers::codex::ClientRequestMethod::Initialize
        )
        .is_err()
    );
}

#[test]
fn codex_ingress_decodes_vendor_payload_and_rejects_wrong_field_type() {
    let bytes = br#"{"method":"item/agentMessage/delta","params":{"threadId":"t","turnId":"r","itemId":"i","delta":"hello"}}"#;
    let frame = decode_codex_line(bytes).unwrap();
    assert!(matches!(frame, Frame::CodexNotification(_)));
    assert_eq!(
        decode_codex_line(&encode_line(&frame).unwrap()).unwrap(),
        frame
    );
    let malformed = br#"{"method":"item/agentMessage/delta","params":{"threadId":"t","turnId":"r","itemId":"i","delta":42}}"#;
    assert!(matches!(
        decode_codex_line(malformed).unwrap(),
        Frame::KnownMethodUnparsed(_)
    ));
}

#[test]
fn codex_future_method_uses_the_one_unknown_variant() {
    let frame =
        decode_codex_line(br#"{"method":"future/notification","params":{"future":true}}"#).unwrap();
    assert!(matches!(frame, Frame::UnknownMethod(_)));
}

#[test]
fn codex_known_methods_keep_future_nested_variants() {
    for wire in [
        br#"{"method":"item/started","params":{"threadId":"t","turnId":"r","startedAtMs":0,"item":{"type":"futureItem","id":"i"}}}"#.as_slice(),
        br#"{"id":7,"method":"mcpServer/elicitation/request","params":{"mode":"futureMode","message":"Confirm","serverName":"mcp","threadId":"t","turnId":null}}"#,
    ] {
        let frame = decode_codex_line(wire).expect("future payload stays under its known method");
        assert!(matches!(frame, Frame::CodexNotification(_) | Frame::CodexServerRequest(_)));
        assert_eq!(decode_codex_line(&encode_line(&frame).unwrap()).unwrap(), frame);
    }
}

#[test]
fn codex_results_correlate_by_registered_method_before_settlement() {
    use studio_runtime_providers::{
        Action, ClientName, RequestId, RequestOutcome, ResponseBody, SessionState, codex,
        codex_initialize_params,
    };
    let (mut session, writes) = SessionState::new_codex_with_recovered_and_prefix(
        7,
        0,
        100,
        codex_initialize_params(ClientName::AccountSession),
        vec![],
        None,
    )
    .unwrap();
    assert!(matches!(
        &writes[0],
        Action::Write(Frame::CodexClientRequest(_))
    ));
    let init = decode_codex_line(br#"{"id":1,"result":{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":"codex/0.167.1"}}"#).unwrap();
    let actions = session.receive(init).unwrap();
    assert!(matches!(
        &actions[0],
        Action::Resolve {
            outcome: RequestOutcome::Reply {
                body: ResponseBody::CodexResult(codex::ClientRequestResult::Initialize(_)),
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &actions[1],
        Action::Write(Frame::CodexClientNotification(_))
    ));
    let request = codex::ClientRequest::from_method_params(
        codex::RequestId::Variant2(Box::new(0)),
        codex::ClientRequestMethod::TurnSteer,
        &studio_json::json!({"threadId":"t","expectedTurnId":"r","input":[]}),
    )
    .unwrap();
    let bad_request = request.clone();
    let (ticket, _) = session
        .submit_codex(request, 1, 10, Some("t".into()))
        .unwrap();
    assert_eq!(ticket.id, RequestId::Number(2));
    let valid = decode_codex_line(br#"{"id":2,"result":{"turnId":"r"}}"#).unwrap();
    let actions = session.receive_at(valid, 12).unwrap();
    assert!(
        matches!(&actions[0], Action::Resolve { ticket: actual, outcome: RequestOutcome::Reply { body: ResponseBody::CodexResult(codex::ClientRequestResult::TurnSteer(result)), late: true } } if actual == &ticket && result.turn_id == "r")
    );
    let (bad_ticket, _) = session
        .submit_codex(bad_request, 2, 10, Some("t".into()))
        .unwrap();
    let malformed = decode_codex_line(br#"{"id":3,"result":{"turnId":42}}"#).unwrap();
    let actions = session.receive(malformed).unwrap();
    assert!(
        matches!(&actions[0], Action::Resolve { ticket, outcome: RequestOutcome::Reply { body: ResponseBody::CodexUnparsedResult { method: codex::ClientRequestMethod::TurnSteer, .. }, .. } } if ticket == &bad_ticket)
    );
}
