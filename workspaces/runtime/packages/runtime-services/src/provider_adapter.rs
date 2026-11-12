//! One provider adapter per `ProviderKind`.
//!
//! The JSON-RPC wire (`thread/*`, `turn/*`, `model/list`, `account/read`) is shared
//! by both peers and stays in the account worker. An adapter owns only what sits
//! beside that wire: how the peer process is launched and the provider-specific
//! decisions that callers used to branch on with provider strings.

use crate::account_driver::auth_cache::CredentialLocation;
use crate::account_driver::{AuthMetadata, AuthProfile, AuthStatus, account_server_root};
use crate::account_worker::WorkerError;
use crate::claude_bridge::{BridgeSnapshot, ClaudeLaunchProfile, ResolvedBridge};
use crate::continuation_runtime::classify_usage_failure;
use crate::thread_prepare::{Snapshot, TurnInput};
use crate::usage_resume::Cause as UsageCause;
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};
use studio_json::{Map, Value, json};
use studio_runtime_process::ProviderLaunchSpec;
pub use studio_runtime_providers::Frame as ProviderEvent;
use studio_runtime_providers::Method;
use studio_runtime_providers::codex;
pub use studio_runtime_providers::{ProviderCapability, ProviderKind};
use studio_runtime_store::path_identity::CanonicalPath;
mod command;
pub(crate) use command::approval;
pub use command::{
    BridgeCommand, Checkpoint, EditAgent, NativeRequest, ThreadStartInput, TitleInput,
    TurnStartInput,
};

/// Inputs for launching the retained provider session of one host account.
pub struct CheckedLaunch<'a> {
    /// The account's registry profile (Claude reads its pinned identity and options).
    pub profile: Option<&'a ClaudeLaunchProfile>,
    /// Provider home (`CODEX_HOME` for Codex).
    pub home: String,
    /// The account has its own home and must not use ambient API keys.
    pub isolated: bool,
    /// Selected Codex executable; `None` uses `codex` from `PATH`.
    pub codex_executable: Option<&'a str>,
    /// Directory holding the Claude bridge (`bridge.mjs` and its SDK).
    pub claude_bridge_dir: &'a Path,
    /// Registry key of the account that the session serves.
    pub account: &'a str,
    /// The state root.
    pub root: PathBuf,
    pub environment: BTreeMap<String, String>,
}

/// A launch spec and the provider executable that the launched session runs.
pub struct ProviderLaunch {
    pub spec: ProviderLaunchSpec,
    pub executable: Option<String>,
}

/// Provider operations and runtime policies, decided exhaustively for every provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderFeature {
    StartThread,
    StartTurn,
    SetTitle,
    EditThread,
    NativeHistory,
    ToolsOnEveryTurn,
    VoiceTools,
    Daybreak,
    WritableSandboxRoots,
    TurnEndClosesSteer,
    FrozenReasoningConfig,
    PreserveContextOnReattach,
    UnsubmittedInspection,
    SteerCompletionReceipts,
    NativeMcpConfiguration,
    BrowserConfiguration,
    HostWorktreeFence,
    IdleSummaryReturn,
    PinnedAccountLaunch,
    BundledExecutable,
    AccountSessionLogin,
    TurnCommitGuard,
    AttributedInput,
    NativeRelease,
}

impl ProviderFeature {
    /// Single owner of internal operation and policy availability.
    #[must_use]
    pub const fn support(
        self,
        provider: ProviderKind,
    ) -> studio_runtime_schema::provider_kind::ProviderSupport {
        use studio_runtime_schema::provider_kind::ProviderSupport::{Supported, Unsupported};
        match provider {
            ProviderKind::Codex => match self {
                Self::StartThread => Supported,
                Self::StartTurn => Supported,
                Self::SetTitle => Supported,
                Self::EditThread => Supported,
                Self::NativeHistory => Supported,
                Self::ToolsOnEveryTurn => Unsupported {
                    reason: "Codex registers tools at thread preparation instead of repeating them on every turn",
                },
                Self::VoiceTools => Supported,
                Self::Daybreak => Supported,
                Self::WritableSandboxRoots => Supported,
                Self::TurnEndClosesSteer => Supported,
                Self::FrozenReasoningConfig => Supported,
                Self::PreserveContextOnReattach => Unsupported {
                    reason: "Codex reads native token usage instead of retaining bridge context state",
                },
                Self::UnsubmittedInspection => Supported,
                Self::SteerCompletionReceipts => Unsupported {
                    reason: "Codex closes steer admission at native turn completion without bridge receipts",
                },
                Self::NativeMcpConfiguration => Supported,
                Self::BrowserConfiguration => Supported,
                Self::HostWorktreeFence => Unsupported {
                    reason: "Codex enforces writable paths through its native sandbox configuration",
                },
                Self::IdleSummaryReturn => Unsupported {
                    reason: "Codex has no bridge resume-return dialog to answer when idle",
                },
                Self::PinnedAccountLaunch => Unsupported {
                    reason: "Codex selects its executable and home without a Claude login profile",
                },
                Self::BundledExecutable => Supported,
                Self::AccountSessionLogin => Supported,
                Self::TurnCommitGuard => Unsupported {
                    reason: "Codex persists native turns without the bridge commit guard",
                },
                Self::AttributedInput => Unsupported {
                    reason: "Codex user input does not use bridge account-attribution fields",
                },
                Self::NativeRelease => Supported,
            },
            ProviderKind::Claude => match self {
                Self::StartThread => Supported,
                Self::StartTurn => Supported,
                Self::SetTitle => Supported,
                Self::EditThread => Supported,
                Self::NativeHistory => Unsupported {
                    reason: "Claude history is read from SDK transcripts rather than Codex thread history",
                },
                Self::ToolsOnEveryTurn => Supported,
                Self::VoiceTools => Unsupported {
                    reason: "Studio voice tools require the Codex realtime session integration",
                },
                Self::Daybreak => Unsupported {
                    reason: "Daybreak is a native Codex turn policy",
                },
                Self::WritableSandboxRoots => Unsupported {
                    reason: "Claude permissions use the bridge tool policy instead of Codex sandbox roots",
                },
                Self::TurnEndClosesSteer => Unsupported {
                    reason: "Claude keeps steer admission open until bridge completion receipts arrive",
                },
                Self::FrozenReasoningConfig => Unsupported {
                    reason: "Claude applies reasoning through session flag settings instead of Codex thread config",
                },
                Self::PreserveContextOnReattach => Supported,
                Self::UnsubmittedInspection => Unsupported {
                    reason: "Unsubmitted inspection uses native Codex thread requests",
                },
                Self::SteerCompletionReceipts => Supported,
                Self::NativeMcpConfiguration => Unsupported {
                    reason: "Claude configures MCP through SDK options instead of native Codex configuration",
                },
                Self::BrowserConfiguration => Unsupported {
                    reason: "Studio browser configuration is supplied through native Codex thread config",
                },
                Self::HostWorktreeFence => Supported,
                Self::IdleSummaryReturn => Supported,
                Self::PinnedAccountLaunch => Supported,
                Self::BundledExecutable => Unsupported {
                    reason: "Claude launches the explicitly configured SDK executable rather than the bundled Codex executable",
                },
                Self::AccountSessionLogin => Unsupported {
                    reason: "Claude authenticates through its login profile instead of Codex account-session login",
                },
                Self::TurnCommitGuard => Supported,
                Self::AttributedInput => Supported,
                Self::NativeRelease => Unsupported {
                    reason: "Claude releases retained bridge queries instead of sending native Codex thread release",
                },
            },
        }
    }
}

/// Provider-neutral request preparation. Runtime traffic stays on this boundary.
pub enum ProviderCommand<'a> {
    StartThread {
        snapshot: &'a Snapshot,
        input: ThreadStartInput,
    },
    StartTurn {
        input: &'a TurnInput,
        params: TurnStartInput,
    },
    SetTitle {
        input: TitleInput,
    },
    EditThread {
        agent: &'a EditAgent,
        checkpoint: &'a Checkpoint,
    },
}

/// Static metadata and local policy. Wire shaping belongs to encode.
pub struct ProviderDescriptor {
    pub kind: ProviderKind,
    pub default_model: &'static str,
    pub pricing_vendor: &'static str,
    pub sign_in_message: &'static str,
    pub account_changed_message: &'static str,
    pub diagnostics: fn() -> NativeRequest,
    pub project_diagnostics: fn(Value, u64) -> (Value, u64),
    pub configured_binary:
        fn(&crate::account_policy::PublicAccount, Option<String>) -> Option<Option<String>>,
    pub owns_model: fn(&str) -> bool,
    pub classify_error: fn(&Value) -> Option<UsageCause>,
    pub is_busy_error: fn(&Value) -> bool,
    pub read_auth: fn(&AuthProfile) -> AuthMetadata,
    pub credential_location: fn(&AuthProfile) -> CredentialLocation,
    pub credential_identity_matches: fn(&Value, &Value) -> bool,
}

impl ProviderDescriptor {
    #[must_use]
    pub fn supports(&self, feature: ProviderFeature) -> bool {
        feature.support(self.kind).is_supported()
    }
}

/// One boundary for provider launch, native requests and native events.
pub trait ProviderAdapter: Send + Sync + 'static {
    fn descriptor(&self) -> &'static ProviderDescriptor;
    fn launch(&self, input: CheckedLaunch<'_>) -> Result<ProviderLaunch, WorkerError>;
    fn encode(&self, command: ProviderCommand<'_>) -> Result<NativeRequest, WorkerError>;
}

/// The store consumes JSON projections; native protocol parsing remains typed.
pub(crate) fn project_frame(frame: ProviderEvent) -> ProviderEvent {
    if catalog_changed(&frame)
        || crate::notification_dispatch::progress::native(&frame).is_some()
        || crate::live_settings::native_event(&frame).is_some()
    {
        return frame;
    }
    use studio_runtime_providers::{
        Frame, NotificationFrame, RequestFrame, ResponseBody, UnknownMethodFrame,
    };
    match &frame {
        Frame::Request(_)
        | Frame::Notification(_)
        | Frame::UnknownMethod(_)
        | Frame::Response {
            body: ResponseBody::Result(_) | ResponseBody::Error(_),
            ..
        } => return frame,
        Frame::KnownMethodUnparsed(_)
        | Frame::CodexClientRequest(_)
        | Frame::CodexServerRequest(_)
        | Frame::CodexNotification(_)
        | Frame::CodexClientNotification(_)
        | Frame::Response { .. } => {}
    }
    let mut unknown = matches!(frame, Frame::KnownMethodUnparsed(_));
    frame.visit_unknown_variants(&mut |_, _| unknown = true);
    let Value::Object(mut fields) = json!(frame) else {
        unreachable!("frame object");
    };
    if unknown
        && matches!(
            frame,
            Frame::KnownMethodUnparsed(_) | Frame::CodexServerRequest(_)
        )
        && frame.id().is_some()
    {
        return Frame::KnownMethodUnparsed(UnknownMethodFrame {
            method: frame.method().expect("native request method").into(),
            fields,
        });
    }
    if let Some(method) = frame.method() {
        let method = Method::from_wire(method).expect("known native method");
        fields.remove("method");
        let params = fields.remove("params");
        if let Some(id) = frame.id() {
            fields.remove("id");
            // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/provider_adapter.rs#internal_sender_witness_native_commands_and_diagnostics
            Frame::Request(RequestFrame {
                id,
                method,
                params,
                fields,
            })
        } else {
            Frame::Notification(NotificationFrame {
                method,
                params,
                fields,
            })
        }
    } else {
        let id = frame.id().expect("response id");
        fields.remove("id");
        let body = if let Some(error) = fields.remove("error") {
            ResponseBody::Error(error)
        } else {
            ResponseBody::Result(fields.remove("result").expect("result payload"))
        };
        Frame::Response { id, body, fields }
    }
}

pub(crate) fn project_entry(
    mut entry: studio_runtime_process::provider_seam::JournalEntry,
) -> studio_runtime_process::provider_seam::JournalEntry {
    use studio_runtime_process::provider_seam::JournalPayload;
    entry.payload = match entry.payload {
        JournalPayload::Frame(frame) => JournalPayload::Frame(project_frame(frame)),
        other @ (JournalPayload::RawStdout { .. }
        | JournalPayload::Stderr(_)
        | JournalPayload::Exit(_)
        | JournalPayload::Other { .. }) => other,
    };
    entry
}

/// The agent's provider keeps a native rollout history. An unknown provider
/// does not: its history is never read or repaired.
#[must_use]
pub fn native_history(agent: &Value) -> bool {
    supports_record(agent, ProviderFeature::NativeHistory)
}

#[must_use]
pub fn supports_record(record: &Value, feature: ProviderFeature) -> bool {
    ProviderKind::of_record(record).is_ok_and(|kind| adapter(kind).descriptor().supports(feature))
}

/// The provider whose model family contains `model`, if any.
#[must_use]
pub fn model_provider(model: &str) -> Option<ProviderKind> {
    ProviderKind::ALL
        .into_iter()
        .find(|kind| (adapter(*kind).descriptor().owns_model)(model))
}

/// Default model of a provider.
#[must_use]
pub fn default_model(provider: ProviderKind) -> &'static str {
    adapter(provider).descriptor().default_model
}

struct CodexAdapter;
struct ClaudeAdapter;

#[must_use]
pub fn adapter(kind: ProviderKind) -> &'static dyn ProviderAdapter {
    match kind {
        ProviderKind::Codex => &CodexAdapter,
        ProviderKind::Claude => &ClaudeAdapter,
    }
}

/// Codex CLI looked up in `PATH` when no executable is selected.
const CODEX_EXECUTABLE: &str = "codex";
/// Codex app-server peer over stdio.
const CODEX_APP_SERVER_ARGS: [&str; 3] = ["app-server", "--listen", "stdio://"];
/// Codex config override: seconds an unsubscribed thread stays loaded in the
/// host app-server. Zero frees it as soon as Studio unsubscribes it (see
/// `account_worker::idle_unload`); Codex's default (30 minutes on 0.163.0)
/// keeps every finished worker's and deleted agent's thread loaded. Resuming an
/// unloaded thread costs about as much CPU as a warm turn. A running
/// app-server started without it is reattached with its own command
/// (`ProviderLaunchSpec::reattach_commands`) and keeps Codex's default until
/// it exits.
const CODEX_THREAD_UNLOAD_DELAY: [&str; 2] = ["-c", "thread_unload_delay_secs=0"];
/// Names of the MCP servers that the Codex home's `config.toml` defines. The
/// app-server merges per-thread `config` into that file, so a server can only
/// be switched off by name (`enabled = false`), never by an empty table.
#[must_use]
pub fn codex_user_mcp_servers(home: &str) -> Vec<String> {
    studio_runtime_store::debug_assert_outside_sql();
    std::fs::read_to_string(Path::new(home).join("config.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|config| match config.get("mcp_servers") {
            Some(toml::Value::Table(servers)) => Some(servers.keys().cloned().collect()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Codex config override that keeps credentials in the account's own home.
const CODEX_FILE_CREDENTIALS: [&str; 2] = ["-c", "cli_auth_credentials_store=\"file\""];
/// Developer instructions added to every Claude session.
const CLAUDE_SESSION_INSTRUCTIONS: &str = "\nThis session uses Claude Code and its native tools. The native Agent tool is off. Studio managed agents replace it, and native agent type lists do not apply. Use Studio command monitors for long-running commands that need output, input, or cancellation. Use Bash for other commands. Studio voice is unavailable. Do not tell the user that an MCP server or connector needs authentication unless the user asks for work that needs it.\n";
/// JSON-RPC error the Claude bridge returns for a request on a busy thread.
const CLAUDE_BUSY_CODE: i64 = -32000;
const CLAUDE_BUSY_MESSAGE: &str = "Claude is still working";
/// Claude Code's own default-model selector.
const CLAUDE_DEFAULT_MODEL: &str = "default";
/// Claude Code model aliases (besides `claude-*` ids).
const CLAUDE_MODEL_ALIASES: [&str; 4] = [CLAUDE_DEFAULT_MODEL, "sonnet", "opus", "haiku"];

impl CodexAdapter {
    fn title(input: TitleInput) -> NativeRequest {
        let mut params = codex::ThreadStartParams::new();
        params.cwd = Some(Some(input.cwd));
        params.model = Some(Some(input.model));
        params.ephemeral = Some(Some(true));
        params.approval_policy = Some(Some(command::approval(true)));
        params.sandbox = Some(Some(codex::SandboxMode::ReadOnly));
        // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/provider_adapter.rs#internal_sender_witness_native_commands_and_diagnostics
        NativeRequest::Codex(Box::new(codex::ClientRequest::ThreadStart {
            id: codex::RequestId::Variant1(Box::default()),
            params,
        }))
    }

    fn host_launch(input: CheckedLaunch<'_>) -> Result<ProviderLaunch, WorkerError> {
        let CheckedLaunch {
            home,
            isolated,
            codex_executable,
            root,
            mut environment,
            ..
        } = input;
        environment.insert("CODEX_HOME".into(), home);
        let credentials: &[&str] = if isolated {
            environment.remove("OPENAI_API_KEY");
            environment.remove("CODEX_API_KEY");
            &CODEX_FILE_CREDENTIALS
        } else {
            &[]
        };
        let launch = |overrides: &[&str]| {
            std::iter::once(codex_executable.unwrap_or(CODEX_EXECUTABLE))
                .chain(CODEX_APP_SERVER_ARGS)
                .chain(overrides.iter().copied())
                .chain(credentials.iter().copied())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let mut spec =
            ProviderLaunchSpec::new(launch(&CODEX_THREAD_UNLOAD_DELAY), environment, root)
                .with_provider(ProviderKind::Codex)
                .with_inherited_cwd();
        // The command before the unload override, which running app-servers keep.
        spec.reattach_commands.push(launch(&[]));
        Ok(ProviderLaunch {
            spec,
            executable: codex_executable.map(str::to_owned),
        })
    }

    fn classify_error(error: &Value) -> Option<UsageCause> {
        classify_usage_failure(error, None)
    }

    /// Codex queues or steers input into a running turn; it has no busy refusal.
    fn is_busy_error(_error: &Value) -> bool {
        false
    }

    fn owns_model(model: &str) -> bool {
        model.starts_with("gpt-")
    }
}

impl ClaudeAdapter {
    fn start_thread(
        s: &Snapshot,
        mut input: ThreadStartInput,
    ) -> Result<NativeRequest, WorkerError> {
        let mut options = s.settings.claude_options.clone();
        if s.settings.role == crate::thread_prepare::Role::Reviewer {
            options.insert("permissionMode".into(), json!("plan"));
            input.approval_policy = Some(Some(command::approval(false)));
            input.sandbox = Some(Some(codex::SandboxMode::ReadOnly));
        }
        if let Some(worktree) = &s.host_worktree {
            let mut environment = options
                .get("env")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            // Claude 2.1.302 gFo/J1t (chunk-hqtnbrjd.js:59378-59432):
            // the fence is a JSON array, not a PATH-style separated string.
            environment.insert("CLAUDE_CODE_HOST_WORKTREE".into(), json!(worktree.tree));
            environment.insert(
                "CLAUDE_CODE_HOST_WORKTREE_FENCE".into(),
                json!(json!(worktree.fence).to_string()),
            );
            options.insert("env".into(), json!(environment));
        }
        // Native Layr shares the same per-thread environment contract as Codex.
        let mut native_env = options
            .get("env")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, value) in &s.guidance.thread_config {
            if let Some(name) = key.strip_prefix("shell_environment_policy.set.")
                && matches!(
                    name,
                    "LAYR_ROOT" | "LAYR_DIRECT" | "LAYR_SOCKET" | "PATH" | "TMPDIR"
                )
            {
                native_env.insert(name.into(), value.clone());
            }
        }
        if !native_env.is_empty() {
            options.insert("env".into(), json!(native_env));
        }

        input.claude = Some(options);
        if s.actor.parent_id.is_some() {
            input.user_mcp = Some(false);
        }
        if s.frozen.is_none()
            && let Some(Some(instructions)) = &mut input.developer_instructions
        {
            let browser = s
                .guidance
                .browser
                .as_ref()
                .and_then(|browser| browser.instructions.as_deref());
            if let Some(browser) = browser {
                let offset = instructions.len() - browser.len() - 1;
                instructions.insert_str(offset, CLAUDE_SESSION_INSTRUCTIONS);
            } else {
                instructions.push_str(CLAUDE_SESSION_INSTRUCTIONS);
            }
        }
        Ok(NativeRequest::Claude(BridgeCommand::Thread(Box::new(
            input,
        ))))
    }
    fn start_turn(
        events: &TurnInput,
        mut input: TurnStartInput,
    ) -> Result<NativeRequest, WorkerError> {
        if let [event] = events.events.as_slice()
            && event.kind == "user"
            && !events.has_assets
            && !event.text.contains('\n')
        {
            let text = crate::event_write::python_strip(&event.text);
            if text.starts_with(['/', '$']) {
                input.claude_command = Some(text.into());
            }
        }
        Ok(NativeRequest::Claude(BridgeCommand::Turn(Box::new(input))))
    }
    fn title(input: TitleInput) -> NativeRequest {
        NativeRequest::Claude(BridgeCommand::Title {
            cwd: input.cwd,
            model: input.model,
            ephemeral: true,
        })
    }
    fn edit_thread(
        agent: &EditAgent,
        checkpoint: &Checkpoint,
    ) -> Result<NativeRequest, WorkerError> {
        let mut settings = command::edit_settings(agent);
        Ok(if let Some(turn) = &checkpoint.turn_id {
            settings.thread_id = Some(
                checkpoint
                    .thread_id
                    .clone()
                    .ok_or_else(|| refused("provider/missing-checkpoint-thread"))?,
            );
            settings.approval_policy = agent.approval_policy.clone();
            settings.sandbox = agent.sandbox.clone();
            NativeRequest::Claude(BridgeCommand::Fork {
                settings: Box::new(settings),
                last_turn_id: turn.clone(),
            })
        } else {
            NativeRequest::Claude(BridgeCommand::Thread(Box::new(settings)))
        })
    }

    fn host_launch(input: CheckedLaunch<'_>) -> Result<ProviderLaunch, WorkerError> {
        // The bridge runs in the account's server directory, as in Python, because
        // its session snapshots there let an agent that Python started resume.
        let bridge_root = account_server_root(&input.root, input.account);
        let (spec, executable) = claude_launch(
            input
                .profile
                .ok_or_else(|| refused(CLAUDE_ACCOUNT_IDENTITY_REFUSAL))?,
            input.claude_bridge_dir,
            bridge_root.clone(),
            input.environment,
        )?;
        // Give each Claude session this server's browser link launcher.
        #[cfg(unix)]
        if let Err(error) = crate::browser_host::publish(&bridge_root, &input.root) {
            eprintln!("browser launcher for Claude sessions: {error}");
        }
        Ok(ProviderLaunch {
            spec,
            executable: Some(executable.to_string_lossy().into_owned()),
        })
    }

    /// A turn that Claude starts itself, for example for a background task
    /// notification, can fail at a subscription limit with only the notice
    /// that Claude Code shows as its error: "You've hit your session limit ·
    /// resets 3:40am (UTC)". The names are Claude Code's limit names.
    fn classify_error(error: &Value) -> Option<UsageCause> {
        static AUTH: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        static LIMIT: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let auth = AUTH.get_or_init(|| {
            regex::Regex::new(r"(?i)oauth session expired|failed to authenticate")
                .expect("static Claude auth pattern")
        });
        let limit = LIMIT.get_or_init(|| {
            regex::Regex::new(
                r"^You've hit your (?:session|weekly|Opus|Sonnet|Fable|usage credit) limit\b",
            )
            .expect("static Claude limit notice pattern")
        });
        classify_usage_failure(error, Some(auth)).or_else(|| {
            error
                .get("message")
                .and_then(Value::as_str)
                .is_some_and(|message| limit.is_match(message))
                .then_some(UsageCause::RateLimit)
        })
    }

    fn is_busy_error(error: &Value) -> bool {
        *error == studio_json::json!({"code": CLAUDE_BUSY_CODE, "message": CLAUDE_BUSY_MESSAGE})
    }

    fn owns_model(model: &str) -> bool {
        model.starts_with("claude-") || CLAUDE_MODEL_ALIASES.contains(&model)
    }
}

macro_rules! provider_adapter {
    ($adapter:ident, $descriptor:ident, $codec:path) => {
        impl ProviderAdapter for $adapter {
            fn descriptor(&self) -> &'static ProviderDescriptor {
                &$descriptor
            }
            fn launch(&self, input: CheckedLaunch<'_>) -> Result<ProviderLaunch, WorkerError> {
                let mut launch = Self::host_launch(input)?;
                launch.spec.codec = $codec();
                Ok(launch)
            }
            fn encode(&self, command: ProviderCommand<'_>) -> Result<NativeRequest, WorkerError> {
                let native = match command {
                    ProviderCommand::StartThread { snapshot, input } => {
                        Self::start_thread(snapshot, input)?
                    }
                    ProviderCommand::StartTurn { input, params } => {
                        Self::start_turn(input, params)?
                    }
                    ProviderCommand::SetTitle { input } => Self::title(input),
                    ProviderCommand::EditThread { agent, checkpoint } => {
                        Self::edit_thread(agent, checkpoint)?
                    }
                };
                Ok(native)
            }
        }
    };
}

static CODEX_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: ProviderKind::Codex,
    default_model: studio_runtime_schema::provider_kind::CODEX_INITIAL_MODEL,
    pricing_vendor: "openai",
    sign_in_message: "Sign in to this Codex account first",
    account_changed_message: "The Codex account changed during authentication",
    diagnostics: || {
        // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/provider_adapter.rs#internal_sender_witness_native_commands_and_diagnostics
        NativeRequest::Codex(Box::new(codex::ClientRequest::ThreadLoadedList {
            id: codex::RequestId::Variant1(Box::default()),
            params: codex::ThreadLoadedListParams {
                cursor: None,
                limit: Some(Some(100)),
            },
        }))
    },
    project_diagnostics: |value, _| {
        let count = value
            .get("data")
            .and_then(Value::as_array)
            .map_or(0, Vec::len) as u64;
        (
            json!({"provider": ProviderKind::Codex.key(), "loadedThreads":count}),
            count,
        )
    },
    configured_binary: |_, launched| Some(launched),
    owns_model: CodexAdapter::owns_model,
    classify_error: CodexAdapter::classify_error,
    is_busy_error: CodexAdapter::is_busy_error,
    read_auth: crate::account_driver::read_codex_auth_cached,
    credential_location: crate::account_driver::codex_location,
    credential_identity_matches: |old, new| old == new,
};
static CLAUDE_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: ProviderKind::Claude,
    default_model: CLAUDE_DEFAULT_MODEL,
    pricing_vendor: "anthropic",
    sign_in_message: studio_runtime_store::account_state::CLAUDE_SIGN_IN_MESSAGE,
    account_changed_message: "The Claude account changed during authentication",
    diagnostics: || NativeRequest::Claude(BridgeCommand::Diagnostics {}),
    project_diagnostics: |value, idle_limit| {
        let mut row = Map::new();
        row.insert("provider".into(), json!(ProviderKind::Claude.key()));
        if let Some(fields) = value.as_object() {
            row.extend(fields.clone());
        }
        row.insert("idleLimitSeconds".into(), json!(idle_limit));
        (Value::Object(row), 0)
    },
    configured_binary: |profile, launched| {
        launched.as_ref()?;
        let configured = profile
            .claude_options
            .as_ref()
            .and_then(|options| options.binary_path.as_str());
        Some(
            crate::account_driver::claude::resolve_login_binary(
                configured,
                &env::vars_os().collect(),
            )
            .map(|binary| binary.to_string_lossy().into_owned()),
        )
    },
    owns_model: ClaudeAdapter::owns_model,
    classify_error: ClaudeAdapter::classify_error,
    is_busy_error: ClaudeAdapter::is_busy_error,
    read_auth: crate::account_driver::read_claude_auth_cached,
    credential_location: crate::account_driver::claude_location,
    credential_identity_matches: |old, new| {
        old == new
            || old
                .as_str()
                .zip(new.as_str())
                .is_some_and(|(old, new)| crate::account_driver::claude_identity::accepts(old, new))
    },
};

provider_adapter!(
    CodexAdapter,
    CODEX_DESCRIPTOR,
    studio_runtime_process::provider_seam::ProviderCodec::codex
);
provider_adapter!(
    ClaudeAdapter,
    CLAUDE_DESCRIPTOR,
    studio_runtime_process::provider_seam::ProviderCodec::default
);

/// Refusal code of a Claude launch whose account row has no saved identity.
/// `claude auth status` writes that identity once it reads the login, so an
/// account without a readable login cannot launch until it signs in.
pub const CLAUDE_ACCOUNT_IDENTITY_REFUSAL: &str = "claude/account-identity";

fn refused(code: &str) -> WorkerError {
    WorkerError::PreparationRefused(code.into())
}

fn claude_launch(
    profile: &ClaudeLaunchProfile,
    bridge_dir: &Path,
    root: PathBuf,
    environment: BTreeMap<String, String>,
) -> Result<(ProviderLaunchSpec, PathBuf), WorkerError> {
    let account_id = profile
        .account_id
        .as_str()
        .filter(|value| value.starts_with("claude:"))
        .ok_or_else(|| refused(CLAUDE_ACCOUNT_IDENTITY_REFUSAL))?;
    let email = profile
        .email
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| refused(CLAUDE_ACCOUNT_IDENTITY_REFUSAL))?;
    let credential_identity = profile
        .credential_identity
        .as_str()
        .ok_or_else(|| refused(CLAUDE_ACCOUNT_IDENTITY_REFUSAL))?;
    if account_id != format!("claude:{email}")
        || !crate::account_driver::claude_identity::belongs_to(credential_identity, email)
    {
        return Err(refused(CLAUDE_ACCOUNT_IDENTITY_REFUSAL));
    }
    let path = environment.get("PATH").map(String::as_str);
    let home = environment.get("HOME").map(PathBuf::from);
    let node = if let Some(configured) = environment.get("STUDIO_NODE_BIN") {
        executable(configured, path)
    } else {
        executable("node", path).or_else(|| {
            [
                PathBuf::from("/opt/homebrew/bin/node"),
                PathBuf::from("/usr/local/bin/node"),
            ]
            .into_iter()
            .chain(home.iter().flat_map(|home| {
                [
                    home.join(".local/share/fnm/aliases/default/bin/node"),
                    home.join("Library/Application Support/fnm/aliases/default/bin/node"),
                    home.join(".volta/bin/node"),
                    home.join(".local/share/mise/shims/node"),
                ]
            }))
            .find_map(|path| executable(&path.to_string_lossy(), None))
        })
    }
    .ok_or_else(|| refused("claude/node-unavailable"))?;
    let binary_path = profile
        .binary_path
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| environment.get("STUDIO_CLAUDE_BIN").map(String::as_str));
    let claude = match binary_path {
        Some(binary) => executable(binary, path),
        None => executable("claude", path).or_else(|| {
            home.as_ref()
                .map(|home| home.join(".local/bin/claude"))
                .and_then(|path| executable(&path.to_string_lossy(), None))
        }),
    }
    .ok_or_else(|| refused("claude/cli-unavailable"))?;
    let metadata = AuthMetadata {
        status: AuthStatus::Ready,
        account_id: Some(account_id.to_owned()),
        email: Some(email.to_owned()),
        plan: profile.plan.as_str().map(str::to_owned),
        organization_name: None,
        credential_identity: Some(credential_identity.to_owned()),
        error: None,
        error_kind: None,
    };
    // The bridge's working directory is the account's server root. A profile added
    // after startup has none yet, and a spawn in a missing cwd fails with ENOENT.
    std::fs::create_dir_all(&root).map_err(|_| refused("claude/account-server-root"))?;
    let resolved = ResolvedBridge {
        node,
        executable: claude.clone(),
        bridge: bridge_dir.join("bridge.mjs"),
        root,
        sdk_present: bridge_dir
            .join("node_modules/@anthropic-ai/claude-agent-sdk/package.json")
            .is_file(),
    };
    let launch = BridgeSnapshot::from_launch(profile)
        .and_then(|snapshot| snapshot.launch(resolved, &metadata, environment))
        .map_err(|error| WorkerError::PreparationRefused(error.to_string()))?;
    Ok((launch.with_provider(ProviderKind::Claude), claude))
}

/// Resolve `configured` (a path, or a name searched in `path`) to a canonical executable.
fn executable(configured: &str, path: Option<&str>) -> Option<PathBuf> {
    let candidate = PathBuf::from(configured);
    let candidate = if candidate.is_absolute() || candidate.components().count() > 1 {
        candidate
    } else {
        env::split_paths(path?)
            .map(|directory| directory.join(&candidate))
            .find(|path| is_executable(path))?
    };
    is_executable(&candidate)
        .then(|| {
            CanonicalPath::existing(&candidate)
                .ok()
                .map(CanonicalPath::into_path_buf)
        })
        .flatten()
}

/// A regular file with an execute bit (any file off Unix).
pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Extra native catalogs supported by the selected provider. Missing identity stays unsupported.
pub fn catalog_requests(
    provider: Option<ProviderKind>,
    cwd: &str,
    thread: Option<&str>,
) -> Vec<(&'static str, crate::catalog_reads::ProviderRequest)> {
    use crate::catalog_reads::ProviderRequest;
    match provider {
        Some(ProviderKind::Codex) => vec![
            (
                "apps",
                // internal-sender-witness: workspaces/runtime/packages/runtime-http/tests/mcp_reconnect.rs#internal_sender_witness_catalog_discovery
                ProviderRequest {
                    method: codex::ClientRequestMethod::AppList.as_str().to_owned(),
                    params: json!(codex::AppsListParams {
                        cursor: None,
                        force_refetch: Some(true),
                        limit: Some(Some(studio_runtime_schema::native_catalog::APPS_PER_PAGE)),
                        thread_id: Some(thread.map(str::to_owned))
                    }),
                },
            ),
            (
                "plugins",
                // internal-sender-witness: workspaces/runtime/packages/runtime-http/tests/mcp_reconnect.rs#internal_sender_witness_catalog_discovery
                ProviderRequest {
                    method: codex::ClientRequestMethod::PluginList.as_str().to_owned(),
                    params: json!(codex::PluginListParams {
                        cwds: Some(Some(vec![cwd.to_owned()])),
                        force_refetch: Some(true),
                        marketplace_kinds: None
                    }),
                },
            ),
        ],
        Some(ProviderKind::Claude) | None => Vec::new(),
    }
}

/// Project each supported catalog once at its owning provider boundary.
pub fn apply_capability_result(
    provider: Option<ProviderKind>,
    result: &mut Value,
    label: &str,
    response: &Value,
) -> Result<(), &'static str> {
    let (data, cursor) = match (provider, label) {
        (Some(ProviderKind::Codex), "apps") => {
            let native: codex::AppsListResponse = serde::Deserialize::deserialize(response)
                .map_err(|_| "Invalid native app catalog")?;
            (
                catalog_entries(
                    native
                        .data
                        .into_iter()
                        .map(|app| (app.id, app.name, app.is_enabled)),
                ),
                json!(native.next_cursor.flatten()),
            )
        }
        (Some(ProviderKind::Codex), "plugins") => {
            let native: codex::PluginListResponse = serde::Deserialize::deserialize(response)
                .map_err(|_| "Invalid native plugin catalog")?;
            for error in native.marketplace_load_errors.into_iter().flatten() {
                crate::catalog_reads::append_capability_error(result, label, &error.message);
            }
            (
                catalog_entries(
                    native
                        .marketplaces
                        .into_iter()
                        .flat_map(|marketplace| marketplace.plugins)
                        .map(|plugin| (plugin.id, plugin.name, Some(plugin.enabled))),
                ),
                Value::Null,
            )
        }
        (Some(ProviderKind::Codex | ProviderKind::Claude), _) => (
            response
                .get("data")
                .or_else(|| {
                    (label == "servers")
                        .then(|| response.get("servers"))
                        .flatten()
                })
                .cloned()
                .unwrap_or_else(|| json!([])),
            response.get("nextCursor").cloned().unwrap_or(Value::Null),
        ),
        (None, _) => return Err("Unsupported provider"),
    };
    result[label] = data.clone();
    if label != "plugins" {
        result[format!("{label}Cursor")] = cursor;
    }
    if label == "servers" {
        result["mcp"] = data;
    }
    Ok(())
}

fn catalog_entries(entries: impl Iterator<Item = (String, String, Option<bool>)>) -> Value {
    use studio_runtime_schema::native_catalog::NativeCatalogEntry;
    json!(
        entries
            .map(|(id, name, enabled)| NativeCatalogEntry { id, name, enabled })
            .collect::<Vec<_>>()
    )
}

/// Account-wide native catalog changes, classified before generic projection.
pub(crate) fn catalog_changed(frame: &ProviderEvent) -> bool {
    use studio_runtime_providers::codex_disposition::{
        self, Disposition, HandlerKey, NotificationHandler, ProjectionHandler,
    };
    matches!(frame, ProviderEvent::CodexNotification(notification) if matches!(codex_disposition::server_notification(notification.method()), Disposition::Handled(HandlerKey::Notification(NotificationHandler::Projection(ProjectionHandler::SkillsChanged | ProjectionHandler::AppListUpdated)))))
        || matches!(frame, ProviderEvent::Notification(notification) if notification.method.as_str() == "skills/changed")
}

#[cfg(test)]
mod tests {
    fn test_claude_launch(
        profile: &studio_json::Value,
        bridge_dir: &std::path::Path,
        root: std::path::PathBuf,
        environment: std::collections::BTreeMap<String, String>,
    ) -> Result<
        (
            studio_runtime_process::ProviderLaunchSpec,
            std::path::PathBuf,
        ),
        crate::account_worker::WorkerError,
    > {
        let profile =
            crate::claude_bridge::ClaudeLaunchProfile::from_record(profile).map_err(|_| {
                crate::account_worker::WorkerError::PreparationRefused(
                    super::CLAUDE_ACCOUNT_IDENTITY_REFUSAL.into(),
                )
            })?;
        super::claude_launch(&profile, bridge_dir, root, environment)
    }

    use super::{ProviderKind, UsageCause, adapter};
    use studio_json::json;

    #[test]
    fn prepared_codex_command_retains_native_dto_and_exact_request_identity() {
        use studio_runtime_providers::{Frame, RequestId};
        let native = adapter(ProviderKind::Codex)
            .encode(super::ProviderCommand::SetTitle {
                input: super::TitleInput {
                    cwd: "/project".into(),
                    model: "chosen".into(),
                },
            })
            .expect("typed command");
        assert!(matches!(native, super::NativeRequest::Codex(_)));
        let id = RequestId::String("durable:title:thread".into());
        let first = native.frame(id.clone()).expect("native frame");
        let retry = native.frame(id.clone()).expect("same request");
        assert_eq!(first, retry);
        assert_eq!(first.id(), Some(id));
        let Frame::CodexClientRequest(request) = first else {
            panic!("typed egress");
        };
        let super::codex::ClientRequest::ThreadStart { params, .. } = *request else {
            panic!("native start");
        };
        assert_eq!(params.cwd, Some(Some("/project".into())));
        assert_eq!(params.model, Some(Some("chosen".into())));
        assert_eq!(params.ephemeral, Some(Some(true)));
    }

    #[test]
    fn internal_sender_witness_native_commands_and_diagnostics() {
        use super::{ProviderCommand, ProviderFeature};
        for kind in ProviderKind::ALL {
            let adapter = adapter(kind);
            for feature in [
                ProviderFeature::StartThread,
                ProviderFeature::StartTurn,
                ProviderFeature::SetTitle,
                ProviderFeature::EditThread,
            ] {
                assert!(adapter.descriptor().supports(feature));
            }
            let request = adapter
                .encode(ProviderCommand::SetTitle {
                    input: super::TitleInput {
                        cwd: "/work".into(),
                        model: "chosen".into(),
                    },
                })
                .expect("title command");
            crate::internal_sender_witness::request(request.method(), &request.params());
            let projected = super::project_frame(
                request
                    .frame(studio_runtime_providers::RequestId::String(
                        "projected".into(),
                    ))
                    .unwrap(),
            );
            assert_eq!(projected.method(), Some(request.method().as_str()));
            let diagnostics = (adapter.descriptor().diagnostics)();
            crate::internal_sender_witness::request(diagnostics.method(), &diagnostics.params());
            assert_eq!(request.method(), super::Method::ThreadStart);
            assert_eq!(request.params()["cwd"], "/work");
            assert_eq!(request.params()["model"], "chosen");
            assert_eq!(request.params()["ephemeral"], true);
        }
        assert!(!super::supports_record(
            &json!({"provider":"future"}),
            ProviderFeature::StartThread
        ));
        assert!(!super::supports_record(
            &json!({}),
            ProviderFeature::StartThread
        ));
    }

    #[test]
    fn each_adapter_reports_its_typed_control_list() {
        for kind in ProviderKind::ALL {
            let descriptor = adapter(kind).descriptor();
            assert_eq!(descriptor.kind, kind);
            assert_eq!(descriptor.kind.capabilities(), kind.capabilities());
        }
    }

    #[test]
    fn internal_sender_witness_both_adapters_fork_the_checkpoint() {
        use super::Method;
        let agent = json!({
            "cwd": "/work", "model": "m", "fastMode": true,
            "threadConfig": {"k": 1}, "approvalPolicy": "never", "sandbox": "read-only"
        });
        let checkpoint = json!({"threadId": "t1", "turnId": "u2"});
        for kind in ProviderKind::ALL {
            let request = adapter(kind)
                .encode(super::ProviderCommand::EditThread {
                    agent: &serde::Deserialize::deserialize(agent.clone()).expect("agent input"),
                    checkpoint: &serde::Deserialize::deserialize(checkpoint.clone())
                        .expect("checkpoint input"),
                })
                .expect("edit command");
            crate::internal_sender_witness::request(request.method(), &request.params());
            assert_eq!(request.method(), Method::ThreadFork, "{kind}");
            assert_eq!(
                request.params(),
                json!({
                    "cwd": "/work", "serviceTier": "priority", "model": "m",
                    "config": {"k": 1}, "threadId": "t1", "lastTurnId": "u2",
                    "approvalPolicy": "never", "sandbox": "read-only"
                }),
                "{kind}"
            );
        }
    }

    #[test]
    fn internal_sender_witness_checkpoint_without_a_turn_starts_a_thread() {
        use super::Method;
        let agent = json!({"cwd": "/work", "model": ""});
        let request = adapter(ProviderKind::Codex)
            .encode(super::ProviderCommand::EditThread {
                agent: &serde::Deserialize::deserialize(agent).expect("agent input"),
                checkpoint: &serde::Deserialize::deserialize(json!({"turnId": null}))
                    .expect("checkpoint input"),
            })
            .expect("edit command");
        crate::internal_sender_witness::request(request.method(), &request.params());
        assert_eq!(request.method(), Method::ThreadStart);
        assert_eq!(
            request.params(),
            json!({"cwd": "/work", "serviceTier": "default", "config": {}})
        );
    }

    #[test]
    fn edit_command_preserves_absent_and_null_permission_settings() {
        for kind in ProviderKind::ALL {
            for source in [
                json!({"cwd":"/work"}),
                json!({"cwd":"/work", "approvalPolicy":null, "sandbox":null}),
            ] {
                let agent =
                    serde::Deserialize::deserialize(source.clone()).expect("stored edit input");
                let checkpoint =
                    serde::Deserialize::deserialize(json!({"threadId":"thread", "turnId":"turn"}))
                        .expect("checkpoint");
                let params = adapter(kind)
                    .encode(super::ProviderCommand::EditThread {
                        agent: &agent,
                        checkpoint: &checkpoint,
                    })
                    .expect("edit command")
                    .params();
                for field in ["approvalPolicy", "sandbox"] {
                    assert_eq!(params.get(field), source.get(field), "{kind}: {field}");
                }
            }
        }
    }

    #[test]
    fn classifiers_share_the_core_and_differ_only_in_provider_evidence() {
        let shared = json!({"message": "HTTP 401 from service"});
        let claude_only = json!({"message": "OAuth session expired"});
        let busy = json!({"code": -32000, "message": "Claude is still working"});
        for kind in ProviderKind::ALL {
            assert_eq!(adapter(kind).descriptor().kind, kind);
            assert_eq!(
                (adapter(kind).descriptor().classify_error)(&shared),
                Some(UsageCause::Auth)
            );
        }
        let codex = adapter(ProviderKind::Codex);
        let claude = adapter(ProviderKind::Claude);
        assert_eq!((codex.descriptor().classify_error)(&claude_only), None);
        assert_eq!(
            (claude.descriptor().classify_error)(&claude_only),
            Some(UsageCause::Auth)
        );
        assert!(!(codex.descriptor().is_busy_error)(&busy));
        assert!((claude.descriptor().is_busy_error)(&busy));
    }

    #[test]
    fn model_families_and_defaults_belong_to_one_provider() {
        use super::{default_model, model_provider};
        for (model, owner) in [
            ("gpt-6-astra", Some(ProviderKind::Codex)),
            ("gpt-6-luna", Some(ProviderKind::Codex)),
            ("gpt-5.6-luna", Some(ProviderKind::Codex)),
            ("claude-sonnet-4", Some(ProviderKind::Claude)),
            ("default", Some(ProviderKind::Claude)),
            ("haiku", Some(ProviderKind::Claude)),
            ("o3", None),
        ] {
            assert_eq!(model_provider(model), owner, "{model}");
        }
        for kind in ProviderKind::ALL {
            let default = adapter(kind).descriptor().default_model;
            assert_eq!(model_provider(default), Some(kind));
            assert_eq!(super::default_model(kind), default);
        }
        assert_eq!(default_model(ProviderKind::Claude), "default");
        assert_eq!(default_model(ProviderKind::Codex), "gpt-6-astra");
    }

    #[test]
    fn auth_sources_belong_to_the_profile_provider() {
        use crate::account_driver::{AuthProfile, AuthStatus};
        let home = tempfile::tempdir().expect("home");
        let profile = |provider| AuthProfile {
            provider,
            home: home.path().to_path_buf(),
            claude_options: Some(studio_json::json!({"configDir": home.path()})),
        };
        let codex = (adapter(ProviderKind::Codex)
            .descriptor()
            .credential_location)(&profile(ProviderKind::Codex));
        assert_eq!(codex.directories, vec![home.path().to_path_buf()]);
        assert_eq!(codex.credential_files.len(), 1);
        assert_eq!(codex.credential_files[0].name, "auth.json");
        let claude = (adapter(ProviderKind::Claude)
            .descriptor()
            .credential_location)(&profile(ProviderKind::Claude));
        assert_eq!(claude.directories, vec![home.path().to_path_buf()]);
        assert!(
            claude
                .credential_files
                .iter()
                .any(|file| file.name == ".credentials.json")
        );
        assert_ne!(codex.identity, claude.identity);
        assert_eq!(
            (adapter(ProviderKind::Codex).descriptor().read_auth)(&profile(ProviderKind::Codex))
                .status,
            AuthStatus::SignedOut
        );
    }

    #[cfg(unix)]
    #[test]
    fn checked_private_profile_launch_keeps_pin_options_and_refusal_order() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temporary root");
        let binary = dir.path().join("fake-executable");
        std::fs::write(&binary, "#!/bin/sh\n").expect("fake executable");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
            .expect("permissions");
        let sdk = dir
            .path()
            .join("node_modules/@anthropic-ai/claude-agent-sdk");
        std::fs::create_dir_all(&sdk).expect("sdk");
        std::fs::write(sdk.join("package.json"), "{}").expect("sdk package");
        let registry = dir.path().join("registry.json");
        let row = studio_json::json!({"id":"claude","provider":"claude","status":"ready","home":dir.path(),"label":"Fixture","source":"CLI","email":"a@example.test","accountId":"claude:a@example.test","_credentialIdentity":"claude:a@example.test","claudeOptions":{"binaryPath":binary,"launchArgs":"--verbose","vendor":{"n":1.0}}});
        let write = |row: &studio_json::Value| {
            std::fs::write(&registry, studio_json::json!({"version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,"defaultAccountKey":"claude","accounts":{"claude":row},"logins":{}}).to_string()).expect("registry");
            crate::account_driver::AccountDriver::open(&registry).expect("driver")
        };
        let driver = write(&row);
        let profile = driver
            .claude_launch_profile("claude")
            .expect("private profile");
        let env = std::collections::BTreeMap::from([
            (
                "STUDIO_NODE_BIN".to_owned(),
                binary.to_string_lossy().into_owned(),
            ),
            (
                "STUDIO_CLAUDE_BIN".to_owned(),
                binary.to_string_lossy().into_owned(),
            ),
        ]);
        let spec = adapter(ProviderKind::Claude)
            .launch(super::CheckedLaunch {
                profile: Some(&profile),
                home: dir.path().to_string_lossy().into_owned(),
                isolated: false,
                codex_executable: None,
                claude_bridge_dir: dir.path(),
                account: "claude",
                root: dir.path().join("state"),
                environment: env.clone(),
            })
            .expect("adapter launch")
            .spec;
        assert_eq!(
            spec.environment["STUDIO_CLAUDE_CREDENTIAL_IDENTITY"],
            "claude:a@example.test"
        );
        let options: studio_json::Value =
            studio_json::from_str(&spec.environment["STUDIO_CLAUDE_OPTIONS"]).expect("options");
        assert_eq!(options["vendor"]["n"], studio_json::json!(1.0));
        assert_eq!(options["extraArgs"]["verbose"], studio_json::Value::Null);
        let mut invalid = row.clone();
        invalid["_credentialIdentity"] = studio_json::json!("claude:other@example.test");
        let invalid = write(&invalid)
            .claude_launch_profile("claude")
            .expect("private profile");
        let error = super::claude_launch(
            &invalid,
            dir.path(),
            dir.path().join("identity-refused"),
            Default::default(),
        )
        .err()
        .expect("refusal");
        assert!(
            matches!(error, crate::account_worker::WorkerError::PreparationRefused(ref code) if code == super::CLAUDE_ACCOUNT_IDENTITY_REFUSAL)
        );
        assert!(!dir.path().join("identity-refused").exists());
        let mut null_options = row;
        null_options["claudeOptions"] = studio_json::Value::Null;
        let null_options = write(&null_options)
            .claude_launch_profile("claude")
            .expect("private profile");
        let error = super::claude_launch(
            &null_options,
            dir.path(),
            dir.path().join("options-refused"),
            env,
        )
        .err()
        .expect("refusal");
        assert!(
            matches!(error, crate::account_worker::WorkerError::PreparationRefused(ref code) if code == "Claude bridge settings are invalid")
        );
        assert!(dir.path().join("options-refused").is_dir());
    }

    /// A profile added after startup has no account server directory yet; the
    /// bridge launch creates it because the bridge spawns with it as its cwd.
    #[cfg(unix)]
    #[test]
    fn claude_launch_creates_a_missing_account_server_root() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp dir");
        let binary = dir.path().join("claude");
        std::fs::write(&binary, "#!/bin/sh\n").expect("fake claude");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
            .expect("make executable");
        let sdk = dir
            .path()
            .join("node_modules/@anthropic-ai/claude-agent-sdk");
        std::fs::create_dir_all(&sdk).expect("sdk dir");
        std::fs::write(sdk.join("package.json"), "{}").expect("sdk package");
        let mut profile = json!({
            "id": "added", "provider": "claude", "status": "ready", "home": dir.path(), "email": "a@example.test",
            "accountId": "claude:a@example.test",
            "_credentialIdentity": "claude:a@example.test",
            "claudeOptions": {"binaryPath": binary, "configDir": dir.path(), "customModels": []}
        });
        let root = dir.path().join("account-servers/added");
        let environment = std::collections::BTreeMap::from([(
            "STUDIO_NODE_BIN".to_owned(),
            binary.to_string_lossy().into_owned(),
        )]);
        test_claude_launch(&profile, dir.path(), root.clone(), environment.clone())
            .expect("launch spec");
        assert!(root.is_dir());
        let identity = crate::account_driver::claude_identity::identity(
            "a@example.test",
            Some("org-a"),
            Some("account-a"),
        );
        profile["_credentialIdentity"] = json!(identity);
        let (launch, _) =
            test_claude_launch(&profile, dir.path(), root, environment).expect("organization pin");
        assert_eq!(
            launch.environment["STUDIO_CLAUDE_CREDENTIAL_IDENTITY"],
            identity
        );
    }

    /// A supervised provider outlives a backend install, so every earlier host
    /// launch that a running provider may still have must reattach under the
    /// current launch: the supervisor accepts its command (see
    /// `retained_command_accepted`), its environment passes the retained check,
    /// and its working directory is unchanged. The last entry of each launch in
    /// `provider-launch-history.json` is the current launch, so any change to
    /// a Codex or Claude launch command, environment or working directory fails
    /// here until it is recorded with a matching reattach entry.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn earlier_host_launches_reattach_under_the_current_launch() {
        use std::collections::BTreeMap;
        use std::os::unix::fs::PermissionsExt;
        #[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
        struct Recorded {
            release: String,
            command: Vec<String>,
            environment: BTreeMap<String, String>,
            cwd: Option<String>,
        }
        let dir = tempfile::tempdir().expect("temp dir");
        let canonical = studio_runtime_store::path_identity::CanonicalPath::existing(dir.path())
            .expect("canonical temp dir");
        let normalize = |value: &str| {
            value
                .replace(&canonical.as_path().to_string_lossy().into_owned(), "<dir>")
                .replace(&dir.path().to_string_lossy().into_owned(), "<dir>")
        };
        let record = |release: &str, spec: &studio_runtime_process::ProviderLaunchSpec| Recorded {
            release: release.to_owned(),
            command: spec.command.iter().map(|part| normalize(part)).collect(),
            environment: spec
                .environment
                .iter()
                .map(|(key, value)| (key.clone(), normalize(value)))
                .collect(),
            cwd: spec
                .cwd
                .as_ref()
                .map(|cwd| normalize(&cwd.to_string_lossy())),
        };
        let input = BTreeMap::from([
            ("HOME".to_owned(), format!("{}/home", dir.path().display())),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ("OPENAI_API_KEY".to_owned(), "fixture-key".to_owned()),
        ]);
        let codex = |isolated: bool| {
            adapter(ProviderKind::Codex)
                .launch(super::CheckedLaunch {
                    profile: None,
                    home: format!("{}/codex", dir.path().display()),
                    isolated,
                    codex_executable: None,
                    claude_bridge_dir: std::path::Path::new(""),
                    account: "default",
                    root: dir.path().to_path_buf(),
                    environment: input.clone(),
                })
                .expect("Codex launch")
                .spec
        };
        let binary = dir.path().join("claude");
        std::fs::write(&binary, "#!/bin/sh\n").expect("fake executable");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
            .expect("make executable");
        let sdk = dir
            .path()
            .join("node_modules/@anthropic-ai/claude-agent-sdk");
        std::fs::create_dir_all(&sdk).expect("sdk dir");
        std::fs::write(sdk.join("package.json"), "{}").expect("sdk package");
        let claude_with = |pin: &str| {
            let profile = json!({
                "id": "account", "provider": "claude", "status": "ready", "home": dir.path(),
                "email": "a@example.test", "accountId": "claude:a@example.test",
                "_credentialIdentity": pin,
                "claudeOptions": {"binaryPath": binary, "configDir": dir.path().join("config"),
                    "customModels": [], "launchArgs": ""}
            });
            let mut claude_input = input.clone();
            claude_input.insert(
                "STUDIO_NODE_BIN".to_owned(),
                binary.to_string_lossy().into_owned(),
            );
            test_claude_launch(
                &profile,
                dir.path(),
                dir.path().join("account-servers/account"),
                claude_input,
            )
            .expect("Claude launch")
            .0
        };
        let claude = claude_with("claude:a@example.test");
        let history: BTreeMap<String, Vec<Recorded>> =
            studio_json::from_str(include_str!("provider-launch-history.json"))
                .expect("launch history");
        let mut failures = Vec::new();
        for (name, spec) in [
            ("codex", codex(false)),
            ("codex-isolated", codex(true)),
            ("claude", claude),
        ] {
            let current = record("current", &spec);
            let current_json = studio_json::to_string_pretty(&current).expect("current launch");
            let Some((last, earlier)) = history.get(name).and_then(|entries| entries.split_last())
            else {
                failures.push(format!(
                    "{name}: no recorded launch; current:\n{current_json}"
                ));
                continue;
            };
            if (&last.command, &last.environment, &last.cwd)
                != (&current.command, &current.environment, &current.cwd)
            {
                failures.push(format!(
                    "{name}: the launch changed. Record it as the last entry of \
                     provider-launch-history.json, with a reattach entry for each \
                     earlier launch:\n{current_json}"
                ));
            }
            let reattach: Vec<Vec<String>> = spec
                .reattach_commands
                .iter()
                .map(|command| command.iter().map(|part| normalize(part)).collect())
                .collect();
            let mut normalized = spec.clone();
            normalized.command = current.command.clone();
            normalized.reattach_commands = reattach;
            normalized.environment = current.environment.clone();
            for earlier in earlier {
                let release = &earlier.release;
                if !studio_runtime_process::retained_command_accepted(&earlier.command, &normalized)
                {
                    failures.push(format!(
                        "{name} {release}: its command has no reattach command"
                    ));
                }
                if !studio_runtime_process::retained_environment_accepted(
                    &earlier.environment,
                    &normalized,
                ) {
                    failures.push(format!(
                        "{name} {release}: its environment fails the retained check"
                    ));
                }
                if earlier.cwd != current.cwd {
                    failures.push(format!("{name} {release}: its working directory changed"));
                }
            }
        }
        // State that changes between launches of the same code: the credential
        // identity pin learns organization and account UUIDs after a bridge
        // starts, Homebrew upgrades node, and the Claude CLI updates itself.
        // Both executables are found through links and launched by their
        // canonical paths. A bridge launched in each earlier state must
        // reattach under the launch of each later state.
        let identity = crate::account_driver::claude_identity::identity;
        let states = [
            (
                "legacy pin",
                identity("a@example.test", None, None),
                "24.1",
                "2.1.1",
            ),
            (
                "organization learned",
                identity("a@example.test", Some("org"), None),
                "24.1",
                "2.1.1",
            ),
            (
                "account learned",
                identity("a@example.test", Some("org"), Some("acct")),
                "24.1",
                "2.1.1",
            ),
            (
                "node upgraded",
                identity("a@example.test", Some("org"), Some("acct")),
                "24.2",
                "2.1.1",
            ),
            (
                "Claude CLI updated",
                identity("a@example.test", Some("org"), Some("acct")),
                "24.2",
                "2.1.2",
            ),
        ];
        let state_dir = dir.path().join("states");
        let launches: Vec<_> = states
            .iter()
            .map(|(_, pin, node, cli)| claude_in_state(&state_dir, pin, node, cli))
            .collect();
        for (index, later) in launches.iter().enumerate() {
            for (earlier_state, earlier) in states.iter().zip(&launches).take(index) {
                if !bridge_reattaches(earlier, later) {
                    failures.push(format!(
                        "claude: a bridge launched in the state \"{}\" does not reattach \
                         in the state \"{}\"",
                        earlier_state.0, states[index].0
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    }

    /// Whether a bridge started by the launch `started` reattaches under the
    /// launch `current`: the supervisor accepts its command and working
    /// directory, and the backend accepts its environment.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn bridge_reattaches(
        started: &studio_runtime_process::ProviderLaunchSpec,
        current: &studio_runtime_process::ProviderLaunchSpec,
    ) -> bool {
        studio_runtime_process::retained_command_accepted(&started.command, current)
            && started.cwd == current.cwd
            && studio_runtime_process::retained_environment_accepted(&started.environment, current)
    }

    /// A Claude launch of account a@example.test in `dir` whose node and Claude
    /// CLI are found through links in `<dir>/bin`, as Homebrew links node and
    /// the Claude installer links its CLI. Each launch points the links at the
    /// executables of `node` and `cli`; the launch has their canonical paths.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn claude_in_state(
        dir: &std::path::Path,
        pin: &str,
        node: &str,
        cli: &str,
    ) -> studio_runtime_process::ProviderLaunchSpec {
        claude_launch_with(dir, "a@example.test", pin, node, cli, "config", "")
    }

    /// See `claude_in_state`; also sets the account email, the configuration
    /// directory below `dir` and the launch arguments.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn claude_launch_with(
        dir: &std::path::Path,
        email: &str,
        pin: &str,
        node: &str,
        cli: &str,
        config: &str,
        launch_args: &str,
    ) -> studio_runtime_process::ProviderLaunchSpec {
        use std::os::unix::fs::PermissionsExt;
        let install = |target: std::path::PathBuf, link: &str| {
            std::fs::create_dir_all(target.parent().expect("parent")).expect("install dir");
            std::fs::write(&target, "#!/bin/sh\n").expect("fake executable");
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))
                .expect("make executable");
            let link = dir.join("bin").join(link);
            std::fs::create_dir_all(dir.join("bin")).expect("link dir");
            let _absent = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(&target, &link).expect("link");
        };
        install(dir.join("Cellar/node").join(node).join("bin/node"), "node");
        install(dir.join("claude/versions").join(cli), "claude");
        let sdk = dir.join("node_modules/@anthropic-ai/claude-agent-sdk");
        std::fs::create_dir_all(&sdk).expect("sdk dir");
        std::fs::write(sdk.join("package.json"), "{}").expect("sdk package");
        let profile = json!({
            "id": "account", "provider": "claude", "status": "ready", "home": dir,
            "email": email, "accountId": format!("claude:{email}"),
            "_credentialIdentity": pin,
            "claudeOptions": {"configDir": dir.join(config), "customModels": [],
                "launchArgs": launch_args}
        });
        let environment = std::collections::BTreeMap::from([
            (
                "HOME".to_owned(),
                dir.join("home").to_string_lossy().into_owned(),
            ),
            (
                "PATH".to_owned(),
                format!("{}:/usr/bin:/bin", dir.join("bin").display()),
            ),
        ]);
        test_claude_launch(
            &profile,
            dir,
            dir.join("account-servers/account"),
            environment,
        )
        .expect("Claude launch")
        .0
    }

    /// The bridge's node and Claude CLI are launched by their canonical paths,
    /// so a Homebrew upgrade of node or a Claude CLI that updates itself gives
    /// the next launch of the same code another program or `STUDIO_CLAUDE_BIN`.
    /// A running bridge is still the provider of the same account and must
    /// reattach. A bridge of another account, configuration directory or
    /// options must not.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn a_bridge_whose_node_or_claude_cli_was_replaced_reattaches() {
        let dir = tempfile::tempdir().expect("temp dir");
        let dir = dir.path();
        let pin = "claude:a@example.test";
        let launch = |email: &str, node: &str, cli: &str, config: &str, launch_args: &str| {
            let pin = if email == "a@example.test" {
                pin.to_owned()
            } else {
                format!("claude:{email}")
            };
            claude_launch_with(dir, email, &pin, node, cli, config, launch_args)
        };
        let started = [
            (
                "node upgrade",
                launch("a@example.test", "24.1", "2.1.2", "config", ""),
            ),
            (
                "Claude CLI update",
                launch("a@example.test", "24.2", "2.1.1", "config", ""),
            ),
            (
                "node upgrade and Claude CLI update",
                launch("a@example.test", "24.1", "2.1.1", "config", ""),
            ),
        ];
        let refused = [
            (
                "another account",
                launch("b@example.test", "24.1", "2.1.1", "config", ""),
            ),
            (
                "another configuration directory",
                launch("a@example.test", "24.1", "2.1.1", "other", ""),
            ),
            (
                "other options",
                launch("a@example.test", "24.1", "2.1.1", "config", "--debug"),
            ),
        ];
        let current = launch("a@example.test", "24.2", "2.1.2", "config", "");
        for (change, started) in &started {
            assert_ne!(
                (&started.command, &started.environment),
                (&current.command, &current.environment),
                "the {change} changes the launch"
            );
            assert!(
                bridge_reattaches(started, &current),
                "a bridge started before the {change} reattaches"
            );
        }
        for (change, started) in &refused {
            assert!(
                !bridge_reattaches(started, &current),
                "a bridge of {change} is refused"
            );
        }
    }

    /// The credential identity pin learns the organization and account UUIDs
    /// after a bridge starts, so the next launch of the same code computes
    /// another `STUDIO_CLAUDE_CREDENTIAL_IDENTITY`. A bridge launched with the
    /// pin before it learned them is the same credential and must reattach.
    /// A bridge of another email must not.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn a_bridge_launched_before_its_identity_pin_learned_more_fields_reattaches() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp dir");
        let binary = dir.path().join("claude");
        std::fs::write(&binary, "#!/bin/sh\n").expect("fake executable");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
            .expect("make executable");
        let sdk = dir
            .path()
            .join("node_modules/@anthropic-ai/claude-agent-sdk");
        std::fs::create_dir_all(&sdk).expect("sdk dir");
        std::fs::write(sdk.join("package.json"), "{}").expect("sdk package");
        let launch = |email: &str, pin: String| {
            let profile = json!({
                "id": "account", "provider": "claude", "status": "ready", "home": dir.path(),
                "email": email, "accountId": format!("claude:{email}"),
                "_credentialIdentity": pin,
                "claudeOptions": {"binaryPath": binary, "configDir": dir.path(), "customModels": []}
            });
            let environment = std::collections::BTreeMap::from([(
                "STUDIO_NODE_BIN".to_owned(),
                binary.to_string_lossy().into_owned(),
            )]);
            test_claude_launch(
                &profile,
                dir.path(),
                dir.path().join("account-servers/account"),
                environment,
            )
            .expect("Claude launch")
            .0
        };
        let learned = |email: &str, organization: Option<&str>, account: Option<&str>| {
            crate::account_driver::claude_identity::identity(email, organization, account)
        };
        let full = launch(
            "a@example.test",
            learned("a@example.test", Some("org"), Some("acct")),
        );
        for earlier in [
            learned("a@example.test", None, None),
            learned("a@example.test", Some("org"), None),
            learned("a@example.test", None, Some("acct")),
        ] {
            let started = launch("a@example.test", earlier.clone());
            assert!(
                studio_runtime_process::retained_environment_accepted(&started.environment, &full),
                "a bridge launched with {earlier} reattaches"
            );
        }
        let other = launch("b@example.test", learned("b@example.test", None, None));
        let mut foreign = full.environment.clone();
        foreign.insert(
            "STUDIO_CLAUDE_CREDENTIAL_IDENTITY".to_owned(),
            other.environment["STUDIO_CLAUDE_CREDENTIAL_IDENTITY"].clone(),
        );
        assert!(
            !studio_runtime_process::retained_environment_accepted(&foreign, &full),
            "a bridge of another credential is refused"
        );
    }
}
