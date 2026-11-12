//! Studio command inputs. Config and SDK options are the providers' open JSON
//! contracts; request structure and permissions stay typed until persistence.
use super::*;
use serde::{Deserialize, Serialize};
use studio_runtime_providers::{Frame, RequestFrame, RequestId};
use studio_runtime_schema::provider_input::ToolDefinition;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::option_option,
    reason = "Preserve missing versus explicit null at the native storage boundary"
)]
pub struct ThreadStartInput {
    pub cwd: String,
    pub config: Option<Map<String, Value>>,
    pub service_tier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<Option<codex::AskForApproval>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Option<codex::SandboxMode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_turns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_mcp: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartInput {
    pub thread_id: String,
    pub cwd: Option<String>,
    pub service_tier: String,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<codex::ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<codex::AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<codex::SandboxPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_tools: Option<Vec<ToolDefinition>>,
    pub input: Vec<crate::turn_prepare::ProviderInput>,
    pub client_user_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_compaction_mode: Option<studio_runtime_schema::idle_compaction::Mode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_summary_roster_included: Option<bool>,
}

pub struct TitleInput {
    pub cwd: String,
    pub model: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(
    clippy::option_option,
    reason = "Stored edit settings distinguish missing from explicit null"
)]
pub struct EditAgent {
    pub cwd: String,
    pub model: Option<String>,
    #[serde(default)]
    pub fast_mode: bool,
    #[serde(default, deserialize_with = "present")]
    pub thread_config: Option<Option<Map<String, Value>>>,
    #[serde(default, deserialize_with = "present")]
    pub approval_policy: Option<Option<codex::AskForApproval>>,
    #[serde(default, deserialize_with = "present")]
    pub sandbox: Option<Option<codex::SandboxMode>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BridgeCommand {
    Thread(Box<ThreadStartInput>),
    Turn(Box<TurnStartInput>),
    Title {
        cwd: String,
        model: String,
        ephemeral: bool,
    },
    Diagnostics {},
    Fork {
        #[serde(flatten)]
        settings: Box<ThreadStartInput>,
        #[serde(rename = "lastTurnId")]
        last_turn_id: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeRequest {
    Codex(Box<codex::ClientRequest>),
    Claude(BridgeCommand),
}

impl NativeRequest {
    pub fn method(&self) -> Method {
        match self {
            Self::Codex(request) => {
                Method::from_wire(request.method().as_str()).expect("vendor method")
            }
            Self::Claude(BridgeCommand::Thread(input)) if input.thread_id.is_some() => {
                Method::ThreadResume
            }
            Self::Claude(BridgeCommand::Thread(_) | BridgeCommand::Title { .. }) => {
                Method::ThreadStart
            }
            Self::Claude(BridgeCommand::Turn(_)) => Method::TurnStart,
            Self::Claude(BridgeCommand::Fork { .. }) => Method::ThreadFork,
            Self::Claude(BridgeCommand::Diagnostics {}) => Method::ClaudeDiagnostics,
        }
    }

    /// JSON projection belongs to persistence and legacy port boundaries only.
    pub fn params(&self) -> Value {
        match self {
            Self::Codex(request) => {
                let mut wire = json!(request);
                wire.as_object_mut()
                    .expect("request object")
                    .remove("params")
                    .expect("request params")
            }
            Self::Claude(params) => json!(params),
        }
    }

    pub fn frame(&self, id: RequestId) -> Result<Frame, studio_runtime_providers::CodecError> {
        Ok(match self {
            Self::Codex(request) => {
                let mut request = request.clone();
                request.set_id(studio_runtime_providers::vendor_request_id(&id)?);
                // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/provider_adapter.rs#internal_sender_witness_native_commands_and_diagnostics
                Frame::CodexClientRequest(request)
            }
            // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/provider_adapter.rs#internal_sender_witness_native_commands_and_diagnostics
            Self::Claude(_) => Frame::Request(RequestFrame {
                id,
                method: self.method(),
                params: Some(self.params()),
                fields: Map::new(),
            }),
        })
    }
}

macro_rules! thread_settings {
    ($params:ident, $input:ident) => {
        $params.cwd = Some(Some($input.cwd));
        $params.config = Some($input.config.map(|config| config.into_iter().collect()));
        $params.model = $input.model;
        $params.service_tier = Some(Some($input.service_tier));
        $params.approval_policy = $input.approval_policy;
        $params.sandbox = $input.sandbox;
        $params.base_instructions = $input.base_instructions;
        $params.developer_instructions = $input.developer_instructions;
    };
}

pub(super) fn edit_settings(agent: &EditAgent) -> ThreadStartInput {
    ThreadStartInput {
        cwd: agent.cwd.clone(),
        config: agent
            .thread_config
            .clone()
            .unwrap_or_else(|| Some(Map::new())),
        model: agent
            .model
            .clone()
            .filter(|model| !model.is_empty())
            .map(Some),
        service_tier: crate::thread_prepare::service_tier(agent.fast_mode).into(),
        ..ThreadStartInput::default()
    }
}

impl CodexAdapter {
    pub(super) fn start_thread(
        snapshot: &Snapshot,
        input: ThreadStartInput,
    ) -> Result<NativeRequest, WorkerError> {
        let id = codex::RequestId::Variant1(Box::default());
        let request = if let Some(thread_id) = input.thread_id {
            let mut params = codex::ThreadResumeParams::new(thread_id);
            thread_settings!(params, input);
            params.exclude_turns = input.exclude_turns;
            // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/account_worker/tests/browser_skill.rs#internal_sender_witness_thread_requests_register_the_chrome_skill_until_one_reply_confirms_it
            codex::ClientRequest::ThreadResume { id, params }
        } else {
            let mut params = codex::ThreadStartParams::new();
            thread_settings!(params, input);
            params.dynamic_tools = if let Some(frozen) = &snapshot.frozen {
                Some(Some(
                    frozen
                        .dynamic_tools
                        .iter()
                        .filter(|tool| {
                            snapshot.root.can_delegate
                                || tool.definition.name != "orchestration_spawn"
                        })
                        .map(|tool| {
                            tool.native
                                .clone()
                                .ok_or_else(|| refused("provider/unprepared-frozen-codex-tool"))
                        })
                        .collect::<Result<_, _>>()?,
                ))
            } else {
                input.dynamic_tools.map(|value| Some(tools(value)))
            };
            // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/account_worker/tests/browser_skill.rs#internal_sender_witness_thread_requests_register_the_chrome_skill_until_one_reply_confirms_it
            codex::ClientRequest::ThreadStart { id, params }
        };
        Ok(NativeRequest::Codex(Box::new(request)))
    }

    pub(super) fn start_turn(
        _: &TurnInput,
        input: TurnStartInput,
    ) -> Result<NativeRequest, WorkerError> {
        let items = input
            .input
            .into_iter()
            .map(|item| match item {
                crate::turn_prepare::ProviderInput::Text { text, .. } => codex::UserInput::Text {
                    text,
                    text_elements: None,
                },
                crate::turn_prepare::ProviderInput::LocalImage { path, .. } => {
                    codex::UserInput::LocalImage { path, detail: None }
                }
            })
            .collect();
        let mut params = codex::TurnStartParams::new(items, input.thread_id);
        params.cwd = input.cwd.map(Some);
        params.service_tier = Some(Some(input.service_tier));
        params.model = Some(input.model);
        params.effort = input.effort.map(Some);
        params.approval_policy = input.approval_policy.map(Some);
        params.sandbox_policy = input.sandbox_policy.map(Some);
        params.client_user_message_id = Some(input.client_user_message_id);
        params.cyber_access_program = Some(Some(codex::CyberAccessProgram::Standard));
        Ok(NativeRequest::Codex(Box::new(
            // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/account_worker/tests/steer.rs#internal_sender_witness_steer
            codex::ClientRequest::TurnStart {
                id: codex::RequestId::Variant1(Box::default()),
                params,
            },
        )))
    }

    pub(super) fn edit_thread(
        agent: &EditAgent,
        checkpoint: &Checkpoint,
    ) -> Result<NativeRequest, WorkerError> {
        let Some(turn) = &checkpoint.turn_id else {
            let input = edit_settings(agent);
            let mut params = codex::ThreadStartParams::new();
            thread_settings!(params, input);
            return Ok(NativeRequest::Codex(Box::new(
                // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/provider_adapter.rs#internal_sender_witness_checkpoint_without_a_turn_starts_a_thread
                codex::ClientRequest::ThreadStart {
                    id: codex::RequestId::Variant1(Box::default()),
                    params,
                },
            )));
        };
        let mut params = codex::ThreadForkParams::new(
            checkpoint
                .thread_id
                .clone()
                .ok_or_else(|| refused("provider/missing-checkpoint-thread"))?,
        );
        params.last_turn_id = Some(Some(turn.clone()));
        let mut input = edit_settings(agent);
        input.approval_policy = agent.approval_policy.clone();
        input.sandbox = agent.sandbox.clone();
        thread_settings!(params, input);
        Ok(NativeRequest::Codex(Box::new(
            // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/provider_adapter.rs#internal_sender_witness_both_adapters_fork_the_checkpoint
            codex::ClientRequest::ThreadFork {
                id: codex::RequestId::Variant1(Box::default()),
                params,
            },
        )))
    }
}

pub(crate) fn approval(never: bool) -> codex::AskForApproval {
    codex::AskForApproval::Variant1(Box::new(if never {
        codex::AskForApprovalVariant1::Never
    } else {
        codex::AskForApprovalVariant1::OnRequest
    }))
}

pub(super) fn tools(tools: Vec<ToolDefinition>) -> Vec<codex::DynamicToolSpec> {
    tools
        .into_iter()
        .map(|tool| codex::DynamicToolSpec::Function {
            name: tool.name,
            description: tool.description,
            input_schema: Value::Object(tool.input_schema),
            defer_loading: tool.extra.get("deferLoading").and_then(Value::as_bool),
        })
        .collect()
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
