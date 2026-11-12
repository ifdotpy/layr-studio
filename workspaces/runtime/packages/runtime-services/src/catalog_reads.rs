//! Pure projections used by the skills and capabilities discovery endpoints.

use crate::account_session_runtime::SessionOutcome as ReceiptOutcome;
use std::collections::{BTreeSet, HashSet};
use std::time::Duration;
use studio_json::{Value, json};
use studio_runtime_providers::Method;

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderRequest {
    pub method: String,
    pub params: Value,
}

pub type CatalogRequest = (&'static str, ProviderRequest);
pub type CatalogResult = (&'static str, Result<Value, CatalogReadError>);

/// Return the full native tool inventory carried by the selected preparation catalog.
pub fn managed_tool_catalog(
    catalog: &crate::thread_prepare::CatalogSnapshot,
) -> Result<Vec<Value>, &'static str> {
    crate::thread_prepare::full_tool_catalog(catalog)
        .into_iter()
        .map(|tool| studio_json::to_value(tool).map_err(|_| "Invalid tool catalog"))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogReadError {
    ProviderExchangeFailed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityAgentSnapshot {
    model: String,
    effort: Value,
    role: String,
}

impl CapabilityAgentSnapshot {
    pub fn from_record(agent: &Value) -> Result<Self, &'static str> {
        let model = agent
            .get("model")
            .and_then(Value::as_str)
            .ok_or("Invalid stored agent")?
            .to_owned();
        let role = agent
            .get("role")
            .and_then(Value::as_str)
            .filter(|role| matches!(*role, "orchestrator" | "implementer" | "reviewer"))
            .ok_or("Invalid stored agent")?
            .to_owned();
        Ok(Self {
            model,
            effort: agent.get("effort").cloned().unwrap_or(Value::Null),
            role,
        })
    }
}

/// Read discovery methods through the selected account's already retained
/// provider session. The account worker owns the durable intent and receipt;
/// this adapter only accepts a written reply bound to the captured connection.
pub async fn account_session_exchange(
    session: &dyn crate::account_session_runtime::AccountSessionPort,
    account: &str,
    requests: Vec<CatalogRequest>,
    timeout: Duration,
) -> Vec<CatalogResult> {
    use crate::account_session_runtime::{ProviderRequest as SessionRequest, ProviderResponse};

    const OPERATION_PREFIX: &str = "catalog-discovery";
    static REQUEST_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let mut responses = Vec::with_capacity(requests.len());
    for (label, request) in requests {
        let response = async {
            let scope = session
                .current_scope(account)
                .ok_or(CatalogReadError::ProviderExchangeFailed)?;
            let deadline = std::time::Instant::now() + timeout;
            let sequence = REQUEST_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let operation = format!(
                "{OPERATION_PREFIX}:{}:{sequence}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| CatalogReadError::ProviderExchangeFailed)?
                    .as_nanos()
            );
            let method = Method::from_wire(&request.method)
                .ok_or(CatalogReadError::ProviderExchangeFailed)?;
            // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/catalog_reads.rs#internal_sender_witness_catalog_account_session_exchange
            let receipt = session
                // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/catalog_reads.rs#internal_sender_witness_catalog_account_session_exchange
                .provider_request(SessionRequest {
                    root_request_id: String::new(),
                    admitted_request: None,
                    account: account.to_owned(),
                    operation: operation.clone(),
                    actor: None,
                    expected_scope: Some(scope.clone()),
                    deleted_actor_session: None,
                    method,
                    params: request.params,
                    deadline,
                })
                .await
                .map_err(|_| CatalogReadError::ProviderExchangeFailed)?;
            let ReceiptOutcome::Written { ticket, response } = receipt.outcome else {
                return Err(CatalogReadError::ProviderExchangeFailed);
            };
            if receipt.scope != scope
                || receipt.operation != operation
                || ticket.method != method
                || ticket.generation != scope.generation
            {
                return Err(CatalogReadError::ProviderExchangeFailed);
            }
            match response {
                ProviderResponse::Reply(response) => Ok(response),
                ProviderResponse::Rejected(_) => Err(CatalogReadError::ProviderExchangeFailed),
            }
        }
        .await;
        responses.push((label, response));
    }
    responses
}

/// Project a provider `skills/list` response onto the requested working directory.
pub fn skills_for_cwd(cwd: &str, response: &Value) -> Result<Value, &'static str> {
    let groups = response
        .get("data")
        .and_then(Value::as_array)
        .ok_or("The provider did not return a skill catalog")?;
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let mut names = HashSet::new();
    for group in groups {
        if group.get("cwd").and_then(Value::as_str) != Some(cwd) {
            continue;
        }
        if let Some(error_value) = group.get("errors") {
            let items = error_value
                .as_array()
                .ok_or("The provider returned an invalid skill catalog")?;
            errors.extend(items.iter().filter_map(|item| {
                let message = item
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| item.as_str());
                message
                    .filter(|message| !message.is_empty())
                    .map(str::to_owned)
            }));
        }
        let Some(skill_value) = group.get("skills") else {
            continue;
        };
        let items = skill_value
            .as_array()
            .ok_or("The provider returned an invalid skill catalog")?;
        for item in items {
            if item.get("enabled") == Some(&Value::Bool(false)) {
                continue;
            }
            let (Some(name), Some(path)) = (
                item.get("name")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty()),
                item.get("path")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty()),
            ) else {
                continue;
            };
            if !names.insert(name.to_owned()) {
                continue;
            }
            let description = item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            skills.push(json!({"name": name, "path": path, "description": description}));
        }
    }
    skills.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .cmp(&right["name"].as_str().unwrap_or_default().to_lowercase())
    });
    Ok(json!({"skills": skills, "errors": errors}))
}

/// Host agents expose their persisted working directory. Historical VM agents are unavailable.
pub fn host_finder_view(agent: &Value) -> Option<Value> {
    if agent.get("executionMode").and_then(Value::as_str) == Some("vm") {
        return None;
    }
    Some(json!({
        "kind": "native",
        "path": agent.get("cwd").cloned().unwrap_or(Value::Null),
    }))
}

/// Construct the stable capability fields before the independent provider probes run.
pub fn capabilities_base(
    agent_id: &str,
    agent: &CapabilityAgentSnapshot,
    managed: Vec<Value>,
    observed: impl IntoIterator<Item = String>,
    at: f64,
) -> Value {
    let mut observed = observed.into_iter().collect::<BTreeSet<_>>();
    let observed = std::mem::take(&mut observed)
        .into_iter()
        .collect::<Vec<_>>();
    json!({
        "agent": agent_id,
        "at": at,
        "managed": managed,
        "observed": observed,
        "skills": [],
        "apps": [],
        "plugins": [],
        "appsCursor": null,
        "servers": [],
        "errors": [],
        "model": agent.model,
        "effort": agent.effort,
        "role": agent.role,
        "nativeInventory": "Codex selects native tools per model and configuration. Observed calls appear below.",
        "observedNative": observed,
        "mcp": [],
        "skillsCursor": null,
        "serversCursor": null
    })
}

pub fn append_capability_error(result: &mut Value, label: &str, error: &str) {
    if let Some(errors) = result.get_mut("errors").and_then(Value::as_array_mut) {
        errors.push(Value::String(format!("{label}: {error}")));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CapabilityAgentSnapshot, ProviderRequest, account_session_exchange, capabilities_base,
        host_finder_view, skills_for_cwd,
    };
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    use studio_json::json;

    struct RetainedDiscovery {
        calls: Arc<Mutex<Vec<studio_json::Value>>>,
    }

    impl crate::account_session_runtime::AccountSessionPort for RetainedDiscovery {
        fn current_scope(
            &self,
            account: &str,
        ) -> Option<crate::account_session_runtime::ConnectionScope> {
            Some(crate::account_session_runtime::ConnectionScope {
                account: account.to_owned(),
                connection: "fixture".to_owned(),
                generation: 7,
            })
        }

        fn provider_request<'a>(
            &'a self,
            request: crate::account_session_runtime::ProviderRequest,
        ) -> crate::account_session_runtime::AccountSessionFuture<
            'a,
            crate::account_session_runtime::SessionReceipt,
        > {
            Box::pin(async move {
                crate::internal_sender_witness::request(request.method, &request.params);
                use crate::account_session_runtime::SessionOutcome as ReceiptOutcome;
                use crate::account_session_runtime::{ProviderResponse, SessionReceipt};
                let scope = request
                    .expected_scope
                    .clone()
                    .expect("captured fixture scope");
                assert!(request.deadline > std::time::Instant::now());
                let method = request.method;
                let operation = request.operation.clone();
                let response = if method == studio_runtime_providers::Method::SkillsList {
                    json!({"data":[{"cwd":"/repo","skills":[]}]})
                } else {
                    json!({"servers":[],"nextCursor":null})
                };
                self.calls.lock().expect("discovery calls").push(json!({
                    "account":request.account,
                    "operation":operation,
                    "method":method.as_str(),
                    "params":request.params,
                }));
                Ok(SessionReceipt {
                    scope,
                    operation,
                    replayed: false,
                    outcome: ReceiptOutcome::Written {
                        ticket: studio_runtime_providers::RequestTicket {
                            generation: 7,
                            id: studio_runtime_providers::RequestId::String("fixture".to_owned()),
                            method,
                        },
                        response: ProviderResponse::Reply(response),
                    },
                })
            })
        }
    }

    #[test]
    fn skills_match_cwd_filter_disabled_dedup_and_error_projection() {
        let response = json!({"data":[
            {"cwd":"/other","skills":[{"name":"ignored","path":"/ignored"}]},
            {"cwd":"/repo","errors":[{"message":"broken"},"warning",{"message":""}],"skills":[
                {"name":"Beta","path":"/b","description":17},
                {"name":"alpha","path":"/a"},
                {"name":"alpha","path":"/duplicate"},
                {"name":"off","path":"/off","enabled":false},
                {"name":"missing-path"},
                {"name":"","path":"/empty"}
            ]}
        ]});
        assert_eq!(
            skills_for_cwd("/repo", &response).expect("valid provider response"),
            json!({"skills":[
                {"name":"alpha","path":"/a","description":""},
                {"name":"Beta","path":"/b","description":""}
            ],"errors":["broken","warning"]})
        );
    }

    #[test]
    fn malformed_skill_catalog_is_reported_and_host_finder_view_is_record_only() {
        assert_eq!(
            skills_for_cwd("/repo", &json!({"data":null})),
            Err("The provider did not return a skill catalog")
        );
        assert_eq!(
            host_finder_view(&json!({"cwd":"/repo","executionMode":"native"})),
            Some(json!({"kind":"native","path":"/repo"}))
        );
        assert_eq!(host_finder_view(&json!({"executionMode":"vm"})), None);
        assert_eq!(
            skills_for_cwd(
                "/repo",
                &json!({"data":[{"cwd":"/repo","errors":null,"skills":[{"name":"hidden","path":"/hidden"}]}]})
            ),
            Err("The provider returned an invalid skill catalog")
        );
        assert_eq!(
            skills_for_cwd("/repo", &json!({"data":[{"cwd":"/repo","skills":null}]})),
            Err("The provider returned an invalid skill catalog")
        );
    }

    #[test]
    fn capabilities_projection_keeps_scoped_observations_and_provider_aliases() {
        let mut result = capabilities_base(
            "agent-a",
            &CapabilityAgentSnapshot::from_record(
                &json!({"model":"model-x","role":"implementer","effort":null}),
            )
            .expect("valid agent snapshot"),
            vec![json!({"name":"managed-tool"})],
            ["tool-z".into(), "tool-a".into(), "tool-z".into()],
            123.0,
        );
        crate::provider_adapter::apply_capability_result(
            Some(studio_runtime_schema::ProviderKind::Codex),
            &mut result,
            "servers",
            &json!({"servers":[{"name":"mcp"}],"nextCursor":"next"}),
        )
        .expect("capability projection");
        assert_eq!(result["observed"], json!(["tool-a", "tool-z"]));
        assert_eq!(result["mcp"], json!([{"name":"mcp"}]));
        assert_eq!(result["serversCursor"], "next");
        assert_eq!(result["agent"], "agent-a");
    }

    #[test]
    fn malformed_capability_agent_is_rejected_by_the_snapshot_constructor() {
        assert!(
            CapabilityAgentSnapshot::from_record(&json!({"model":"m","role":"unknown"})).is_err()
        );
        assert!(CapabilityAgentSnapshot::from_record(&json!({"model":"m","role":"lead"})).is_err());
        assert!(
            CapabilityAgentSnapshot::from_record(
                &json!({"model":"m","role":"reviewer","frozenNativeParams":{"dynamicTools":{}}})
            )
            .is_ok()
        );
    }

    #[tokio::test]
    async fn internal_sender_witness_catalog_account_session_exchange() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let session = RetainedDiscovery {
            calls: Arc::clone(&calls),
        };
        let responses = account_session_exchange(
            &session,
            "claude-account",
            vec![
                (
                    "skills",
                    ProviderRequest {
                        method: "skills/list".to_owned(),
                        params: json!({"cwds":["/repo"],"forceReload":false}),
                    },
                ),
                (
                    "servers",
                    ProviderRequest {
                        method: "mcpServerStatus/list".to_owned(),
                        params: json!({"threadId":"thread-1","limit":100}),
                    },
                ),
            ],
            Duration::from_secs(5),
        )
        .await;
        assert_eq!(
            responses,
            vec![
                ("skills", Ok(json!({"data":[{"cwd":"/repo","skills":[]}]}))),
                ("servers", Ok(json!({"servers":[],"nextCursor":null}))),
            ]
        );
        let calls = calls.lock().expect("discovery calls");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["account"], "claude-account");
        assert_eq!(calls[0]["method"], "skills/list");
        assert_eq!(calls[1]["method"], "mcpServerStatus/list");
        assert_ne!(calls[0]["operation"], calls[1]["operation"]);
    }
}
