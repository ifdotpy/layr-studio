//! A committed conversation change must decode as the agent response it returns.
#![allow(clippy::disallowed_methods)]

use serde_json::json;
use studio_runtime_http::generated::AgentResponse;
use studio_runtime_schema::agent_progress::AgentCommandProgress;
use studio_runtime_services::sync_views::{AgentViewContext, agent_entity_view};

#[test]
fn lead_with_a_running_command_projects_to_the_agent_response() {
    let record = json!({
        "id": "lead", "isLead": true, "rootId": "lead", "epoch": 3,
        "status": "waiting", "accountKey": "pm-account", "provider": "claude",
        "compactions": 0, "name": "PM", "naming": "manual", "autoWake": false,
        "workerDefaults": {
            "accountKey": "worker-account", "model": "gpt-6.1-sol", "effort": "medium",
            "fastMode": false, "daybreakEnabled": false,
            "cyberAccessProgram": "standard", "delegationDepth": 1
        }
    });
    let context = AgentViewContext {
        launcher_alive: true,
        command_progress: vec![AgentCommandProgress {
            monitor_id: "monitor".into(),
            stage: "just test-fast".into(),
            started_at: 1.0,
            last_output_line: "running".into(),
        }],
        ..AgentViewContext::default()
    };
    let view = agent_entity_view(&record, &context).expect("agent view");
    let decoded = serde_json::from_value::<AgentResponse>(view);
    assert!(
        decoded.is_ok(),
        "a running command must not make the committed response fail: {:?}",
        decoded.err()
    );
}
