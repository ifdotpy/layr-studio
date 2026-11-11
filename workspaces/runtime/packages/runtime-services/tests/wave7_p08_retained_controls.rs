use std::sync::{Arc, Mutex};
use studio_json::{Value, json};
use studio_runtime_services::{
    claude_controls::{ControlError, service::*},
    claude_runtime::*,
};

struct AccountPort {
    binding: Mutex<ControlBinding>,
    calls: Mutex<Vec<ClaudeControlCall>>,
    fault: &'static str,
}
fn binding() -> ControlBinding {
    ControlBinding::new("account".into(), "retained-connection".into(), 7).expect("binding")
}
impl ClaudeSessionPort for AccountPort {
    fn binding(&self) -> ControlBinding {
        self.binding.lock().expect("binding").clone()
    }
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "Decision lives in codex_disposition; this narrow scope handles only its declared methods"
    )]
    fn request(&self, call: ClaudeControlCall) -> ControlFuture<'_, ClaudeControlReceipt> {
        Box::pin(async move {
            self.calls.lock().expect("calls").push(call.clone());
            let mut identity = call.identity;
            match self.fault {
                "lost" => return Err(ControlError::Unknown),
                "deadline" => std::future::pending::<()>().await,
                "agent" => identity.scope.agent = "foreign".into(),
                "epoch" => identity.scope.epoch += 1,
                "request" => identity.scope.request_id = Some("foreign".into()),
                "payload" => identity.signature = "foreign".into(),
                "method" => identity.method = studio_runtime_providers::Method::ClaudeCommands,
                "account" => {
                    identity.binding =
                        ControlBinding::new("foreign".into(), "retained-connection".into(), 7)
                            .expect("binding")
                }
                "connection" => {
                    identity.binding =
                        ControlBinding::new("account".into(), "foreign".into(), 7).expect("binding")
                }
                "generation" => {
                    identity.binding =
                        ControlBinding::new("account".into(), "retained-connection".into(), 8)
                            .expect("binding")
                }
                "restart" => {
                    *self.binding.lock().expect("binding") =
                        ControlBinding::new("account".into(), "replacement".into(), 8)
                            .expect("binding")
                }
                _ => {}
            }
            let result = match identity.method {
                studio_runtime_providers::Method::ClaudeState => json!({"tasks":[],"turns":[]}),
                studio_runtime_providers::Method::ClaudeSettings => json!({"settings":{}}),
                _ => json!({"ok":true}),
            };
            Ok(ClaudeControlReceipt { identity, result })
        })
    }
}
fn port(fault: &'static str) -> Arc<AccountPort> {
    Arc::new(AccountPort {
        binding: Mutex::new(binding()),
        calls: Mutex::new(vec![]),
        fault,
    })
}
fn scope() -> ControlScope {
    ControlScope {
        agent: "agent".into(),
        epoch: 2,
        request_id: Some("logical-request".into()),
    }
}
#[tokio::test(start_paused = true)]
async fn retained_controls_bind_every_receipt_field_and_never_retry_a_lost_reply() {
    for fault in [
        "none",
        "lost",
        "agent",
        "epoch",
        "request",
        "payload",
        "method",
        "account",
        "connection",
        "generation",
        "restart",
    ] {
        let port = port(fault);
        let session = RetainedClaudeSession::new(port.clone());
        let result = session
            .call_scoped(
                scope(),
                ControlInvocation::Settings {
                    thread_id: "thread".into(),
                    settings: Default::default(),
                    request_id: "logical-request".into(),
                },
            )
            .await;
        if fault == "none" {
            assert!(matches!(result, Ok(ControlReply::Settings(_))));
        } else {
            assert!(matches!(result, Err(ControlError::Unknown)), "{fault}");
        }
        assert_eq!(port.calls.lock().expect("calls").len(), 1, "{fault}");
    }
}
#[tokio::test(start_paused = true)]
async fn replaced_generation_and_unscoped_or_unrecognized_calls_never_reach_the_owner() {
    let port = port("none");
    let session = RetainedClaudeSession::new(port.clone());
    *port.binding.lock().expect("binding") =
        ControlBinding::new("account".into(), "replacement".into(), 8).expect("binding");
    assert!(matches!(
        session
            .call_scoped(
                scope(),
                ControlInvocation::State {
                    thread_id: "thread".into()
                }
            )
            .await,
        Err(ControlError::Changed)
    ));
    assert!(port.calls.lock().expect("calls").is_empty());
}
struct NoWrites;
impl ControlStorage for NoWrites {
    fn replay(
        &self,
        _: studio_runtime_services::claude_controls::ControlRequest,
    ) -> ControlFuture<'_, Option<Result<Value, ControlError>>> {
        panic!("read must not write")
    }
    fn reserve(
        &self,
        _: studio_runtime_services::claude_controls::ControlRequest,
        _: ControlBinding,
    ) -> ControlFuture<'_, studio_runtime_services::claude_controls::ControlSnapshot> {
        panic!("read must not write")
    }
    fn submitted(
        &self,
        _: studio_runtime_services::claude_controls::ControlSnapshot,
        _: studio_runtime_services::claude_controls::ControlRequest,
        _: NativeProof,
    ) -> ControlFuture<'_, studio_runtime_services::claude_controls::ControlSnapshot> {
        panic!("read must not write")
    }
    fn provider(
        &self,
        _: studio_runtime_services::claude_controls::ControlSnapshot,
        _: studio_runtime_services::claude_controls::ControlRequest,
        _: MutationReply,
    ) -> ControlFuture<'_, studio_runtime_services::claude_controls::ControlSnapshot> {
        panic!("read must not write")
    }
    fn complete(
        &self,
        _: studio_runtime_services::claude_controls::ControlSnapshot,
        _: studio_runtime_services::claude_controls::ControlRequest,
    ) -> ControlFuture<'_, Value> {
        panic!("read must not write")
    }
    fn failed(
        &self,
        _: studio_runtime_services::claude_controls::ControlSnapshot,
        _: FailureEvidence,
        _: ControlError,
    ) -> ControlFuture<'_, ()> {
        panic!("read must not write")
    }
}
#[tokio::test(start_paused = true)]
async fn controls_caller_supplies_scope_and_bounds_the_retained_request_deadline() {
    let agent = json!({"compactions":0,"id":"agent","rootId":"agent","provider":"claude","epoch":2,"accountKey":"account","cwd":"/fixture","status":"paused","threadId":"thread"});
    for fault in ["none", "deadline"] {
        let request_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let port = port(fault);
        let controls = Controls::new(
            Arc::new(NoWrites),
            Arc::new(RetainedClaudeSession::new(port.clone())),
        );
        let result = controls
            .read_or_stop(
                &agent,
                &studio_runtime_schema::claude_controls::ClaudeSessionRead::StopTask {
                    id: "agent".to_owned().try_into().unwrap(),
                    task_id: "task".to_owned().try_into().unwrap(),
                    request_id: request_id.clone().try_into().unwrap(),
                },
                None,
            )
            .await;
        assert_eq!(result.is_ok(), fault == "none");
        let calls = port.calls.lock().expect("calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].identity.scope.agent, "agent");
        assert_eq!(calls[0].identity.scope.epoch, 2);
        assert_eq!(
            calls[0].identity.scope.request_id.as_deref(),
            Some(request_id.as_str())
        );
    }
}

#[tokio::test(start_paused = true)]
async fn opaque_stop_ids_have_distinct_stable_request_identities() {
    let port = port("none");
    let controls = Controls::new(
        Arc::new(NoWrites),
        Arc::new(RetainedClaudeSession::new(port.clone())),
    );
    let first = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let second = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    let third = studio_runtime_schema::persistence::RequestIdentity::issue_root();
    for (thread, task, request) in [
        ("a:b", "c", &first),
        ("a", "b:c", &second),
        ("a:b", "c", &first),
        ("a:b", "c", &third),
    ] {
        let agent = json!({"compactions":0,"id":"agent","rootId":"agent","provider":"claude","epoch":2,"accountKey":"account","cwd":"/fixture","status":"paused","threadId":thread});
        let body = studio_runtime_schema::claude_controls::ClaudeSessionRead::StopTask {
            id: "agent".to_owned().try_into().unwrap(),
            task_id: task.to_owned().try_into().unwrap(),
            request_id: request.to_owned().try_into().unwrap(),
        };
        controls
            .read_or_stop(&agent, &body, None)
            .await
            .expect("stop");
    }
    let calls = port.calls.lock().expect("calls");
    assert_eq!(calls.len(), 4);
    assert_ne!(
        calls[0].identity.scope.request_id, calls[1].identity.scope.request_id,
        "distinct requests retain their supplied identities"
    );
    assert_eq!(
        calls[0].identity.scope.request_id, calls[2].identity.scope.request_id,
        "the same request retains its identity"
    );
    assert_eq!(
        calls[3].identity.scope.request_id.as_deref(),
        Some(third.as_str())
    );
    assert_eq!(
        calls[0].invocation.params(),
        json!({"threadId":"a:b","taskId":"c"})
    );
    assert_eq!(
        calls[1].invocation.params(),
        json!({"threadId":"a","taskId":"b:c"})
    );
}
