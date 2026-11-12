//! Explicit Studio decisions for the pinned vendor methods. This file is not
//! generated: adding a vendor method must make these matches fail to compile.
use studio_codex_protocol::{
    ClientNotificationMethod, ClientRequestMethod, ServerNotificationMethod, ServerRequestMethod,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Handled(HandlerKey),
    Ignored { reason: &'static str },
}

/// Native setting echoes cannot overwrite Studio stored session settings.
pub const THREAD_SETTINGS_ECHO_REASON: &str =
    "Studio owns session settings; uncorrelated native settings echoes cannot change local intent.";

/// Why native queue mutations and change notifications are intentionally deferred.
/// Queue-list reads remain safety checks for unload and recovery.
const STUDIO_QUEUE_OWNERSHIP_REASON: &str = "Studio owns the durable cross-provider queue; native queue is not populated by Studio. Busy-turn input uses turn/steer, not native queued submissions.";

/// Dispatch identity; receiver keys are generated only for Handled entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandlerKey {
    Sender(ClientRequestMethod),
    Notification(NotificationHandler),
    Callback(CallbackHandler),
    Initialized,
}

macro_rules! receiver_policy {
    ($function:ident, $method:ident, $key:ident, $family:ident;
        handled { $( $group:ident($group_key:ident) { $( $handled:ident => $variant:ident, )* } )* }
        ignored { $( $ignored:ident => $reason:expr, )* }
    ) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $key { $( $group($group_key), )* }
        $(
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            pub enum $group_key { $( $variant, )* }
        )*
        pub const fn $function(method: $method) -> Disposition {
            match method {
                $( $( $method::$handled => Disposition::Handled(HandlerKey::$family($key::$group($group_key::$variant))), )* )*
                $( $method::$ignored => Disposition::Ignored { reason: $reason }, )*
            }
        }
    };
    ($function:ident, $method:ident, $key:ident, $family:ident;
        handled { $( $handled:ident, )* }
        ignored { $( $ignored:ident => $reason:expr, )* }
    ) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $key { $( $handled, )* }
        pub const fn $function(method: $method) -> Disposition {
            match method {
                $( $method::$handled => Disposition::Handled(HandlerKey::$family($key::$handled)), )*
                $( $method::$ignored => Disposition::Ignored { reason: $reason }, )*
            }
        }
    };
}

/// Actual emitted methods are checked by the existing internal_sender_witness
/// registry and its shared request helper. Removing a sender's last call site
/// can leave a stale Handled entry; this intentionally adds no second registry.
pub const fn client_request(method: ClientRequestMethod) -> Disposition {
    match method {
        ClientRequestMethod::TurnSettingsUpdate
        | ClientRequestMethod::AppList
        | ClientRequestMethod::PluginList
        | ClientRequestMethod::Initialize
        | ClientRequestMethod::ThreadStart
        | ClientRequestMethod::ThreadResume
        | ClientRequestMethod::ThreadFork
        | ClientRequestMethod::ThreadUnsubscribe
        | ClientRequestMethod::ThreadQueueList
        | ClientRequestMethod::ThreadSettingsUpdate
        | ClientRequestMethod::ThreadCompactStart
        | ClientRequestMethod::ThreadBackgroundTerminalsClean
        | ClientRequestMethod::ThreadBackgroundTerminalsList
        | ClientRequestMethod::ThreadBackgroundTerminalsTerminate
        | ClientRequestMethod::ThreadList
        | ClientRequestMethod::ThreadLoadedList
        | ClientRequestMethod::ThreadRead
        | ClientRequestMethod::ThreadTurnsList
        | ClientRequestMethod::ThreadItemsList
        | ClientRequestMethod::SkillsList
        | ClientRequestMethod::SkillsExtraRootsSet
        | ClientRequestMethod::TurnStart
        | ClientRequestMethod::TurnSteer
        | ClientRequestMethod::TurnInterrupt
        | ClientRequestMethod::ThreadRealtimeStart
        | ClientRequestMethod::ThreadRealtimeAppendSpeech
        | ClientRequestMethod::ThreadRealtimeStop
        | ClientRequestMethod::ReviewStart
        | ClientRequestMethod::ModelList
        | ClientRequestMethod::McpServerStatusList
        | ClientRequestMethod::AccountLoginStart
        | ClientRequestMethod::AccountLoginCancel
        | ClientRequestMethod::AccountRateLimitsRead
        | ClientRequestMethod::AccountRateLimitResetCreditConsume
        | ClientRequestMethod::CommandExec
        | ClientRequestMethod::CommandExecWrite
        | ClientRequestMethod::CommandExecTerminate
        | ClientRequestMethod::CommandExecResize
        | ClientRequestMethod::ProcessSpawn
        | ClientRequestMethod::ProcessWriteStdin
        | ClientRequestMethod::ProcessKill
        | ClientRequestMethod::ProcessResizePty
        | ClientRequestMethod::ConfigRead
        | ClientRequestMethod::AccountRead => Disposition::Handled(HandlerKey::Sender(method)),
        ClientRequestMethod::ServerDiagnostics
        | ClientRequestMethod::UserVerificationStatus
        | ClientRequestMethod::UserVerificationEnroll
        | ClientRequestMethod::UserVerificationDelete
        | ClientRequestMethod::UserVerificationVerify
        | ClientRequestMethod::UserVerificationCancel
        | ClientRequestMethod::ThreadSample
        | ClientRequestMethod::ThreadSampleCancel
        | ClientRequestMethod::ThreadArchive
        | ClientRequestMethod::ThreadDelete
        | ClientRequestMethod::ThreadIncrementElicitation
        | ClientRequestMethod::ThreadDecrementElicitation
        | ClientRequestMethod::ThreadNameSet
        | ClientRequestMethod::ThreadPredictionRequest
        | ClientRequestMethod::ThreadMetadataUpdate
        | ClientRequestMethod::ThreadAttachmentAdd
        | ClientRequestMethod::ThreadAttachmentList
        | ClientRequestMethod::ThreadAttachmentOwnerList
        | ClientRequestMethod::ThreadAttachmentRemove
        | ClientRequestMethod::ThreadSectionMove
        | ClientRequestMethod::ThreadMemoryModeSet
        | ClientRequestMethod::ThreadMemoryRead
        | ClientRequestMethod::MemoryStatus
        | ClientRequestMethod::MemoryReset
        | ClientRequestMethod::RolloutCompress
        | ClientRequestMethod::ThreadUnarchive
        | ClientRequestMethod::ThreadShellCommand
        | ClientRequestMethod::ThreadApproveGuardianDeniedAction
        | ClientRequestMethod::ThreadRevert
        | ClientRequestMethod::ProjectList
        | ClientRequestMethod::ProjectRead
        | ClientRequestMethod::ProjectCreate
        | ClientRequestMethod::ProjectImport
        | ClientRequestMethod::ProjectUpdate
        | ClientRequestMethod::ProjectMove
        | ClientRequestMethod::ProjectDelete
        | ClientRequestMethod::ThreadSectionList
        | ClientRequestMethod::ThreadSectionCreate
        | ClientRequestMethod::ThreadSectionUpdate
        | ClientRequestMethod::ThreadSectionDelete
        | ClientRequestMethod::ThreadSearch
        | ClientRequestMethod::ThreadSearchOccurrences
        | ClientRequestMethod::ThreadReadStateUpdate
        | ClientRequestMethod::ThreadItemsRead
        | ClientRequestMethod::ThreadGuardianReview
        | ClientRequestMethod::ThreadInjectItems
        | ClientRequestMethod::HooksList
        | ClientRequestMethod::MarketplaceAdd
        | ClientRequestMethod::MarketplaceRemove
        | ClientRequestMethod::MarketplaceUpgrade
        | ClientRequestMethod::PluginSearch
        | ClientRequestMethod::PluginInstalled
        | ClientRequestMethod::PluginReconcile
        | ClientRequestMethod::PluginRead
        | ClientRequestMethod::PluginSkillRead
        | ClientRequestMethod::PluginShareSave
        | ClientRequestMethod::PluginShareUpdateTargets
        | ClientRequestMethod::PluginShareList
        | ClientRequestMethod::PluginShareCheckout
        | ClientRequestMethod::PluginShareDelete
        | ClientRequestMethod::AppRead
        | ClientRequestMethod::AppInstalled
        | ClientRequestMethod::FsReadFile
        | ClientRequestMethod::FsWriteFile
        | ClientRequestMethod::FsCreateDirectory
        | ClientRequestMethod::FsGetMetadata
        | ClientRequestMethod::FsReadDirectory
        | ClientRequestMethod::FsRemove
        | ClientRequestMethod::FsCopy
        | ClientRequestMethod::FsWatch
        | ClientRequestMethod::FsUnwatch
        | ClientRequestMethod::SkillsConfigWrite
        | ClientRequestMethod::PluginInstall
        | ClientRequestMethod::PluginUninstall
        | ClientRequestMethod::ThreadRealtimeAppendAudio
        | ClientRequestMethod::ThreadRealtimeAppendText
        | ClientRequestMethod::ThreadTimelineList
        | ClientRequestMethod::ThreadRealtimeListVoices
        | ClientRequestMethod::AccountGatewayOAuthRead
        | ClientRequestMethod::AccountGatewayOAuthLogin
        | ClientRequestMethod::AccountGatewayOAuthCancel
        | ClientRequestMethod::ModelProviderCapabilitiesRead
        | ClientRequestMethod::ExperimentalFeatureList
        | ClientRequestMethod::PermissionProfileList
        | ClientRequestMethod::ExperimentalFeatureEnablementSet
        | ClientRequestMethod::RemoteControlEnable
        | ClientRequestMethod::RemoteControlDisable
        | ClientRequestMethod::RemoteControlStatusRead
        | ClientRequestMethod::RemoteControlPairingStart
        | ClientRequestMethod::RemoteControlPairingStatus
        | ClientRequestMethod::RemoteControlClientList
        | ClientRequestMethod::RemoteControlClientRevoke
        | ClientRequestMethod::CollaborationModeList
        | ClientRequestMethod::MockExperimentalMethod
        | ClientRequestMethod::EnvironmentAdd
        | ClientRequestMethod::EnvironmentInfo
        | ClientRequestMethod::EnvironmentStatus
        | ClientRequestMethod::McpServerOauthLogin
        | ClientRequestMethod::ConfigMcpServerReload
        | ClientRequestMethod::McpServerResourceRead
        | ClientRequestMethod::McpServerEventStreamStart
        | ClientRequestMethod::McpServerEventStreamStop
        | ClientRequestMethod::McpServerToolCall
        | ClientRequestMethod::WindowsSandboxSetupStart
        | ClientRequestMethod::WindowsSandboxReadiness
        | ClientRequestMethod::AccountBedrockDiscover
        | ClientRequestMethod::AccountBedrockSetup
        | ClientRequestMethod::AccountGcpSetup
        | ClientRequestMethod::AccountBedrockCheckGovCloudRequirements
        | ClientRequestMethod::AccountSessionList
        | ClientRequestMethod::AccountSessionSwitch
        | ClientRequestMethod::AccountLogout
        | ClientRequestMethod::AccountUsageRead
        | ClientRequestMethod::AccountWorkspaceMessagesRead
        | ClientRequestMethod::AccountSendAddCreditsNudgeEmail
        | ClientRequestMethod::FeedbackUpload
        | ClientRequestMethod::FeedbackSave
        | ClientRequestMethod::ExternalAgentConfigDetect
        | ClientRequestMethod::ExternalAgentConfigImport
        | ClientRequestMethod::ExternalAgentConfigImportRecordHistory
        | ClientRequestMethod::ExternalAgentConfigImportReadHistories
        | ClientRequestMethod::ConfigValueWrite
        | ClientRequestMethod::ConfigBatchWrite
        | ClientRequestMethod::ConfigRequirementsRead
        | ClientRequestMethod::FuzzyFileSearch
        | ClientRequestMethod::FuzzyFileSearchSessionStart
        | ClientRequestMethod::FuzzyFileSearchSessionUpdate
        | ClientRequestMethod::FuzzyFileSearchSessionStop => Disposition::Ignored {
            reason: "Studio does not issue this vendor request.",
        },
        ClientRequestMethod::ThreadQueueAdd
        | ClientRequestMethod::ThreadQueueUpdate
        | ClientRequestMethod::ThreadQueueDelete
        | ClientRequestMethod::ThreadQueueReorder
        | ClientRequestMethod::ThreadQueueStart => Disposition::Ignored {
            reason: STUDIO_QUEUE_OWNERSHIP_REASON,
        },
        ClientRequestMethod::ThreadGoalSet
        | ClientRequestMethod::ThreadGoalGet
        | ClientRequestMethod::ThreadGoalClear => Disposition::Ignored {
            reason: "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        },
    }
}

receiver_policy! {
    server_notification, ServerNotificationMethod, NotificationHandler, Notification;
    handled {
        Notice(NoticeHandler) {
            Error => Error,
            Warning => Warning,
            GuardianWarning => Guardian,
            ConfigWarning => Config,
            DeprecationNotice => Deprecation,
            ModelRerouted => Rerouted,
            McpServerStartupStatusUpdated => McpStartup,
            McpServerOauthLoginCompleted => McpLogin,
            AutoApprovalReviewStrictReviewRequired => StrictReview,
            HookStarted => HookStarted,
            HookCompleted => HookCompleted,
            ModelSafetyBufferingUpdated => Safety,
            ModelVerification => Verification,
            ItemAutoApprovalReviewStarted => ReviewStarted,
            ItemAutoApprovalReviewCompleted => ReviewCompleted,
            ModelProviderAuthRecoveryStarted => AuthStarted,
            ModelProviderAuthRecoveryCompleted => AuthCompleted,
        }
        Projection(ProjectionHandler) {
            AppListUpdated => AppListUpdated,
            SkillsChanged => SkillsChanged,
            ThreadStatusChanged => ThreadStatusChanged,
            ThreadClosed => ThreadClosed,
            ThreadTokenUsageUpdated => ThreadTokenUsageUpdated,
            TurnStarted => TurnStarted,
            TurnCompleted => TurnCompleted,
            TurnDiffUpdated => TurnDiffUpdated,
            TurnPlanUpdated => TurnPlanUpdated,
            ItemAgentMessageDelta => ItemAgentMessageDelta,
            CommandExecOutputDelta => CommandExecOutputDelta,
            ItemCommandExecutionOutputDelta => ItemCommandExecutionOutputDelta,
            ServerRequestResolved => ServerRequestResolved,
            AccountUpdated => AccountUpdated,
            AccountRateLimitsUpdated => AccountRateLimitsUpdated,
            ItemReasoningTextDelta => ItemReasoningTextDelta,
            ThreadRealtimeStarted => ThreadRealtimeStarted,
            ThreadRealtimeItemAdded => ThreadRealtimeItemAdded,
            ThreadRealtimeItemTranscriptDelta => ThreadRealtimeItemTranscriptDelta,
            ThreadRealtimeOutputAudioDelta => ThreadRealtimeOutputAudioDelta,
            ThreadRealtimeItemStarted => ThreadRealtimeItemStarted,
            ThreadRealtimeItemCompleted => ThreadRealtimeItemCompleted,
            ThreadRealtimeTranscriptDelta => ThreadRealtimeTranscriptDelta,
            ThreadRealtimeTranscriptDone => ThreadRealtimeTranscriptDone,
            ThreadRealtimeSdp => ThreadRealtimeSdp,
            ThreadRealtimeError => ThreadRealtimeError,
            ThreadRealtimeClosed => ThreadRealtimeClosed,
            AccountLoginCompleted => AccountLoginCompleted,
        }
        Progress(ProgressHandler) {
            ItemStarted => ItemStarted,
            ItemCompleted => ItemCompleted,
            ItemPlanDelta => ItemPlanDelta,
            ItemMcpToolCallProgress => ItemMcpToolCallProgress,
            ItemReasoningSummaryTextDelta => ItemReasoningSummaryTextDelta,
            ItemReasoningSummaryPartAdded => ItemReasoningSummaryPartAdded,
        }
    }
    ignored {
        ThreadStarted => "Studio retains this notification without interpreting its payload.",
        ThreadReadStateChanged => "Studio retains this notification without interpreting its payload.",
        ThreadArchived => "Studio retains this notification without interpreting its payload.",
        ThreadDeleted => "Studio retains this notification without interpreting its payload.",
        ThreadUnarchived => "Studio retains this notification without interpreting its payload.",
        ThreadReverted => "Studio retains this notification without interpreting its payload.",
        ThreadNameUpdated => "Studio retains this notification without interpreting its payload.",
        ThreadSectionChanged => "Studio retains this notification without interpreting its payload.",
        ThreadAttachmentUpdated => "Studio retains this notification without interpreting its payload.",
        ThreadGoalUpdated => "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        ThreadPredictionUpdated => "Studio retains this notification without interpreting its payload.",
        ThreadGoalCleared => "Studio retired goals/budgets in m2026_11_11_retired_agent_keys; native goals are deferred until an owner design exists.",
        ThreadQueueChanged => STUDIO_QUEUE_OWNERSHIP_REASON,
        ProjectChanged => "Studio retains this notification without interpreting its payload.",
        ThreadProjectUpdated => "Studio retains this notification without interpreting its payload.",
        ThreadEnvironmentConnected => "Studio retains this notification without interpreting its payload.",
        ThreadEnvironmentDisconnected => "Studio retains this notification without interpreting its payload.",
        ThreadSettingsUpdated => THREAD_SETTINGS_ECHO_REASON,
        ProcessOutputDelta => "Studio retains this notification without interpreting its payload.",
        ProcessExited => "Studio retains this notification without interpreting its payload.",
        ItemCommandExecutionTerminalInteraction => "Studio retains this notification without interpreting its payload.",
        ItemFileChangeOutputDelta => "Studio retains this notification without interpreting its payload.",
        ItemFileChangePatchUpdated => "Studio retains this notification without interpreting its payload.",
        McpServerEventStreamNotification => "Studio retains this notification without interpreting its payload.",
        AccountGatewayOAuthChanged => "Studio retains this notification without interpreting its payload.",
        RemoteControlStatusChanged => "Studio retains this notification without interpreting its payload.",
        ExternalAgentConfigImportProgress => "Studio retains this notification without interpreting its payload.",
        ExternalAgentConfigImportCompleted => "Studio retains this notification without interpreting its payload.",
        FsChanged => "Studio retains this notification without interpreting its payload.",
        ThreadCompacted => "Studio retains this notification without interpreting its payload.",
        TurnModerationMetadata => "Studio retains this notification without interpreting its payload.",
        FuzzyFileSearchSessionUpdated => "Studio retains this notification without interpreting its payload.",
        FuzzyFileSearchSessionCompleted => "Studio retains this notification without interpreting its payload.",
        WindowsWorldWritableWarning => "Studio retains this notification without interpreting its payload.",
        WindowsSandboxSetupCompleted => "Studio retains this notification without interpreting its payload.",
    }
}

receiver_policy! {
    server_request, ServerRequestMethod, CallbackHandler, Callback;
    handled {
        ItemCommandExecutionRequestApproval,
        ItemFileChangeRequestApproval,
        ItemToolRequestUserInput,
        McpServerElicitationRequest,
        ItemPermissionsRequestApproval,
        ItemToolCall,
        CurrentTimeRead,
    }
    ignored {
        AccountChatgptAuthTokensRefresh => "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
        AttestationGenerate => "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
        ApplyPatchApproval => "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
        ExecCommandApproval => "Studio has no handler for this vendor callback; existing unsupported-request handling applies.",
    }
}

pub const fn client_notification(method: ClientNotificationMethod) -> Disposition {
    match method {
        ClientNotificationMethod::Initialized => Disposition::Handled(HandlerKey::Initialized),
    }
}

#[cfg(test)]
mod queue_disposition_tests {
    use super::*;

    #[test]
    fn studio_queue_ownership_keeps_native_mutations_and_changes_deferred() {
        let decisions = [
            client_request(ClientRequestMethod::ThreadQueueAdd),
            client_request(ClientRequestMethod::ThreadQueueUpdate),
            client_request(ClientRequestMethod::ThreadQueueDelete),
            client_request(ClientRequestMethod::ThreadQueueReorder),
            client_request(ClientRequestMethod::ThreadQueueStart),
            server_notification(ServerNotificationMethod::ThreadQueueChanged),
        ];
        for decision in decisions {
            let Disposition::Ignored { reason } = decision else {
                panic!("Studio does not populate the native queue");
            };
            assert!(reason.contains("durable cross-provider queue"));
            assert_eq!(decision, decisions[0], "one queue ownership decision");
        }
        assert_eq!(
            client_request(ClientRequestMethod::ThreadQueueList),
            Disposition::Handled(HandlerKey::Sender(ClientRequestMethod::ThreadQueueList))
        );
        assert_eq!(
            client_request(ClientRequestMethod::TurnSteer),
            Disposition::Handled(HandlerKey::Sender(ClientRequestMethod::TurnSteer))
        );
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
