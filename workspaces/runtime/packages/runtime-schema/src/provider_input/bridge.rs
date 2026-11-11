//! Studio-owned Claude bridge wire DTOs. SDK types are mapped by the bridge.
//! Opaque JSON is limited to native tool values and retained vendor evidence.
use crate::raw_json::RawJson;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
/// Temporary signed move bundle; verified by the bridge canonical/hash checks.
pub type OpaqueMoveBundle = RawJson;
#[derive(Serialize, JsonSchema)]
pub struct EmptyParams {
    #[serde(flatten)]
    pub ignored: BTreeMap<String, RawJson>,
}
#[derive(Serialize)]
pub enum Never {}
impl JsonSchema for Never {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "BridgeNever".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!(false)
    }
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserQuestion {
    pub id: String,
    pub question: String,
    pub header: String,
    pub options: Vec<UserQuestionOption>,
    pub multi_select: bool,
    pub is_other: bool,
    pub is_secret: bool,
}
#[derive(Serialize, JsonSchema)]
pub struct UserQuestionOption {
    pub label: String,
    pub description: String,
}

/// Single method owner: every wire method binds its params and success result.
macro_rules! bridge_methods {
    ($( $variant:ident $field:ident : $wire:literal => ($params:ty, $result:ty), )*) => {
        #[derive(Serialize, JsonSchema)]
        pub struct BridgeMethods {
            $( #[serde(rename = $wire)] pub $field: ($params, $result), )*
        }
        #[derive(Serialize, JsonSchema)]
        #[serde(tag="method", content="params")]
        pub enum BridgeRequestBody { $( #[serde(rename=$wire)] $variant($params), )* }
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum BridgeMethod { $( $variant, )* }
        impl BridgeMethod {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire,)* }
            }
            #[must_use]
            pub fn from_wire(value: &str) -> Option<Self> {
                match value { $($wire => Some(Self::$variant),)* _ => None }
            }
        }
    };
}
bridge_methods! {
    Initialize initialize: "initialize" => (EmptyParams, InitializeResult),
    Initialized initialized: "initialized" => (EmptyParams, BridgeEmpty),
    ModelList model_list: "model/list" => (EmptyParams, ModelPage),
    ClaudeActivity claude_activity: "claude/activity" => (EmptyParams, ActivityResult),
    AccountRead account_read: "account/read" => (AccountReadParams, AccountReadResult),
    AccountRateLimitsRead account_rate_limits_read: "account/rateLimits/read" => (CwdParams, RateLimits),
    ClaudeCommands claude_commands: "claude/commands" => (CwdParams, Vec<BridgeCommand>),
    ClaudeMoveVersions claude_move_versions: "claude/moveVersions" => (EmptyParams, MoveVersions),
    ClaudeMoveIdentity claude_move_identity: "claude/moveIdentity" => (EmptyParams, MoveIdentity),
    ClaudeMoveProof claude_move_proof: "claude/moveProof" => (ThreadParams, OpaqueMoveBundle),
    ClaudeMovePreflight claude_move_preflight: "claude/movePreflight" => (MovePreflightParams, MovePreflightResult),
    ClaudeMoveExport claude_move_export: "claude/moveExport" => (ThreadParams, MoveExportResult),
    ClaudeMoveImport claude_move_import: "claude/moveImport" => (MoveImportParams, MoveImportResult),
    ClaudeRepairReport claude_repair_report: "claude/repairReport" => (ThreadParams, RepairReport),
    ClaudeFileBackups claude_file_backups: "claude/fileBackups" => (ClaudeFileBackupsParams, ClaudeFileBackupsResult),
    ClaudeState claude_state: "claude/state" => (ThreadParams, ClaudeState),
    ClaudeDiagnostics claude_diagnostics: "claude/diagnostics" => (EmptyParams, ClaudeDiagnosticsResult),
    ClaudeSettings claude_settings: "claude/settings" => (ClaudeSettingsParams, ClaudeSettingsResult),
    ThreadStart thread_start: "thread/start" => (ThreadStartParams, ThreadStartResult),
    ThreadTurnsList thread_turns_list: "thread/turns/list" => (ThreadTurnsListParams, TurnPage),
    ThreadTurnsItemsList thread_turns_items_list: "thread/turns/items/list" => (ThreadTurnsItemsListParams, ItemPage),
    ThreadList thread_list: "thread/list" => (PageParams, ThreadPage),
    SkillsExtraRootsSet skills_extra_roots_set: "skills/extraRoots/set" => (EmptyParams, Never),
    ThreadResume thread_resume: "thread/resume" => (ThreadResumeParams, ThreadResult),
    ThreadRead thread_read: "thread/read" => (ThreadReadParams, ThreadResult),
    ThreadUnsubscribe thread_unsubscribe: "thread/unsubscribe" => (ThreadParams, ThreadUnsubscribeResult),
    ThreadLoadedList thread_loaded_list: "thread/loaded/list" => (EmptyParams, StringPage),
    ThreadFork thread_fork: "thread/fork" => (ThreadForkParams, ThreadResult),
    ThreadCompactStart thread_compact_start: "thread/compact/start" => (ThreadCompactStartParams, TurnResult),
    TurnStart turn_start: "turn/start" => (TurnStartParams, TurnResult),
    TurnSteer turn_steer: "turn/steer" => (TurnSteerParams, TurnSteerResult),
    TurnInterrupt turn_interrupt: "turn/interrupt" => (TurnInterruptParams, InterruptReceipt),
    ThreadNameSet thread_name_set: "thread/name/set" => (ThreadNameSetParams, BridgeEmpty),
    ThreadBackgroundTerminalsList thread_background_terminals_list: "thread/backgroundTerminals/list" => (ThreadParams, ThreadBackgroundTerminalsListResult),
    ThreadQueueList thread_queue_list: "thread/queue/list" => (ThreadParams, ThreadQueueListResult),
    ThreadCommandExecutionBackground thread_command_execution_background: "thread/commandExecution/background" => (ThreadCommandExecutionBackgroundParams, ThreadCommandExecutionBackgroundResult),
    ClaudeStopTask claude_stop_task: "claude/stopTask" => (ClaudeStopTaskParams, BridgeEmpty),
    ClaudeShellPolicy claude_shell_policy: "claude/shellPolicy" => (ClaudeShellPolicyParams, ClaudeShellPolicyResult),
    McpServerStatusList mcp_server_status_list: "mcpServerStatus/list" => (McpServerStatusListParams, McpStatusPage),
    McpServerReconnect mcp_server_reconnect: "mcpServer/reconnect" => (ThreadNameSetParams, BridgeEmpty),
    SkillsList skills_list: "skills/list" => (SkillsListParams, SkillsListResult),
    ConfigRead config_read: "config/read" => (BTreeMap<String, RawJson>, RawJson),
    CommandExec command_exec: "command/exec" => (BTreeMap<String, RawJson>, RawJson),
    CommandExecWrite command_exec_write: "command/exec/write" => (BTreeMap<String, RawJson>, RawJson),
    CommandExecResize command_exec_resize: "command/exec/resize" => (BTreeMap<String, RawJson>, RawJson),
    CommandExecTerminate command_exec_terminate: "command/exec/terminate" => (BTreeMap<String, RawJson>, RawJson),
}

#[derive(Serialize, JsonSchema)]
#[schemars(rename = "BridgeNotifications")]
pub struct BridgeNotifications {
    #[serde(rename = "item/modelOutput/firstDelta")]
    pub first_model_output: FirstModelOutput,
    #[serde(rename = "item/started")]
    pub item_started: ItemEvent,
    #[serde(rename = "item/completed")]
    pub item_completed: ItemEvent,
    #[serde(rename = "turn/started")]
    pub turn_started: NotificationTurnStarted,
    #[serde(rename = "turn/completed")]
    pub turn_completed: NotificationTurnCompleted,
    #[serde(rename = "thread/tokenUsage/updated")]
    pub thread_token_usage_updated: NotificationThreadTokenUsageUpdated,
    #[serde(rename = "account/rateLimits/updated")]
    pub account_rate_limits_updated: RateLimits,
    #[serde(rename = "provider/generationStarted")]
    pub provider_generation_started: NotificationProviderGenerationStarted,
    #[serde(rename = "item/agentMessage/delta")]
    pub item_agent_message_delta: NotificationItemAgentMessageDelta,
    #[serde(rename = "thread/idleCompaction/updated")]
    pub thread_idle_compaction_updated: NotificationThreadIdleCompactionUpdated,
    #[serde(rename = "thread/shellPolicy/updated")]
    pub thread_shell_policy_updated: NotificationThreadShellPolicyUpdated,
    #[serde(rename = "turn/plan/updated")]
    pub turn_plan_updated: NotificationTurnPlanUpdated,
    #[serde(rename = "serverRequest/resolved")]
    pub server_request_resolved: NotificationServerRequestResolved,
    #[serde(rename = "command/exec/outputDelta")]
    pub command_exec_output_delta: RawJson,
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "method", content = "params")]
pub enum BridgeNotification {
    #[serde(rename = "item/modelOutput/firstDelta")]
    FirstModelOutput(FirstModelOutput),
    #[serde(rename = "item/started")]
    ItemStarted(ItemEvent),
    #[serde(rename = "item/completed")]
    ItemCompleted(ItemEvent),
    #[serde(rename = "turn/started")]
    TurnStarted(NotificationTurnStarted),
    #[serde(rename = "turn/completed")]
    TurnCompleted(NotificationTurnCompleted),
    #[serde(rename = "thread/tokenUsage/updated")]
    ThreadTokenUsageUpdated(NotificationThreadTokenUsageUpdated),
    #[serde(rename = "account/rateLimits/updated")]
    AccountRateLimitsUpdated(RateLimits),
    #[serde(rename = "provider/generationStarted")]
    ProviderGenerationStarted(NotificationProviderGenerationStarted),
    #[serde(rename = "item/agentMessage/delta")]
    ItemAgentMessageDelta(NotificationItemAgentMessageDelta),
    #[serde(rename = "thread/idleCompaction/updated")]
    ThreadIdleCompactionUpdated(NotificationThreadIdleCompactionUpdated),
    #[serde(rename = "thread/shellPolicy/updated")]
    ThreadShellPolicyUpdated(NotificationThreadShellPolicyUpdated),
    #[serde(rename = "turn/plan/updated")]
    TurnPlanUpdated(NotificationTurnPlanUpdated),
    #[serde(rename = "serverRequest/resolved")]
    ServerRequestResolved(NotificationServerRequestResolved),
    #[serde(rename = "command/exec/outputDelta")]
    CommandExecOutputDelta(RawJson),
}

#[derive(Serialize, JsonSchema)]
#[schemars(rename = "BridgeServerRequests")]
pub struct BridgeServerRequests {
    #[serde(rename = "item/tool/requestUserInput")]
    pub item_tool_request_user_input: (
        ItemToolRequestUserInputParams,
        ItemToolRequestUserInputResult,
    ),
    #[serde(rename = "item/commandExecution/requestApproval")]
    pub item_command_execution_request_approval: (
        ItemCommandExecutionRequestApprovalParams,
        ItemCommandExecutionRequestApprovalResult,
    ),
    #[serde(rename = "item/tool/call")]
    pub item_tool_call: (ItemToolCallParams, ItemToolCallResult),
    #[serde(rename = "mcpServer/elicitation/request")]
    pub mcp_server_elicitation_request: (
        McpServerElicitationRequestParams,
        McpServerElicitationRequestResult,
    ),
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "method", content = "params")]
pub enum BridgeServerRequestBody {
    #[serde(rename = "item/tool/requestUserInput")]
    ItemToolRequestUserInput(ItemToolRequestUserInputParams),
    #[serde(rename = "item/commandExecution/requestApproval")]
    ItemCommandExecutionRequestApproval(ItemCommandExecutionRequestApprovalParams),
    #[serde(rename = "item/tool/call")]
    ItemToolCall(ItemToolCallParams),
    #[serde(rename = "mcpServer/elicitation/request")]
    McpServerElicitationRequest(Box<McpServerElicitationRequestParams>),
}

#[derive(Serialize, JsonSchema)]
pub struct InitializeResult {
    #[serde(rename = "userAgent")]
    pub user_agent: InitializeResultUserAgent,
    #[serde(rename = "platform")]
    pub platform: InitializeResultPlatform,
    #[serde(rename = "capabilities")]
    pub capabilities: InitializeResultCapabilities,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BridgeEmpty {}

#[derive(Serialize, JsonSchema)]
pub struct ModelPage {
    #[serde(rename = "data")]
    pub data: Vec<crate::model_catalog::ProviderModel>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ActivityResult {
    #[serde(rename = "busy")]
    pub busy: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct AccountReadParams {
    #[serde(rename = "passive", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub passive: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct AccountReadResult {
    #[serde(rename = "account")]
    pub account: AccountReadResultAccount,
    #[serde(rename = "requiresOpenaiAuth")]
    pub requires_openai_auth: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct CwdParams {
    #[serde(rename = "cwd", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub cwd: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct RateLimits {
    #[serde(rename = "rateLimits")]
    pub rate_limits: RateBucket,
    #[serde(rename = "rateLimitsByLimitId")]
    pub rate_limits_by_limit_id: BTreeMap<String, RateBucket>,
    #[serde(rename = "subscriptionType", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub subscription_type: Option<Option<String>>,
    #[serde(rename = "extraUsage", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<RateLimitsExtraUsage>")]
    pub extra_usage: Option<Option<RateLimitsExtraUsage>>,
    #[serde(rename = "accountId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub account_id: Option<String>,
    #[serde(rename = "claudeRateLimit", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::usage_limits::ClaudeRateLimit")]
    pub claude_rate_limit: Option<crate::usage_limits::ClaudeRateLimit>,
}

#[derive(Serialize, JsonSchema)]
pub struct BridgeCommand {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "description")]
    pub description: String,
    #[serde(rename = "argumentHint")]
    pub argument_hint: String,
    #[serde(rename = "aliases")]
    pub aliases: Vec<String>,
    #[serde(rename = "builtin")]
    pub builtin: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct MoveVersions {
    #[serde(rename = "cli")]
    pub cli: String,
    #[serde(rename = "sdk")]
    pub sdk: String,
}

#[derive(Serialize, JsonSchema)]
pub struct MoveIdentity {
    #[serde(rename = "email")]
    pub email: String,
    #[serde(rename = "organizationId")]
    pub organization_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct MovePreflightParams {
    #[serde(rename = "proof")]
    pub proof: OpaqueMoveBundle,
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "cwd")]
    pub cwd: String,
}

#[derive(Serialize, JsonSchema)]
pub struct MovePreflightResult {
    #[serde(rename = "snapshotHash")]
    pub snapshot_hash: String,
    #[serde(rename = "optionsHash")]
    pub options_hash: String,
    #[serde(rename = "proofMethod")]
    pub proof_method: PickProofMethod,
}

#[derive(Serialize, JsonSchema)]
pub struct MoveExportResult {
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "session")]
    pub session: OpaqueMoveBundle,
}

#[derive(Serialize, JsonSchema)]
pub struct MoveImportParams {
    #[serde(rename = "session")]
    pub session: OpaqueMoveBundle,
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "cwd")]
    pub cwd: String,
}

#[derive(Serialize, JsonSchema)]
pub struct MoveImportResult {
    #[serde(rename = "thread")]
    pub thread: Thread,
}

#[derive(Serialize, JsonSchema)]
pub struct RepairReport {
    #[serde(rename = "skippedRows")]
    pub skipped_rows: f64,
    #[serde(rename = "invalidSequence")]
    pub invalid_sequence: bool,
    #[serde(rename = "status")]
    pub status: RepairReportStatus,
    #[serde(rename = "scopeConfirmed")]
    pub scope_confirmed: Option<bool>,
    #[serde(rename = "nativeHistoryConfirmed")]
    pub native_history_confirmed: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeFileBackupsParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "settledRequestIds", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<String>>")]
    pub settled_request_ids: Option<Option<Vec<String>>>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeFileBackupsResult {
    #[serde(rename = "count")]
    pub count: f64,
    #[serde(rename = "bytes")]
    pub bytes: f64,
    #[serde(rename = "maxCount")]
    pub max_count: f64,
    #[serde(rename = "maxBytes")]
    pub max_bytes: f64,
    #[serde(rename = "released")]
    pub released: f64,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeState {
    #[serde(rename = "settings")]
    pub settings: ClaudeSettings,
    #[serde(rename = "turns")]
    pub turns: Vec<StateTurnsItem>,
    #[serde(rename = "tasks")]
    pub tasks: Vec<Task>,
    #[serde(rename = "version", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub version: Option<String>,
    #[serde(rename = "nativeId")]
    pub native_id: String,
    #[serde(
        rename = "nativeSessionOutcome",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "StateNativeSessionOutcome")]
    pub native_session_outcome: Option<StateNativeSessionOutcome>,
    #[serde(rename = "nativeSessionError", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub native_session_error: Option<String>,
    #[serde(rename = "contextWindow", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub context_window: Option<f64>,
    #[serde(rename = "totalTokens", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub total_tokens: Option<f64>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeDiagnosticsResult {
    #[serde(rename = "liveQueries")]
    pub live_queries: f64,
    #[serde(rename = "activeTurns")]
    pub active_turns: f64,
    #[serde(rename = "backgroundQueries")]
    pub background_queries: f64,
    #[serde(rename = "idleQueries")]
    pub idle_queries: f64,
    #[serde(rename = "sessionCache")]
    pub session_cache: ClaudeDiagnosticsResultSessionCache,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "settings")]
    pub settings: ClaudeSettings,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsResult {
    #[serde(rename = "settings")]
    pub settings: ClaudeSettings,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadStartParams {
    #[serde(rename = "ephemeral", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub ephemeral: Option<bool>,
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub model: Option<String>,
    #[serde(rename = "approvalPolicy", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::ApprovalPolicy")]
    pub approval_policy: Option<crate::claude_permissions::ApprovalPolicy>,
    #[serde(
        rename = "activePermissionProfile",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "RawJson")]
    pub active_permission_profile: Option<RawJson>,
    #[serde(rename = "sandbox", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::SandboxMode")]
    pub sandbox: Option<crate::claude_permissions::SandboxMode>,
    #[serde(rename = "claude", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettings")]
    pub claude: Option<ClaudeSettings>,
    #[serde(
        rename = "developerInstructions",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub developer_instructions: Option<String>,
    #[serde(rename = "capabilityProfile", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub capability_profile: Option<RawJson>,
    #[serde(rename = "userMcp", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub user_mcp: Option<bool>,
    #[serde(rename = "cwd")]
    pub cwd: String,
    #[serde(rename = "dynamicTools", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<crate::provider_input::ToolDefinition>")]
    pub dynamic_tools: Option<Vec<crate::provider_input::ToolDefinition>>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadStartResult {
    #[serde(rename = "cwd", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub cwd: Option<String>,
    #[serde(rename = "ephemeral", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub ephemeral: Option<bool>,
    #[serde(rename = "approvalPolicy", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::ApprovalPolicy")]
    pub approval_policy: Option<crate::claude_permissions::ApprovalPolicy>,
    #[serde(
        rename = "activePermissionProfile",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "RawJson")]
    pub active_permission_profile: Option<RawJson>,
    #[serde(rename = "claude", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettings")]
    pub claude: Option<ClaudeSettings>,
    #[serde(
        rename = "developerInstructions",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub developer_instructions: Option<String>,
    #[serde(rename = "dynamicTools", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<crate::provider_input::ToolDefinition>")]
    pub dynamic_tools: Option<Vec<crate::provider_input::ToolDefinition>>,
    #[serde(rename = "capabilityProfile", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub capability_profile: Option<RawJson>,
    #[serde(rename = "userMcp", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub user_mcp: Option<bool>,
    #[serde(rename = "thread")]
    pub thread: Thread,
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "sandbox")]
    pub sandbox: (),
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadTurnsListParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "cursor", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub cursor: Option<Option<String>>,
    #[serde(rename = "limit", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub limit: Option<f64>,
    #[serde(rename = "sortDirection", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ThreadTurnsListParamsSortDirection")]
    pub sort_direction: Option<ThreadTurnsListParamsSortDirection>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnPage {
    #[serde(rename = "data")]
    pub data: Vec<Turn>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadTurnsItemsListParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "cursor", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub cursor: Option<Option<String>>,
    #[serde(rename = "limit", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub limit: Option<f64>,
    #[serde(rename = "turnId")]
    pub turn_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemPage {
    #[serde(rename = "data")]
    pub data: Vec<Item>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct PageParams {
    #[serde(rename = "cursor", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub cursor: Option<Option<String>>,
    #[serde(rename = "limit", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub limit: Option<f64>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadPage {
    #[serde(rename = "data")]
    pub data: Vec<Thread>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadResumeParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "ephemeral", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub ephemeral: Option<bool>,
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub model: Option<String>,
    #[serde(rename = "approvalPolicy", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::ApprovalPolicy")]
    pub approval_policy: Option<crate::claude_permissions::ApprovalPolicy>,
    #[serde(
        rename = "activePermissionProfile",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "RawJson")]
    pub active_permission_profile: Option<RawJson>,
    #[serde(rename = "sandbox", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::SandboxMode")]
    pub sandbox: Option<crate::claude_permissions::SandboxMode>,
    #[serde(rename = "claude", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettings")]
    pub claude: Option<ClaudeSettings>,
    #[serde(
        rename = "developerInstructions",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub developer_instructions: Option<String>,
    #[serde(rename = "capabilityProfile", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub capability_profile: Option<RawJson>,
    #[serde(rename = "userMcp", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub user_mcp: Option<bool>,
    #[serde(rename = "cwd", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub cwd: Option<String>,
    #[serde(rename = "dynamicTools", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<crate::provider_input::ToolDefinition>")]
    pub dynamic_tools: Option<Vec<crate::provider_input::ToolDefinition>>,
    #[serde(rename = "excludeTurns", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub exclude_turns: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadResult {
    #[serde(rename = "thread")]
    pub thread: Thread,
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub model: Option<String>,
    #[serde(rename = "sandbox")]
    pub sandbox: (),
    #[serde(rename = "approvalPolicy", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<crate::claude_permissions::ApprovalPolicy>")]
    pub approval_policy: Option<Option<crate::claude_permissions::ApprovalPolicy>>,
    #[serde(rename = "activePermissionProfile")]
    pub active_permission_profile: RawJson,
    #[serde(rename = "reattached", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub reattached: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadReadParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "includeTurns", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub include_turns: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadUnsubscribeResult {
    #[serde(rename = "status")]
    pub status: ThreadUnsubscribeResultStatus,
}

#[derive(Serialize, JsonSchema)]
pub struct StringPage {
    #[serde(rename = "data")]
    pub data: Vec<String>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadForkParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "ephemeral", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub ephemeral: Option<bool>,
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub model: Option<String>,
    #[serde(rename = "approvalPolicy", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::ApprovalPolicy")]
    pub approval_policy: Option<crate::claude_permissions::ApprovalPolicy>,
    #[serde(
        rename = "activePermissionProfile",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "RawJson")]
    pub active_permission_profile: Option<RawJson>,
    #[serde(rename = "sandbox", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::SandboxMode")]
    pub sandbox: Option<crate::claude_permissions::SandboxMode>,
    #[serde(rename = "claude", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettings")]
    pub claude: Option<ClaudeSettings>,
    #[serde(
        rename = "developerInstructions",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub developer_instructions: Option<String>,
    #[serde(rename = "capabilityProfile", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub capability_profile: Option<RawJson>,
    #[serde(rename = "userMcp", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub user_mcp: Option<bool>,
    #[serde(rename = "cwd", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub cwd: Option<String>,
    #[serde(rename = "dynamicTools", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<crate::provider_input::ToolDefinition>")]
    pub dynamic_tools: Option<Vec<crate::provider_input::ToolDefinition>>,
    #[serde(rename = "lastTurnId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub last_turn_id: Option<Option<String>>,
    #[serde(rename = "excludeTurns", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub exclude_turns: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadCompactStartParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub request_id: Option<String>,
    #[serde(rename = "idleCompaction", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ThreadCompactStartParamsIdleCompaction")]
    pub idle_compaction: Option<ThreadCompactStartParamsIdleCompaction>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnResult {
    #[serde(rename = "turn")]
    pub turn: Pick2,
    #[serde(rename = "steered", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub steered: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnStartParams {
    #[serde(rename = "approvalPolicy", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::claude_permissions::ApprovalPolicy")]
    pub approval_policy: Option<crate::claude_permissions::ApprovalPolicy>,
    #[serde(rename = "dynamicTools", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<crate::provider_input::ToolDefinition>")]
    pub dynamic_tools: Option<Vec<crate::provider_input::ToolDefinition>>,
    #[serde(rename = "input")]
    pub input: Vec<TurnParamsInputItem>,
    #[serde(
        rename = "clientUserMessageId",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub client_user_message_id: Option<String>,
    #[serde(rename = "effort", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnParamsEffort")]
    pub effort: Option<TurnParamsEffort>,
    #[serde(rename = "serviceTier", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub service_tier: Option<String>,
    #[serde(rename = "claudeCommand", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub claude_command: Option<String>,
    #[serde(rename = "idleCompaction", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ThreadCompactStartParamsIdleCompaction")]
    pub idle_compaction: Option<ThreadCompactStartParamsIdleCompaction>,
    #[serde(rename = "idleCompactionMode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub idle_compaction_mode: Option<String>,
    #[serde(
        rename = "idleSummaryRosterIncluded",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub idle_summary_roster_included: Option<bool>,
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub model: Option<Option<String>>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnSteerParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "expectedTurnId")]
    pub expected_turn_id: String,
    #[serde(rename = "clientUserMessageId")]
    pub client_user_message_id: String,
    #[serde(rename = "input")]
    pub input: Vec<TurnParamsInputItem>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnSteerResult {
    #[serde(rename = "turnId")]
    pub turn_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnInterruptParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub turn_id: Option<String>,
    #[serde(
        rename = "stopBackgroundTasks",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub stop_background_tasks: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct InterruptReceipt {
    #[serde(rename = "cancelled")]
    pub cancelled: Vec<Option<String>>,
    #[serde(rename = "still_queued")]
    pub still_queued: Vec<Option<String>>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadNameSetParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "name")]
    pub name: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadBackgroundTerminalsListResult {
    #[serde(rename = "data")]
    pub data: Vec<Task>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
    #[serde(rename = "terminals")]
    pub terminals: Vec<Task>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadQueueListResult {
    #[serde(rename = "data")]
    pub data: Vec<QueuedInput>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
    #[serde(rename = "items")]
    pub items: Vec<QueuedInput>,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadCommandExecutionBackgroundParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadCommandExecutionBackgroundResult {
    #[serde(rename = "status")]
    pub status: ThreadCommandExecutionBackgroundResultStatus,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeStopTaskParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "taskId")]
    pub task_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeShellPolicyParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "settings")]
    pub settings: crate::shell_policy::Settings,
    #[serde(rename = "eligible")]
    pub eligible: bool,
    #[serde(rename = "critical")]
    pub critical: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeShellPolicyResult {
    #[serde(rename = "watched")]
    pub watched: f64,
}

#[derive(Serialize, JsonSchema)]
pub struct McpServerStatusListParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "cursor", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub cursor: Option<Option<String>>,
    #[serde(rename = "limit", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub limit: Option<f64>,
}

#[derive(Serialize, JsonSchema)]
pub struct McpStatusPage {
    #[serde(rename = "data")]
    pub data: Vec<Omit>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct SkillsListParams {
    #[serde(rename = "cwds", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub cwds: Option<Vec<String>>,
}

#[derive(Serialize, JsonSchema)]
pub struct SkillsListResult {
    #[serde(rename = "data")]
    pub data: Vec<SkillsListResultDataItem>,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemEvent {
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub turn_id: Option<String>,
    #[serde(rename = "item")]
    pub item: crate::provider_input::items::FreshItem,
    #[serde(rename = "startedAtMs", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub started_at_ms: Option<f64>,
    #[serde(rename = "completedAtMs", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub completed_at_ms: Option<f64>,
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationTurnStarted {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turn")]
    pub turn: NotificationTurnStartedTurn,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationTurnCompleted {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turn")]
    pub turn: NotificationTurnCompletedTurn,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationThreadTokenUsageUpdated {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub turn_id: Option<Option<String>>,
    #[serde(rename = "studioUsage")]
    pub studio_usage: crate::usage::UsageObservation,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationProviderGenerationStarted {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "responseId")]
    pub response_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationItemAgentMessageDelta {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(rename = "delta")]
    pub delta: String,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationThreadIdleCompactionUpdated {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub turn_id: Option<String>,
    #[serde(rename = "report", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::idle_compaction::CacheReport")]
    pub report: Option<crate::idle_compaction::CacheReport>,
    #[serde(rename = "preparedSummary", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "NotificationThreadIdleCompactionUpdatedPreparedSummary")]
    pub prepared_summary: Option<NotificationThreadIdleCompactionUpdatedPreparedSummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationThreadShellPolicyUpdated {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "report")]
    pub report: ShellPolicyReport,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationTurnPlanUpdated {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "plan")]
    pub plan: Vec<PlanStep>,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationServerRequestResolved {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "NotificationServerRequestResolvedRequestId")]
    pub request_id: Option<NotificationServerRequestResolvedRequestId>,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemToolRequestUserInputParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(rename = "questions")]
    pub questions: Vec<UserQuestion>,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemToolRequestUserInputResult {
    #[serde(rename = "answers", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, ItemToolRequestUserInputResultAnswersValue>")]
    pub answers: Option<BTreeMap<String, ItemToolRequestUserInputResultAnswersValue>>,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemCommandExecutionRequestApprovalParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(rename = "command")]
    pub command: String,
    #[serde(rename = "cwd")]
    pub cwd: String,
    #[serde(rename = "reason")]
    pub reason: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemCommandExecutionRequestApprovalResult {
    #[serde(rename = "decision")]
    pub decision: ItemCommandExecutionRequestApprovalResultDecision,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemToolCallParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub turn_id: Option<String>,
    #[serde(rename = "callId")]
    pub call_id: String,
    #[serde(rename = "tool")]
    pub tool: String,
    #[serde(rename = "arguments")]
    pub arguments: BTreeMap<String, RawJson>,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemToolCallResult {
    #[serde(rename = "success", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub success: Option<bool>,
    #[serde(rename = "contentItems", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<ItemToolCallResultContentItemsItem>")]
    pub content_items: Option<Vec<ItemToolCallResultContentItemsItem>>,
}

#[derive(Serialize, JsonSchema)]
pub struct McpServerElicitationRequestParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub turn_id: Option<String>,
    #[serde(rename = "request")]
    pub request: ElicitationRequest,
    #[serde(rename = "serverName")]
    pub server_name: String,
    #[serde(rename = "message")]
    pub message: String,
    #[serde(rename = "mode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "McpServerElicitationRequestParamsMode")]
    pub mode: Option<McpServerElicitationRequestParamsMode>,
    #[serde(rename = "url", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub url: Option<String>,
    #[serde(rename = "elicitationId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub elicitation_id: Option<String>,
    #[serde(rename = "requestedSchema", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, RawJson>")]
    pub requested_schema: Option<BTreeMap<String, RawJson>>,
    #[serde(rename = "title", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub title: Option<String>,
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub display_name: Option<String>,
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub description: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct McpServerElicitationRequestResult {
    #[serde(rename = "action")]
    pub action: McpServerElicitationRequestResultAction,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "McpServerElicitationRequestResultMeta")]
    pub _meta: Option<McpServerElicitationRequestResultMeta>,
    #[serde(rename = "content", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<BTreeMap<String, McpServerElicitationRequestResultContentValue>>")]
    pub content: Option<Option<BTreeMap<String, McpServerElicitationRequestResultContentValue>>>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, RawJson>,
}

#[derive(Serialize, JsonSchema)]
pub enum InitializeResultUserAgent {
    #[serde(rename = "studio-claude-bridge")]
    StudioClaudeBridge,
}

#[derive(Serialize, JsonSchema)]
pub enum InitializeResultPlatform {
    #[serde(rename = "aix")]
    Aix,
    #[serde(rename = "android")]
    Android,
    #[serde(rename = "darwin")]
    Darwin,
    #[serde(rename = "freebsd")]
    Freebsd,
    #[serde(rename = "haiku")]
    Haiku,
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "openbsd")]
    Openbsd,
    #[serde(rename = "sunos")]
    Sunos,
    #[serde(rename = "win32")]
    Win32,
    #[serde(rename = "cygwin")]
    Cygwin,
    #[serde(rename = "netbsd")]
    Netbsd,
}

#[derive(Serialize, JsonSchema)]
pub struct InitializeResultCapabilities {
    #[serde(rename = "claudeVersion")]
    pub claude_version: f64,
    #[serde(rename = "passiveAccountRead")]
    pub passive_account_read: bool,
    #[serde(rename = "activityRead")]
    pub activity_read: bool,
    #[serde(rename = "steerCompletionReceipts")]
    pub steer_completion_receipts: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct AccountReadResultAccount {
    #[serde(rename = "type")]
    pub r#type: AccountReadResultAccountType,
    #[serde(rename = "email", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub email: Option<String>,
    #[serde(rename = "organizationUuid", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub organization_uuid: Option<String>,
    #[serde(rename = "accountUuid", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub account_uuid: Option<String>,
    #[serde(rename = "_credentialIdentity")]
    pub _credential_identity: String,
    #[serde(rename = "planType", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub plan_type: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct RateBucket {
    #[serde(rename = "primary", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<RateBucketPrimary>")]
    pub primary: Option<Option<RateBucketPrimary>>,
    #[serde(rename = "secondary", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<RateBucketPrimary>")]
    pub secondary: Option<Option<RateBucketPrimary>>,
    #[serde(rename = "limitId")]
    pub limit_id: String,
    #[serde(rename = "limitName")]
    pub limit_name: String,
    #[serde(rename = "claudeRateLimit", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "crate::usage_limits::ClaudeRateLimit")]
    pub claude_rate_limit: Option<crate::usage_limits::ClaudeRateLimit>,
}

#[derive(Serialize, JsonSchema)]
pub struct RateLimitsExtraUsage {
    #[serde(rename = "is_enabled")]
    pub is_enabled: bool,
    #[serde(rename = "monthly_limit")]
    pub monthly_limit: Option<f64>,
    #[serde(rename = "used_credits")]
    pub used_credits: Option<f64>,
    #[serde(rename = "utilization")]
    pub utilization: Option<f64>,
    #[serde(rename = "currency", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub currency: Option<Option<String>>,
}

#[derive(Serialize, JsonSchema)]
pub enum PickProofMethod {
    #[serde(rename = "saved_snapshot_and_current_ordered_mcp_catalogs")]
    SavedSnapshotAndCurrentOrderedMcpCatalogs,
    #[serde(rename = "saved_snapshot_and_identical_studio_options")]
    SavedSnapshotAndIdenticalStudioOptions,
}

#[derive(Serialize, JsonSchema)]
pub struct Thread {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "cwd")]
    pub cwd: String,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub created_at: Option<f64>,
    #[serde(rename = "updatedAt", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub updated_at: Option<f64>,
    #[serde(rename = "preview")]
    pub preview: String,
    #[serde(rename = "name")]
    pub name: Option<String>,
    #[serde(rename = "historyVersion")]
    pub history_version: String,
    #[serde(rename = "turns")]
    pub turns: Vec<Turn>,
    #[serde(rename = "journalRepair", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "JournalRepairReport")]
    pub journal_repair: Option<JournalRepairReport>,
    #[serde(rename = "status")]
    pub status: ThreadStatus,
    #[serde(rename = "modelProvider")]
    pub model_provider: AccountReadResultAccountType,
}

#[derive(Serialize, JsonSchema)]
pub enum RepairReportStatus {
    #[serde(rename = "needs_repair")]
    NeedsRepair,
    #[serde(rename = "repaired")]
    Repaired,
    #[serde(rename = "none")]
    None,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettings {
    #[serde(rename = "env", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, String>")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(rename = "permissionMode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<crate::claude_permissions::PermissionMode>")]
    pub permission_mode: Option<Option<crate::claude_permissions::PermissionMode>>,
    #[serde(rename = "thinking", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<bool>")]
    pub thinking: Option<Option<bool>>,
    #[serde(rename = "autoCompactWindow", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<f64>")]
    pub auto_compact_window: Option<Option<f64>>,
    #[serde(rename = "mcpAutoBackground", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub mcp_auto_background: Option<bool>,
    #[serde(rename = "sandbox", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandbox")]
    pub sandbox: Option<ClaudeSettingsSandbox>,
    #[serde(
        rename = "nativeReviewerEnvironment",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "BTreeMap<String, String>")]
    pub native_reviewer_environment: Option<BTreeMap<String, String>>,
}

#[derive(Serialize, JsonSchema)]
pub struct StateTurnsItem {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub status: Option<String>,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnError")]
    pub error: Option<TurnError>,
    #[serde(rename = "canRestoreFiles")]
    pub can_restore_files: bool,
    #[serde(rename = "restoreFilesUnavailableReason")]
    pub restore_files_unavailable_reason: Option<String>,
    #[serde(rename = "text")]
    pub text: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Task {
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub description: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TaskType")]
    pub r#type: Option<TaskType>,
    #[serde(rename = "task_id", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub task_id: Option<String>,
    #[serde(rename = "run_id", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub run_id: Option<String>,
    #[serde(rename = "tool_use_id", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub tool_use_id: Option<String>,
    #[serde(rename = "subagent_type", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub subagent_type: Option<String>,
    #[serde(rename = "is_backgrounded", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub is_backgrounded: Option<bool>,
    #[serde(rename = "spawn_depth", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub spawn_depth: Option<f64>,
    #[serde(rename = "parent_task_id", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub parent_task_id: Option<String>,
    #[serde(rename = "task_type", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub task_type: Option<String>,
    #[serde(rename = "workflow_name", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub workflow_name: Option<String>,
    #[serde(rename = "prompt", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub prompt: Option<String>,
    #[serde(rename = "skip_transcript", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub skip_transcript: Option<bool>,
    #[serde(rename = "ambient", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub ambient: Option<bool>,
    #[serde(rename = "uuid", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub uuid: Option<String>,
    #[serde(rename = "session_id", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub session_id: Option<String>,
    #[serde(rename = "end_time", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub end_time: Option<f64>,
    #[serde(rename = "total_paused_ms", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub total_paused_ms: Option<f64>,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub error: Option<String>,
    #[serde(rename = "output_file", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub output_file: Option<String>,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TaskStatus")]
    pub status: Option<TaskStatus>,
}

#[derive(Serialize, JsonSchema)]
pub enum StateNativeSessionOutcome {
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeDiagnosticsResultSessionCache {
    #[serde(rename = "cachedSessions")]
    pub cached_sessions: f64,
    #[serde(rename = "retainedBytes")]
    pub retained_bytes: f64,
    #[serde(rename = "pendingWrites")]
    pub pending_writes: f64,
    #[serde(rename = "unsavedSessions")]
    pub unsaved_sessions: f64,
    #[serde(rename = "journalBytes")]
    pub journal_bytes: f64,
}

#[derive(Serialize, JsonSchema)]
pub enum ThreadTurnsListParamsSortDirection {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

#[derive(Serialize, JsonSchema)]
pub struct Turn {
    #[serde(rename = "synthetic", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub synthetic: Option<bool>,
    #[serde(rename = "id")]
    pub id: String,
    #[serde(
        rename = "clientUserMessageId",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub client_user_message_id: Option<String>,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub status: Option<String>,
    #[serde(rename = "startOutcome", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub start_outcome: Option<String>,
    #[serde(rename = "items")]
    pub items: Vec<Item>,
    #[serde(rename = "nativeSessionId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub native_session_id: Option<String>,
    #[serde(
        rename = "nativeHistoryDetached",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub native_history_detached: Option<bool>,
    #[serde(rename = "nativeMessageId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub native_message_id: Option<String>,
    #[serde(
        rename = "fileCheckpointMessageId",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub file_checkpoint_message_id: Option<String>,
    #[serde(
        rename = "fileCheckpointSource",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "TurnFileCheckpointSource")]
    pub file_checkpoint_source: Option<TurnFileCheckpointSource>,
    #[serde(rename = "idleCompaction", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub idle_compaction: Option<bool>,
    #[serde(rename = "interrupted", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub interrupted: Option<bool>,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnError")]
    pub error: Option<TurnError>,
    #[serde(rename = "stallError", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnError")]
    pub stall_error: Option<TurnError>,
    #[serde(rename = "limitError", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnLimitError")]
    pub limit_error: Option<TurnLimitError>,
    #[serde(rename = "apiErrorInfo", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub api_error_info: Option<Option<String>>,
    #[serde(rename = "claudeFailure", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailure")]
    pub claude_failure: Option<TurnClaudeFailure>,
}

#[derive(Serialize, JsonSchema)]
pub struct LegacyItemFields {
    #[serde(rename = "nativeId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub native_id: Option<String>,
    #[serde(rename = "nativeIds", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub native_ids: Option<Vec<String>>,
    #[serde(
        rename = "nativeSubmissionAttemptId",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub native_submission_attempt_id: Option<String>,
    #[serde(rename = "delivery", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub delivery: Option<String>,
    #[serde(rename = "deliveryStatus", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub delivery_status: Option<String>,
    #[serde(rename = "content", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub content: Option<RawJson>,
    #[serde(rename = "text", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub text: Option<String>,
    #[serde(rename = "thinking", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ItemThinking")]
    pub thinking: Option<ItemThinking>,
    #[serde(rename = "nativeSubagent", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub native_subagent: Option<bool>,
    #[serde(
        rename = "canMoveToBackground",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub can_move_to_background: Option<bool>,
    #[serde(rename = "backgroundTaskId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub background_task_id: Option<String>,
    #[serde(rename = "backgroundedByUser", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub backgrounded_by_user: Option<bool>,
    #[serde(rename = "outputFile", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub output_file: Option<String>,
    #[serde(rename = "stalled", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub stalled: Option<bool>,
    #[serde(rename = "compactionMetadata", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub compaction_metadata: Option<RawJson>,
    #[serde(
        rename = "idleCompactionRequestId",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub idle_compaction_request_id: Option<String>,
    #[serde(
        rename = "idleCompactionOnReturn",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub idle_compaction_on_return: Option<bool>,
    #[serde(rename = "clientId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub client_id: Option<String>,
    #[serde(rename = "elapsedSeconds", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub elapsed_seconds: Option<f64>,
    #[serde(rename = "phase", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub phase: Option<String>,
    #[serde(rename = "studioNotice", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub studio_notice: Option<bool>,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub error: Option<String>,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub status: Option<String>,
    #[serde(rename = "tool", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub tool: Option<String>,
    #[serde(rename = "arguments", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, RawJson>")]
    pub arguments: Option<BTreeMap<String, RawJson>>,
    #[serde(rename = "command", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub command: Option<RawJson>,
    #[serde(rename = "cwd", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub cwd: Option<String>,
    #[serde(rename = "aggregatedOutput", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub aggregated_output: Option<String>,
    #[serde(rename = "exitCode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<f64>")]
    pub exit_code: Option<Option<f64>>,
    #[serde(rename = "result", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub result: Option<RawJson>,
    #[serde(rename = "server", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub server: Option<String>,
    #[serde(rename = "changes", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<ItemChangesItem>")]
    pub changes: Option<Vec<ItemChangesItem>>,
}

#[derive(Serialize, JsonSchema)]
pub enum ThreadUnsubscribeResultStatus {
    #[serde(rename = "unsubscribed")]
    Unsubscribed,
    #[serde(rename = "notLoaded")]
    NotLoaded,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadCompactStartParamsIdleCompaction {
    #[serde(rename = "prefix")]
    pub prefix: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "mode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub mode: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct Pick2 {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub status: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum TurnParamsInputItem {
    Value0(TurnParamsInputItem0),
    Value1(TurnParamsInputItem1),
}

#[derive(Serialize, JsonSchema)]
pub enum TurnParamsEffort {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "max")]
    Max,
}

#[derive(Serialize, JsonSchema)]
pub struct QueuedInput {
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub id: Option<String>,
    #[serde(rename = "status")]
    pub status: QueuedInputStatus,
}

#[derive(Serialize, JsonSchema)]
pub enum ThreadCommandExecutionBackgroundResultStatus {
    #[serde(rename = "backgrounded")]
    Backgrounded,
    #[serde(rename = "not_running")]
    NotRunning,
}

#[derive(Serialize, JsonSchema)]
pub struct Omit {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "status")]
    pub status: OmitStatus,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub error: Option<String>,
    #[serde(rename = "serverInfo", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "OmitServerInfo")]
    pub server_info: Option<OmitServerInfo>,
    #[serde(rename = "scope", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub scope: Option<String>,
    #[serde(rename = "source", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub source: Option<String>,
    #[serde(rename = "tools", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<OmitToolsItem>")]
    pub tools: Option<Vec<OmitToolsItem>>,
}

#[derive(Serialize, JsonSchema)]
pub struct SkillsListResultDataItem {
    #[serde(rename = "cwd")]
    pub cwd: String,
    #[serde(rename = "skills")]
    pub skills: Vec<SkillsListResultDataItemSkillsItem>,
    #[serde(rename = "errors")]
    pub errors: Vec<SkillsListResultDataItemErrorsItem>,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationTurnStartedTurn {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "status")]
    pub status: NotificationTurnStartedTurnStatus,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationTurnCompletedTurn {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "status")]
    pub status: NotificationTurnCompletedTurnStatus,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnError")]
    pub error: Option<TurnError>,
    #[serde(
        rename = "clientUserMessageId",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String")]
    pub client_user_message_id: Option<String>,
    #[serde(rename = "startOutcome", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "NotificationTurnCompletedTurnStartOutcome")]
    pub start_outcome: Option<NotificationTurnCompletedTurnStartOutcome>,
    #[serde(rename = "unappliedSteers", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<Option<String>>")]
    pub unapplied_steers: Option<Vec<Option<String>>>,
}

#[derive(Serialize, JsonSchema)]
pub struct NotificationThreadIdleCompactionUpdatedPreparedSummary {
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ShellPolicyReport {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "taskId")]
    pub task_id: String,
    #[serde(rename = "kind")]
    pub kind: ShellPolicyReportKind,
    #[serde(rename = "message")]
    pub message: String,
    #[serde(rename = "at")]
    pub at: f64,
    #[serde(rename = "outputFile", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub output_file: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct PlanStep {
    #[serde(rename = "step", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub step: Option<RawJson>,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub status: Option<RawJson>,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum NotificationServerRequestResolvedRequestId {
    Value0(String),
    Value1(f64),
}

#[derive(Serialize, JsonSchema)]
pub struct ItemToolRequestUserInputResultAnswersValue {
    #[serde(rename = "answers")]
    pub answers: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub enum ItemCommandExecutionRequestApprovalResultDecision {
    #[serde(rename = "accept")]
    Accept,
    #[serde(rename = "acceptForSession")]
    AcceptForSession,
    #[serde(rename = "decline")]
    Decline,
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemToolCallResultContentItemsItem {
    #[serde(rename = "text", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub text: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, RawJson>,
}

#[derive(Serialize, JsonSchema)]
pub struct ElicitationRequest {
    #[serde(rename = "serverName")]
    pub server_name: String,
    #[serde(rename = "message")]
    pub message: String,
    #[serde(rename = "mode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "McpServerElicitationRequestParamsMode")]
    pub mode: Option<McpServerElicitationRequestParamsMode>,
    #[serde(rename = "url", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub url: Option<String>,
    #[serde(rename = "elicitationId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub elicitation_id: Option<String>,
    #[serde(rename = "requestedSchema", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, RawJson>")]
    pub requested_schema: Option<BTreeMap<String, RawJson>>,
    #[serde(rename = "title", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub title: Option<String>,
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub display_name: Option<String>,
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub description: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub enum McpServerElicitationRequestParamsMode {
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "form")]
    Form,
}

#[derive(Serialize, JsonSchema)]
pub enum McpServerElicitationRequestResultAction {
    #[serde(rename = "accept")]
    Accept,
    #[serde(rename = "decline")]
    Decline,
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(Serialize, JsonSchema)]
pub struct McpServerElicitationRequestResultMeta {
    #[serde(rename = "progressToken", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "McpServerElicitationRequestResultMetaProgressToken")]
    pub progress_token: Option<McpServerElicitationRequestResultMetaProgressToken>,
    #[serde(
        rename = "io.modelcontextprotocol/related-task",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "McpServerElicitationRequestResultMetaIoModelcontextprotocolRelatedTask")]
    pub io_modelcontextprotocol_related_task:
        Option<McpServerElicitationRequestResultMetaIoModelcontextprotocolRelatedTask>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, RawJson>,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum McpServerElicitationRequestResultContentValue {
    Value0(String),
    Value1(f64),
    Value2(bool),
    Value3(bool),
    Value4(Vec<String>),
}

#[derive(Serialize, JsonSchema)]
pub enum AccountReadResultAccountType {
    #[serde(rename = "claude")]
    Claude,
}

#[derive(Serialize, JsonSchema)]
pub struct RateBucketPrimary {
    #[serde(rename = "resetsAt", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<f64>")]
    pub resets_at: Option<Option<f64>>,
    #[serde(rename = "usedPercent", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<f64>")]
    pub used_percent: Option<Option<f64>>,
    #[serde(rename = "windowDurationMins", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<f64>")]
    pub window_duration_mins: Option<Option<f64>>,
}

#[derive(Serialize, JsonSchema)]
pub struct JournalRepairReport {
    #[serde(rename = "status")]
    pub status: JournalRepairReportStatus,
    #[serde(rename = "skippedRows")]
    pub skipped_rows: f64,
    #[serde(rename = "invalidSequence")]
    pub invalid_sequence: bool,
    #[serde(rename = "scopeConfirmed")]
    pub scope_confirmed: bool,
    #[serde(rename = "nativeHistoryConfirmed")]
    pub native_history_confirmed: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct ThreadStatus {
    #[serde(rename = "type")]
    pub r#type: ThreadStatusType,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandbox {
    #[serde(rename = "enabled", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub enabled: Option<bool>,
    #[serde(rename = "failIfUnavailable", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub fail_if_unavailable: Option<bool>,
    #[serde(
        rename = "autoAllowBashIfSandboxed",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub auto_allow_bash_if_sandboxed: Option<bool>,
    #[serde(
        rename = "allowUnsandboxedCommands",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub allow_unsandboxed_commands: Option<bool>,
    #[serde(rename = "network", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxNetwork")]
    pub network: Option<ClaudeSettingsSandboxNetwork>,
    #[serde(rename = "filesystem", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxFilesystem")]
    pub filesystem: Option<ClaudeSettingsSandboxFilesystem>,
    #[serde(rename = "credentials", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentials")]
    pub credentials: Option<ClaudeSettingsSandboxCredentials>,
    #[serde(rename = "ignoreViolations", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, Vec<String>>")]
    pub ignore_violations: Option<BTreeMap<String, Vec<String>>>,
    #[serde(
        rename = "enableWeakerNestedSandbox",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub enable_weaker_nested_sandbox: Option<bool>,
    #[serde(
        rename = "allowNestedUserNamespaces",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub allow_nested_user_namespaces: Option<bool>,
    #[serde(
        rename = "enableWeakerNetworkIsolation",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub enable_weaker_network_isolation: Option<bool>,
    #[serde(rename = "allowAppleEvents", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub allow_apple_events: Option<bool>,
    #[serde(rename = "excludedCommands", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub excluded_commands: Option<Vec<String>>,
    #[serde(rename = "ripgrep", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxRipgrep")]
    pub ripgrep: Option<ClaudeSettingsSandboxRipgrep>,
    #[serde(rename = "bwrapPath", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub bwrap_path: Option<String>,
    #[serde(rename = "socatPath", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub socat_path: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, RawJson>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnError {
    #[serde(rename = "resetsAt", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub resets_at: Option<f64>,
    #[serde(rename = "message")]
    pub message: String,
    #[serde(rename = "codexErrorInfo", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnErrorCodexErrorInfo")]
    pub codex_error_info: Option<TurnErrorCodexErrorInfo>,
    #[serde(rename = "claudeFailure", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailure")]
    pub claude_failure: Option<TurnClaudeFailure>,
    #[serde(rename = "data", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnErrorData")]
    pub data: Option<TurnErrorData>,
}

#[derive(Serialize, JsonSchema)]
pub enum TaskType {
    #[serde(rename = "system")]
    System,
}

#[derive(Serialize, JsonSchema)]
pub enum TaskStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "killed")]
    Killed,
    #[serde(rename = "paused")]
    Paused,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnFileCheckpointSource {
    #[serde(rename = "nativeId", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub native_id: Option<String>,
    #[serde(rename = "cwd")]
    pub cwd: String,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnLimitError {
    #[serde(rename = "codexErrorInfo", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub codex_error_info: Option<String>,
    #[serde(rename = "resetsAt", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub resets_at: Option<f64>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnClaudeFailure {
    #[serde(rename = "apiError", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureApiError")]
    pub api_error: Option<TurnClaudeFailureApiError>,
    #[serde(rename = "apiErrorCode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub api_error_code: Option<String>,
    #[serde(rename = "apiErrorOrigin", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureApiErrorOrigin")]
    pub api_error_origin: Option<TurnClaudeFailureApiErrorOrigin>,
    #[serde(rename = "apiErrorStatus", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub api_error_status: Option<f64>,
    #[serde(rename = "effort", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureEffort")]
    pub effort: Option<TurnClaudeFailureEffort>,
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureError")]
    pub error: Option<TurnClaudeFailureError>,
    #[serde(rename = "maxContextTokens", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub max_context_tokens: Option<f64>,
    #[serde(rename = "media", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureMedia")]
    pub media: Option<TurnClaudeFailureMedia>,
    #[serde(rename = "mediaReason", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureMediaReason")]
    pub media_reason: Option<TurnClaudeFailureMediaReason>,
    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub message: Option<String>,
    #[serde(rename = "promptTokens", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub prompt_tokens: Option<f64>,
    #[serde(rename = "provider", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureProvider")]
    pub provider: Option<TurnClaudeFailureProvider>,
    #[serde(rename = "remedy", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "TurnClaudeFailureRemedy")]
    pub remedy: Option<TurnClaudeFailureRemedy>,
    #[serde(rename = "title", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub title: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ItemThinking {
    Value0(String),
    Value1(bool),
    Value2(bool),
}

#[derive(Serialize, JsonSchema)]
pub struct ItemChangesItem {
    #[serde(rename = "path", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub path: Option<RawJson>,
    #[serde(rename = "kind")]
    pub kind: ItemChangesItemKind,
    #[serde(rename = "diff")]
    pub diff: RawJson,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnParamsInputItem0 {
    #[serde(rename = "type")]
    pub r#type: TurnParamsInputItem0Type,
    #[serde(rename = "text")]
    pub text: String,
    #[serde(rename = "inputOrigin", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "super::ProviderInputOrigin")]
    pub input_origin: Option<super::ProviderInputOrigin>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnParamsInputItem1 {
    #[serde(rename = "type")]
    pub r#type: TurnParamsInputItem1Type,
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "inputOrigin", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "super::ProviderInputOrigin")]
    pub input_origin: Option<super::ProviderInputOrigin>,
}

#[derive(Serialize, JsonSchema)]
pub enum QueuedInputStatus {
    #[serde(rename = "queued")]
    Queued,
}

#[derive(Serialize, JsonSchema)]
pub enum OmitStatus {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "needs-auth")]
    NeedsAuth,
}

#[derive(Serialize, JsonSchema)]
pub struct OmitServerInfo {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "version")]
    pub version: String,
}

#[derive(Serialize, JsonSchema)]
pub struct OmitToolsItem {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub description: Option<String>,
    #[serde(rename = "annotations", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "OmitToolsItemAnnotations")]
    pub annotations: Option<OmitToolsItemAnnotations>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, RawJson>")]
    pub _meta: Option<BTreeMap<String, RawJson>>,
}

#[derive(Serialize, JsonSchema)]
pub struct SkillsListResultDataItemSkillsItem {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "description")]
    pub description: String,
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct SkillsListResultDataItemErrorsItem {
    #[serde(rename = "message")]
    pub message: String,
}

#[derive(Serialize, JsonSchema)]
pub enum NotificationTurnStartedTurnStatus {
    #[serde(rename = "inProgress")]
    InProgress,
}

#[derive(Serialize, JsonSchema)]
pub enum NotificationTurnCompletedTurnStatus {
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Serialize, JsonSchema)]
pub enum NotificationTurnCompletedTurnStartOutcome {
    #[serde(rename = "not_applied")]
    NotApplied,
}

#[derive(Serialize, JsonSchema)]
pub enum ShellPolicyReportKind {
    #[serde(rename = "quiet")]
    Quiet,
    #[serde(rename = "cleanup_requested")]
    CleanupRequested,
    #[serde(rename = "cleanup_stopped")]
    CleanupStopped,
    #[serde(rename = "cleanup_unknown")]
    CleanupUnknown,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum McpServerElicitationRequestResultMetaProgressToken {
    Value0(String),
    Value1(f64),
}

#[derive(Serialize, JsonSchema)]
pub struct McpServerElicitationRequestResultMetaIoModelcontextprotocolRelatedTask {
    #[serde(rename = "taskId")]
    pub task_id: String,
}

#[derive(Serialize, JsonSchema)]
pub enum JournalRepairReportStatus {
    #[serde(rename = "needs_repair")]
    NeedsRepair,
    #[serde(rename = "repaired")]
    Repaired,
}

#[derive(Serialize, JsonSchema)]
pub enum ThreadStatusType {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "idle")]
    Idle,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxNetwork {
    #[serde(rename = "allowedDomains", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(rename = "deniedDomains", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub denied_domains: Option<Vec<String>>,
    #[serde(rename = "strictAllowlist", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub strict_allowlist: Option<bool>,
    #[serde(
        rename = "allowManagedDomainsOnly",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub allow_managed_domains_only: Option<bool>,
    #[serde(rename = "allowUnixSockets", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub allow_unix_sockets: Option<Vec<String>>,
    #[serde(
        rename = "allowAllUnixSockets",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub allow_all_unix_sockets: Option<bool>,
    #[serde(rename = "allowLocalBinding", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub allow_local_binding: Option<bool>,
    #[serde(rename = "allowMachLookup", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub allow_mach_lookup: Option<Vec<String>>,
    #[serde(rename = "httpProxyPort", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub http_proxy_port: Option<f64>,
    #[serde(rename = "socksProxyPort", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub socks_proxy_port: Option<f64>,
    #[serde(rename = "tlsTerminate", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxNetworkTlsTerminate")]
    pub tls_terminate: Option<ClaudeSettingsSandboxNetworkTlsTerminate>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxFilesystem {
    #[serde(rename = "allowWrite", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub allow_write: Option<Vec<String>>,
    #[serde(rename = "denyWrite", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub deny_write: Option<Vec<String>>,
    #[serde(rename = "denyRead", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub deny_read: Option<Vec<String>>,
    #[serde(rename = "allowRead", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub allow_read: Option<Vec<String>>,
    #[serde(
        rename = "allowManagedReadPathsOnly",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub allow_managed_read_paths_only: Option<bool>,
    #[serde(rename = "disabled", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub disabled: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxCredentials {
    #[serde(rename = "files", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<ClaudeSettingsSandboxCredentialsFilesItem>")]
    pub files: Option<Vec<ClaudeSettingsSandboxCredentialsFilesItem>>,
    #[serde(rename = "envVars", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<ClaudeSettingsSandboxCredentialsEnvVarsItem>")]
    pub env_vars: Option<Vec<ClaudeSettingsSandboxCredentialsEnvVarsItem>>,
    #[serde(
        rename = "allowPlaintextInject",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    pub allow_plaintext_inject: Option<bool>,
    #[serde(rename = "awsPairs", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<ClaudeSettingsSandboxCredentialsAwsPairsItem>")]
    pub aws_pairs: Option<Vec<ClaudeSettingsSandboxCredentialsAwsPairsItem>>,
    #[serde(rename = "sigv4", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsSigv4")]
    pub sigv4: Option<ClaudeSettingsSandboxCredentialsSigv4>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxRipgrep {
    #[serde(rename = "command")]
    pub command: String,
    #[serde(rename = "args", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub args: Option<Vec<String>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum TurnErrorCodexErrorInfo {
    Value0(String),
    Value1(TurnErrorCodexErrorInfo1),
    Value2(TurnErrorCodexErrorInfo2),
    Value3(TurnErrorCodexErrorInfo3),
    Value4(TurnErrorCodexErrorInfo4),
    Value5(TurnErrorCodexErrorInfo5),
    Value6(BTreeMap<String, RawJson>),
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorData {
    #[serde(rename = "claudeToolStall", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "StallReport")]
    pub claude_tool_stall: Option<StallReport>,
    #[serde(rename = "claudeStopTimeout", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub claude_stop_timeout: Option<bool>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, RawJson>,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureApiError {
    #[serde(rename = "max_output_tokens")]
    MaxOutputTokens,
    #[serde(rename = "dlp_request_denied")]
    DlpRequestDenied,
    #[serde(rename = "claude_code_version_too_old")]
    ClaudeCodeVersionTooOld,
    #[serde(rename = "safety_monitor_blocked")]
    SafetyMonitorBlocked,
    #[serde(rename = "effort_requires_thinking")]
    EffortRequiresThinking,
    #[serde(rename = "advisor_incompatible")]
    AdvisorIncompatible,
    #[serde(rename = "tool_history_mismatch")]
    ToolHistoryMismatch,
    #[serde(rename = "autocompact_thrashing")]
    AutocompactThrashing,
    #[serde(rename = "pdf_too_large")]
    PdfTooLarge,
    #[serde(rename = "pdf_password_protected")]
    PdfPasswordProtected,
    #[serde(rename = "media_removed")]
    MediaRemoved,
    #[serde(rename = "no_response")]
    NoResponse,
    #[serde(rename = "tls_untrusted_ca")]
    TlsUntrustedCa,
    #[serde(rename = "gateway_content_type")]
    GatewayContentType,
    #[serde(rename = "provider_credentials")]
    ProviderCredentials,
    #[serde(rename = "gateway_signin_required")]
    GatewaySigninRequired,
    #[serde(rename = "gateway_session_expired")]
    GatewaySessionExpired,
    #[serde(rename = "api_key_auth_disabled")]
    ApiKeyAuthDisabled,
    #[serde(rename = "org_disabled_credential")]
    OrgDisabledCredential,
    #[serde(rename = "invalid_credential_header")]
    InvalidCredentialHeader,
    #[serde(rename = "model_requires_usage_credits")]
    ModelRequiresUsageCredits,
    #[serde(rename = "long_context_credits_required")]
    LongContextCreditsRequired,
    #[serde(rename = "consent_unanswered")]
    ConsentUnanswered,
    #[serde(rename = "no_allowed_fallback")]
    NoAllowedFallback,
    #[serde(rename = "model_substitution_disabled")]
    ModelSubstitutionDisabled,
    #[serde(rename = "field_not_granted")]
    FieldNotGranted,
    #[serde(rename = "usage_limit_reached")]
    UsageLimitReached,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureApiErrorOrigin {
    #[serde(rename = "first_party")]
    FirstParty,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureEffort {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    Xhigh,
    #[serde(rename = "max")]
    Max,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureError {
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "max_output_tokens")]
    MaxOutputTokens,
    #[serde(rename = "authentication_failed")]
    AuthenticationFailed,
    #[serde(rename = "oauth_org_not_allowed")]
    OauthOrgNotAllowed,
    #[serde(rename = "account_on_hold")]
    AccountOnHold,
    #[serde(rename = "verification_required")]
    VerificationRequired,
    #[serde(rename = "billing_error")]
    BillingError,
    #[serde(rename = "rate_limit")]
    RateLimit,
    #[serde(rename = "overloaded")]
    Overloaded,
    #[serde(rename = "invalid_request")]
    InvalidRequest,
    #[serde(rename = "model_not_found")]
    ModelNotFound,
    #[serde(rename = "server_error")]
    ServerError,
    #[serde(rename = "cloud_credential_error")]
    CloudCredentialError,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureMedia {
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "document")]
    Document,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureMediaReason {
    #[serde(rename = "unprocessable")]
    Unprocessable,
    #[serde(rename = "unsupported_by_model")]
    UnsupportedByModel,
    #[serde(rename = "media_budget")]
    MediaBudget,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureProvider {
    #[serde(rename = "bedrock")]
    Bedrock,
    #[serde(rename = "anthropicAws")]
    AnthropicAws,
    #[serde(rename = "mantle")]
    Mantle,
    #[serde(rename = "anthropicGoogleCloud")]
    AnthropicGoogleCloud,
    #[serde(rename = "vertex")]
    Vertex,
    #[serde(rename = "foundry")]
    Foundry,
    #[serde(rename = "gateway")]
    Gateway,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnClaudeFailureRemedy {
    #[serde(rename = "refresh_command")]
    RefreshCommand,
    #[serde(rename = "refresh_credentials")]
    RefreshCredentials,
    #[serde(rename = "adc")]
    Adc,
    #[serde(rename = "gateway_token")]
    GatewayToken,
    #[serde(rename = "host_managed")]
    HostManaged,
    #[serde(rename = "model_access")]
    ModelAccess,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemChangesItemKind {
    #[serde(rename = "type")]
    pub r#type: String,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnParamsInputItem0Type {
    #[serde(rename = "text")]
    Text,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnParamsInputItem1Type {
    #[serde(rename = "localImage")]
    LocalImage,
}

#[derive(Serialize, JsonSchema)]
pub struct OmitToolsItemAnnotations {
    #[serde(rename = "readOnly", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub read_only: Option<bool>,
    #[serde(rename = "destructive", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub destructive: Option<bool>,
    #[serde(rename = "openWorld", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub open_world: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxNetworkTlsTerminate {
    #[serde(rename = "caCertPath", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub ca_cert_path: Option<String>,
    #[serde(rename = "caKeyPath", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub ca_key_path: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxCredentialsFilesItem {
    #[serde(rename = "path")]
    pub path: String,
    #[serde(rename = "mode")]
    pub mode: ClaudeSettingsSandboxCredentialsFilesItemMode,
    #[serde(rename = "extract", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub extract: Option<String>,
    #[serde(rename = "onExtractNoMatch", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsFilesItemOnExtractNoMatch")]
    pub on_extract_no_match: Option<ClaudeSettingsSandboxCredentialsFilesItemOnExtractNoMatch>,
    #[serde(rename = "decode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsFilesItemDecode")]
    pub decode: Option<ClaudeSettingsSandboxCredentialsFilesItemDecode>,
    #[serde(rename = "maskClaims", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub mask_claims: Option<Vec<String>>,
    #[serde(rename = "maskDuplicates", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub mask_duplicates: Option<bool>,
    #[serde(rename = "injectHosts", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub inject_hosts: Option<Vec<String>>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxCredentialsEnvVarsItem {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "mode")]
    pub mode: ClaudeSettingsSandboxCredentialsFilesItemMode,
    #[serde(rename = "extract", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub extract: Option<String>,
    #[serde(rename = "onExtractNoMatch", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsFilesItemOnExtractNoMatch")]
    pub on_extract_no_match: Option<ClaudeSettingsSandboxCredentialsFilesItemOnExtractNoMatch>,
    #[serde(rename = "decode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsFilesItemDecode")]
    pub decode: Option<ClaudeSettingsSandboxCredentialsFilesItemDecode>,
    #[serde(rename = "maskClaims", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub mask_claims: Option<Vec<String>>,
    #[serde(rename = "injectHosts", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<String>")]
    pub inject_hosts: Option<Vec<String>>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxCredentialsAwsPairsItem {
    #[serde(rename = "accessKeyIdVar")]
    pub access_key_id_var: String,
    #[serde(rename = "secretAccessKeyVar")]
    pub secret_access_key_var: String,
    #[serde(rename = "sessionTokenVar", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub session_token_var: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ClaudeSettingsSandboxCredentialsSigv4 {
    #[serde(rename = "streaming", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsSigv4Streaming")]
    pub streaming: Option<ClaudeSettingsSandboxCredentialsSigv4Streaming>,
    #[serde(rename = "presigned", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsSigv4Streaming")]
    pub presigned: Option<ClaudeSettingsSandboxCredentialsSigv4Streaming>,
    #[serde(rename = "sigv4a", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ClaudeSettingsSandboxCredentialsSigv4Streaming")]
    pub sigv4a: Option<ClaudeSettingsSandboxCredentialsSigv4Streaming>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorCodexErrorInfo1 {
    #[serde(rename = "httpConnectionFailed")]
    pub http_connection_failed: TurnErrorCodexErrorInfo1HttpConnectionFailed,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorCodexErrorInfo2 {
    #[serde(rename = "responseStreamConnectionFailed")]
    pub response_stream_connection_failed: TurnErrorCodexErrorInfo1HttpConnectionFailed,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorCodexErrorInfo3 {
    #[serde(rename = "responseStreamDisconnected")]
    pub response_stream_disconnected: TurnErrorCodexErrorInfo1HttpConnectionFailed,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorCodexErrorInfo4 {
    #[serde(rename = "responseTooManyFailedAttempts")]
    pub response_too_many_failed_attempts: TurnErrorCodexErrorInfo1HttpConnectionFailed,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorCodexErrorInfo5 {
    #[serde(rename = "activeTurnNotSteerable")]
    pub active_turn_not_steerable: TurnErrorCodexErrorInfo5ActiveTurnNotSteerable,
}

#[derive(Serialize, JsonSchema)]
pub struct StallReport {
    #[serde(rename = "timeoutMs")]
    pub timeout_ms: Option<f64>,
    #[serde(rename = "tool", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub tool: Option<String>,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: f64,
    #[serde(rename = "silentMs")]
    pub silent_ms: f64,
    #[serde(rename = "toolUseId")]
    pub tool_use_id: String,
}

#[derive(Serialize, JsonSchema)]
pub enum ClaudeSettingsSandboxCredentialsFilesItemMode {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "mask")]
    Mask,
}

#[derive(Serialize, JsonSchema)]
pub enum ClaudeSettingsSandboxCredentialsFilesItemOnExtractNoMatch {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "warn")]
    Warn,
}

#[derive(Serialize, JsonSchema)]
pub enum ClaudeSettingsSandboxCredentialsFilesItemDecode {
    #[serde(rename = "jwt")]
    Jwt,
}

#[derive(Serialize, JsonSchema)]
pub enum ClaudeSettingsSandboxCredentialsSigv4Streaming {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "passthrough")]
    Passthrough,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorCodexErrorInfo1HttpConnectionFailed {
    #[serde(rename = "httpStatusCode", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<f64>")]
    pub http_status_code: Option<Option<f64>>,
}

#[derive(Serialize, JsonSchema)]
pub struct TurnErrorCodexErrorInfo5ActiveTurnNotSteerable {
    #[serde(rename = "turnKind")]
    pub turn_kind: TurnErrorCodexErrorInfo5ActiveTurnNotSteerableTurnKind,
}

#[derive(Serialize, JsonSchema)]
pub enum TurnErrorCodexErrorInfo5ActiveTurnNotSteerableTurnKind {
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "compact")]
    Compact,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FirstModelOutput {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Item {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub item_type: Option<String>,
    #[serde(flatten)]
    pub fields: LegacyItemFields,
}
#[derive(Serialize, JsonSchema)]
pub struct HistoricalItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(flatten)]
    pub fields: LegacyItemFields,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalItemEvent {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub turn_id: Option<String>,
    pub item: HistoricalItem,
    #[serde(rename = "startedAtMs", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub started_at_ms: Option<f64>,
    #[serde(rename = "completedAtMs", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "f64")]
    pub completed_at_ms: Option<f64>,
}
#[derive(Serialize, JsonSchema)]
pub struct BridgeHistoricalNotifications {
    #[serde(rename = "item/started")]
    pub started: HistoricalItemEvent,
    #[serde(rename = "item/completed")]
    pub completed: HistoricalItemEvent,
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "method", content = "params")]
pub enum BridgeHistoricalNotification {
    #[serde(rename = "item/started")]
    Started(HistoricalItemEvent),
    #[serde(rename = "item/completed")]
    Completed(HistoricalItemEvent),
}

#[derive(Serialize, JsonSchema)]
pub struct BridgeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "super::RequestId")]
    pub id: Option<super::RequestId>,
    #[serde(flatten)]
    pub body: BridgeRequestBody,
}
#[derive(Serialize, JsonSchema)]
pub struct BridgeServerRequest {
    pub id: super::RequestId,
    #[serde(flatten)]
    pub body: BridgeServerRequestBody,
}
#[derive(Serialize, JsonSchema)]
pub struct BridgeError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RawJson")]
    pub data: Option<RawJson>,
}
#[derive(Serialize, JsonSchema)]
pub struct BridgeSuccessResponse {
    pub id: super::RequestId,
    pub result: RawJson,
}
#[derive(Serialize, JsonSchema)]
pub struct BridgeErrorResponse {
    pub id: super::RequestId,
    pub error: BridgeError,
}
#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum BridgeResponse {
    Success(BridgeSuccessResponse),
    Error(BridgeErrorResponse),
}
#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum BridgeOutbound {
    Notification(Box<BridgeNotification>),
    Historical(Box<BridgeHistoricalNotification>),
    Request(BridgeServerRequest),
    Response(BridgeResponse),
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BridgeProviderOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<f64>")]
    pub auto_compact_window: Option<Option<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Vec<BridgeCustomModel>")]
    pub custom_models: Option<Vec<BridgeCustomModel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "BTreeMap<String, Option<String>>")]
    pub extra_args: Option<BTreeMap<String, Option<String>>>,
}
#[derive(Serialize, JsonSchema)]
pub struct BridgeCustomModel {
    pub id: String,
    pub label: String,
}
