//! Correlate callback replies with their durable server-request method before submission.
use crate::account_worker::WorkerError;
use rusqlite::{OptionalExtension, Transaction, params};
use studio_runtime_process::provider_seam::ProviderCodec;
use studio_runtime_providers::codex::{ServerRequestMethod, ServerRequestResult};
use studio_runtime_providers::{Frame, ProviderKind, ResponseBody};
use studio_runtime_schema::{approvals::RequestKind, persistence::ToolRequestKind};

#[derive(Clone, Copy, Debug)]
pub(crate) enum CallbackReference {
    Immediate,
    Managed,
}

const IMMEDIATE: &str = "SELECT json_extract(record,'$.payload.accepted.method') FROM runtime_tool_requests WHERE origin_key=?1 AND kind=?2";
const MANAGED: &str = "SELECT json_extract(owner.record,'$.payload.card'), json_extract(owner.record,'$.payload.tool_request'), a.kind, t.kind FROM runtime_tool_requests owner LEFT JOIN approvals a ON a.id=json_extract(owner.record,'$.payload.card') LEFT JOIN runtime_tool_requests t ON t.id=json_extract(owner.record,'$.payload.tool_request') WHERE owner.origin_key=?1 AND owner.kind=?2";

fn refused(code: &'static str) -> WorkerError {
    WorkerError::PreparationRefused(format!("callback-reply/{code}"))
}

pub(crate) fn prepare_reply(
    tx: &Transaction<'_>,
    origin_key: &str,
    reference: CallbackReference,
    frame: &Frame,
    codec: ProviderCodec,
) -> Result<Frame, WorkerError> {
    let Frame::Response { id, body, fields } = frame else {
        return Err(refused("expected-response"));
    };
    // Generic provider replies retain their existing protocol and need no Codex correlation.
    if codec.kind != Some(ProviderKind::Codex) {
        return Ok(frame.clone());
    }
    let body = match body {
        ResponseBody::Uncorrelated(raw) => ResponseBody::CodexServerResult(
            ServerRequestResult::decode(method(tx, origin_key, reference)?, raw.as_str())
                .map_err(|_| refused("result-shape"))?,
        ),
        ResponseBody::Result(value) => ResponseBody::CodexServerResult(
            ServerRequestResult::from_deserializer(
                method(tx, origin_key, reference)?,
                value.clone(),
            )
            .map_err(|_| refused("result-shape"))?,
        ),
        // Error replies reject traffic without interpreting a successful payload.
        // Admission still validates the durable callback identity and request id.
        ResponseBody::CodexError(error) => ResponseBody::CodexError(error.clone()),
        ResponseBody::Error(error) => ResponseBody::CodexError(
            serde::Deserialize::deserialize(error.clone()).map_err(|_| refused("frame-shape"))?,
        ),
        ResponseBody::CodexResult(_)
        | ResponseBody::CodexUnparsedResult { .. }
        | ResponseBody::CodexServerResult(_) => return Err(refused("unexpected-response-body")),
    };
    Ok(Frame::Response {
        id: id.clone(),
        body,
        fields: fields.clone(),
    })
}

fn method(
    tx: &Transaction<'_>,
    origin_key: &str,
    reference: CallbackReference,
) -> Result<ServerRequestMethod, WorkerError> {
    match reference {
        CallbackReference::Immediate => {
            let method = tx
                .query_row(
                    IMMEDIATE,
                    params![origin_key, ToolRequestKind::ProviderCallback],
                    |row| Ok((row.get::<_, Option<String>>(0)?,)),
                )
                .optional()?;
            let method = method
                .ok_or_else(|| refused("missing-owner"))?
                .0
                .ok_or_else(|| refused("missing-method"))?;
            ServerRequestMethod::from_wire(&method).ok_or_else(|| refused("unknown-method"))
        }
        CallbackReference::Managed => {
            let row = tx
                .query_row(
                    MANAGED,
                    params![origin_key, ToolRequestKind::ProviderCallback],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| refused("missing-owner"))?;
            match row {
                (Some(_), None, Some(kind), None) => approval_method(&kind),
                (None, Some(_), None, Some(kind)) => {
                    match kind
                        .parse::<ToolRequestKind>()
                        .map_err(|_| refused("unknown-tool-kind"))?
                    {
                        ToolRequestKind::ManagedTool => Ok(ServerRequestMethod::ItemToolCall),
                        ToolRequestKind::ProviderCallback | ToolRequestKind::Unrecognized => {
                            Err(refused("unknown-tool-kind"))
                        }
                    }
                }
                (Some(_), Some(_), _, _) => Err(refused("ambiguous-reference")),
                (None, None, _, _) => Err(refused("missing-reference")),
                _ => Err(refused("missing-reference")),
            }
        }
    }
}

fn approval_method(kind: &str) -> Result<ServerRequestMethod, WorkerError> {
    use serde::Deserialize;
    let kind = RequestKind::deserialize(
        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(kind),
    )
    .map_err(|_| refused("unknown-approval-kind"))?;
    match kind {
        RequestKind::Exec => Ok(ServerRequestMethod::ItemCommandExecutionRequestApproval),
        RequestKind::Patch => Ok(ServerRequestMethod::ItemFileChangeRequestApproval),
        RequestKind::Permissions => Ok(ServerRequestMethod::ItemPermissionsRequestApproval),
        RequestKind::Question => Ok(ServerRequestMethod::ItemToolRequestUserInput),
        RequestKind::Elicitation => Ok(ServerRequestMethod::McpServerElicitationRequest),
        RequestKind::ToolUse
        | RequestKind::AgentQuestion
        | RequestKind::MonitorStart
        | RequestKind::BrowserOpen => Err(refused("unsupported-approval-kind")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;
    use studio_json::Map;
    use studio_json::{Value, json};
    use studio_runtime_providers::RequestId;
    use studio_runtime_store::{
        interaction_tables::ApprovalsRow, peer_update_settings::canvas_tx,
        queue_schema::ToolRequestsRow,
    };

    fn fixture(check: impl FnOnce(&Transaction<'_>) -> Result<(), WorkerError>) {
        let mut db = rusqlite::Connection::open_in_memory().expect("database");
        ToolRequestsRow::descriptor()
            .install_if_absent(&db)
            .expect("tool schema");
        ApprovalsRow::descriptor()
            .install_if_absent(&db)
            .expect("approval schema");
        let tx = db.transaction().expect("transaction");
        check(&tx).expect("check");
    }

    fn owner(tx: &Transaction<'_>, payload: &Value) -> Result<(), WorkerError> {
        let record = json!({"kind":"provider_callback","originKey":"origin","payload":payload});
        tool_request(tx, "owner", &record)
    }

    fn tool_request(tx: &Transaction<'_>, id: &str, record: &Value) -> Result<(), WorkerError> {
        canvas_tx(tx).insert(&ToolRequestsRow {
            id: id.into(),
            record: record.to_string().into(),
            agent: None,
            kind: None,
            status: None,
            request_id: None,
            origin_key: None,
            updated: None,
        })?;
        Ok(())
    }

    fn reply(value: Value) -> Frame {
        Frame::Response {
            id: RequestId::Number(7),
            body: ResponseBody::Result(value),
            fields: Map::new(),
        }
    }

    fn prepared(
        tx: &Transaction<'_>,
        reference: CallbackReference,
        value: Value,
    ) -> Result<ServerRequestMethod, WorkerError> {
        let Frame::Response {
            id: RequestId::Number(7),
            body: ResponseBody::CodexServerResult(result),
            ..
        } = prepare_reply(
            tx,
            "origin",
            reference,
            &reply(value),
            ProviderCodec::codex(),
        )?
        else {
            panic!("reply must carry a generated server result and preserve its id");
        };
        Ok(result.method())
    }

    #[test]
    fn immediate_reply_uses_accepted_method() {
        fixture(|tx| {
            owner(tx, &json!({"accepted":{"method":"currentTime/read"}}))?;
            assert_eq!(
                prepared(tx, CallbackReference::Immediate, json!({"currentTimeAt":1}))?,
                ServerRequestMethod::CurrentTimeRead
            );
            Ok(())
        });
    }

    #[test]
    fn mismatched_result_shape_is_rejected() {
        fixture(|tx| {
            owner(tx, &json!({"accepted":{"method":"currentTime/read"}}))?;
            let error = prepared(
                tx,
                CallbackReference::Immediate,
                json!({"decision":"accept"}),
            )
            .expect_err("wrong method shape");
            assert_eq!(error.to_string(), "callback-reply/result-shape");
            Ok(())
        });
    }

    #[test]
    fn managed_approval_reply_uses_card_kind() {
        fixture(|tx| {
            owner(tx, &json!({"card":"card"}))?;
            use studio_runtime_schema::approvals::{
                AgentId, ApprovalId, Detail, ProviderObject, Request, Timestamp, TurnId,
            };
            use studio_runtime_store::approvals::{self, NewApproval};
            approvals::create(
                tx,
                NewApproval {
                    id: ApprovalId("card".into()),
                    agent: AgentId("agent".into()),
                    turn: Some(TurnId("turn".into())),
                    epoch: None,
                    request: Request::Exec {
                        command: "command".into(),
                        cwd: "/project".into(),
                        reason: None,
                    },
                    delivery: None,
                    detail: Detail {
                        params: ProviderObject(Map::new()),
                        preview: None,
                        answer_error: None,
                        host_close_reason: None,
                    },
                    fire_at: None,
                },
                Timestamp(0.0),
            )
            .expect("approval");
            assert_eq!(
                prepared(tx, CallbackReference::Managed, json!({"decision":"accept"}))?,
                ServerRequestMethod::ItemCommandExecutionRequestApproval
            );
            Ok(())
        });
    }

    #[test]
    fn managed_tool_reply_uses_tool_call_method() {
        fixture(|tx| {
            owner(tx, &json!({"tool_request":"tool"}))?;
            tool_request(tx, "tool", &json!({"kind":"managed_tool"}))?;
            assert_eq!(
                prepared(
                    tx,
                    CallbackReference::Managed,
                    json!({"success":true,"contentItems":[]})
                )?,
                ServerRequestMethod::ItemToolCall
            );
            Ok(())
        });
    }

    #[test]
    fn missing_reference_is_rejected() {
        fixture(|tx| {
            owner(tx, &json!({"card":"absent"}))?;
            assert_eq!(
                prepared(tx, CallbackReference::Managed, json!({}))
                    .expect_err("missing card")
                    .to_string(),
                "callback-reply/missing-reference"
            );
            Ok(())
        });
    }

    #[test]
    fn invalid_owners_and_references_fail_closed() {
        let cases = [
            (None, CallbackReference::Immediate, "missing-owner"),
            (None, CallbackReference::Managed, "missing-owner"),
            (
                Some(json!({"accepted":{}})),
                CallbackReference::Immediate,
                "missing-method",
            ),
            (
                Some(json!({"accepted":{"method":"future/request"}})),
                CallbackReference::Immediate,
                "unknown-method",
            ),
            (
                Some(json!({})),
                CallbackReference::Managed,
                "missing-reference",
            ),
            (
                Some(json!({"card":"card","tool_request":"tool"})),
                CallbackReference::Managed,
                "ambiguous-reference",
            ),
            (
                Some(json!({"tool_request":"absent"})),
                CallbackReference::Managed,
                "missing-reference",
            ),
        ];
        for (payload, reference, code) in cases {
            fixture(|tx| {
                if let Some(payload) = payload {
                    owner(tx, &payload)?;
                }
                let error = prepared(tx, reference, json!({})).expect_err("invalid owner");
                assert_eq!(error.to_string(), format!("callback-reply/{code}"));
                Ok(())
            });
        }
    }

    #[test]
    fn generic_reply_needs_no_codex_owner() {
        fixture(|tx| {
            let frame = reply(json!({"bridge":"result"}));
            assert_eq!(
                prepare_reply(
                    tx,
                    "absent",
                    CallbackReference::Managed,
                    &frame,
                    ProviderCodec::default()
                )?,
                frame
            );
            Ok(())
        });
    }

    #[test]
    fn codex_errors_do_not_require_a_pinned_method() {
        fixture(|tx| {
            owner(tx, &json!({"accepted":{"method":"unknown/providerMethod"}}))?;
            for code in [-32601, -32603] {
                let frame = Frame::Response {
                    id: RequestId::Number(7),
                    body: ResponseBody::Error(json!({"code":code,"message":"refused"})),
                    fields: Map::new(),
                };
                let Frame::Response {
                    id,
                    body: ResponseBody::CodexError(error),
                    ..
                } = prepare_reply(
                    tx,
                    "origin",
                    CallbackReference::Immediate,
                    &frame,
                    ProviderCodec::codex(),
                )?
                else {
                    panic!("error reply must retain its typed error");
                };
                assert_eq!(id, RequestId::Number(7));
                assert_eq!(error.code, code);
            }
            Ok(())
        });
    }

    #[test]
    fn approval_kinds_map_exhaustively() {
        for (kind, method) in [
            (
                "exec",
                ServerRequestMethod::ItemCommandExecutionRequestApproval,
            ),
            ("patch", ServerRequestMethod::ItemFileChangeRequestApproval),
            (
                "permissions",
                ServerRequestMethod::ItemPermissionsRequestApproval,
            ),
            ("question", ServerRequestMethod::ItemToolRequestUserInput),
            (
                "elicitation",
                ServerRequestMethod::McpServerElicitationRequest,
            ),
        ] {
            assert_eq!(approval_method(kind).expect("Codex kind"), method);
        }
        for kind in [
            "tool_use",
            "agent_question",
            "monitor_start",
            "browser_open",
        ] {
            assert_eq!(
                approval_method(kind)
                    .expect_err("unsupported kind")
                    .to_string(),
                "callback-reply/unsupported-approval-kind"
            );
        }
        assert_eq!(
            approval_method("future_kind")
                .expect_err("unknown kind")
                .to_string(),
            "callback-reply/unknown-approval-kind"
        );
    }

    #[test]
    fn managed_tool_reference_requires_managed_tool_kind() {
        fixture(|tx| {
            owner(tx, &json!({"tool_request":"tool"}))?;
            tool_request(tx, "tool", &json!({"kind":"unrecognized"}))?;
            assert_eq!(
                prepared(
                    tx,
                    CallbackReference::Managed,
                    json!({"success":true,"contentItems":[]})
                )
                .expect_err("wrong referenced kind")
                .to_string(),
                "callback-reply/unknown-tool-kind"
            );
            Ok(())
        });
    }

    #[test]
    fn callback_method_queries_use_existing_indexes() {
        fixture(|tx| {
            for (sql, expected) in [
                (IMMEDIATE, vec!["runtime_tool_request_origin"]),
                (
                    MANAGED,
                    vec![
                        "runtime_tool_request_origin",
                        "sqlite_autoindex_approvals_1",
                        "sqlite_autoindex_runtime_tool_requests_1",
                    ],
                ),
            ] {
                let mut statement = tx.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
                let plan = statement
                    .query_map(
                        params!["origin", ToolRequestKind::ProviderCallback],
                        |row| row.get::<_, String>(3),
                    )?
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\n");
                println!("{plan}");
                for index in expected {
                    assert!(plan.contains(index), "missing {index}: {plan}");
                }
                assert!(!plan.contains("SCAN"), "unexpected table scan: {plan}");
            }
            Ok(())
        });
    }
}
