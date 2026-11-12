//! Explicit Studio decisions for the pinned vendor methods. This file is not
//! generated: adding a vendor method must make these matches fail to compile.
use studio_codex_protocol::{
    ClientNotificationMethod, ClientRequestMethod, ServerNotificationMethod, ServerRequestMethod,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Handled,
    Ignored { reason: &'static str },
}

pub const fn client_request(method: ClientRequestMethod) -> Disposition {
    match method {
        ClientRequestMethod::Initialize => Disposition::Handled,
        ClientRequestMethod::ServerDiagnostics => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::UserVerificationStatus => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::UserVerificationEnroll => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::UserVerificationDelete => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::UserVerificationVerify => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::UserVerificationCancel => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadStart => Disposition::Handled,
        ClientRequestMethod::ThreadResume => Disposition::Handled,
        ClientRequestMethod::ThreadFork => Disposition::Handled,
        ClientRequestMethod::ThreadSample => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSampleCancel => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadArchive => Disposition::Handled,
        ClientRequestMethod::ThreadDelete => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadUnsubscribe => Disposition::Handled,
        ClientRequestMethod::ThreadIncrementElicitation => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadDecrementElicitation => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadNameSet => Disposition::Handled,
        ClientRequestMethod::ThreadPredictionRequest => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadGoalSet => Disposition::Ignored {
            reason: "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        },
        ClientRequestMethod::ThreadGoalGet => Disposition::Ignored {
            reason: "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        },
        ClientRequestMethod::ThreadGoalClear => Disposition::Ignored {
            reason: "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        },
        ClientRequestMethod::ThreadQueueAdd => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadQueueList => Disposition::Handled,
        ClientRequestMethod::ThreadQueueUpdate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadQueueDelete => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadQueueReorder => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadQueueStart => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadMetadataUpdate => Disposition::Handled,
        ClientRequestMethod::ThreadAttachmentAdd => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadAttachmentList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadAttachmentOwnerList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadAttachmentRemove => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSectionMove => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSettingsUpdate => Disposition::Handled,
        ClientRequestMethod::ThreadMemoryModeSet => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadMemoryRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::MemoryStatus => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::MemoryReset => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RolloutCompress => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadUnarchive => Disposition::Handled,
        ClientRequestMethod::ThreadCompactStart => Disposition::Handled,
        ClientRequestMethod::ThreadShellCommand => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadApproveGuardianDeniedAction => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadBackgroundTerminalsClean => Disposition::Handled,
        ClientRequestMethod::ThreadBackgroundTerminalsList => Disposition::Handled,
        ClientRequestMethod::ThreadBackgroundTerminalsTerminate => Disposition::Handled,
        ClientRequestMethod::ThreadRevert => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadList => Disposition::Handled,
        ClientRequestMethod::ProjectList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ProjectRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ProjectCreate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ProjectImport => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ProjectUpdate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ProjectMove => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ProjectDelete => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSectionList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSectionCreate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSectionUpdate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSectionDelete => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSearch => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadSearchOccurrences => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadLoadedList => Disposition::Handled,
        ClientRequestMethod::ThreadRead => Disposition::Handled,
        ClientRequestMethod::ThreadReadStateUpdate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadTurnsList => Disposition::Handled,
        ClientRequestMethod::ThreadItemsList => Disposition::Handled,
        ClientRequestMethod::ThreadItemsRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadGuardianReview => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadInjectItems => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::SkillsList => Disposition::Handled,
        ClientRequestMethod::SkillsExtraRootsSet => Disposition::Handled,
        ClientRequestMethod::HooksList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::MarketplaceAdd => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::MarketplaceRemove => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::MarketplaceUpgrade => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginSearch => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginInstalled => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginReconcile => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginSkillRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginShareSave => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginShareUpdateTargets => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginShareList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginShareCheckout => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginShareDelete => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AppRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AppList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AppInstalled => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsReadFile => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsWriteFile => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsCreateDirectory => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsGetMetadata => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsReadDirectory => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsRemove => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsCopy => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsWatch => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FsUnwatch => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::SkillsConfigWrite => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginInstall => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PluginUninstall => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::TurnStart => Disposition::Handled,
        ClientRequestMethod::TurnSettingsUpdate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::TurnSteer => Disposition::Handled,
        ClientRequestMethod::TurnInterrupt => Disposition::Handled,
        ClientRequestMethod::ThreadRealtimeStart => Disposition::Handled,
        ClientRequestMethod::ThreadRealtimeAppendAudio => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadRealtimeAppendText => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadRealtimeAppendSpeech => Disposition::Handled,
        ClientRequestMethod::ThreadRealtimeStop => Disposition::Handled,
        ClientRequestMethod::ThreadTimelineList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadRealtimeListVoices => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ReviewStart => Disposition::Handled,
        ClientRequestMethod::ModelList => Disposition::Handled,
        ClientRequestMethod::AccountGatewayOAuthRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountGatewayOAuthLogin => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountGatewayOAuthCancel => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ModelProviderCapabilitiesRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ExperimentalFeatureList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::PermissionProfileList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ExperimentalFeatureEnablementSet => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RemoteControlEnable => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RemoteControlDisable => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RemoteControlStatusRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RemoteControlPairingStart => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RemoteControlPairingStatus => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RemoteControlClientList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::RemoteControlClientRevoke => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::CollaborationModeList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::MockExperimentalMethod => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::EnvironmentAdd => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::EnvironmentInfo => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::EnvironmentStatus => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::McpServerOauthLogin => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ConfigMcpServerReload => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::McpServerStatusList => Disposition::Handled,
        ClientRequestMethod::McpServerResourceRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::McpServerEventStreamStart => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::McpServerEventStreamStop => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::McpServerToolCall => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::WindowsSandboxSetupStart => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::WindowsSandboxReadiness => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountLoginStart => Disposition::Handled,
        ClientRequestMethod::AccountBedrockDiscover => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountBedrockSetup => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountGcpSetup => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountBedrockCheckGovCloudRequirements => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountLoginCancel => Disposition::Handled,
        ClientRequestMethod::AccountSessionList => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountSessionSwitch => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountLogout => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountRateLimitsRead => Disposition::Handled,
        ClientRequestMethod::AccountRateLimitResetCreditConsume => Disposition::Handled,
        ClientRequestMethod::AccountUsageRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountWorkspaceMessagesRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountSendAddCreditsNudgeEmail => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FeedbackUpload => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FeedbackSave => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::CommandExec => Disposition::Handled,
        ClientRequestMethod::CommandExecWrite => Disposition::Handled,
        ClientRequestMethod::CommandExecTerminate => Disposition::Handled,
        ClientRequestMethod::CommandExecResize => Disposition::Handled,
        ClientRequestMethod::ProcessSpawn => Disposition::Handled,
        ClientRequestMethod::ProcessWriteStdin => Disposition::Handled,
        ClientRequestMethod::ProcessKill => Disposition::Handled,
        ClientRequestMethod::ProcessResizePty => Disposition::Handled,
        ClientRequestMethod::ConfigRead => Disposition::Handled,
        ClientRequestMethod::ExternalAgentConfigDetect => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ExternalAgentConfigImport => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ExternalAgentConfigImportRecordHistory => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ExternalAgentConfigImportReadHistories => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ConfigValueWrite => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ConfigBatchWrite => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ConfigRequirementsRead => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::AccountRead => Disposition::Handled,
        ClientRequestMethod::FuzzyFileSearch => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FuzzyFileSearchSessionStart => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FuzzyFileSearchSessionUpdate => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::FuzzyFileSearchSessionStop => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
    }
}

pub const fn server_notification(method: ServerNotificationMethod) -> Disposition {
    match method {
        ServerNotificationMethod::Error => Disposition::Handled,
        ServerNotificationMethod::ThreadStarted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadStatusChanged => Disposition::Handled,
        ServerNotificationMethod::ThreadReadStateChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadArchived => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadDeleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadUnarchived => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadClosed => Disposition::Handled,
        ServerNotificationMethod::ThreadReverted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::SkillsChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadNameUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadSectionChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadAttachmentUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadGoalUpdated => Disposition::Ignored {
            reason: "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        },
        ServerNotificationMethod::ThreadPredictionUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadGoalCleared => Disposition::Ignored {
            reason: "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        },
        ServerNotificationMethod::ThreadQueueChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ProjectChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadProjectUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadEnvironmentConnected => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadEnvironmentDisconnected => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadSettingsUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadTokenUsageUpdated => Disposition::Handled,
        ServerNotificationMethod::TurnStarted => Disposition::Handled,
        ServerNotificationMethod::HookStarted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::TurnCompleted => Disposition::Handled,
        ServerNotificationMethod::HookCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::TurnDiffUpdated => Disposition::Handled,
        ServerNotificationMethod::TurnPlanUpdated => Disposition::Handled,
        ServerNotificationMethod::ItemStarted => Disposition::Handled,
        ServerNotificationMethod::ItemAutoApprovalReviewStarted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ItemAutoApprovalReviewCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::AutoApprovalReviewStrictReviewRequired => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ItemCompleted => Disposition::Handled,
        ServerNotificationMethod::ItemAgentMessageDelta => Disposition::Handled,
        ServerNotificationMethod::ItemPlanDelta => Disposition::Handled,
        ServerNotificationMethod::CommandExecOutputDelta => Disposition::Handled,
        ServerNotificationMethod::ProcessOutputDelta => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ProcessExited => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ItemCommandExecutionOutputDelta => Disposition::Handled,
        ServerNotificationMethod::ItemCommandExecutionTerminalInteraction => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ItemFileChangeOutputDelta => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ItemFileChangePatchUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ServerRequestResolved => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ItemMcpToolCallProgress => Disposition::Handled,
        ServerNotificationMethod::McpServerOauthLoginCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::McpServerStartupStatusUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::McpServerEventStreamNotification => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::AccountUpdated => Disposition::Handled,
        ServerNotificationMethod::AccountGatewayOAuthChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::AccountRateLimitsUpdated => Disposition::Handled,
        ServerNotificationMethod::AppListUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::RemoteControlStatusChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ExternalAgentConfigImportProgress => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ExternalAgentConfigImportCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::FsChanged => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ItemReasoningSummaryTextDelta => Disposition::Handled,
        ServerNotificationMethod::ItemReasoningSummaryPartAdded => Disposition::Handled,
        ServerNotificationMethod::ItemReasoningTextDelta => Disposition::Handled,
        ServerNotificationMethod::ThreadCompacted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ModelRerouted => Disposition::Handled,
        ServerNotificationMethod::ModelVerification => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ModelProviderAuthRecoveryStarted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ModelProviderAuthRecoveryCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::TurnModerationMetadata => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ModelSafetyBufferingUpdated => Disposition::Handled,
        ServerNotificationMethod::Warning => Disposition::Handled,
        ServerNotificationMethod::GuardianWarning => Disposition::Handled,
        ServerNotificationMethod::DeprecationNotice => Disposition::Handled,
        ServerNotificationMethod::ConfigWarning => Disposition::Handled,
        ServerNotificationMethod::FuzzyFileSearchSessionUpdated => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::FuzzyFileSearchSessionCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadRealtimeStarted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadRealtimeItemAdded => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadRealtimeItemStarted => Disposition::Handled,
        ServerNotificationMethod::ThreadRealtimeItemTranscriptDelta => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadRealtimeItemCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadRealtimeTranscriptDelta => Disposition::Handled,
        ServerNotificationMethod::ThreadRealtimeTranscriptDone => Disposition::Handled,
        ServerNotificationMethod::ThreadRealtimeOutputAudioDelta => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::ThreadRealtimeSdp => Disposition::Handled,
        ServerNotificationMethod::ThreadRealtimeError => Disposition::Handled,
        ServerNotificationMethod::ThreadRealtimeClosed => Disposition::Handled,
        ServerNotificationMethod::WindowsWorldWritableWarning => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::WindowsSandboxSetupCompleted => Disposition::Ignored {
            reason: "Studio retains this notification without interpreting its payload.",
        },
        ServerNotificationMethod::AccountLoginCompleted => Disposition::Handled,
    }
}

pub const fn server_request(method: ServerRequestMethod) -> Disposition {
    match method {
        ServerRequestMethod::ItemCommandExecutionRequestApproval => Disposition::Handled,
        ServerRequestMethod::ItemFileChangeRequestApproval => Disposition::Handled,
        ServerRequestMethod::ItemToolRequestUserInput => Disposition::Handled,
        ServerRequestMethod::McpServerElicitationRequest => Disposition::Handled,
        ServerRequestMethod::ItemPermissionsRequestApproval => Disposition::Handled,
        ServerRequestMethod::ItemToolCall => Disposition::Handled,
        ServerRequestMethod::AccountChatgptAuthTokensRefresh => Disposition::Ignored {
            reason: "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
        },
        ServerRequestMethod::AttestationGenerate => Disposition::Ignored {
            reason: "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
        },
        ServerRequestMethod::CurrentTimeRead => Disposition::Handled,
        ServerRequestMethod::ApplyPatchApproval => Disposition::Ignored {
            reason: "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
        },
        ServerRequestMethod::ExecCommandApproval => Disposition::Ignored {
            reason: "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
        },
    }
}

pub const fn client_notification(method: ClientNotificationMethod) -> Disposition {
    match method {
        ClientNotificationMethod::Initialized => Disposition::Handled,
    }
}

#[cfg(test)]
mod goal_disposition_tests {
    use super::*;

    #[test]
    fn retired_native_goals_are_deferred_consistently() {
        for decision in [
            client_request(ClientRequestMethod::ThreadGoalSet),
            client_request(ClientRequestMethod::ThreadGoalGet),
            client_request(ClientRequestMethod::ThreadGoalClear),
            server_notification(ServerNotificationMethod::ThreadGoalUpdated),
            server_notification(ServerNotificationMethod::ThreadGoalCleared),
        ] {
            let Disposition::Ignored { reason } = decision else {
                panic!("retired goals cannot be advertised as handled");
            };
            assert!(reason.contains("m2026_11_11_retired_agent_keys"));
        }
    }
}
