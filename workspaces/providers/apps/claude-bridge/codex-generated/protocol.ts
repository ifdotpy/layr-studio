// Generated from codex-cli 0.167.1 full experimental JSON Schema; do not edit.
// Owner: provider-contract-sources.ts; SHA-256: 486c7c8daa5dc7f3ab2a1fb56de5316804caf7a4210fda1dc28d3f0f4e90cfe1

export type AbsolutePathBuf = string;

export type AdditionalPermissionProfile = {
  fileSystem?: AdditionalFileSystemPermissions | null;
  network?: AdditionalNetworkPermissions | null;
};

export type ApplyPatchApprovalParams = {
  callId: string;
  conversationId: ThreadId;
  fileChanges: Record<string, FileChange>;
  grantRoot?: string | null;
  reason?: string | null;
};

export type ApplyPatchApprovalResponse = { decision: ReviewDecision };

export type AttestationGenerateParams = Record<string, unknown>;

export type AttestationGenerateResponse = { token: string };

export type ChatgptAuthTokensRefreshParams = {
  previousAccountId?: string | null;
  reason: ChatgptAuthTokensRefreshReason;
};

export type ChatgptAuthTokensRefreshReason = "unauthorized";

export type ChatgptAuthTokensRefreshResponse = {
  accessToken: string;
  chatgptAccountId: string;
  chatgptPlanType?: string | null;
};

export type ClientInfo = {
  name: string;
  title?: string | null;
  version: string;
};

export type ClientNotification = { method: "initialized" };

export type ClientRequest =
  | { id: RequestId; method: "initialize"; params: InitializeParams }
  | {
      id: RequestId;
      method: "server/diagnostics";
      params: ServerDiagnosticsParams;
    }
  | {
      id: RequestId;
      method: "userVerification/status";
      params: UserVerificationStatusParams;
    }
  | {
      id: RequestId;
      method: "userVerification/enroll";
      params: UserVerificationEnrollParams;
    }
  | {
      id: RequestId;
      method: "userVerification/delete";
      params: UserVerificationDeleteParams;
    }
  | {
      id: RequestId;
      method: "userVerification/verify";
      params: UserVerificationVerifyParams;
    }
  | {
      id: RequestId;
      method: "userVerification/cancel";
      params: UserVerificationCancelParams;
    }
  | { id: RequestId; method: "thread/start"; params: ThreadStartParams }
  | { id: RequestId; method: "thread/resume"; params: ThreadResumeParams }
  | { id: RequestId; method: "thread/fork"; params: ThreadForkParams }
  | { id: RequestId; method: "thread/sample"; params: ThreadSampleParams }
  | {
      id: RequestId;
      method: "thread/sample/cancel";
      params: ThreadSampleCancelParams;
    }
  | { id: RequestId; method: "thread/archive"; params: ThreadArchiveParams }
  | { id: RequestId; method: "thread/delete"; params: ThreadDeleteParams }
  | {
      id: RequestId;
      method: "thread/unsubscribe";
      params: ThreadUnsubscribeParams;
    }
  | {
      id: RequestId;
      method: "thread/increment_elicitation";
      params: ThreadIncrementElicitationParams;
    }
  | {
      id: RequestId;
      method: "thread/decrement_elicitation";
      params: ThreadDecrementElicitationParams;
    }
  | { id: RequestId; method: "thread/name/set"; params: ThreadSetNameParams }
  | {
      id: RequestId;
      method: "thread/prediction/request";
      params: ThreadPredictionRequestParams;
    }
  | { id: RequestId; method: "thread/goal/set"; params: ThreadGoalSetParams }
  | { id: RequestId; method: "thread/goal/get"; params: ThreadGoalGetParams }
  | {
      id: RequestId;
      method: "thread/goal/clear";
      params: ThreadGoalClearParams;
    }
  | { id: RequestId; method: "thread/queue/add"; params: ThreadQueueAddParams }
  | {
      id: RequestId;
      method: "thread/queue/list";
      params: ThreadQueueListParams;
    }
  | {
      id: RequestId;
      method: "thread/queue/update";
      params: ThreadQueueUpdateParams;
    }
  | {
      id: RequestId;
      method: "thread/queue/delete";
      params: ThreadQueueDeleteParams;
    }
  | {
      id: RequestId;
      method: "thread/queue/reorder";
      params: ThreadQueueReorderParams;
    }
  | {
      id: RequestId;
      method: "thread/queue/start";
      params: ThreadQueueStartParams;
    }
  | {
      id: RequestId;
      method: "thread/metadata/update";
      params: ThreadMetadataUpdateParams;
    }
  | {
      id: RequestId;
      method: "thread/attachment/add";
      params: ThreadAttachmentAddParams;
    }
  | {
      id: RequestId;
      method: "thread/attachment/list";
      params: ThreadAttachmentListParams;
    }
  | {
      id: RequestId;
      method: "thread/attachmentOwner/list";
      params: ThreadAttachmentOwnerListParams;
    }
  | {
      id: RequestId;
      method: "thread/attachment/remove";
      params: ThreadAttachmentRemoveParams;
    }
  | {
      id: RequestId;
      method: "thread/section/move";
      params: ThreadSectionMoveParams;
    }
  | {
      id: RequestId;
      method: "thread/settings/update";
      params: ThreadSettingsUpdateParams;
    }
  | {
      id: RequestId;
      method: "thread/memoryMode/set";
      params: ThreadMemoryModeSetParams;
    }
  | {
      id: RequestId;
      method: "thread/memory/read";
      params: ThreadMemoryReadParams;
    }
  | { id: RequestId; method: "memory/status"; params: MemoryStatusParams }
  | { id: RequestId; method: "memory/reset"; params?: null }
  | { id: RequestId; method: "rollout/compress"; params?: null }
  | { id: RequestId; method: "thread/unarchive"; params: ThreadUnarchiveParams }
  | {
      id: RequestId;
      method: "thread/compact/start";
      params: ThreadCompactStartParams;
    }
  | {
      id: RequestId;
      method: "thread/shellCommand";
      params: ThreadShellCommandParams;
    }
  | {
      id: RequestId;
      method: "thread/approveGuardianDeniedAction";
      params: ThreadApproveGuardianDeniedActionParams;
    }
  | {
      id: RequestId;
      method: "thread/backgroundTerminals/clean";
      params: ThreadBackgroundTerminalsCleanParams;
    }
  | {
      id: RequestId;
      method: "thread/backgroundTerminals/list";
      params: ThreadBackgroundTerminalsListParams;
    }
  | {
      id: RequestId;
      method: "thread/backgroundTerminals/terminate";
      params: ThreadBackgroundTerminalsTerminateParams;
    }
  | { id: RequestId; method: "thread/revert"; params: ThreadRevertParams }
  | { id: RequestId; method: "thread/list"; params: ThreadListParams }
  | { id: RequestId; method: "project/list"; params: ProjectListParams }
  | { id: RequestId; method: "project/read"; params: ProjectReadParams }
  | { id: RequestId; method: "project/create"; params: ProjectCreateParams }
  | { id: RequestId; method: "project/import"; params: ProjectImportParams }
  | { id: RequestId; method: "project/update"; params: ProjectUpdateParams }
  | { id: RequestId; method: "project/move"; params: ProjectMoveParams }
  | { id: RequestId; method: "project/delete"; params: ProjectDeleteParams }
  | {
      id: RequestId;
      method: "threadSection/list";
      params: ThreadSectionListParams;
    }
  | {
      id: RequestId;
      method: "threadSection/create";
      params: ThreadSectionCreateParams;
    }
  | {
      id: RequestId;
      method: "threadSection/update";
      params: ThreadSectionUpdateParams;
    }
  | {
      id: RequestId;
      method: "threadSection/delete";
      params: ThreadSectionDeleteParams;
    }
  | { id: RequestId; method: "thread/search"; params: ThreadSearchParams }
  | {
      id: RequestId;
      method: "thread/searchOccurrences";
      params: ThreadSearchOccurrencesParams;
    }
  | {
      id: RequestId;
      method: "thread/loaded/list";
      params: ThreadLoadedListParams;
    }
  | { id: RequestId; method: "thread/read"; params: ThreadReadParams }
  | {
      id: RequestId;
      method: "thread/readState/update";
      params: ThreadReadStateUpdateParams;
    }
  | {
      id: RequestId;
      method: "thread/turns/list";
      params: ThreadTurnsListParams;
    }
  | {
      id: RequestId;
      method: "thread/items/list";
      params: ThreadItemsListParams;
    }
  | {
      id: RequestId;
      method: "thread/items/read";
      params: ThreadItemsReadParams;
    }
  | {
      id: RequestId;
      method: "thread/guardianReview";
      params: ThreadGuardianReviewParams;
    }
  | {
      id: RequestId;
      method: "thread/inject_items";
      params: ThreadInjectItemsParams;
    }
  | { id: RequestId; method: "skills/list"; params: SkillsListParams }
  | {
      id: RequestId;
      method: "skills/extraRoots/set";
      params: SkillsExtraRootsSetParams;
    }
  | { id: RequestId; method: "hooks/list"; params: HooksListParams }
  | { id: RequestId; method: "marketplace/add"; params: MarketplaceAddParams }
  | {
      id: RequestId;
      method: "marketplace/remove";
      params: MarketplaceRemoveParams;
    }
  | {
      id: RequestId;
      method: "marketplace/upgrade";
      params: MarketplaceUpgradeParams;
    }
  | { id: RequestId; method: "plugin/list"; params: PluginListParams }
  | { id: RequestId; method: "plugin/search"; params: PluginSearchParams }
  | { id: RequestId; method: "plugin/installed"; params: PluginInstalledParams }
  | { id: RequestId; method: "plugin/reconcile"; params: PluginReconcileParams }
  | { id: RequestId; method: "plugin/read"; params: PluginReadParams }
  | {
      id: RequestId;
      method: "plugin/skill/read";
      params: PluginSkillReadParams;
    }
  | {
      id: RequestId;
      method: "plugin/share/save";
      params: PluginShareSaveParams;
    }
  | {
      id: RequestId;
      method: "plugin/share/updateTargets";
      params: PluginShareUpdateTargetsParams;
    }
  | {
      id: RequestId;
      method: "plugin/share/list";
      params: PluginShareListParams;
    }
  | {
      id: RequestId;
      method: "plugin/share/checkout";
      params: PluginShareCheckoutParams;
    }
  | {
      id: RequestId;
      method: "plugin/share/delete";
      params: PluginShareDeleteParams;
    }
  | { id: RequestId; method: "app/read"; params: AppsReadParams }
  | { id: RequestId; method: "app/list"; params: AppsListParams }
  | { id: RequestId; method: "app/installed"; params: AppsInstalledParams }
  | { id: RequestId; method: "fs/readFile"; params: FsReadFileParams }
  | { id: RequestId; method: "fs/writeFile"; params: FsWriteFileParams }
  | {
      id: RequestId;
      method: "fs/createDirectory";
      params: FsCreateDirectoryParams;
    }
  | { id: RequestId; method: "fs/getMetadata"; params: FsGetMetadataParams }
  | { id: RequestId; method: "fs/readDirectory"; params: FsReadDirectoryParams }
  | { id: RequestId; method: "fs/remove"; params: FsRemoveParams }
  | { id: RequestId; method: "fs/copy"; params: FsCopyParams }
  | { id: RequestId; method: "fs/watch"; params: FsWatchParams }
  | { id: RequestId; method: "fs/unwatch"; params: FsUnwatchParams }
  | {
      id: RequestId;
      method: "skills/config/write";
      params: SkillsConfigWriteParams;
    }
  | { id: RequestId; method: "plugin/install"; params: PluginInstallParams }
  | { id: RequestId; method: "plugin/uninstall"; params: PluginUninstallParams }
  | { id: RequestId; method: "turn/start"; params: TurnStartParams }
  | {
      id: RequestId;
      method: "turn/settings/update";
      params: TurnSettingsUpdateParams;
    }
  | { id: RequestId; method: "turn/steer"; params: TurnSteerParams }
  | { id: RequestId; method: "turn/interrupt"; params: TurnInterruptParams }
  | {
      id: RequestId;
      method: "thread/realtime/start";
      params: ThreadRealtimeStartParams;
    }
  | {
      id: RequestId;
      method: "thread/realtime/appendAudio";
      params: ThreadRealtimeAppendAudioParams;
    }
  | {
      id: RequestId;
      method: "thread/realtime/appendText";
      params: ThreadRealtimeAppendTextParams;
    }
  | {
      id: RequestId;
      method: "thread/realtime/appendSpeech";
      params: ThreadRealtimeAppendSpeechParams;
    }
  | {
      id: RequestId;
      method: "thread/realtime/stop";
      params: ThreadRealtimeStopParams;
    }
  | {
      id: RequestId;
      method: "thread/timeline/list";
      params: ThreadTimelineListParams;
    }
  | {
      id: RequestId;
      method: "thread/realtime/listVoices";
      params: ThreadRealtimeListVoicesParams;
    }
  | { id: RequestId; method: "review/start"; params: ReviewStartParams }
  | { id: RequestId; method: "model/list"; params: ModelListParams }
  | { id: RequestId; method: "account/gatewayOAuth/read"; params?: null }
  | { id: RequestId; method: "account/gatewayOAuth/login"; params?: null }
  | { id: RequestId; method: "account/gatewayOAuth/cancel"; params?: null }
  | {
      id: RequestId;
      method: "modelProvider/capabilities/read";
      params: ModelProviderCapabilitiesReadParams;
    }
  | {
      id: RequestId;
      method: "experimentalFeature/list";
      params: ExperimentalFeatureListParams;
    }
  | {
      id: RequestId;
      method: "permissionProfile/list";
      params: PermissionProfileListParams;
    }
  | {
      id: RequestId;
      method: "experimentalFeature/enablement/set";
      params: ExperimentalFeatureEnablementSetParams;
    }
  | {
      id: RequestId;
      method: "remoteControl/enable";
      params?: RemoteControlEnableParams | null;
    }
  | {
      id: RequestId;
      method: "remoteControl/disable";
      params?: RemoteControlDisableParams | null;
    }
  | { id: RequestId; method: "remoteControl/status/read"; params?: null }
  | {
      id: RequestId;
      method: "remoteControl/pairing/start";
      params: RemoteControlPairingStartParams;
    }
  | {
      id: RequestId;
      method: "remoteControl/pairing/status";
      params: RemoteControlPairingStatusParams;
    }
  | {
      id: RequestId;
      method: "remoteControl/client/list";
      params: RemoteControlClientsListParams;
    }
  | {
      id: RequestId;
      method: "remoteControl/client/revoke";
      params: RemoteControlClientsRevokeParams;
    }
  | {
      id: RequestId;
      method: "collaborationMode/list";
      params: CollaborationModeListParams;
    }
  | {
      id: RequestId;
      method: "mock/experimentalMethod";
      params: MockExperimentalMethodParams;
    }
  | { id: RequestId; method: "environment/add"; params: EnvironmentAddParams }
  | { id: RequestId; method: "environment/info"; params: EnvironmentInfoParams }
  | {
      id: RequestId;
      method: "environment/status";
      params: EnvironmentStatusParams;
    }
  | {
      id: RequestId;
      method: "mcpServer/oauth/login";
      params: McpServerOauthLoginParams;
    }
  | { id: RequestId; method: "config/mcpServer/reload"; params?: null }
  | {
      id: RequestId;
      method: "mcpServerStatus/list";
      params: ListMcpServerStatusParams;
    }
  | {
      id: RequestId;
      method: "mcpServer/resource/read";
      params: McpResourceReadParams;
    }
  | {
      id: RequestId;
      method: "mcpServer/event/stream/start";
      params: McpServerEventStreamStartParams;
    }
  | {
      id: RequestId;
      method: "mcpServer/event/stream/stop";
      params: McpServerEventStreamStopParams;
    }
  | {
      id: RequestId;
      method: "mcpServer/tool/call";
      params: McpServerToolCallParams;
    }
  | {
      id: RequestId;
      method: "windowsSandbox/setupStart";
      params: WindowsSandboxSetupStartParams;
    }
  | { id: RequestId; method: "windowsSandbox/readiness"; params?: null }
  | { id: RequestId; method: "account/login/start"; params: LoginAccountParams }
  | {
      id: RequestId;
      method: "account/bedrock/discover";
      params: BedrockDiscoverParams;
    }
  | {
      id: RequestId;
      method: "account/bedrock/setup";
      params: BedrockSetupParams;
    }
  | { id: RequestId; method: "account/gcp/setup"; params: GcpSetupParams }
  | {
      id: RequestId;
      method: "account/bedrock/checkGovCloudRequirements";
      params: BedrockCheckGovCloudRequirementsParams;
    }
  | {
      id: RequestId;
      method: "account/login/cancel";
      params: CancelLoginAccountParams;
    }
  | {
      id: RequestId;
      method: "accountSession/list";
      params: AccountSessionsListParams;
    }
  | {
      id: RequestId;
      method: "accountSession/switch";
      params: AccountSessionsSwitchParams;
    }
  | { id: RequestId; method: "account/logout"; params?: null }
  | {
      id: RequestId;
      method: "account/rateLimits/read";
      params?: GetAccountRateLimitsParams | null;
    }
  | {
      id: RequestId;
      method: "account/rateLimitResetCredit/consume";
      params: ConsumeAccountRateLimitResetCreditParams;
    }
  | {
      id: RequestId;
      method: "account/usage/read";
      params?: GetAccountTokenUsageParams | null;
    }
  | { id: RequestId; method: "account/workspaceMessages/read"; params?: null }
  | {
      id: RequestId;
      method: "account/sendAddCreditsNudgeEmail";
      params: SendAddCreditsNudgeEmailParams;
    }
  | { id: RequestId; method: "feedback/upload"; params: FeedbackUploadParams }
  | { id: RequestId; method: "feedback/save"; params: FeedbackSaveParams }
  | { id: RequestId; method: "command/exec"; params: CommandExecParams }
  | {
      id: RequestId;
      method: "command/exec/write";
      params: CommandExecWriteParams;
    }
  | {
      id: RequestId;
      method: "command/exec/terminate";
      params: CommandExecTerminateParams;
    }
  | {
      id: RequestId;
      method: "command/exec/resize";
      params: CommandExecResizeParams;
    }
  | { id: RequestId; method: "process/spawn"; params: ProcessSpawnParams }
  | {
      id: RequestId;
      method: "process/writeStdin";
      params: ProcessWriteStdinParams;
    }
  | { id: RequestId; method: "process/kill"; params: ProcessKillParams }
  | {
      id: RequestId;
      method: "process/resizePty";
      params: ProcessResizePtyParams;
    }
  | { id: RequestId; method: "config/read"; params: ConfigReadParams }
  | {
      id: RequestId;
      method: "externalAgentConfig/detect";
      params: ExternalAgentConfigDetectParams;
    }
  | {
      id: RequestId;
      method: "externalAgentConfig/import";
      params: ExternalAgentConfigImportParams;
    }
  | {
      id: RequestId;
      method: "externalAgentConfig/import/recordHistory";
      params: ExternalAgentConfigImportHistoryRecordParams;
    }
  | {
      id: RequestId;
      method: "externalAgentConfig/import/readHistories";
      params?: null;
    }
  | {
      id: RequestId;
      method: "config/value/write";
      params: ConfigValueWriteParams;
    }
  | {
      id: RequestId;
      method: "config/batchWrite";
      params: ConfigBatchWriteParams;
    }
  | { id: RequestId; method: "configRequirements/read"; params?: null }
  | { id: RequestId; method: "account/read"; params: GetAccountParams }
  | { id: RequestId; method: "fuzzyFileSearch"; params: FuzzyFileSearchParams }
  | {
      id: RequestId;
      method: "fuzzyFileSearch/sessionStart";
      params: FuzzyFileSearchSessionStartParams;
    }
  | {
      id: RequestId;
      method: "fuzzyFileSearch/sessionUpdate";
      params: FuzzyFileSearchSessionUpdateParams;
    }
  | {
      id: RequestId;
      method: "fuzzyFileSearch/sessionStop";
      params: FuzzyFileSearchSessionStopParams;
    };

export type CommandExecutionApprovalDecision =
  | "accept"
  | "acceptForSession"
  | { acceptWithExecpolicyAmendment: { execpolicy_amendment: Array<string> } }
  | {
      applyNetworkPolicyAmendment: {
        network_policy_amendment: NetworkPolicyAmendment;
      };
    }
  | "decline"
  | "cancel";

export type CommandExecutionApprovalKind =
  | ("command" | "writeStdin")
  | "humanCommand";

export type CommandExecutionRequestApprovalParams = {
  additionalPermissions?: AdditionalPermissionProfile | null;
  approvalId?: string | null;
  availableDecisions?: Array<CommandExecutionApprovalDecision> | null;
  command?: string | null;
  commandActions?: Array<CommandAction> | null;
  cwd?: LegacyAppPathString | null;
  environmentId?: string | null;
  itemId: string;
  kind?: CommandExecutionApprovalKind;
  networkApprovalContext?: NetworkApprovalContext | null;
  proposedExecpolicyAmendment?: Array<string> | null;
  proposedNetworkPolicyAmendments?: Array<NetworkPolicyAmendment> | null;
  reason?: string | null;
  startedAtMs: number;
  threadId: string;
  turnId: string;
};

export type CommandExecutionRequestApprovalResponse = {
  decision: CommandExecutionApprovalDecision;
};

export type CurrentTimeReadParams = { threadId: string };

export type CurrentTimeReadResponse = { currentTimeAt: number };

export type DynamicToolCallParams = {
  arguments: unknown;
  callId: string;
  namespace?: string | null;
  threadId: string;
  tool: string;
  turnId: string;
};

export type DynamicToolCallResponse = {
  contentItems: Array<DynamicToolCallOutputContentItem>;
  success: boolean;
};

export type ExecCommandApprovalParams = {
  approvalId?: string | null;
  callId: string;
  command: Array<string>;
  conversationId: ThreadId;
  cwd: string;
  parsedCmd: Array<ParsedCommand>;
  reason?: string | null;
};

export type ExecCommandApprovalResponse = { decision: ReviewDecision };

export type FileChange =
  | { content: string; type: "add" }
  | { content: string; type: "delete" }
  | { move_path?: string | null; type: "update"; unified_diff: string };

export type FileChangeApprovalDecision =
  | "accept"
  | "acceptForSession"
  | "decline"
  | "cancel";

export type FileChangeRequestApprovalParams = {
  grantRoot?: string | null;
  itemId: string;
  reason?: string | null;
  startedAtMs: number;
  threadId: string;
  turnId: string;
};

export type FileChangeRequestApprovalResponse = {
  decision: FileChangeApprovalDecision;
};

export type FuzzyFileSearchMatchType = "file" | "directory";

export type FuzzyFileSearchParams = {
  cancellationToken?: string | null;
  query: string;
  roots: Array<string>;
};

export type FuzzyFileSearchResponse = { files: Array<FuzzyFileSearchResult> };

export type FuzzyFileSearchResult = {
  file_name: string;
  indices?: Array<number> | null;
  match_type: FuzzyFileSearchMatchType;
  path: string;
  root: string;
  score: number;
};

export type FuzzyFileSearchSessionCompletedNotification = { sessionId: string };

export type FuzzyFileSearchSessionStartParams = {
  roots: Array<string>;
  sessionId: string;
};

export type FuzzyFileSearchSessionStartResponse = Record<string, unknown>;

export type FuzzyFileSearchSessionStopParams = { sessionId: string };

export type FuzzyFileSearchSessionStopResponse = Record<string, unknown>;

export type FuzzyFileSearchSessionUpdateParams = {
  query: string;
  sessionId: string;
};

export type FuzzyFileSearchSessionUpdateResponse = Record<string, unknown>;

export type FuzzyFileSearchSessionUpdatedNotification = {
  files: Array<FuzzyFileSearchResult>;
  query: string;
  sessionId: string;
};

export type GrantedPermissionProfile = {
  fileSystem?: AdditionalFileSystemPermissions | null;
  network?: AdditionalNetworkPermissions | null;
};

export type InitializeCapabilities = {
  experimentalApi?: boolean;
  explicitGatewayOauth?: boolean;
  extensions?: Record<string, unknown> | null;
  mcpServerOpenaiFormElicitation?: boolean;
  optOutNotificationMethods?: Array<string> | null;
  requestAttestation?: boolean;
  requiredHumanApproval?: boolean;
};

export type InitializeParams = {
  capabilities?: InitializeCapabilities | null;
  clientInfo: ClientInfo;
};

export type InitializeResponse = {
  codexHome: AbsolutePathBuf;
  platformFamily: string;
  platformOs: string;
  userAgent: string;
};

export type JSONRPCError = { error: JSONRPCErrorError; id: RequestId };

export type JSONRPCErrorError = {
  code: number;
  data?: unknown;
  message: string;
};

export type JSONRPCMessage =
  | JSONRPCRequest
  | JSONRPCNotification
  | JSONRPCResponse
  | JSONRPCError;

export type JSONRPCNotification = { method: string; params?: unknown };

export type JSONRPCRequest = {
  id: RequestId;
  method: string;
  params?: unknown;
  trace?: W3cTraceContext | null;
};

export type JSONRPCResponse = { id: RequestId; result: unknown };

export type McpElicitationArrayType = "array";

export type McpElicitationBooleanSchema = {
  default?: boolean | null;
  description?: string | null;
  title?: string | null;
  type: McpElicitationBooleanType;
};

export type McpElicitationBooleanType = "boolean";

export type McpElicitationConstOption = { const: string; title: string };

export type McpElicitationEnumSchema =
  | McpElicitationSingleSelectEnumSchema
  | McpElicitationMultiSelectEnumSchema
  | McpElicitationLegacyTitledEnumSchema;

export type McpElicitationLegacyTitledEnumSchema = {
  default?: string | null;
  description?: string | null;
  enum: Array<string>;
  enumNames?: Array<string> | null;
  title?: string | null;
  type: McpElicitationStringType;
};

export type McpElicitationMultiSelectEnumSchema =
  | McpElicitationUntitledMultiSelectEnumSchema
  | McpElicitationTitledMultiSelectEnumSchema;

export type McpElicitationNumberSchema = {
  default?: number | null;
  description?: string | null;
  maximum?: number | null;
  minimum?: number | null;
  title?: string | null;
  type: McpElicitationNumberType;
};

export type McpElicitationNumberType = "number" | "integer";

export type McpElicitationObjectType = "object";

export type McpElicitationPrimitiveSchema =
  | McpElicitationEnumSchema
  | McpElicitationStringSchema
  | McpElicitationNumberSchema
  | McpElicitationBooleanSchema;

export type McpElicitationSchema = {
  $schema?: string | null;
  properties: Record<string, McpElicitationPrimitiveSchema>;
  required?: Array<string> | null;
  type: McpElicitationObjectType;
};

export type McpElicitationSingleSelectEnumSchema =
  | McpElicitationUntitledSingleSelectEnumSchema
  | McpElicitationTitledSingleSelectEnumSchema;

export type McpElicitationStringFormat = "email" | "uri" | "date" | "date-time";

export type McpElicitationStringSchema = {
  default?: string | null;
  description?: string | null;
  format?: McpElicitationStringFormat | null;
  maxLength?: number | null;
  minLength?: number | null;
  title?: string | null;
  type: McpElicitationStringType;
};

export type McpElicitationStringType = "string";

export type McpElicitationTitledEnumItems = {
  anyOf: Array<McpElicitationConstOption>;
};

export type McpElicitationTitledMultiSelectEnumSchema = {
  default?: Array<string> | null;
  description?: string | null;
  items: McpElicitationTitledEnumItems;
  maxItems?: number | null;
  minItems?: number | null;
  title?: string | null;
  type: McpElicitationArrayType;
};

export type McpElicitationTitledSingleSelectEnumSchema = {
  default?: string | null;
  description?: string | null;
  oneOf: Array<McpElicitationConstOption>;
  title?: string | null;
  type: McpElicitationStringType;
};

export type McpElicitationUntitledEnumItems = {
  enum: Array<string>;
  type: McpElicitationStringType;
};

export type McpElicitationUntitledMultiSelectEnumSchema = {
  default?: Array<string> | null;
  description?: string | null;
  items: McpElicitationUntitledEnumItems;
  maxItems?: number | null;
  minItems?: number | null;
  title?: string | null;
  type: McpElicitationArrayType;
};

export type McpElicitationUntitledSingleSelectEnumSchema = {
  default?: string | null;
  description?: string | null;
  enum: Array<string>;
  title?: string | null;
  type: McpElicitationStringType;
};

export type McpServerElicitationAction = "accept" | "decline" | "cancel";

export type McpServerElicitationRequestParams = (
  | {
      _meta?: unknown;
      challenge: string;
      description: string;
      mode: "openai/userVerification";
      title: string;
    }
  | {
      _meta?: unknown;
      message: string;
      mode: "form";
      requestedSchema: McpElicitationSchema;
    }
  | {
      _meta?: unknown;
      message: string;
      mode: "openai/form";
      requestedSchema: unknown;
    }
  | {
      _meta?: unknown;
      message: string;
      mode: "openaiForm";
      requestedSchema: unknown;
    }
  | {
      _meta?: unknown;
      elicitationId: string;
      message: string;
      mode: "url";
      url: string;
    }
) & { serverName: string; threadId: string; turnId?: string | null };

export type McpServerElicitationRequestResponse = {
  _meta?: unknown;
  action: McpServerElicitationAction;
  content?: unknown;
};

export type NetworkApprovalContext = {
  host: string;
  protocol: NetworkApprovalProtocol;
};

export type NetworkPolicyAmendment = {
  action: NetworkPolicyRuleAction;
  host: string;
};

export type NetworkPolicyRuleAction = "allow" | "deny";

export type ParsedCommand =
  | { cmd: string; name: string; path: string; type: "read" }
  | { cmd: string; path?: string | null; type: "list_files" }
  | { cmd: string; path?: string | null; query?: string | null; type: "search" }
  | { cmd: string; type: "unknown" };

export type PermissionGrantScope = "turn" | "session";

export type PermissionsRequestApprovalParams = {
  cwd: LegacyAppPathString;
  environmentId?: string | null;
  itemId: string;
  permissions: RequestPermissionProfile;
  reason?: string | null;
  startedAtMs: number;
  threadId: string;
  turnId: string;
};

export type PermissionsRequestApprovalResponse = {
  permissions: GrantedPermissionProfile;
  scope?: PermissionGrantScope;
  strictAutoReview?: boolean | null;
};

export type RequestId = string | number;

export type ReviewDecision =
  | "approved"
  | {
      approved_execpolicy_amendment: {
        proposed_execpolicy_amendment: Array<string>;
      };
    }
  | "approved_for_session"
  | "approved_mcp_policy_amendment"
  | {
      network_policy_amendment: {
        network_policy_amendment: NetworkPolicyAmendment;
      };
    }
  | { denied: { rejection: string } }
  | "timed_out"
  | "abort";

export type ServerNotification = (
  | { method: "error"; params: ErrorNotification }
  | { method: "thread/started"; params: ThreadStartedNotification }
  | { method: "thread/status/changed"; params: ThreadStatusChangedNotification }
  | {
      method: "thread/readState/changed";
      params: ThreadReadStateChangedNotification;
    }
  | { method: "thread/archived"; params: ThreadArchivedNotification }
  | { method: "thread/deleted"; params: ThreadDeletedNotification }
  | { method: "thread/unarchived"; params: ThreadUnarchivedNotification }
  | { method: "thread/closed"; params: ThreadClosedNotification }
  | { method: "thread/reverted"; params: ThreadRevertedNotification }
  | { method: "skills/changed"; params: SkillsChangedNotification }
  | { method: "thread/name/updated"; params: ThreadNameUpdatedNotification }
  | {
      method: "thread/section/changed";
      params: ThreadSectionChangedNotification;
    }
  | {
      method: "thread/attachment/updated";
      params: ThreadAttachmentUpdatedNotification;
    }
  | { method: "thread/goal/updated"; params: ThreadGoalUpdatedNotification }
  | {
      method: "thread/prediction/updated";
      params: ThreadPredictionUpdatedNotification;
    }
  | { method: "thread/goal/cleared"; params: ThreadGoalClearedNotification }
  | { method: "thread/queue/changed"; params: ThreadQueueChangedNotification }
  | { method: "project/changed"; params: ProjectChangedNotification }
  | {
      method: "thread/project/updated";
      params: ThreadProjectUpdatedNotification;
    }
  | {
      method: "thread/environment/connected";
      params: EnvironmentConnectionNotification;
    }
  | {
      method: "thread/environment/disconnected";
      params: EnvironmentConnectionNotification;
    }
  | {
      method: "thread/settings/updated";
      params: ThreadSettingsUpdatedNotification;
    }
  | {
      method: "thread/tokenUsage/updated";
      params: ThreadTokenUsageUpdatedNotification;
    }
  | { method: "turn/started"; params: TurnStartedNotification }
  | { method: "hook/started"; params: HookStartedNotification }
  | { method: "turn/completed"; params: TurnCompletedNotification }
  | { method: "hook/completed"; params: HookCompletedNotification }
  | { method: "turn/diff/updated"; params: TurnDiffUpdatedNotification }
  | { method: "turn/plan/updated"; params: TurnPlanUpdatedNotification }
  | { method: "item/started"; params: ItemStartedNotification }
  | {
      method: "item/autoApprovalReview/started";
      params: ItemGuardianApprovalReviewStartedNotification;
    }
  | {
      method: "item/autoApprovalReview/completed";
      params: ItemGuardianApprovalReviewCompletedNotification;
    }
  | {
      method: "autoApprovalReview/strictReviewRequired";
      params: StrictReviewRequiredNotification;
    }
  | { method: "item/completed"; params: ItemCompletedNotification }
  | { method: "item/agentMessage/delta"; params: AgentMessageDeltaNotification }
  | { method: "item/plan/delta"; params: PlanDeltaNotification }
  | {
      method: "command/exec/outputDelta";
      params: CommandExecOutputDeltaNotification;
    }
  | { method: "process/outputDelta"; params: ProcessOutputDeltaNotification }
  | { method: "process/exited"; params: ProcessExitedNotification }
  | {
      method: "item/commandExecution/outputDelta";
      params: CommandExecutionOutputDeltaNotification;
    }
  | {
      method: "item/commandExecution/terminalInteraction";
      params: TerminalInteractionNotification;
    }
  | {
      method: "item/fileChange/outputDelta";
      params: FileChangeOutputDeltaNotification;
    }
  | {
      method: "item/fileChange/patchUpdated";
      params: FileChangePatchUpdatedNotification;
    }
  | {
      method: "serverRequest/resolved";
      params: ServerRequestResolvedNotification;
    }
  | {
      method: "item/mcpToolCall/progress";
      params: McpToolCallProgressNotification;
    }
  | {
      method: "mcpServer/oauthLogin/completed";
      params: McpServerOauthLoginCompletedNotification;
    }
  | {
      method: "mcpServer/startupStatus/updated";
      params: McpServerStatusUpdatedNotification;
    }
  | {
      method: "mcpServer/event/stream/notification";
      params: McpServerEventStreamNotification;
    }
  | { method: "account/updated"; params: AccountUpdatedNotification }
  | {
      method: "account/gatewayOAuth/changed";
      params: GatewayOAuthChangedNotification;
    }
  | {
      method: "account/rateLimits/updated";
      params: AccountRateLimitsUpdatedNotification;
    }
  | { method: "app/list/updated"; params: AppListUpdatedNotification }
  | {
      method: "remoteControl/status/changed";
      params: RemoteControlStatusChangedNotification;
    }
  | {
      method: "externalAgentConfig/import/progress";
      params: ExternalAgentConfigImportProgressNotification;
    }
  | {
      method: "externalAgentConfig/import/completed";
      params: ExternalAgentConfigImportCompletedNotification;
    }
  | { method: "fs/changed"; params: FsChangedNotification }
  | {
      method: "item/reasoning/summaryTextDelta";
      params: ReasoningSummaryTextDeltaNotification;
    }
  | {
      method: "item/reasoning/summaryPartAdded";
      params: ReasoningSummaryPartAddedNotification;
    }
  | {
      method: "item/reasoning/textDelta";
      params: ReasoningTextDeltaNotification;
    }
  | { method: "thread/compacted"; params: ContextCompactedNotification }
  | { method: "model/rerouted"; params: ModelReroutedNotification }
  | { method: "model/verification"; params: ModelVerificationNotification }
  | {
      method: "modelProvider/authRecoveryStarted";
      params: AuthRecoveryNotification;
    }
  | {
      method: "modelProvider/authRecoveryCompleted";
      params: AuthRecoveryNotification;
    }
  | {
      method: "turn/moderationMetadata";
      params: TurnModerationMetadataNotification;
    }
  | {
      method: "model/safetyBuffering/updated";
      params: ModelSafetyBufferingUpdatedNotification;
    }
  | { method: "warning"; params: WarningNotification }
  | { method: "guardianWarning"; params: GuardianWarningNotification }
  | { method: "deprecationNotice"; params: DeprecationNoticeNotification }
  | { method: "configWarning"; params: ConfigWarningNotification }
  | {
      method: "fuzzyFileSearch/sessionUpdated";
      params: FuzzyFileSearchSessionUpdatedNotification;
    }
  | {
      method: "fuzzyFileSearch/sessionCompleted";
      params: FuzzyFileSearchSessionCompletedNotification;
    }
  | {
      method: "thread/realtime/started";
      params: ThreadRealtimeStartedNotification;
    }
  | {
      method: "thread/realtime/itemAdded";
      params: ThreadRealtimeItemAddedNotification;
    }
  | {
      method: "thread/realtime/item/started";
      params: ThreadRealtimeItemStartedNotification;
    }
  | {
      method: "thread/realtime/item/transcript/delta";
      params: ThreadRealtimeItemTranscriptDeltaNotification;
    }
  | {
      method: "thread/realtime/item/completed";
      params: ThreadRealtimeItemCompletedNotification;
    }
  | {
      method: "thread/realtime/transcript/delta";
      params: ThreadRealtimeTranscriptDeltaNotification;
    }
  | {
      method: "thread/realtime/transcript/done";
      params: ThreadRealtimeTranscriptDoneNotification;
    }
  | {
      method: "thread/realtime/outputAudio/delta";
      params: ThreadRealtimeOutputAudioDeltaNotification;
    }
  | { method: "thread/realtime/sdp"; params: ThreadRealtimeSdpNotification }
  | { method: "thread/realtime/error"; params: ThreadRealtimeErrorNotification }
  | {
      method: "thread/realtime/closed";
      params: ThreadRealtimeClosedNotification;
    }
  | {
      method: "windows/worldWritableWarning";
      params: WindowsWorldWritableWarningNotification;
    }
  | {
      method: "windowsSandbox/setupCompleted";
      params: WindowsSandboxSetupCompletedNotification;
    }
  | {
      method: "account/login/completed";
      params: AccountLoginCompletedNotification;
    }
) &
  unknown;

export type ServerRequest =
  | {
      id: RequestId;
      method: "item/commandExecution/requestApproval";
      params: CommandExecutionRequestApprovalParams;
    }
  | {
      id: RequestId;
      method: "item/fileChange/requestApproval";
      params: FileChangeRequestApprovalParams;
    }
  | {
      id: RequestId;
      method: "item/tool/requestUserInput";
      params: ToolRequestUserInputParams;
    }
  | {
      id: RequestId;
      method: "mcpServer/elicitation/request";
      params: McpServerElicitationRequestParams;
    }
  | {
      id: RequestId;
      method: "item/permissions/requestApproval";
      params: PermissionsRequestApprovalParams;
    }
  | { id: RequestId; method: "item/tool/call"; params: DynamicToolCallParams }
  | {
      id: RequestId;
      method: "account/chatgptAuthTokens/refresh";
      params: ChatgptAuthTokensRefreshParams;
    }
  | {
      id: RequestId;
      method: "attestation/generate";
      params: AttestationGenerateParams;
    }
  | { id: RequestId; method: "currentTime/read"; params: CurrentTimeReadParams }
  | {
      id: RequestId;
      method: "applyPatchApproval";
      params: ApplyPatchApprovalParams;
    }
  | {
      id: RequestId;
      method: "execCommandApproval";
      params: ExecCommandApprovalParams;
    };

export type ToolRequestUserInputAnswer = { answers: Array<string> };

export type ToolRequestUserInputOption = { description: string; label: string };

export type ToolRequestUserInputParams = {
  autoResolutionMs?: number | null;
  isBlocking: boolean;
  itemId: string;
  questions: Array<ToolRequestUserInputQuestion>;
  threadId: string;
  turnId: string;
};

export type ToolRequestUserInputQuestion = {
  header: string;
  id: string;
  isOther?: boolean;
  isSecret?: boolean;
  options?: Array<ToolRequestUserInputOption> | null;
  question: string;
};

export type ToolRequestUserInputResponse = {
  answers: Record<string, ToolRequestUserInputAnswer>;
};

export type W3cTraceContext = {
  traceparent?: string | null;
  tracestate?: string | null;
};

export type Account =
  | { type: "apiKey" }
  | {
      email: string | null;
      planType: PlanType;
      type: "chatgpt";
      workspaceName?: string | null;
    }
  | { type: "amazonBedrock"; usesCodexManagedCredentials?: boolean }
  | { type: "gcp" };

export type AccountLoginCompletedNotification = {
  error?: string | null;
  loginId?: string | null;
  onboardingEntrypoint?: DesktopOnboardingEntrypoint | null;
  success: boolean;
};

export type AccountRateLimitsUpdatedNotification = {
  rateLimits: RateLimitSnapshot;
};

export type AccountRoutingOverride = "NO_CONSTRAINT" | "us" | "us_cr";

export type AccountSession = {
  displayName?: string | null;
  email?: string | null;
  imageUrl?: string | null;
  isActive: boolean;
  lastUsedAt: number;
  selectedWorkspaceAccountId?: string | null;
  sessionId: string;
  userId?: string | null;
  workspaces: Array<AccountSessionWorkspace>;
};

export type AccountSessionWorkspace = {
  accountId: string;
  imageUrl?: string | null;
  kind?: AccountSessionWorkspaceKind | null;
  name?: string | null;
};

export type AccountSessionWorkspaceKind = "personal" | "workspace";

export type AccountSessionsListParams = { refreshWorkspaceMetadata?: boolean };

export type AccountSessionsResponse = {
  activeSessionId?: string | null;
  sessions: Array<AccountSession>;
};

export type AccountSessionsSwitchParams = {
  accountId: string;
  allowBrowserLogin?: boolean;
  sessionId: string;
};

export type AccountSessionsSwitchResponse =
  | { activeSessionId?: string | null; sessions: Array<AccountSession> }
  | { auth_url: string; login_id: string };

export type AccountTokenUsageDailyBucket = {
  startDate: string;
  tokens: number;
};

export type AccountTokenUsageSummary = {
  currentStreakDays?: number | null;
  lifetimeTokens?: number | null;
  longestRunningTurnSec?: number | null;
  longestStreakDays?: number | null;
  peakDailyTokens?: number | null;
};

export type AccountUpdatedNotification = {
  authMode?: AuthMode | null;
  planType?: PlanType | null;
};

export type ActivePermissionProfile = { extends?: string | null; id: string };

export type AddCreditsNudgeCreditType = "credits" | "usage_limit";

export type AddCreditsNudgeEmailStatus = "sent" | "cooldown_active";

export type AdditionalContextEntry = {
  kind: AdditionalContextKind;
  name?: string | null;
  value: string;
};

export type AdditionalContextKind = "untrusted" | "application";

export type AdditionalFileSystemPermissions = {
  entries?: Array<FileSystemSandboxEntry> | null;
  globScanMaxDepth?: number | null;
  read?: Array<LegacyAppPathString> | null;
  write?: Array<LegacyAppPathString> | null;
};

export type AdditionalNetworkPermissions = { enabled?: boolean | null };

export type AgentMessageDelivery = "async";

export type AgentMessageDeltaNotification = {
  delta: string;
  itemId: string;
  threadId: string;
  turnId: string;
};

export type AgentMessageInputContent =
  | { text: string; type: "input_text" }
  | { encrypted_content: string; type: "encrypted_content" };

export type AgentPath = string;

export type AllowDenyRequirement = "allow" | "deny";

export type AnalyticsConfig = { enabled?: boolean | null } & Record<
  string,
  unknown
>;

export type AppBranding = {
  category?: string | null;
  developer?: string | null;
  isDiscoverableApp: boolean;
  privacyPolicy?: string | null;
  termsOfService?: string | null;
  website?: string | null;
};

export type AppConfig = {
  approvals_reviewer?: ApprovalsReviewer | null;
  default_tools_approval_mode?: AppToolApproval | null;
  default_tools_enabled?: boolean | null;
  destructive_enabled?: boolean | null;
  enabled?: boolean;
  links?: AppLinksConfig | null;
  omit_tools_from?: Array<ToolExposureSurface> | null;
  open_world_enabled?: boolean | null;
  tools?: AppToolsConfig | null;
};

export type AppInfo = {
  appMetadata?: AppMetadata | null;
  branding?: AppBranding | null;
  description?: string | null;
  distributionChannel?: string | null;
  iconAssets?: Record<string, string> | null;
  iconDarkAssets?: Record<string, string> | null;
  id: string;
  installUrl?: string | null;
  isAccessible?: boolean;
  isEnabled?: boolean;
  labels?: Record<string, string> | null;
  logoUrl?: string | null;
  logoUrlDark?: string | null;
  name: string;
  pluginDisplayNames?: Array<string>;
};

export type AppLinkConfig = {
  approvals_reviewer?: ApprovalsReviewer | null;
  default_tools_approval_mode?: AppToolApproval | null;
};

export type AppLinksConfig = Record<string, unknown>;

export type AppListUpdatedNotification = { data: Array<AppInfo> };

export type AppMetadata = {
  categories?: Array<string> | null;
  developer?: string | null;
  firstPartyRequiresInstall?: boolean | null;
  review?: AppReview | null;
  screenshots?: Array<AppScreenshot> | null;
  seoDescription?: string | null;
  showInComposerWhenUnlinked?: boolean | null;
  subCategories?: Array<string> | null;
  version?: string | null;
  versionId?: string | null;
  versionNotes?: string | null;
};

export type AppReview = { status: string };

export type AppScreenshot = {
  fileId?: string | null;
  url?: string | null;
  userPrompt: string;
};

export type AppSummary = {
  category?: string | null;
  description?: string | null;
  id: string;
  installUrl?: string | null;
  name: string;
};

export type AppTemplateSummary = {
  canonicalConnectorId?: string | null;
  category?: string | null;
  description?: string | null;
  logoUrl?: string | null;
  logoUrlDark?: string | null;
  materializedAppIds: Array<string>;
  name: string;
  reason?: AppTemplateUnavailableReason | null;
  templateId: string;
};

export type AppTemplateUnavailableReason =
  | "NOT_CONFIGURED_FOR_WORKSPACE"
  | "NO_ACTIVE_WORKSPACE";

export type AppToolApproval = "auto" | "prompt" | "writes" | "approve";

export type AppToolConfig = {
  approval_mode?: AppToolApproval | null;
  enabled?: boolean | null;
};

export type AppToolSummary = {
  description: string;
  disabledReason?: string | null;
  isEnabled?: boolean;
  isReadOnly?: boolean;
  name: string;
  title?: string | null;
};

export type AppToolsConfig = Record<string, unknown>;

export type ApplicationNetworkRequirements = {
  domains: Record<string, NetworkDomainPermission>;
  enabled: boolean;
};

export type ApplicationRequirements = {
  network?: ApplicationNetworkRequirements | null;
  storage?: ApplicationStorageRequirements | null;
};

export type ApplicationStorageRequirements = {
  defaultProjectParent?: LegacyAppPathString | null;
  projectlessWorkspaceRoot?: LegacyAppPathString | null;
};

export type ApprovalsReviewer = "user" | "auto_review" | "guardian_subagent";

export type AppsConfig = { _default?: AppsDefaultConfig | null };

export type AppsDefaultConfig = {
  approvals_reviewer?: ApprovalsReviewer | null;
  default_tools_approval_mode?: AppToolApproval | null;
  destructive_enabled?: boolean;
  enabled?: boolean;
  open_world_enabled?: boolean;
};

export type AppsInstalledParams = {
  forceRefresh?: boolean;
  threadId?: string | null;
};

export type AppsInstalledResponse = { apps: Array<InstalledApp> };

export type AppsListParams = {
  cursor?: string | null;
  forceRefetch?: boolean;
  limit?: number | null;
  threadId?: string | null;
};

export type AppsListResponse = {
  data: Array<AppInfo>;
  nextCursor?: string | null;
};

export type AppsReadParams = {
  appIds: Array<string>;
  includeTools?: boolean;
  threadId?: string | null;
};

export type AppsReadResponse = {
  apps: Array<ConnectorMetadata>;
  missingAppIds: Array<string>;
};

export type AskForApproval =
  | ("untrusted" | "on-request" | "never")
  | {
      granular: {
        mcp_elicitations: boolean;
        request_permissions?: boolean;
        rules: boolean;
        sandbox_approval: boolean;
        skill_approval?: boolean;
      };
    };

export type AsyncUserInputQuestion = {
  options?: Array<string> | null;
  title: string;
};

export type AuthMode =
  | "apikey"
  | "chatgpt"
  | "chatgptAuthTokens"
  | "headers"
  | "agentIdentity"
  | "personalAccessToken"
  | "bedrockApiKey"
  | "bedrockAccessKeys";

export type AuthRecoveryNotification = {
  message: string;
  provider: string;
  threadId: string;
  turnId: string;
};

export type AutoCompactTokenLimitScope = "total" | "body_after_prefix";

export type AutoReviewDecisionSource = "agent";

export type AutoReviewRequirements = {
  ignoreRules?: Array<string> | null;
  requiredOnModels?: Array<string> | null;
};

export type AwsCredentialType = "accessKeys" | "bedrockApiKey";

export type BedrockAwsProfile = { name: string; region?: string | null };

export type BedrockCheckGovCloudRequirementsParams = Record<string, unknown>;

export type BedrockCheckGovCloudRequirementsResponse = {
  isGovCloud: boolean;
  shouldWarn: boolean;
};

export type BedrockDiscoverParams = Record<string, unknown>;

export type BedrockDiscoverResponse = {
  environmentCredentials: Array<BedrockEnvironmentCredential>;
  profiles: Array<BedrockAwsProfile>;
};

export type BedrockEnvironmentCredential = {
  region?: string | null;
  type: AwsCredentialType;
};

export type BedrockSetupParams =
  | { profile: string; region: string; type: "profile" }
  | { region: string; type: "environment" };

export type BedrockSetupResponse = Record<string, unknown>;

export type BrowserUseAccessApprovalLifetime = "turn" | "thread";

export type BrowserUseConfig = {
  allow_history_access?: boolean | null;
  default_origin_policy?: BrowserUseOriginPolicyConfig | null;
  origins?: Record<string, BrowserUseOriginPolicyConfig> | null;
};

export type BrowserUseExtensionRequirements = {
  requestHeaders?: Array<RequestHeader> | null;
};

export type BrowserUseOriginPolicy = {
  access?: AllowDenyRequirement | null;
  accessApprovalLifetime?: BrowserUseAccessApprovalLifetime | null;
  autoReview?: AllowDenyRequirement | null;
  downloads?: AllowDenyRequirement | null;
  fullCdpAccess?: AllowDenyRequirement | null;
  persistentApproval?: boolean | null;
  uploads?: AllowDenyRequirement | null;
};

export type BrowserUseOriginPolicyConfig = {
  access?: AllowDenyRequirement | null;
  downloads?: AllowDenyRequirement | null;
  full_cdp_access?: AllowDenyRequirement | null;
  uploads?: AllowDenyRequirement | null;
};

export type BrowserUseRequirements = {
  allowGlobalPersistentApproval?: boolean | null;
  allowHistoryAccess?: boolean | null;
  allowWebmcp?: boolean | null;
  defaultOriginPolicy?: BrowserUseOriginPolicy | null;
  disableAutoReview?: boolean | null;
  extension?: BrowserUseExtensionRequirements | null;
  origins?: Record<string, BrowserUseOriginPolicy> | null;
};

export type ByteRange = { end: number; start: number };

export type CancelLoginAccountParams = { loginId: string };

export type CancelLoginAccountResponse = { status: CancelLoginAccountStatus };

export type CancelLoginAccountStatus = "canceled" | "notFound";

export type CapabilityRootLocation = {
  environmentId: string;
  path: string;
  type: "environment";
};

export type CliAuthCredentialsStoreMode =
  | "file"
  | "keyring"
  | "auto"
  | "ephemeral";

export type CodexErrorInfo =
  | (
      | "contextWindowExceeded"
      | "sessionBudgetExceeded"
      | "usageLimitExceeded"
      | "rateLimitExceeded"
      | "flexUnavailable"
      | "serverOverloaded"
      | "modelNotFound"
      | "policyCapacityUnavailable"
      | "cyberPolicy"
      | "misalignmentPolicyViolation"
      | "tooManyDenials"
      | "internalServerError"
      | "unauthorized"
      | "badRequest"
      | "threadRollbackFailed"
      | "sandboxError"
      | "other"
    )
  | { httpConnectionFailed: { httpStatusCode?: number | null } }
  | { responseStreamConnectionFailed: { httpStatusCode?: number | null } }
  | { responseStreamDisconnected: { httpStatusCode?: number | null } }
  | { responseTooManyFailedAttempts: { httpStatusCode?: number | null } }
  | { activeTurnNotSteerable: { turnKind: NonSteerableTurnKind } }
  | (string | Record<string, unknown>);

export type CodexResponseHandoffMode = "thinking" | "commentary" | "bemTags";

export type CollabAgentState = {
  message?: string | null;
  status: CollabAgentStatus;
};

export type CollabAgentStatus =
  | "pendingInit"
  | "running"
  | "interrupted"
  | "completed"
  | "errored"
  | "shutdown"
  | "notFound";

export type CollabAgentTool =
  | "spawnAgent"
  | "sendInput"
  | "resumeAgent"
  | "wait"
  | "closeAgent"
  | "sendMessage"
  | "followupTask"
  | "interruptAgent"
  | "listAgents";

export type CollabAgentToolCallStatus =
  | "inProgress"
  | "completed"
  | "failed"
  | "interrupted";

export type CollaborationMode = { mode: ModeKind; settings: Settings };

export type CollaborationModeListParams = Record<string, unknown>;

export type CollaborationModeListResponse = {
  data: Array<CollaborationModeMask>;
};

export type CollaborationModeMask = {
  mode?: ModeKind | null;
  model?: string | null;
  name: string;
  reasoning_effort?: (ReasoningEffort | null) | null;
};

export type CommandAction =
  | { command: string; name: string; path: LegacyAppPathString; type: "read" }
  | { command: string; path?: string | null; type: "listFiles" }
  | {
      command: string;
      path?: string | null;
      query?: string | null;
      type: "search";
    }
  | { command: string; type: "unknown" };

export type CommandExecOutputDeltaNotification = {
  capReached: boolean;
  deltaBase64: string;
  processId: string;
  stream: CommandExecOutputStream;
};

export type CommandExecOutputStream = "stdout" | "stderr";

export type CommandExecParams = {
  command: Array<string>;
  cwd?: string | null;
  disableOutputCap?: boolean;
  disableTimeout?: boolean;
  env?: Record<string, string | null> | null;
  outputBytesCap?: number | null;
  permissionProfile?: string | null;
  processId?: string | null;
  sandboxPolicy?: SandboxPolicy | null;
  size?: CommandExecTerminalSize | null;
  streamStdin?: boolean;
  streamStdoutStderr?: boolean;
  timeoutMs?: number | null;
  tty?: boolean;
};

export type CommandExecResizeParams = {
  processId: string;
  size: CommandExecTerminalSize;
};

export type CommandExecResizeResponse = Record<string, unknown>;

export type CommandExecResponse = {
  exitCode: number;
  stderr: string;
  stdout: string;
};

export type CommandExecTerminalSize = { cols: number; rows: number };

export type CommandExecTerminateParams = { processId: string };

export type CommandExecTerminateResponse = Record<string, unknown>;

export type CommandExecWriteParams = {
  closeStdin?: boolean;
  deltaBase64?: string | null;
  processId: string;
};

export type CommandExecWriteResponse = Record<string, unknown>;

export type CommandExecutionOutputDeltaNotification = {
  delta: string;
  itemId: string;
  threadId: string;
  turnId: string;
};

export type CommandExecutionSource =
  | "agent"
  | "userShell"
  | "unifiedExecStartup"
  | "unifiedExecInteraction";

export type CommandExecutionStatus =
  | "inProgress"
  | "completed"
  | "failed"
  | "declined";

export type CommandMigration = { name: string };

export type ComputerUseConfig = {
  default_app_access?: AllowDenyRequirement | null;
  macos?: ComputerUseMacosConfig | null;
  windows?: ComputerUseWindowsConfig | null;
};

export type ComputerUseMacosConfig = {
  bundle_ids?: Record<string, AllowDenyRequirement> | null;
};

export type ComputerUseMacosRequirements = {
  bundleIds?: Record<string, AllowDenyRequirement> | null;
};

export type ComputerUseRequirements = {
  allowLockedComputerUse?: boolean | null;
  allowPersistentApproval?: boolean | null;
  defaultAppAccess?: AllowDenyRequirement | null;
  macos?: ComputerUseMacosRequirements | null;
  windows?: ComputerUseWindowsRequirements | null;
};

export type ComputerUseWindowsConfig = {
  aumids?: Record<string, AllowDenyRequirement> | null;
  exes?: Array<ComputerUseWindowsExeConfig> | null;
};

export type ComputerUseWindowsExeConfig = {
  access: AllowDenyRequirement;
  binary_name?: string | null;
  product_name: string;
  publisher_name: string;
};

export type ComputerUseWindowsExeRequirement = {
  access: AllowDenyRequirement;
  binaryName?: string | null;
  productName: string;
  publisherName: string;
};

export type ComputerUseWindowsRequirements = {
  aumids?: Record<string, AllowDenyRequirement> | null;
  exes?: Array<ComputerUseWindowsExeRequirement> | null;
};

export type Config = {
  analytics?: AnalyticsConfig | null;
  approval_policy?: AskForApproval | null;
  approvals_reviewer?: ApprovalsReviewer | null;
  apps?: AppsConfig | null;
  browser_use?: BrowserUseConfig | null;
  compact_prompt?: string | null;
  computer_use?: ComputerUseConfig | null;
  desktop?: Record<string, unknown> | null;
  developer_instructions?: string | null;
  forced_chatgpt_workspace_id?: ForcedChatgptWorkspaceIds | null;
  forced_login_method?: ForcedLoginMethod | null;
  instructions?: string | null;
  model?: string | null;
  model_auto_compact_token_limit?: number | null;
  model_auto_compact_token_limit_scope?: AutoCompactTokenLimitScope | null;
  model_context_window?: number | null;
  model_provider?: string | null;
  model_reasoning_effort?: ReasoningEffort | null;
  model_reasoning_summary?: ReasoningSummary | null;
  model_verbosity?: Verbosity | null;
  review_model?: string | null;
  sandbox_mode?: SandboxMode | null;
  sandbox_workspace_write?: SandboxWorkspaceWrite | null;
  service_tier?: string | null;
  tools?: ToolsV2 | null;
  web_search?: WebSearchMode | null;
} & Record<string, unknown>;

export type ConfigBatchWriteParams = {
  edits: Array<ConfigEdit>;
  expectedVersion?: string | null;
  filePath?: string | null;
  reloadUserConfig?: boolean;
};

export type ConfigEdit = {
  keyPath: string;
  mergeStrategy: MergeStrategy;
  value: unknown;
};

export type ConfigLayer = {
  config: unknown;
  disabledReason?: string | null;
  name: ConfigLayerSource;
  version: string;
};

export type ConfigLayerMetadata = { name: ConfigLayerSource; version: string };

export type ConfigLayerSource =
  | { file: AbsolutePathBuf; type: "packagedDefaults" }
  | { domain: string; key: string; type: "mdm" }
  | { file: AbsolutePathBuf; type: "system" }
  | { id: string; name: string; type: "enterpriseManaged" }
  | { file: AbsolutePathBuf; profile?: string | null; type: "user" }
  | { dotCodexFolder: AbsolutePathBuf; type: "project" }
  | { type: "sessionFlags" }
  | { file: AbsolutePathBuf; type: "legacyManagedConfigTomlFromFile" }
  | { type: "legacyManagedConfigTomlFromMdm" };

export type ConfigReadParams = { cwd?: string | null; includeLayers?: boolean };

export type ConfigReadResponse = {
  config: Config;
  layers?: Array<ConfigLayer> | null;
  origins: Record<string, ConfigLayerMetadata>;
};

export type ConfigRequirements = {
  additionalDeveloperInstructions?: string | null;
  allowAppshots?: boolean | null;
  allowBrowserAndComputerUse?: boolean | null;
  allowLoginShell?: boolean | null;
  allowManagedHooksOnly?: boolean | null;
  allowRemoteControl?: boolean | null;
  allowedApprovalPolicies?: Array<AskForApproval> | null;
  allowedApprovalsReviewers?: Array<ApprovalsReviewer> | null;
  allowedLoginMethods?: Array<ForcedLoginMethod> | null;
  allowedPermissionProfiles?: Record<string, boolean> | null;
  allowedSandboxModes?: Array<SandboxMode> | null;
  allowedWebSearchModes?: Array<WebSearchMode> | null;
  allowedWindowsSandboxImplementations?: Array<WindowsSandboxImplementation> | null;
  application?: ApplicationRequirements | null;
  autoReview?: AutoReviewRequirements | null;
  browserUse?: BrowserUseRequirements | null;
  chatgptBaseUrl?: string | null;
  checkForUpdateOnStartup?: boolean | null;
  cliAuthCredentialsStore?: CliAuthCredentialsStoreMode | null;
  computerUse?: ComputerUseRequirements | null;
  defaultPermissions?: string | null;
  enforceResidency?: ResidencyRequirement | null;
  featureRequirements?: Record<string, boolean> | null;
  feedback?: FeedbackRequirements | null;
  hooks?: ManagedHooksRequirements | null;
  inAppBrowser?: InAppBrowserRequirements | null;
  logDir?: string | null;
  modelCatalogJson?: string | null;
  modelProvider?: string | null;
  modelProviders?: Record<string, unknown> | null;
  models?: ModelsRequirements | null;
  network?: NetworkRequirements | null;
  otel?: OtelRequirements | null;
  sqliteHome?: string | null;
};

export type ConfigRequirementsReadResponse = {
  requirements?: ConfigRequirements | null;
  supportsIndependentSpeedModes?: boolean | null;
};

export type ConfigValueWriteParams = {
  expectedVersion?: string | null;
  filePath?: string | null;
  keyPath: string;
  mergeStrategy: MergeStrategy;
  value: unknown;
};

export type ConfigWarningNotification = {
  details?: string | null;
  path?: string | null;
  range?: TextRange | null;
  summary: string;
};

export type ConfigWriteResponse = {
  filePath: AbsolutePathBuf;
  overriddenMetadata?: OverriddenMetadata | null;
  status: WriteStatus;
  version: string;
};

export type ConfigurationReasoning = { effort: ReasoningEffort };

export type ConfiguredHookHandler =
  | {
      additionalContextLimit?: number | null;
      async: boolean;
      command: string;
      commandWindows?: string | null;
      statusMessage?: string | null;
      timeoutSec?: number | null;
      type: "command";
    }
  | {
      input: Record<string, unknown>;
      server: string;
      statusMessage?: string | null;
      timeoutSec?: number | null;
      tool: string;
      type: "mcp_tool";
    }
  | { type: "prompt" }
  | { type: "agent" };

export type ConfiguredHookMatcherGroup = {
  hooks: Array<ConfiguredHookHandler>;
  matcher?: string | null;
};

export type ConnectorMetadata = {
  description?: string | null;
  distributionChannel?: string | null;
  iconUrl?: string | null;
  iconUrlDark?: string | null;
  id: string;
  installUrl?: string | null;
  name: string;
  pluginDisplayNames?: Array<string>;
  toolSummaries?: Array<AppToolSummary> | null;
};

export type ConsumeAccountRateLimitResetCreditOutcome =
  | "reset"
  | "nothingToReset"
  | "noCredit"
  | "alreadyRedeemed";

export type ConsumeAccountRateLimitResetCreditParams = {
  creditId?: string | null;
  idempotencyKey: string;
};

export type ConsumeAccountRateLimitResetCreditResponse = {
  outcome: ConsumeAccountRateLimitResetCreditOutcome;
};

export type ContentItem =
  | { text: string; type: "input_text" }
  | (({ image_url: string } | { file_id: string }) & {
      detail?: ImageDetail | null;
      type: "input_image";
    })
  | (({ audio_url: string } | { file_id: string }) & { type: "input_audio" })
  | {
      annotations?: Array<unknown> | null;
      logprobs?: Array<unknown> | null;
      text: string;
      type: "output_text";
    };

export type ContextCompactedNotification = { threadId: string; turnId: string };

export type ConversationTextRole = "user" | "developer" | "assistant";

export type CreditsSnapshot = {
  balance?: string | null;
  hasCredits: boolean;
  unlimited: boolean;
};

export type CyberAccessProgram = "standard" | "daybreakBlue" | "daybreakRed";

export type DeprecationNoticeNotification = {
  details?: string | null;
  summary: string;
};

export type DesktopOnboardingEntrypoint = "life_sciences";

export type DynamicToolCallOutputContentItem =
  | { text: string; type: "inputText" }
  | { imageUrl: string; type: "inputImage" }
  | { audioUrl: string; type: "inputAudio" };

export type DynamicToolCallStatus = "inProgress" | "completed" | "failed";

export type DynamicToolNamespaceTool = {
  deferLoading?: boolean;
  description: string;
  inputSchema: unknown;
  name: string;
  type: "function";
};

export type DynamicToolSpec =
  | {
      deferLoading?: boolean;
      description: string;
      inputSchema: unknown;
      name: string;
      type: "function";
    }
  | {
      description: string;
      name: string;
      tools: Array<DynamicToolNamespaceTool>;
      type: "namespace";
    };

export type EnvironmentAddParams = {
  authBearerToken?: string | null;
  connectTimeoutMs?: number | null;
  environmentId: string;
  execServerUrl: string;
  skills?: EnvironmentSkillsParams | null;
  websocketRequestId?: string | null;
};

export type EnvironmentAddResponse = Record<string, unknown>;

export type EnvironmentConnectionNotification = {
  environmentId: string;
  threadId: string;
};

export type EnvironmentInfoParams = { environmentId: string };

export type EnvironmentInfoResponse = {
  cwd?: PathUri | null;
  shell: EnvironmentShellInfo;
};

export type EnvironmentShellInfo = { name: string; path: string };

export type EnvironmentSkillsParams = { required?: Array<string> | null };

export type EnvironmentStatusKind =
  | "ready"
  | "pending"
  | "disconnected"
  | "unknown";

export type EnvironmentStatusParams = { environmentId: string };

export type EnvironmentStatusResponse = {
  error?: string | null;
  status: EnvironmentStatusKind;
};

export type ErrorNotification = {
  error: TurnError;
  threadId: string;
  turnId: string;
  willRetry: boolean;
};

export type ExperimentalFeature = {
  announcement?: string | null;
  defaultEnabled: boolean;
  description?: string | null;
  displayName?: string | null;
  enabled: boolean;
  name: string;
  stage: ExperimentalFeatureStage;
};

export type ExperimentalFeatureEnablementSetParams = {
  enablement: Record<string, boolean>;
};

export type ExperimentalFeatureEnablementSetResponse = {
  enablement: Record<string, boolean>;
};

export type ExperimentalFeatureListParams = {
  cursor?: string | null;
  limit?: number | null;
  threadId?: string | null;
};

export type ExperimentalFeatureListResponse = {
  data: Array<ExperimentalFeature>;
  nextCursor?: string | null;
};

export type ExperimentalFeatureStage =
  | "beta"
  | "underDevelopment"
  | "stable"
  | "deprecated"
  | "removed";

export type ExternalAgentConfigDetectParams = {
  cwds?: Array<string> | null;
  includeHome?: boolean;
  maxSessionAgeDays?: number | null;
  maxSessions?: number | null;
  migrationSource?: string | null;
  source?: string | null;
};

export type ExternalAgentConfigDetectResponse = {
  connectors?: Array<ExternalAgentDetectedConnectorCandidate>;
  items: Array<ExternalAgentConfigMigrationItem>;
};

export type ExternalAgentConfigImportCompletedNotification = {
  importId: string;
  itemTypeResults: Array<ExternalAgentConfigImportTypeResult>;
};

export type ExternalAgentConfigImportHistoriesReadResponse = {
  connectors: Array<ExternalAgentImportedConnectorCandidate>;
  data: Array<ExternalAgentConfigImportHistory>;
};

export type ExternalAgentConfigImportHistory = {
  completedAtMs: number;
  failures: Array<ExternalAgentConfigImportItemTypeFailure>;
  importId: string;
  providerId?: string | null;
  successes: Array<ExternalAgentConfigImportItemTypeSuccess>;
};

export type ExternalAgentConfigImportHistoryRecordParams = {
  itemTypeResults: Array<ExternalAgentConfigImportHistoryRecordTypeResultParams>;
  providerId: string;
};

export type ExternalAgentConfigImportHistoryRecordResponse = {
  importId: string;
};

export type ExternalAgentConfigImportHistoryRecordSuccessParams = {
  cwd?: string | null;
  itemType: ExternalAgentConfigMigrationItemType;
  source?: string | null;
  target?: string | null;
  title?: string | null;
};

export type ExternalAgentConfigImportHistoryRecordTypeResultParams = {
  failures: Array<ExternalAgentConfigImportItemTypeFailure>;
  itemType: ExternalAgentConfigMigrationItemType;
  successes: Array<ExternalAgentConfigImportHistoryRecordSuccessParams>;
};

export type ExternalAgentConfigImportItemTypeFailure = {
  cwd?: string | null;
  errorType?: string | null;
  failureStage: string;
  itemType: ExternalAgentConfigMigrationItemType;
  message: string;
  source?: string | null;
  subErrorType?: string | null;
};

export type ExternalAgentConfigImportItemTypeSuccess = {
  cwd?: string | null;
  itemType: ExternalAgentConfigMigrationItemType;
  source?: string | null;
  target?: string | null;
  title?: string | null;
};

export type ExternalAgentConfigImportParams = {
  migrationItems: Array<ExternalAgentConfigMigrationItem>;
  migrationSource?: string | null;
  providerId?: string | null;
  source?: string | null;
};

export type ExternalAgentConfigImportProgressNotification = {
  importId: string;
  itemTypeResults: Array<ExternalAgentConfigImportTypeResult>;
};

export type ExternalAgentConfigImportResponse = { importId: string };

export type ExternalAgentConfigImportTypeResult = {
  failures: Array<ExternalAgentConfigImportItemTypeFailure>;
  itemType: ExternalAgentConfigMigrationItemType;
  successes: Array<ExternalAgentConfigImportItemTypeSuccess>;
};

export type ExternalAgentConfigMigrationItem = {
  cwd?: string | null;
  description: string;
  details?: MigrationDetails | null;
  itemType: ExternalAgentConfigMigrationItemType;
};

export type ExternalAgentConfigMigrationItemType =
  | "AGENTS_MD"
  | "CONFIG"
  | "SKILLS"
  | "PLUGINS"
  | "MCP_SERVER_CONFIG"
  | "SUBAGENTS"
  | "HOOKS"
  | "COMMANDS"
  | "MEMORY"
  | "SESSIONS";

export type ExternalAgentDetectedConnectorCandidate = {
  name: string;
  sessionCount: number;
  source: ExternalAgentDetectedConnectorSource;
};

export type ExternalAgentDetectedConnectorSource =
  | "remoteMcpServersConfig"
  | "sessionToolUse";

export type ExternalAgentImportedConnectorCandidate = {
  name: string;
  sessionCount: number;
  source: ExternalAgentImportedConnectorSource;
};

export type ExternalAgentImportedConnectorSource = "remoteMcpServersConfig";

export type FeedbackRequirements = { enabled?: boolean | null };

export type FeedbackSaveParams = {
  classification: string;
  extraLogFiles?: Array<string> | null;
  includeLogs?: boolean;
  reason?: string | null;
  savePath: LegacyAppPathString;
  tags?: Record<string, string> | null;
  threadId?: string | null;
};

export type FeedbackSaveResponse = {
  promptHash?: string | null;
  savedPath: LegacyAppPathString;
  threadId: string;
};

export type FeedbackUploadParams = {
  classification: string;
  extraLogFiles?: Array<string> | null;
  includeLogs?: boolean;
  reason?: string | null;
  tags?: Record<string, string> | null;
  threadId?: string | null;
};

export type FeedbackUploadResponse = {
  promptHash?: string | null;
  threadId: string;
};

export type FileChangeOutputDeltaNotification = {
  delta: string;
  itemId: string;
  threadId: string;
  turnId: string;
};

export type FileChangePatchUpdatedNotification = {
  changes: Array<FileUpdateChange>;
  itemId: string;
  threadId: string;
  turnId: string;
};

export type FileSystemAccessMode = "read" | "write" | "deny";

export type FileSystemPath =
  | { path: LegacyAppPathString; type: "path" }
  | { pattern: string; type: "glob_pattern" }
  | { type: "special"; value: FileSystemSpecialPath };

export type FileSystemSandboxEntry = {
  access: FileSystemAccessMode;
  path: FileSystemPath;
};

export type FileSystemSpecialPath =
  | { kind: "root" }
  | { kind: "minimal" }
  | { kind: "project_roots"; subpath?: LegacyAppPathString | null }
  | { kind: "tmpdir" }
  | { kind: "slash_tmp" }
  | { kind: "unknown"; path: string; subpath?: LegacyAppPathString | null };

export type FileUpdateChange = {
  diff: string;
  kind: PatchChangeKind;
  path: string;
};

export type ForcedChatgptWorkspaceIds = string | Array<string>;

export type ForcedLoginMethod = "chatgpt" | "api";

export type FsChangedNotification = {
  changedPaths: Array<AbsolutePathBuf>;
  watchId: string;
};

export type FsCopyParams = {
  destinationPath: AbsolutePathBuf;
  recursive?: boolean;
  sourcePath: AbsolutePathBuf;
};

export type FsCopyResponse = Record<string, unknown>;

export type FsCreateDirectoryParams = {
  path: AbsolutePathBuf;
  recursive?: boolean | null;
};

export type FsCreateDirectoryResponse = Record<string, unknown>;

export type FsGetMetadataParams = { path: AbsolutePathBuf };

export type FsGetMetadataResponse = {
  createdAtMs: number;
  isDirectory: boolean;
  isFile: boolean;
  isSymlink: boolean;
  modifiedAtMs: number;
};

export type FsReadDirectoryEntry = {
  fileName: string;
  isDirectory: boolean;
  isFile: boolean;
};

export type FsReadDirectoryParams = { path: AbsolutePathBuf };

export type FsReadDirectoryResponse = { entries: Array<FsReadDirectoryEntry> };

export type FsReadFileParams = { path: AbsolutePathBuf };

export type FsReadFileResponse = { dataBase64: string };

export type FsRemoveParams = {
  force?: boolean | null;
  path: AbsolutePathBuf;
  recursive?: boolean | null;
};

export type FsRemoveResponse = Record<string, unknown>;

export type FsUnwatchParams = { watchId: string };

export type FsUnwatchResponse = Record<string, unknown>;

export type FsWatchParams = { path: AbsolutePathBuf; watchId: string };

export type FsWatchResponse = { path: AbsolutePathBuf };

export type FsWriteFileParams = { dataBase64: string; path: AbsolutePathBuf };

export type FsWriteFileResponse = Record<string, unknown>;

export type FunctionCallOutputBody =
  | string
  | Array<FunctionCallOutputContentItem>;

export type FunctionCallOutputContentItem =
  | { text: string; type: "input_text" }
  | (({ image_url: string } | { file_id: string }) & {
      detail?: ImageDetail | null;
      type: "input_image";
    })
  | { audio_url: string; type: "input_audio" }
  | { encrypted_content: string; type: "encrypted_content" };

export type GatewayOAuthCancelResponse = Record<string, unknown>;

export type GatewayOAuthChangedNotification = {
  authUrl?: string | null;
  error?: string | null;
  providerId: string;
  status: GatewayOAuthStatus;
};

export type GatewayOAuthLoginResponse = Record<string, unknown>;

export type GatewayOAuthReadResponse = {
  error?: string | null;
  providerId: string;
  providerName: string;
  required: boolean;
  status?: GatewayOAuthStatus | null;
};

export type GatewayOAuthStatus =
  | "notReady"
  | "started"
  | "succeeded"
  | "failed";

export type GcpSetupParams = {
  credentialFile?: string | null;
  location?: string | null;
  projectId: string;
};

export type GcpSetupResponse = Record<string, unknown>;

export type GetAccountParams = { refreshToken?: boolean };

export type GetAccountRateLimitsParams = {
  excludeResetCreditDetails?: boolean;
  supportsLunaReserve?: boolean;
};

export type GetAccountRateLimitsResponse = {
  accountId?: string | null;
  ordinaryUsageAllowed?: boolean | null;
  rateLimitResetCredits?: RateLimitResetCreditsSummary | null;
  rateLimitUpsell?: unknown;
  rateLimits: RateLimitSnapshot;
  rateLimitsByLimitId?: Record<string, RateLimitSnapshot> | null;
};

export type GetAccountResponse = {
  account?: Account | null;
  requiresOpenaiAuth: boolean;
  workspaceRouting?: WorkspaceRouting | null;
};

export type GetAccountTokenUsageParams = { threadId?: string | null };

export type GetAccountTokenUsageResponse = {
  dailyUsageBuckets?: Array<AccountTokenUsageDailyBucket> | null;
  summary: AccountTokenUsageSummary;
  threadUsage?: ThreadUsage | null;
};

export type GetWorkspaceMessagesResponse = {
  featureEnabled: boolean;
  messages: Array<WorkspaceMessage>;
};

export type GitInfo = {
  branch?: string | null;
  originUrl?: string | null;
  sha?: string | null;
};

export type GuardianApprovalReview = {
  rationale?: string | null;
  riskLevel?: GuardianRiskLevel | null;
  status: GuardianApprovalReviewStatus;
  userAuthorization?: GuardianUserAuthorization | null;
};

export type GuardianApprovalReviewAction =
  | {
      command: string;
      cwd: LegacyAppPathString;
      source: GuardianCommandSource;
      type: "command";
    }
  | {
      argv: Array<string>;
      cwd: AbsolutePathBuf;
      program: string;
      source: GuardianCommandSource;
      type: "execve";
    }
  | {
      approvalId: string;
      cwd: LegacyAppPathString;
      processId: string;
      stdin: string;
      type: "writeStdin";
    }
  | {
      cwd: LegacyAppPathString;
      files: Array<LegacyAppPathString>;
      type: "applyPatch";
    }
  | {
      host: string;
      port: number;
      protocol: NetworkApprovalProtocol;
      target: string;
      type: "networkAccess";
    }
  | {
      connectorId?: string | null;
      connectorName?: string | null;
      server: string;
      toolName: string;
      toolTitle?: string | null;
      type: "mcpToolCall";
    }
  | {
      permissions: RequestPermissionProfile;
      reason?: string | null;
      type: "requestPermissions";
    };

export type GuardianApprovalReviewStatus =
  | "inProgress"
  | "approved"
  | "denied"
  | "timedOut"
  | "aborted";

export type GuardianAssessmentOutcome = "allow" | "deny";

export type GuardianCommandSource = "shell" | "unifiedExec";

export type GuardianRiskLevel = "low" | "medium" | "high" | "critical";

export type GuardianUserAuthorization = "unknown" | "low" | "medium" | "high";

export type GuardianWarningNotification = { message: string; threadId: string };

export type HookCompletedNotification = {
  run: HookRunSummary;
  threadId: string;
  turnId?: string | null;
};

export type HookErrorInfo = { message: string; path: string };

export type HookEventName =
  | "preModelRequest"
  | "postModelRequest"
  | "preToolUse"
  | "permissionRequest"
  | "postToolUse"
  | "preCompact"
  | "postCompact"
  | "sessionStart"
  | "sessionEnd"
  | "userPromptSubmit"
  | "subagentStart"
  | "subagentStop"
  | "stop"
  | "interrupt";

export type HookExecutionMode = "sync" | "async";

export type HookHandlerType = "command" | "mcpTool" | "prompt" | "agent";

export type HookMetadata = (
  | { async?: boolean; command: string; handlerType: "command" }
  | { handlerType: "mcpTool"; server: string; tool: string }
  | { handlerType: "prompt" }
  | { handlerType: "agent" }
) & {
  additionalContextLimit?: number | null;
  currentHash: string;
  displayOrder: number;
  enabled: boolean;
  environmentId?: string | null;
  eventName: HookEventName;
  isManaged: boolean;
  key: string;
  matcher?: string | null;
  pluginId?: string | null;
  source: HookSource;
  sourcePath: AbsolutePathBuf;
  statusMessage?: string | null;
  timeoutSec: number;
  trustStatus: HookTrustStatus;
};

export type HookMigration = { name: string };

export type HookOutputEntry = { kind: HookOutputEntryKind; text: string };

export type HookOutputEntryKind =
  | "warning"
  | "stop"
  | "feedback"
  | "context"
  | "error";

export type HookPromptFragment = { hookRunId: string; text: string };

export type HookRunStatus =
  | "running"
  | "completed"
  | "failed"
  | "blocked"
  | "stopped";

export type HookRunSummary = {
  completedAt?: number | null;
  displayOrder: number;
  durationMs?: number | null;
  entries: Array<HookOutputEntry>;
  eventName: HookEventName;
  executionMode: HookExecutionMode;
  handlerType: HookHandlerType;
  id: string;
  scope: HookScope;
  source?: HookSource;
  sourcePath: LegacyAppPathString;
  startedAt: number;
  status: HookRunStatus;
  statusMessage?: string | null;
};

export type HookScope = "thread" | "turn";

export type HookSource =
  | "system"
  | "user"
  | "project"
  | "mdm"
  | "sessionFlags"
  | "plugin"
  | "cloudRequirements"
  | "cloudManagedConfig"
  | "legacyManagedConfigFile"
  | "legacyManagedConfigMdm"
  | "unknown";

export type HookStartedNotification = {
  run: HookRunSummary;
  threadId: string;
  turnId?: string | null;
};

export type HookTrustStatus = "managed" | "untrusted" | "trusted" | "modified";

export type HooksListEntry = {
  cwd: string;
  errors: Array<HookErrorInfo>;
  hooks: Array<HookMetadata>;
  warnings: Array<string>;
};

export type HooksListParams = {
  cwds?: Array<string>;
  threadId?: string | null;
};

export type HooksListResponse = { data: Array<HooksListEntry> };

export type ImageDetail = "auto" | "low" | "high" | "original";

export type ImageGenerationFailure = {
  limitId: string;
  resetsAt?: number | null;
  type: "usageLimitExceeded";
};

export type ImageGenerationInputImage = {
  path: AbsolutePathBuf;
  sha256: string;
};

export type InAppBrowserRequirements = {
  allowExternalBrowserSettingsImport?: boolean | null;
};

export type InputModality = "text" | "image" | "audio";

export type InstalledApp = {
  callable: boolean;
  enabled: boolean;
  id: string;
  runtimeName?: string | null;
};

export type InternalChatMessageMetadataPassthrough = {
  turn_id?: string | null;
};

export type ItemCompletedNotification = {
  completedAtMs: number;
  item: ThreadItem;
  threadId: string;
  turnId: string;
};

export type ItemGuardianApprovalReviewCompletedNotification = {
  action: GuardianApprovalReviewAction;
  completedAtMs: number;
  decisionSource: AutoReviewDecisionSource;
  review: GuardianApprovalReview;
  reviewId: string;
  startedAtMs: number;
  targetItemId?: string | null;
  threadId: string;
  turnId: string;
};

export type ItemGuardianApprovalReviewStartedNotification = {
  action: GuardianApprovalReviewAction;
  review: GuardianApprovalReview;
  reviewId: string;
  startedAtMs: number;
  targetItemId?: string | null;
  threadId: string;
  turnId: string;
};

export type ItemStartedNotification = {
  item: ThreadItem;
  startedAtMs: number;
  threadId: string;
  turnId: string;
};

export type LegacyAppPathString = string;

export type ListMcpServerStatusParams = {
  cursor?: string | null;
  detail?: McpServerStatusDetail | null;
  limit?: number | null;
  serverName?: string | null;
  threadId?: string | null;
};

export type ListMcpServerStatusResponse = {
  data: Array<McpServerStatus>;
  nextCursor?: string | null;
};

export type LocalShellAction = {
  command: Array<string>;
  env?: Record<string, string> | null;
  timeout_ms?: number | null;
  type: "exec";
  user?: string | null;
  working_directory?: string | null;
};

export type LocalShellStatus = "completed" | "in_progress" | "incomplete";

export type LoginAccountParams =
  | { apiKey: string; type: "apiKey" }
  | {
      appBrand?: LoginAppBrand | null;
      codexStreamlinedLogin?: boolean;
      type: "chatgpt";
      useHostedLoginSuccessPage?: boolean;
    }
  | { type: "chatgptDeviceCode" }
  | {
      accessToken: string;
      chatgptAccountId: string;
      chatgptPlanType?: string | null;
      type: "chatgptAuthTokens";
    }
  | { apiKey: string; region: string; type: "amazonBedrock" }
  | {
      accessKeyId: string;
      region: string;
      secretAccessKey: string;
      sessionToken?: string | null;
      type: "amazonBedrockAccessKeys";
    };

export type LoginAccountResponse =
  | { type: "apiKey" }
  | { authUrl: string; loginId: string; type: "chatgpt" }
  | {
      loginId: string;
      type: "chatgptDeviceCode";
      userCode: string;
      verificationUrl: string;
    }
  | { type: "chatgptAuthTokens" }
  | { type: "amazonBedrock" };

export type LoginAppBrand = "codex" | "chatgpt";

export type LogoutAccountResponse = Record<string, unknown>;

export type ManagedHooksRequirements = {
  Interrupt?: Array<ConfiguredHookMatcherGroup>;
  PermissionRequest: Array<ConfiguredHookMatcherGroup>;
  PostCompact: Array<ConfiguredHookMatcherGroup>;
  PostModelRequest?: Array<ConfiguredHookMatcherGroup>;
  PostToolUse: Array<ConfiguredHookMatcherGroup>;
  PreCompact: Array<ConfiguredHookMatcherGroup>;
  PreModelRequest?: Array<ConfiguredHookMatcherGroup>;
  PreToolUse: Array<ConfiguredHookMatcherGroup>;
  SessionEnd?: Array<ConfiguredHookMatcherGroup>;
  SessionStart: Array<ConfiguredHookMatcherGroup>;
  Stop: Array<ConfiguredHookMatcherGroup>;
  SubagentStart: Array<ConfiguredHookMatcherGroup>;
  SubagentStop: Array<ConfiguredHookMatcherGroup>;
  UserPromptSubmit: Array<ConfiguredHookMatcherGroup>;
  managedDir?: string | null;
  windowsManagedDir?: string | null;
};

export type MarketplaceAddParams = {
  refName?: string | null;
  source: string;
  sparsePaths?: Array<string> | null;
};

export type MarketplaceAddResponse = {
  alreadyAdded: boolean;
  installedRoot: AbsolutePathBuf;
  marketplaceName: string;
};

export type MarketplaceInterface = { displayName?: string | null };

export type MarketplaceLoadErrorInfo = {
  marketplacePath: AbsolutePathBuf;
  message: string;
};

export type MarketplaceRemoveParams = { marketplaceName: string };

export type MarketplaceRemoveResponse = {
  installedRoot?: AbsolutePathBuf | null;
  marketplaceName: string;
};

export type MarketplaceUpgradeErrorInfo = {
  marketplaceName: string;
  message: string;
};

export type MarketplaceUpgradeParams = { marketplaceName?: string | null };

export type MarketplaceUpgradeResponse = {
  errors: Array<MarketplaceUpgradeErrorInfo>;
  selectedMarketplaces: Array<string>;
  upgradedRoots: Array<AbsolutePathBuf>;
};

export type McpAppDisplayMode = "inline" | "fullscreen";

export type McpAppUi = {
  preferredModelDisplayMode: McpAppDisplayMode;
  resourceUri: string;
};

export type McpAuthStatus =
  | "unknown"
  | "unsupported"
  | "notLoggedIn"
  | "bearerToken"
  | "oAuth";

export type McpResourceReadParams = {
  connectorId?: string | null;
  originCallId?: string | null;
  server: string;
  target?: McpResourceReadTarget | null;
  threadId?: string | null;
  uri: string;
};

export type McpResourceReadResponse = {
  contents: Array<ResourceContent>;
  originCallId?: string | null;
};

export type McpResourceReadTarget = {
  connectorId: string;
  linkId: string | null;
};

export type McpServerConnectionStatus =
  | "notStarted"
  | "starting"
  | "connected"
  | "authenticationRequired"
  | "failed"
  | "cancelled"
  | "disabled";

export type McpServerEventNotification = { method: string; params: unknown };

export type McpServerEventStreamNotification = {
  notification: McpServerEventNotification;
  subscriptionId: string;
};

export type McpServerEventStreamStartParams = {
  _meta?: unknown;
  arguments: unknown;
  name: string;
  server: string;
  subscriptionId: string;
  threadId: string;
};

export type McpServerEventStreamStartResponse = Record<string, unknown>;

export type McpServerEventStreamStopParams = { subscriptionId: string };

export type McpServerEventStreamStopResponse = Record<string, unknown>;

export type McpServerInfo = {
  description?: string | null;
  icons?: Array<unknown> | null;
  name: string;
  title?: string | null;
  version: string;
  websiteUrl?: string | null;
};

export type McpServerMigration = { name: string };

export type McpServerOauthClientRegistration = "auto" | "cimd" | "dcr";

export type McpServerOauthLoginCompletedNotification = {
  error?: string | null;
  loginId?: string | null;
  name: string;
  success: boolean;
  threadId?: string | null;
};

export type McpServerOauthLoginParams = {
  clientRegistration?: McpServerOauthClientRegistration | null;
  name: string;
  scopes?: Array<string> | null;
  threadId?: string | null;
  timeoutSecs?: number | null;
};

export type McpServerOauthLoginResponse = {
  authorizationUrl: string;
  loginId?: string | null;
};

export type McpServerRefreshResponse = Record<string, unknown>;

export type McpServerStartupFailureReason = "reauthenticationRequired";

export type McpServerStartupState =
  | "starting"
  | "ready"
  | "failed"
  | "cancelled";

export type McpServerStatus = {
  authStatus: McpAuthStatus;
  httpOrigin?: string | null;
  name: string;
  pluginId?: string | null;
  resourceTemplates: Array<ResourceTemplate>;
  resources: Array<Resource>;
  runtimeStatus?: McpServerConnectionStatus | null;
  serverCapabilities?: unknown;
  serverInfo?: McpServerInfo | null;
  toolListMeta?: unknown;
  tools: Record<string, Tool>;
  toolsError?: string | null;
};

export type McpServerStatusDetail = "full" | "toolsAndAuthOnly";

export type McpServerStatusUpdatedNotification = {
  error?: string | null;
  failureReason?: McpServerStartupFailureReason | null;
  name: string;
  status: McpServerStartupState;
  threadId?: string | null;
};

export type McpServerToolCallParams = {
  _meta?: unknown;
  arguments?: unknown;
  server: string;
  threadId: string;
  tool: string;
};

export type McpServerToolCallResponse = {
  _meta?: unknown;
  content: Array<unknown>;
  isError?: boolean | null;
  structuredContent?: unknown;
};

export type McpToolCallAppContext = {
  actionName?: string | null;
  appName?: string | null;
  connectorId: string;
  linkId?: string | null;
  resourceUri?: string | null;
};

export type McpToolCallError = { message: string };

export type McpToolCallProgressNotification = {
  itemId: string;
  message: string;
  threadId: string;
  turnId: string;
};

export type McpToolCallResult = {
  _meta?: unknown;
  content: Array<unknown>;
  structuredContent?: unknown;
};

export type McpToolCallStatus = "inProgress" | "completed" | "failed";

export type MemoryCitation = {
  entries: Array<MemoryCitationEntry>;
  threadIds: Array<string>;
};

export type MemoryCitationEntry = {
  lineEnd: number;
  lineStart: number;
  note: string;
  path: string;
};

export type MemoryResetResponse = Record<string, unknown>;

export type MemoryStatusParams = { minConsolidatedThreads?: number | null };

export type MemoryStatusResponse = {
  v2ConsolidatedThreads: number;
  v2Ready: boolean;
};

export type MergeStrategy = "replace" | "upsert";

export type MessagePhase = "commentary" | "partial_answer" | "final_answer";

export type MigrationDetails = {
  commands?: Array<CommandMigration>;
  hooks?: Array<HookMigration>;
  mcpServers?: Array<McpServerMigration>;
  memory?: Array<string>;
  plugins?: Array<PluginsMigration>;
  sessions?: Array<SessionMigration>;
  skills?: Array<SkillMigration>;
  subagents?: Array<SubagentMigration>;
};

export type MisalignmentErrorDetails = {
  detailedExplanation?: string | null;
  errorType?: string | null;
  reviewTarget?: string | null;
  steer?: MisalignmentSteer | null;
};

export type MisalignmentSteer = { message: string };

export type MockExperimentalMethodParams = { value?: string | null };

export type MockExperimentalMethodResponse = { echoed?: string | null };

export type ModeKind = "plan" | "default";

export type Model = {
  additionalSpeedTiers?: Array<string>;
  availabilityNux?: ModelAvailabilityNux | null;
  availableAccessPrograms?: ModelAccessPrograms | null;
  defaultReasoningEffort: ReasoningEffort;
  defaultServiceTier?: string | null;
  description: string;
  displayName: string;
  hidden: boolean;
  id: string;
  inputModalities?: Array<InputModality>;
  isDefault: boolean;
  model: string;
  modelSpecialty?: string | null;
  multiAgentVersion?: MultiAgentVersion | null;
  serviceTiers?: Array<ModelServiceTier>;
  supportedReasoningEfforts: Array<ReasoningEffortOption>;
  supportsPersonality?: boolean;
  upgrade?: string | null;
  upgradeInfo?: ModelUpgradeInfo | null;
};

export type ModelAccessPrograms = { cyber: Array<CyberAccessProgram> };

export type ModelAvailabilityNux = { message: string };

export type ModelListParams = {
  cursor?: string | null;
  includeHidden?: boolean | null;
  limit?: number | null;
};

export type ModelListResponse = {
  data: Array<Model>;
  nextCursor?: string | null;
};

export type ModelProviderCapabilitiesReadParams = Record<string, unknown>;

export type ModelProviderCapabilitiesReadResponse = {
  imageGeneration: boolean;
  webSearch: boolean;
};

export type ModelProviderError =
  | {
      currentModelProvider: string;
      modelProvider: string;
      type: "modelProviderMismatch";
    }
  | { modelProvider: string; type: "modelProviderNotConfigured" }
  | {
      modelProvider: string;
      requiredModelProvider: string;
      type: "modelProviderPolicyConflict";
    };

export type ModelRerouteReason = "highRiskCyberActivity";

export type ModelReroutedNotification = {
  fromModel: string;
  reason: ModelRerouteReason;
  threadId: string;
  toModel: string;
  turnId: string;
};

export type ModelSafetyBufferingUpdatedNotification = {
  fasterModel?: string | null;
  model: string;
  reasons: Array<string>;
  showBufferingUi: boolean;
  threadId: string;
  turnId: string;
  useCases: Array<string>;
};

export type ModelServiceTier = {
  description: string;
  id: string;
  name: string;
};

export type ModelUpgradeInfo = {
  migrationMarkdown?: string | null;
  model: string;
  modelLink?: string | null;
  retirementAt?: number | null;
  upgradeCopy?: string | null;
};

export type ModelVerification = "trustedAccessForCyber";

export type ModelVerificationNotification = {
  threadId: string;
  turnId: string;
  verifications: Array<ModelVerification>;
};

export type ModelsRequirements = { newThread?: NewThreadModelDefaults | null };

export type MultiAgentMode =
  | ("explicitRequestOnly" | "proactive")
  | { custom: string };

export type MultiAgentVersion = "disabled" | "v1" | "v2";

export type NetworkAccess = "restricted" | "enabled";

export type NetworkApprovalProtocol =
  | "http"
  | "https"
  | "socks5Tcp"
  | "socks5Udp";

export type NetworkDomainPermission = "allow" | "deny";

export type NetworkRequirements = {
  allowLocalBinding?: boolean | null;
  allowUnixSockets?: Array<string> | null;
  allowUpstreamProxy?: boolean | null;
  allowedDomains?: Array<string> | null;
  dangerouslyAllowAllUnixSockets?: boolean | null;
  dangerouslyAllowNonLoopbackProxy?: boolean | null;
  deniedDomains?: Array<string> | null;
  domains?: Record<string, NetworkDomainPermission> | null;
  enabled?: boolean | null;
  httpPort?: number | null;
  managedAllowedDomainsOnly?: boolean | null;
  reviewDomains?: Array<string> | null;
  socksPort?: number | null;
  unixSockets?: Record<string, NetworkUnixSocketPermission> | null;
};

export type NetworkUnixSocketPermission = "allow" | "deny";

export type NewThreadModelDefaults = {
  model?: string | null;
  modelReasoningEffort?: ReasoningEffort | null;
  serviceTier?: string | null;
};

export type NonSteerableTurnKind = "review" | "compact";

export type NullableGetAccountRateLimitsParams = GetAccountRateLimitsParams | null;

export type NullableGetAccountTokenUsageParams = GetAccountTokenUsageParams | null;

export type NullableRemoteControlDisableParams = RemoteControlDisableParams | null;

export type NullableRemoteControlEnableParams = RemoteControlEnableParams | null;

export type OtelExporter =
  | ("none" | "statsig")
  | {
      "otlp-http": {
        endpoint: string;
        headers: Record<string, string>;
        protocol: OtelHttpProtocol;
        tls?: OtelTlsConfig | null;
      };
    }
  | {
      "otlp-grpc": {
        endpoint: string;
        headers: Record<string, string>;
        tls?: OtelTlsConfig | null;
      };
    };

export type OtelHttpProtocol = "binary" | "json";

export type OtelRequirements = {
  environment?: string | null;
  exporter?: OtelExporter | null;
  log_agent_responses?: boolean | null;
  log_guardian_assessments?: boolean | null;
  log_sse_deltas?: boolean | null;
  log_user_prompt?: boolean | null;
  metrics_exporter?: OtelExporter | null;
  span_attributes?: Record<string, string> | null;
  tool_result?: OtelToolResultLogConfig | null;
  trace_exporter?: OtelExporter | null;
  tracestate?: Record<string, Record<string, string>> | null;
};

export type OtelTlsConfig = {
  "ca-certificate"?: string | null;
  "client-certificate"?: string | null;
  "client-private-key"?: string | null;
};

export type OtelToolResultLogConfig = { max_bytes: number };

export type OverriddenMetadata = {
  effectiveValue: unknown;
  message: string;
  overridingLayer: ConfigLayerMetadata;
};

export type PatchApplyStatus =
  | "inProgress"
  | "completed"
  | "failed"
  | "declined";

export type PatchChangeKind =
  | { type: "add" }
  | { type: "delete" }
  | { move_path?: string | null; type: "update" };

export type PathUri = string;

export type PermissionProfileListParams = {
  cursor?: string | null;
  cwd?: string | null;
  limit?: number | null;
};

export type PermissionProfileListResponse = {
  data: Array<PermissionProfileSummary>;
  nextCursor?: string | null;
};

export type PermissionProfileSummary = {
  allowed: boolean;
  description?: string | null;
  id: string;
};

export type Personality = "none" | "friendly" | "pragmatic";

export type PlanDeltaNotification = {
  delta: string;
  itemId: string;
  threadId: string;
  turnId: string;
};

export type PlanType =
  | "free"
  | "go"
  | "plus"
  | "pro"
  | "prolite"
  | "promax"
  | "team"
  | "self_serve_business_prolite"
  | "self_serve_business_usage_based"
  | "business"
  | "ent26"
  | "enterprise_cbp_automation"
  | "enterprise_cbp_usage_based"
  | "enterprise"
  | "edu"
  | "edu_plus"
  | "edu_pro"
  | "sci"
  | "sci_plus"
  | "unknown";

export type PluginAuthPolicy = "ON_INSTALL" | "ON_USE";

export type PluginAvailability = "DISABLED_BY_ADMIN" | "AVAILABLE";

export type PluginDetail = {
  appTemplates: Array<AppTemplateSummary>;
  apps: Array<AppSummary>;
  description?: string | null;
  hooks: Array<PluginHookSummary>;
  marketplaceName: string;
  marketplacePath?: AbsolutePathBuf | null;
  mcpServers: Array<string>;
  onboardingSkill?: SkillSummary | null;
  scheduledTasks?: Array<ScheduledTaskSummary> | null;
  shareUrl?: string | null;
  skills: Array<SkillSummary>;
  summary: PluginSummary;
};

export type PluginDisabledReason =
  | "disabled_by_admin"
  | "plan_not_eligible"
  | "required_app_unavailable"
  | "unknown";

export type PluginHookSummary = { eventName: HookEventName; key: string };

export type PluginInstallParams = {
  installAttemptId?: string | null;
  marketplacePath?: AbsolutePathBuf | null;
  pluginName: string;
  remoteMarketplaceName?: string | null;
};

export type PluginInstallPolicy =
  | "NOT_AVAILABLE"
  | "AVAILABLE"
  | "INSTALLED_BY_DEFAULT";

export type PluginInstallPolicySource =
  | "WORKSPACE_SETTING"
  | "IMPLICIT_CANONICAL_APP";

export type PluginInstallResponse = {
  appsNeedingAuth: Array<AppSummary>;
  authPolicy: PluginAuthPolicy;
};

export type PluginInstalledParams = {
  cwds?: Array<AbsolutePathBuf> | null;
  installSuggestionPluginNames?: Array<string> | null;
};

export type PluginInstalledResponse = {
  marketplaceLoadErrors?: Array<MarketplaceLoadErrorInfo>;
  marketplaces: Array<PluginMarketplaceEntry>;
};

export type PluginInterface = {
  brandColor?: string | null;
  capabilities: Array<string>;
  category?: string | null;
  composerIcon?: AbsolutePathBuf | null;
  composerIconUrl?: string | null;
  defaultPrompt?: Array<string> | null;
  developerName?: string | null;
  displayName?: string | null;
  logo?: AbsolutePathBuf | null;
  logoDark?: AbsolutePathBuf | null;
  logoUrl?: string | null;
  logoUrlDark?: string | null;
  longDescription?: string | null;
  privacyPolicyUrl?: string | null;
  screenshotUrls: Array<string>;
  screenshots: Array<AbsolutePathBuf>;
  shortDescription?: string | null;
  termsOfServiceUrl?: string | null;
  websiteUrl?: string | null;
};

export type PluginListMarketplaceKind =
  | "local"
  | "vertical"
  | "workspace-directory"
  | "shared-with-me"
  | "created-by-me-remote";

export type PluginListParams = {
  cwds?: Array<AbsolutePathBuf> | null;
  forceRefetch?: boolean;
  marketplaceKinds?: Array<PluginListMarketplaceKind> | null;
};

export type PluginListResponse = {
  featuredPluginIds?: Array<string>;
  marketplaceLoadErrors?: Array<MarketplaceLoadErrorInfo>;
  marketplaces: Array<PluginMarketplaceEntry>;
};

export type PluginMarketplaceEntry = {
  interface?: MarketplaceInterface | null;
  name: string;
  path?: AbsolutePathBuf | null;
  plugins: Array<PluginSummary>;
};

export type PluginReadParams = {
  marketplacePath?: AbsolutePathBuf | null;
  pluginName: string;
  remoteMarketplaceName?: string | null;
};

export type PluginReadResponse = { plugin: PluginDetail };

export type PluginReconcileChangedPlugin = {
  hasApps: boolean;
  hasHooks: boolean;
  hasMcps: boolean;
  hasSkills: boolean;
  id: string;
};

export type PluginReconcileParams = { reason?: string | null };

export type PluginReconcileResponse = {
  changedPlugins: Array<PluginReconcileChangedPlugin>;
  failedMaterializationRemotePluginIds: Array<string>;
  failedRemotePluginIds: Array<string>;
};

export type PluginSearchParams = {
  cursor?: string | null;
  cwds?: Array<AbsolutePathBuf> | null;
  limit?: number | null;
  scope?: PluginSearchScope | null;
  searchTerm: string;
};

export type PluginSearchResponse = {
  data: Array<PluginSearchResult>;
  nextCursor?: string | null;
};

export type PluginSearchResult = {
  marketplaceName: string;
  marketplacePath?: AbsolutePathBuf | null;
  plugin: PluginSummary;
};

export type PluginSearchScope = "global" | "workspace" | "personal";

export type PluginShareCheckoutParams = { remotePluginId: string };

export type PluginShareCheckoutResponse = {
  marketplaceName: string;
  marketplacePath: AbsolutePathBuf;
  pluginId: string;
  pluginName: string;
  pluginPath: AbsolutePathBuf;
  remotePluginId: string;
  remoteVersion?: string | null;
};

export type PluginShareContext = {
  canPublishToWorkspace?: boolean | null;
  creatorAccountUserId?: string | null;
  creatorName?: string | null;
  discoverability?: PluginShareDiscoverability | null;
  remotePluginId: string;
  remoteVersion?: string | null;
  sharePrincipals?: Array<PluginSharePrincipal> | null;
  shareUrl?: string | null;
};

export type PluginShareDeleteParams = { remotePluginId: string };

export type PluginShareDeleteResponse = Record<string, unknown>;

export type PluginShareDiscoverability = "LISTED" | "UNLISTED" | "PRIVATE";

export type PluginShareListItem = {
  localPluginPath?: AbsolutePathBuf | null;
  plugin: PluginSummary;
};

export type PluginShareListParams = Record<string, unknown>;

export type PluginShareListResponse = { data: Array<PluginShareListItem> };

export type PluginSharePrincipal = {
  name: string;
  principalId: string;
  principalType: PluginSharePrincipalType;
  role: PluginSharePrincipalRole;
};

export type PluginSharePrincipalRole = "reader" | "editor" | "owner";

export type PluginSharePrincipalType = "user" | "group" | "workspace";

export type PluginShareSaveParams = {
  discoverability?: PluginShareDiscoverability | null;
  pluginPath: AbsolutePathBuf;
  remotePluginId?: string | null;
  shareTargets?: Array<PluginShareTarget> | null;
};

export type PluginShareSaveResponse = {
  canPublishToWorkspace?: boolean | null;
  remotePluginId: string;
  shareUrl: string;
};

export type PluginShareTarget = {
  principalId: string;
  principalType: PluginSharePrincipalType;
  role: PluginShareTargetRole;
};

export type PluginShareTargetRole = "reader" | "editor";

export type PluginShareUpdateDiscoverability =
  | "UNLISTED"
  | "PRIVATE"
  | "LISTED";

export type PluginShareUpdateTargetsParams = {
  discoverability: PluginShareUpdateDiscoverability;
  remotePluginId: string;
  shareTargets: Array<PluginShareTarget>;
};

export type PluginShareUpdateTargetsResponse = {
  discoverability: PluginShareDiscoverability;
  principals: Array<PluginSharePrincipal>;
};

export type PluginSkillReadParams = {
  remoteMarketplaceName: string;
  remotePluginId: string;
  skillName: string;
};

export type PluginSkillReadResponse = { contents?: string | null };

export type PluginSource =
  | { path: AbsolutePathBuf; type: "local" }
  | {
      path?: string | null;
      refName?: string | null;
      sha?: string | null;
      type: "git";
      url: string;
    }
  | {
      package: string;
      registry?: string | null;
      type: "npm";
      version?: string | null;
    }
  | { type: "remote" };

export type PluginSummary = {
  authPolicy: PluginAuthPolicy;
  availability?: PluginAvailability;
  disabledReason?: PluginDisabledReason | null;
  eligiblePlanTypes?: Array<string> | null;
  enabled: boolean;
  id: string;
  installPolicy: PluginInstallPolicy;
  installPolicySource?: PluginInstallPolicySource | null;
  installed: boolean;
  installedAt?: number | null;
  interface?: PluginInterface | null;
  keywords?: Array<string>;
  localVersion?: string | null;
  mustShowInstallationInterstitial?: boolean | null;
  name: string;
  remotePluginId?: string | null;
  shareContext?: PluginShareContext | null;
  source: PluginSource;
  version?: string | null;
};

export type PluginUninstallParams = { pluginId: string };

export type PluginUninstallResponse = Record<string, unknown>;

export type PluginsMigration = {
  marketplaceName: string;
  pluginNames: Array<string>;
};

export type ProcessExitedNotification = {
  exitCode: number;
  outputDrainTimedOut?: boolean | null;
  processHandle: string;
  stderr: string;
  stderrCapReached: boolean;
  stdout: string;
  stdoutCapReached: boolean;
};

export type ProcessKillParams = { processHandle: string };

export type ProcessKillResponse = Record<string, unknown>;

export type ProcessOutputDeltaNotification = {
  capReached: boolean;
  deltaBase64: string;
  processHandle: string;
  stream: ProcessOutputStream;
};

export type ProcessOutputStream = "stdout" | "stderr";

export type ProcessResizePtyParams = {
  processHandle: string;
  size: ProcessTerminalSize;
};

export type ProcessResizePtyResponse = Record<string, unknown>;

export type ProcessSpawnParams = {
  command: Array<string>;
  cwd: AbsolutePathBuf;
  env?: Record<string, string | null> | null;
  outputBytesCap?: number | null;
  processHandle: string;
  size?: ProcessTerminalSize | null;
  streamStdin?: boolean;
  streamStdoutStderr?: boolean;
  timeoutMs?: number | null;
  tty?: boolean;
};

export type ProcessSpawnResponse = Record<string, unknown>;

export type ProcessTerminalSize = { cols: number; rows: number };

export type ProcessWriteStdinParams = {
  closeStdin?: boolean;
  deltaBase64?: string | null;
  processHandle: string;
};

export type ProcessWriteStdinResponse = Record<string, unknown>;

export type Project = {
  createdAt: number;
  id: string;
  metadata: Record<string, string>;
  name: string;
  position: number;
  recencyAt?: number | null;
  roots: Array<ProjectRoot>;
  updatedAt: number;
};

export type ProjectChangeType = "created" | "updated" | "deleted";

export type ProjectChangedNotification = {
  changeType: ProjectChangeType;
  projectId: string;
};

export type ProjectCreateParams = {
  idempotencyKey: string;
  metadata?: Record<string, string> | null;
  name: string;
  roots: Array<ProjectRoot>;
};

export type ProjectCreateResponse = { project: Project };

export type ProjectDeleteParams = { projectId: string };

export type ProjectDeleteResponse = Record<string, unknown>;

export type ProjectImportParams = {
  idempotencyKey: string;
  metadata?: Record<string, string> | null;
  name: string;
  roots: Array<ProjectRoot>;
  threads?: Array<string> | null;
};

export type ProjectImportResponse = { project: Project };

export type ProjectListParams = {
  cursor?: string | null;
  limit?: number | null;
  sortDirection?: SortDirection | null;
  sortKey?: ProjectSortKey | null;
};

export type ProjectListResponse = {
  data: Array<Project>;
  nextCursor?: string | null;
};

export type ProjectMoveParams = {
  beforeProjectId?: string | null;
  projectId: string;
};

export type ProjectMoveResponse = Record<string, unknown>;

export type ProjectReadParams = { projectId: string };

export type ProjectReadResponse = { project: Project };

export type ProjectRoot = { path: AbsolutePathBuf };

export type ProjectSortKey = "position" | "recencyAt";

export type ProjectUpdateParams = {
  metadata?: Record<string, string> | null;
  name?: string | null;
  projectId: string;
  roots?: Array<ProjectRoot> | null;
};

export type ProjectUpdateResponse = { project: Project };

export type QueuedSubmission = {
  clientUserMessageId: string;
  id: string;
  input: Array<UserInput>;
};

export type RateLimitReachedType =
  | "rate_limit_reached"
  | "workspace_owner_credits_depleted"
  | "workspace_member_credits_depleted"
  | "workspace_owner_usage_limit_reached"
  | "workspace_member_usage_limit_reached";

export type RateLimitResetCredit = {
  description?: string | null;
  expiresAt?: number | null;
  grantedAt: number;
  id: string;
  resetType: RateLimitResetType;
  status: RateLimitResetCreditStatus;
  title?: string | null;
};

export type RateLimitResetCreditStatus =
  | "available"
  | "redeeming"
  | "redeemed"
  | "unknown";

export type RateLimitResetCreditsSummary = {
  availableCount: number;
  credits?: Array<RateLimitResetCredit> | null;
};

export type RateLimitResetType = "codexRateLimits" | "unknown";

export type RateLimitSnapshot = {
  credits?: CreditsSnapshot | null;
  individualLimit?: SpendControlLimitSnapshot | null;
  limitId?: string | null;
  limitName?: string | null;
  normalModelSlug?: string | null;
  planType?: PlanType | null;
  primary?: RateLimitWindow | null;
  rateLimitReachedType?: RateLimitReachedType | null;
  secondary?: RateLimitWindow | null;
  spendControlReached?: boolean | null;
};

export type RateLimitWindow = {
  resetsAt?: number | null;
  usedPercent: number;
  windowDurationMins?: number | null;
};

export type RawResponseCompletedNotification = {
  responseId: string;
  threadId: string;
  turnId: string;
  usage?: TokenUsageBreakdown | null;
  usageMetadata?: ResponseUsageMetadata | null;
};

export type RawResponseItemCompletedNotification = {
  item: ResponseItem;
  threadId: string;
  turnId: string;
};

export type RealtimeConversationVersion = "v1" | "v2" | "v3";

export type RealtimeOutputModality = "text" | "audio";

export type RealtimeVoice =
  | "alloy"
  | "arbor"
  | "ash"
  | "aube"
  | "ballad"
  | "breeze"
  | "bubbie"
  | "cedar"
  | "coral"
  | "cove"
  | "echo"
  | "ember"
  | "haetsal"
  | "hanul"
  | "himari"
  | "juniper"
  | "leher"
  | "maple"
  | "marin"
  | "miko"
  | "neer"
  | "porto"
  | "rindo"
  | "rio"
  | "rivage"
  | "sage"
  | "selva"
  | "shimmer"
  | "sol"
  | "sonna"
  | "spruce"
  | "tinta"
  | "vale"
  | "verse"
  | "viola";

export type RealtimeVoicesList = {
  defaultV1: RealtimeVoice;
  defaultV2: RealtimeVoice;
  v1: Array<RealtimeVoice>;
  v2: Array<RealtimeVoice>;
  v3?: Array<RealtimeVoice>;
};

export type ReasoningEffort = string;

export type ReasoningEffortOption = {
  description: string;
  reasoningEffort: ReasoningEffort;
};

export type ReasoningItemContent =
  | { text: string; type: "reasoning_text" }
  | { text: string; type: "text" };

export type ReasoningItemReasoningSummary = {
  text: string;
  type: "summary_text";
};

export type ReasoningSummary = ("auto" | "concise" | "detailed") | "none";

export type ReasoningSummaryPartAddedNotification = {
  itemId: string;
  summaryIndex: number;
  threadId: string;
  turnId: string;
};

export type ReasoningSummaryTextDeltaNotification = {
  delta: string;
  itemId: string;
  summaryIndex: number;
  threadId: string;
  turnId: string;
};

export type ReasoningTextDeltaNotification = {
  contentIndex: number;
  delta: string;
  itemId: string;
  threadId: string;
  turnId: string;
};

export type RemoteControlClient = {
  appVersion?: string | null;
  clientId: string;
  deviceModel?: string | null;
  deviceType?: string | null;
  displayName?: string | null;
  lastSeenAt?: number | null;
  osVersion?: string | null;
  platform?: string | null;
};

export type RemoteControlClientsListOrder = "asc" | "desc";

export type RemoteControlClientsListParams = {
  cursor?: string | null;
  environmentId: string;
  limit?: number | null;
  order?: RemoteControlClientsListOrder | null;
};

export type RemoteControlClientsListResponse = {
  data: Array<RemoteControlClient>;
  nextCursor?: string | null;
};

export type RemoteControlClientsRevokeParams = {
  clientId: string;
  environmentId: string;
};

export type RemoteControlClientsRevokeResponse = Record<string, unknown>;

export type RemoteControlConnectionStatus =
  | "disabled"
  | "connecting"
  | "connected"
  | "errored";

export type RemoteControlDisableParams = { ephemeral?: boolean };

export type RemoteControlDisableResponse = {
  environmentId?: string | null;
  installationId: string;
  serverName: string;
  status: RemoteControlConnectionStatus;
};

export type RemoteControlEnableParams = { ephemeral?: boolean };

export type RemoteControlEnableResponse = {
  environmentId?: string | null;
  installationId: string;
  serverName: string;
  status: RemoteControlConnectionStatus;
};

export type RemoteControlPairingStartParams = { manualCode?: boolean };

export type RemoteControlPairingStartResponse = {
  environmentId: string;
  expiresAt: number;
  manualPairingCode?: string | null;
  pairingCode: string;
};

export type RemoteControlPairingStatusParams = {
  manualPairingCode?: string | null;
  pairingCode?: string | null;
};

export type RemoteControlPairingStatusResponse = { claimed: boolean };

export type RemoteControlStatusChangedNotification = {
  environmentId?: string | null;
  installationId: string;
  serverName: string;
  status: RemoteControlConnectionStatus;
};

export type RemoteControlStatusReadResponse = {
  environmentId?: string | null;
  installationId: string;
  serverName: string;
  status: RemoteControlConnectionStatus;
};

export type RequestHeader = { name: string; value: string };

export type RequestPermissionProfile = {
  fileSystem?: AdditionalFileSystemPermissions | null;
  network?: AdditionalNetworkPermissions | null;
};

export type ResidencyRequirement = "us";

export type Resource = {
  _meta?: unknown;
  annotations?: unknown;
  description?: string | null;
  icons?: Array<unknown> | null;
  mimeType?: string | null;
  name: string;
  size?: number | null;
  title?: string | null;
  uri: string;
};

export type ResourceContent =
  | { _meta?: unknown; mimeType?: string | null; text: string; uri: string }
  | { _meta?: unknown; blob: string; mimeType?: string | null; uri: string };

export type ResourceTemplate = {
  annotations?: unknown;
  description?: string | null;
  mimeType?: string | null;
  name: string;
  title?: string | null;
  uriTemplate: string;
};

export type ResponseItem =
  | {
      id?: string | null;
      role: string;
      tools: Array<unknown>;
      type: "additional_tools";
    }
  | {
      content: Array<ContentItem>;
      encrypted_content?: string | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      phase?: MessagePhase | null;
      role: string;
      status?: string | null;
      type: "message";
    }
  | {
      author: string;
      content: Array<AgentMessageInputContent>;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      recipient: string;
      type: "agent_message";
    }
  | {
      content?: Array<ReasoningItemContent> | null;
      encrypted_content?: string | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      summary: Array<ReasoningItemReasoningSummary>;
      type: "reasoning";
    }
  | {
      action: LocalShellAction;
      call_id?: string | null;
      encrypted_content?: string | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      status: LocalShellStatus;
      type: "local_shell_call";
    }
  | {
      arguments: string;
      call_id: string;
      encrypted_content?: string | null;
      encrypted_function_args?: Array<string> | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      name: string;
      namespace?: string | null;
      status?: string | null;
      type: "function_call";
    }
  | {
      arguments: unknown;
      call_id?: string | null;
      encrypted_content?: string | null;
      execution: string;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      status?: string | null;
      type: "tool_search_call";
    }
  | {
      call_id?: string | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      name?: string | null;
      namespace?: string | null;
      output: FunctionCallOutputBody;
      type: "function_call_output";
    }
  | {
      call_id: string;
      encrypted_content?: string | null;
      id?: string | null;
      input: string;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      name: string;
      namespace?: string | null;
      status?: string | null;
      type: "custom_tool_call";
    }
  | {
      call_id: string;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      name?: string | null;
      output: FunctionCallOutputBody;
      type: "custom_tool_call_output";
    }
  | {
      call_id?: string | null;
      execution: string;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      status: string;
      tools: Array<unknown>;
      type: "tool_search_output";
    }
  | {
      action?: ResponsesApiWebSearchAction | null;
      encrypted_content?: string | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      status?: string | null;
      type: "web_search_call";
    }
  | {
      encrypted_content?: string | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      result: string;
      revised_prompt?: string | null;
      status: string;
      type: "image_generation_call";
    }
  | {
      encrypted_content: string;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      type: "compaction";
    }
  | { reasoning: ConfigurationReasoning; type: "configuration_update" }
  | { type: "compaction_trigger" }
  | {
      encrypted_content?: string | null;
      id?: string | null;
      internal_chat_message_metadata_passthrough?: InternalChatMessageMetadataPassthrough | null;
      type: "context_compaction";
    }
  | { type: "other" };

export type ResponseUsageMetadata = {
  amount?: string | null;
  metadata?: unknown;
};

export type ResponsesApiWebSearchAction =
  | { queries?: Array<string> | null; query?: string | null; type: "search" }
  | { type: "open_page"; url?: string | null }
  | { pattern?: string | null; type: "find_in_page"; url?: string | null }
  | { type: "other" };

export type ReviewDelivery = "inline";

export type ReviewStartParams = {
  delivery?: ReviewDelivery | null;
  target: ReviewTarget;
  threadId: string;
};

export type ReviewStartResponse = { reviewThreadId: string; turn: Turn };

export type ReviewTarget =
  | { type: "uncommittedChanges" }
  | { branch: string; type: "baseBranch" }
  | { sha: string; title?: string | null; type: "commit" }
  | { instructions: string; type: "custom" };

export type RolloutCompressResponse = Record<string, unknown>;

export type SandboxMode =
  | "read-only"
  | "workspace-write"
  | "danger-full-access";

export type SandboxPolicy =
  | { type: "dangerFullAccess" }
  | { networkAccess?: boolean; type: "readOnly" }
  | { networkAccess?: NetworkAccess; type: "externalSandbox" }
  | {
      excludeSlashTmp?: boolean;
      excludeTmpdirEnvVar?: boolean;
      networkAccess?: boolean;
      type: "workspaceWrite";
      writableRoots?: Array<AbsolutePathBuf>;
    };

export type SandboxWorkspaceWrite = {
  exclude_slash_tmp?: boolean;
  exclude_tmpdir_env_var?: boolean;
  network_access?: boolean;
  writable_roots?: Array<string>;
};

export type ScheduledTaskSchedule =
  | {
      days?: Array<ScheduledTaskWeekday> | null;
      intervalHours: number;
      type: "hourly";
    }
  | { time: string; type: "daily" }
  | { time: string; type: "weekdays" }
  | { days: Array<ScheduledTaskWeekday>; time: string; type: "weekly" };

export type ScheduledTaskSummary = {
  key: string;
  name: string;
  prompt: string;
  schedule: ScheduledTaskSchedule;
};

export type ScheduledTaskWeekday =
  | "MO"
  | "TU"
  | "WE"
  | "TH"
  | "FR"
  | "SA"
  | "SU";

export type SelectedCapabilityRoot = {
  id: string;
  location: CapabilityRootLocation;
};

export type SendAddCreditsNudgeEmailParams = {
  creditType: AddCreditsNudgeCreditType;
};

export type SendAddCreditsNudgeEmailResponse = {
  status: AddCreditsNudgeEmailStatus;
};

export type ServerDiagnosticsGauge = { name: string; value: number };

export type ServerDiagnosticsParams = Record<string, unknown>;

export type ServerDiagnosticsProcess = {
  id: number;
  physicalFootprintBytes?: number | null;
  residentMemoryBytes?: number | null;
};

export type ServerDiagnosticsResponse = {
  gauges: Array<ServerDiagnosticsGauge>;
  process: ServerDiagnosticsProcess;
};

export type ServerRequestResolvedNotification = {
  requestId: RequestId;
  threadId: string;
};

export type SessionMigration = {
  cwd: string;
  path: string;
  title?: string | null;
};

export type SessionSource =
  | ("cli" | "vscode" | "exec" | "appServer" | "unknown")
  | { custom: string }
  | { subAgent: SubAgentSource };

export type Settings = {
  developer_instructions?: string | null;
  model: string;
  reasoning_effort?: ReasoningEffort | null;
};

export type SkillDependencies = { tools: Array<SkillToolDependency> };

export type SkillErrorInfo = { message: string; path: string };

export type SkillInterface = {
  brandColor?: string | null;
  defaultPrompt?: string | null;
  displayName?: string | null;
  iconLarge?: AbsolutePathBuf | null;
  iconLargeUrl?: string | null;
  iconSmall?: AbsolutePathBuf | null;
  iconSmallUrl?: string | null;
  shortDescription?: string | null;
};

export type SkillMetadata = {
  dependencies?: SkillDependencies | null;
  description: string;
  enabled: boolean;
  interface?: SkillInterface | null;
  name: string;
  path: LegacyAppPathString;
  pluginId?: string | null;
  scope: SkillScope;
  shortDescription?: string | null;
};

export type SkillMigration = { name: string };

export type SkillScope = "user" | "repo" | "system" | "admin";

export type SkillSummary = {
  description: string;
  enabled: boolean;
  interface?: SkillInterface | null;
  name: string;
  path?: LegacyAppPathString | null;
  shortDescription?: string | null;
};

export type SkillToolDependency = {
  command?: string | null;
  description?: string | null;
  transport?: string | null;
  type: string;
  url?: string | null;
  value: string;
};

export type SkillsChangedNotification = Record<string, unknown>;

export type SkillsConfigWriteParams = {
  enabled: boolean;
  name?: string | null;
  path?: AbsolutePathBuf | null;
};

export type SkillsConfigWriteResponse = { effectiveEnabled: boolean };

export type SkillsExtraRootsSetParams = { extraRoots: Array<AbsolutePathBuf> };

export type SkillsExtraRootsSetResponse = Record<string, unknown>;

export type SkillsListEntry = {
  cwd: string;
  errors: Array<SkillErrorInfo>;
  skills: Array<SkillMetadata>;
};

export type SkillsListParams = { cwds?: Array<string>; forceReload?: boolean };

export type SkillsListResponse = { data: Array<SkillsListEntry> };

export type SortDirection = "asc" | "desc";

export type SpendControlLimitSnapshot = {
  limit: string;
  remainingPercent: number;
  resetsAt: number;
  used: string;
};

export type StrictReviewRequiredNotification = {
  startedAtMs: number;
  threadId: string;
  turnId: string;
};

export type SubAgentActivityKind =
  | "started"
  | "interacted"
  | "interrupted"
  | "completed";

export type SubAgentSource =
  | ("review" | "compact" | "memory_consolidation")
  | {
      thread_spawn: {
        agent_nickname?: string | null;
        agent_path?: AgentPath | null;
        agent_role?: string | null;
        depth: number;
        parent_thread_id: ThreadId;
      };
    }
  | { other: string };

export type SubagentMigration = { name: string };

export type TerminalInteractionNotification = {
  itemId: string;
  processId: string;
  stdin: string;
  threadId: string;
  turnId: string;
};

export type TextElement = { byteRange: ByteRange; placeholder?: string | null };

export type TextPosition = { column: number; line: number };

export type TextRange = { end: TextPosition; start: TextPosition };

export type Thread = {
  agentNickname?: string | null;
  agentRole?: string | null;
  canAcceptDirectInput?: boolean | null;
  cliVersion: string;
  createdAt: number;
  cwd: AbsolutePathBuf;
  daybreakEnabled?: boolean | null;
  environments?: Array<ThreadEnvironment> | null;
  ephemeral: boolean;
  extra?: ThreadExtra | null;
  forkedFromId?: string | null;
  gitInfo?: GitInfo | null;
  historyMode?: ThreadHistoryMode;
  id: string;
  model?: string | null;
  modelProvider: string;
  name?: string | null;
  originator?: string | null;
  parentThreadId?: string | null;
  path?: string | null;
  preview: string;
  projectId: string | null;
  reasoningEffort?: ReasoningEffort | null;
  recencyAt?: number | null;
  section?: ThreadSection | null;
  sectionEnteredAt?: number | null;
  sessionId: string;
  source: SessionSource;
  status: ThreadStatus;
  threadSource?: ThreadSource | null;
  turns: Array<Turn>;
  updatedAt: number;
};

export type ThreadActiveFlag = "waitingOnApproval" | "waitingOnUserInput";

export type ThreadApproveGuardianDeniedActionParams = {
  event: unknown;
  threadId: string;
};

export type ThreadApproveGuardianDeniedActionResponse = Record<string, unknown>;

export type ThreadArchiveParams = { threadId: string };

export type ThreadArchiveResponse = Record<string, unknown>;

export type ThreadArchivedNotification = { threadId: string };

export type ThreadAttachment = {
  attachmentType: string;
  createdAt: number;
  id: string;
  identityKey: string;
  payload: unknown;
};

export type ThreadAttachmentAddOutcome = "created" | "existing";

export type ThreadAttachmentAddParams = {
  attachmentType: string;
  identityKey: string;
  payload: unknown;
  threadId: string;
};

export type ThreadAttachmentAddResponse = {
  attachment: ThreadAttachment;
  outcome: ThreadAttachmentAddOutcome;
};

export type ThreadAttachmentListParams = {
  cursor?: string | null;
  limit?: number | null;
  threadId: string;
};

export type ThreadAttachmentListResponse = {
  data: Array<ThreadAttachment>;
  nextCursor?: string | null;
};

export type ThreadAttachmentOperation = "created" | "deleted";

export type ThreadAttachmentOwner = { archived: boolean; threadId: string };

export type ThreadAttachmentOwnerListParams = {
  archived?: boolean | null;
  attachmentType: string;
  cursor?: string | null;
  identityKey: string;
  limit?: number | null;
};

export type ThreadAttachmentOwnerListResponse = {
  data: Array<ThreadAttachmentOwner>;
  nextCursor?: string | null;
};

export type ThreadAttachmentRemoveParams = {
  attachmentType: string;
  identityKey: string;
  threadId: string;
};

export type ThreadAttachmentRemoveResponse = Record<string, unknown>;

export type ThreadAttachmentUpdatedNotification = {
  attachmentId: string;
  attachmentType: string;
  identityKey: string;
  operation: ThreadAttachmentOperation;
  threadId: string;
};

export type ThreadBackgroundTerminal = {
  command: string;
  cpuPercent?: number | null;
  cwd: LegacyAppPathString;
  itemId: string;
  osPid?: number | null;
  processId: string;
  rssKb?: number | null;
};

export type ThreadBackgroundTerminalsCleanParams = { threadId: string };

export type ThreadBackgroundTerminalsCleanResponse = Record<string, unknown>;

export type ThreadBackgroundTerminalsListParams = {
  cursor?: string | null;
  limit?: number | null;
  threadId: string;
};

export type ThreadBackgroundTerminalsListResponse = {
  data: Array<ThreadBackgroundTerminal>;
  nextCursor?: string | null;
};

export type ThreadBackgroundTerminalsTerminateParams = {
  processId: string;
  threadId: string;
};

export type ThreadBackgroundTerminalsTerminateResponse = {
  terminated: boolean;
};

export type ThreadClosedNotification = { threadId: string };

export type ThreadCompactStartParams = { threadId: string };

export type ThreadCompactStartResponse = Record<string, unknown>;

export type ThreadDecrementElicitationParams = { threadId: string };

export type ThreadDecrementElicitationResponse = {
  count: number;
  paused: boolean;
};

export type ThreadDeleteParams = { threadId: string };

export type ThreadDeleteResponse = Record<string, unknown>;

export type ThreadDeletedNotification = { threadId: string };

export type ThreadEnvironment = {
  cwd: LegacyAppPathString;
  environmentId: string;
  runtimeWorkspaceRoots: Array<LegacyAppPathString>;
};

export type ThreadExtra = Record<string, unknown>;

export type ThreadForkParams = {
  approvalPolicy?: AskForApproval | null;
  approvalsReviewer?: ApprovalsReviewer | null;
  attachToSource?: boolean;
  baseInstructions?: string | null;
  beforeTurnId?: string | null;
  config?: Record<string, unknown> | null;
  cwd?: string | null;
  deferGoalContinuation?: boolean;
  developerInstructions?: string | null;
  ephemeral?: boolean;
  excludeTurns?: boolean;
  experimentalDisablePredictionMcp?: boolean;
  experimentalPredictionMode?: boolean;
  lastTurnId?: string | null;
  model?: string | null;
  modelProvider?: string | null;
  path?: string | null;
  permissions?: string | null;
  runtimeWorkspaceRoots?: Array<AbsolutePathBuf> | null;
  sandbox?: SandboxMode | null;
  serviceTier?: string | null;
  threadId: string;
  threadSource?: ThreadSource | null;
};

export type ThreadForkResponse = {
  activePermissionProfile?: ActivePermissionProfile | null;
  approvalPolicy: AskForApproval;
  approvalsReviewer: ApprovalsReviewer;
  cwd: AbsolutePathBuf;
  disabledPluginIds?: Array<string>;
  instructionSources?: Array<LegacyAppPathString>;
  model: string;
  modelProvider: string;
  multiAgentMode?: MultiAgentMode;
  reasoningEffort?: ReasoningEffort | null;
  runtimeWorkspaceRoots?: Array<AbsolutePathBuf>;
  sandbox: SandboxPolicy;
  serviceTier?: string | null;
  thread: Thread;
};

export type ThreadGoal = {
  createdAt: number;
  objective: string;
  status: ThreadGoalStatus;
  threadId: string;
  timeUsedSeconds: number;
  tokenBudget?: number | null;
  tokensUsed: number;
  updatedAt: number;
};

export type ThreadGoalClearParams = {
  origin?: ThreadGoalMutationOrigin | null;
  threadId: string;
};

export type ThreadGoalClearResponse = { cleared: boolean };

export type ThreadGoalClearedNotification = { threadId: string };

export type ThreadGoalGetParams = { threadId: string };

export type ThreadGoalGetResponse = { goal?: ThreadGoal | null };

export type ThreadGoalMutationOrigin = "user" | "automatic";

export type ThreadGoalSetParams = {
  objective?: string | null;
  origin?: ThreadGoalMutationOrigin | null;
  status?: ThreadGoalStatus | null;
  threadId: string;
  tokenBudget?: number | null;
};

export type ThreadGoalSetResponse = { goal: ThreadGoal };

export type ThreadGoalStatus =
  | "active"
  | "paused"
  | "blocked"
  | "usageLimited"
  | "budgetLimited"
  | "complete";

export type ThreadGoalUpdatedNotification = {
  goal: ThreadGoal;
  threadId: string;
  turnId?: string | null;
};

export type ThreadGuardianAssessment = {
  outcome: GuardianAssessmentOutcome;
  rationale: string;
  riskLevel: GuardianRiskLevel;
  userAuthorization: GuardianUserAuthorization;
};

export type ThreadGuardianReviewParams = { threadId: string };

export type ThreadGuardianReviewResponse = {
  assessment?: ThreadGuardianAssessment | null;
  callId: string;
};

export type ThreadHistoryMode = "legacy" | "paginated";

export type ThreadId = string;

export type ThreadIncrementElicitationParams = { threadId: string };

export type ThreadIncrementElicitationResponse = {
  count: number;
  paused: boolean;
};

export type ThreadInjectItemsParams = {
  items: Array<unknown>;
  threadId: string;
};

export type ThreadInjectItemsResponse = Record<string, unknown>;

export type ThreadItem = (
  | {
      clientId?: string | null;
      content: Array<UserInput>;
      id: string;
      type: "userMessage";
    }
  | { fragments: Array<HookPromptFragment>; id: string; type: "hookPrompt" }
  | {
      delivery?: AgentMessageDelivery | null;
      id: string;
      memoryCitation?: MemoryCitation | null;
      phase?: MessagePhase | null;
      questions?: Array<AsyncUserInputQuestion> | null;
      text: string;
      type: "agentMessage";
    }
  | {
      id: string;
      name: string;
      namespace?: string | null;
      output: FunctionCallOutputBody;
      type: "functionCallOutput";
    }
  | { id: string; text: string; type: "plan" }
  | {
      content?: Array<string>;
      id: string;
      summary?: Array<string>;
      type: "reasoning";
    }
  | {
      aggregatedOutput?: string | null;
      command: string;
      commandActions: Array<CommandAction>;
      cwd: LegacyAppPathString;
      durationMs?: number | null;
      exitCode?: number | null;
      id: string;
      pluginId?: string | null;
      processId?: string | null;
      scriptPath?: string | null;
      source?: CommandExecutionSource;
      status: CommandExecutionStatus;
      type: "commandExecution";
    }
  | {
      changes: Array<FileUpdateChange>;
      id: string;
      status: PatchApplyStatus;
      type: "fileChange";
    }
  | {
      appContext?: McpToolCallAppContext | null;
      arguments: unknown;
      durationMs?: number | null;
      error?: McpToolCallError | null;
      id: string;
      mcpAppResourceUri?: string | null;
      mcpAppUi?: McpAppUi | null;
      pluginId?: string | null;
      readOnlyHint?: boolean | null;
      result?: McpToolCallResult | null;
      server: string;
      status: McpToolCallStatus;
      tool: string;
      type: "mcpToolCall";
    }
  | {
      arguments: unknown;
      contentItems?: Array<DynamicToolCallOutputContentItem> | null;
      durationMs?: number | null;
      id: string;
      namespace?: string | null;
      status: DynamicToolCallStatus;
      success?: boolean | null;
      tool: string;
      type: "dynamicToolCall";
    }
  | {
      agentsStates: Record<string, CollabAgentState>;
      id: string;
      model?: string | null;
      prompt?: string | null;
      reasoningEffort?: ReasoningEffort | null;
      receiverThreadIds: Array<string>;
      senderThreadId: string;
      status: CollabAgentToolCallStatus;
      tool: CollabAgentTool;
      type: "collabAgentToolCall";
    }
  | {
      agentPath: string;
      agentThreadId: string;
      id: string;
      kind: SubAgentActivityKind;
      model?: string | null;
      reasoningEffort?: ReasoningEffort | null;
      type: "subAgentActivity";
    }
  | {
      action?: WebSearchAction | null;
      id: string;
      query: string;
      results?: Array<unknown> | null;
      type: "webSearch";
    }
  | { id: string; path: LegacyAppPathString; type: "imageView" }
  | { durationMs: number; id: string; type: "sleep" }
  | (({ fileId: string } | { result: string }) & {
      failure?: ImageGenerationFailure | null;
      generationId?: string | null;
      id: string;
      inputImages?: Array<ImageGenerationInputImage>;
      revisedPrompt?: string | null;
      savedPath?: AbsolutePathBuf | null;
      status: string;
      transparentBackground?: boolean | null;
      type: "imageGeneration";
    })
  | { id: string; review: string; type: "enteredReviewMode" }
  | { id: string; review: string; type: "exitedReviewMode" }
  | { id: string; type: "contextCompaction" }
) & { completedAtMs?: number | null; startedAtMs?: number | null };

export type ThreadItemEntry = {
  completedAtMs?: number | null;
  item: ThreadItem;
  startedAtMs?: number | null;
  turnId: string;
};

export type ThreadItemsListAnchor = { itemId: string; type: "item" };

export type ThreadItemsListCursor = string | ThreadItemsListAnchor;

export type ThreadItemsListParams = {
  cursor?: ThreadItemsListCursor | null;
  limit?: number | null;
  sortDirection?: SortDirection | null;
  threadId: string;
  turnId?: string | null;
};

export type ThreadItemsListResponse = {
  backwardsCursor?: string | null;
  data: Array<ThreadItemEntry>;
  nextCursor?: string | null;
};

export type ThreadItemsReadParams = {
  itemIds: Array<string>;
  threadId: string;
  turnId: string;
};

export type ThreadItemsReadResponse = { data: Array<ThreadItemEntry> };

export type ThreadListCwdFilter = string | Array<string>;

export type ThreadListParams = {
  ancestorThreadId?: string | null;
  archived?: boolean | null;
  attachedToThreadId?: string | null;
  cursor?: string | null;
  cwd?: ThreadListCwdFilter | null;
  excludedThreadIds?: Array<string> | null;
  limit?: number | null;
  modelProviders?: Array<string> | null;
  originators?: Array<string> | null;
  parentThreadId?: string | null;
  projectId?: string | null;
  searchTerm?: string | null;
  sectionId?: string | null;
  sortDirection?: SortDirection | null;
  sortKey?: ThreadSortKey | null;
  sourceKinds?: Array<ThreadSourceKind> | null;
  useStateDbOnly?: boolean;
};

export type ThreadListResponse = {
  backwardsCursor?: string | null;
  data: Array<Thread>;
  nextCursor?: string | null;
  readStates?: Record<string, ThreadReadState> | null;
};

export type ThreadLoadedListParams = {
  cursor?: string | null;
  limit?: number | null;
};

export type ThreadLoadedListResponse = {
  data: Array<string>;
  nextCursor?: string | null;
};

export type ThreadMemoryMode = "enabled" | "disabled";

export type ThreadMemoryModeSetParams = {
  mode: ThreadMemoryMode;
  threadId: string;
};

export type ThreadMemoryModeSetResponse = Record<string, unknown>;

export type ThreadMemoryReadParams = { threadId: string };

export type ThreadMemoryReadResponse = { summary?: string | null };

export type ThreadMetadataGitInfoUpdateParams = {
  branch?: string | null;
  originUrl?: string | null;
  sha?: string | null;
};

export type ThreadMetadataUpdateParams = {
  daybreakEnabled?: boolean | null;
  gitInfo?: ThreadMetadataGitInfoUpdateParams | null;
  projectId?: string | null;
  threadId: string;
};

export type ThreadMetadataUpdateResponse = { thread: Thread };

export type ThreadNameUpdatedNotification = {
  threadId: string;
  threadName?: string | null;
};

export type ThreadPredictionRequestParams = {
  sourceTurnId: string;
  threadId: string;
};

export type ThreadPredictionRequestResponse = Record<string, unknown>;

export type ThreadPredictionResult =
  | { text?: string | null; type: "completed" }
  | { type: "failed" };

export type ThreadPredictionUpdatedNotification = {
  result: ThreadPredictionResult;
  sourceTurnId: string;
  threadId: string;
};

export type ThreadProjectUpdatedNotification = {
  projectId: string | null;
  threadId: string;
};

export type ThreadQueueAddParams = {
  clientUserMessageId: string;
  input: Array<UserInput>;
  threadId: string;
};

export type ThreadQueueAddResponse = { queuedSubmission: QueuedSubmission };

export type ThreadQueueChangedNotification = { threadId: string };

export type ThreadQueueDeleteParams = {
  queuedSubmissionId: string;
  threadId: string;
};

export type ThreadQueueDeleteResponse = { deleted: boolean };

export type ThreadQueueListParams = {
  cursor?: string | null;
  limit?: number | null;
  threadId: string;
};

export type ThreadQueueListResponse = {
  data: Array<QueuedSubmission>;
  nextCursor?: string | null;
};

export type ThreadQueueReorderParams = {
  queuedSubmissionIds: Array<string>;
  threadId: string;
};

export type ThreadQueueReorderResponse = Record<string, unknown>;

export type ThreadQueueStartParams = {
  queuedSubmissionId?: string | null;
  threadId: string;
};

export type ThreadQueueStartResponse = { turn: Turn };

export type ThreadQueueUpdateParams = {
  input: Array<UserInput>;
  queuedSubmissionId: string;
  threadId: string;
};

export type ThreadQueueUpdateResponse = { queuedSubmission: QueuedSubmission };

export type ThreadReadParams = { includeTurns?: boolean; threadId: string };

export type ThreadReadResponse = {
  readState?: ThreadReadState | null;
  thread: Thread;
};

export type ThreadReadState = {
  firstUnread?: ThreadUnreadPosition | null;
  revision: string;
};

export type ThreadReadStateChangedNotification = {
  readState: ThreadReadState;
  threadId: string;
};

export type ThreadReadStateOperation = { type: "read" } | { type: "unread" };

export type ThreadReadStateUpdateParams = {
  expectedRevision: string;
  operation: ThreadReadStateOperation;
  threadId: string;
};

export type ThreadReadStateUpdateResponse = { readState: ThreadReadState };

export type ThreadRealtimeAppendAudioParams = {
  audio: ThreadRealtimeAudioChunk;
  threadId: string;
};

export type ThreadRealtimeAppendAudioResponse = Record<string, unknown>;

export type ThreadRealtimeAppendSpeechParams = {
  text: string;
  threadId: string;
};

export type ThreadRealtimeAppendSpeechResponse = Record<string, unknown>;

export type ThreadRealtimeAppendTextParams = {
  role?: ConversationTextRole;
  text: string;
  threadId: string;
};

export type ThreadRealtimeAppendTextResponse = Record<string, unknown>;

export type ThreadRealtimeAudioChunk = {
  data: string;
  itemId?: string | null;
  numChannels: number;
  sampleRate: number;
  samplesPerChannel?: number | null;
};

export type ThreadRealtimeBemItemPresentation =
  | { type: "wholeItem" }
  | { type: "inlineMarkdown" }
  | { index: number; type: "inlineVisualization" };

export type ThreadRealtimeClosedNotification = {
  reason?: string | null;
  threadId: string;
};

export type ThreadRealtimeErrorNotification = {
  message: string;
  threadId: string;
};

export type ThreadRealtimeInitialItem = {
  role: ConversationTextRole;
  text: string;
};

export type ThreadRealtimeItem = (
  | { type: "realtimeSessionStarted" }
  | {
      role: ThreadRealtimeTranscriptRole;
      text: string;
      type: "transcriptSegment";
    }
  | {
      item_id: string;
      presentation: ThreadRealtimeBemItemPresentation;
      turn_id: string;
      type: "bemItemPromoted";
    }
  | { outcome: ThreadRealtimeSessionOutcome; type: "realtimeSessionClosed" }
) & { id: string; realtimeSessionId: string };

export type ThreadRealtimeItemAddedNotification = {
  item: unknown;
  threadId: string;
};

export type ThreadRealtimeItemCompletedNotification = {
  item: ThreadRealtimeItem;
  threadId: string;
};

export type ThreadRealtimeItemStartedNotification = {
  item: ThreadRealtimeItem;
  threadId: string;
};

export type ThreadRealtimeItemTranscriptDeltaNotification = {
  delta: string;
  itemId: string;
  threadId: string;
};

export type ThreadRealtimeListVoicesParams = Record<string, unknown>;

export type ThreadRealtimeListVoicesResponse = { voices: RealtimeVoicesList };

export type ThreadRealtimeOutputAudioDeltaNotification = {
  audio: ThreadRealtimeAudioChunk;
  threadId: string;
};

export type ThreadRealtimeSdpNotification = { sdp: string; threadId: string };

export type ThreadRealtimeSessionOutcome = "ended" | "failed";

export type ThreadRealtimeStartParams = {
  backendReasoningStatus?: boolean;
  clientManagedHandoffs?: boolean | null;
  codexResponseHandoffChannelPrefixes?: Record<string, Array<string>> | null;
  codexResponseHandoffMode?: CodexResponseHandoffMode | null;
  codexResponseItemPrefix?: string | null;
  codexResponsesAsItems?: boolean | null;
  delegationAckFiller?: boolean | null;
  flushTranscriptTailOnSessionEnd?: boolean | null;
  includeStartupContext?: boolean | null;
  initialItems?: Array<ThreadRealtimeInitialItem> | null;
  model?: string | null;
  outputModality: RealtimeOutputModality;
  prompt?: string | null;
  realtimeEndInstructions?: string | null;
  realtimeSessionId?: string | null;
  realtimeStartInstructions?: string | null;
  threadId: string;
  transport?: ThreadRealtimeStartTransport | null;
  version?: RealtimeConversationVersion | null;
  voice?: RealtimeVoice | null;
  voiceSessionId?: string | null;
};

export type ThreadRealtimeStartResponse = Record<string, unknown>;

export type ThreadRealtimeStartTransport =
  | { type: "websocket" }
  | { sdp: string; type: "webrtc" }
  | { callId: string; type: "existingCall" };

export type ThreadRealtimeStartedNotification = {
  realtimeSessionId?: string | null;
  threadId: string;
  version: RealtimeConversationVersion;
};

export type ThreadRealtimeStopParams = { threadId: string };

export type ThreadRealtimeStopResponse = Record<string, unknown>;

export type ThreadRealtimeTranscriptDeltaNotification = {
  delta: string;
  role: string;
  threadId: string;
};

export type ThreadRealtimeTranscriptDoneNotification = {
  role: string;
  text: string;
  threadId: string;
};

export type ThreadRealtimeTranscriptRole = "user" | "assistant";

export type ThreadResumeInitialTurnsPageParams = {
  itemsView?: TurnItemsView | null;
  limit?: number | null;
  sortDirection?: SortDirection | null;
};

export type ThreadResumeParams = {
  approvalPolicy?: AskForApproval | null;
  approvalsReviewer?: ApprovalsReviewer | null;
  baseInstructions?: string | null;
  config?: Record<string, unknown> | null;
  cwd?: string | null;
  developerInstructions?: string | null;
  excludeTurns?: boolean;
  history?: Array<ResponseItem> | null;
  initialTurnsPage?: ThreadResumeInitialTurnsPageParams | null;
  model?: string | null;
  modelProvider?: string | null;
  path?: string | null;
  permissions?: string | null;
  personality?: Personality | null;
  runtimeWorkspaceRoots?: Array<AbsolutePathBuf> | null;
  sandbox?: SandboxMode | null;
  serviceTier?: string | null;
  threadId: string;
};

export type ThreadResumeResponse = {
  activePermissionProfile?: ActivePermissionProfile | null;
  approvalPolicy: AskForApproval;
  approvalsReviewer: ApprovalsReviewer;
  collaborationMode?: CollaborationMode | null;
  cwd: AbsolutePathBuf;
  disabledPluginIds?: Array<string>;
  initialTurnsPage?: TurnsPage | null;
  instructionSources?: Array<LegacyAppPathString>;
  itemsBackwardsCursor?: string | null;
  model: string;
  modelProvider: string;
  multiAgentMode?: MultiAgentMode;
  reasoningEffort?: ReasoningEffort | null;
  runtimeWorkspaceRoots?: Array<AbsolutePathBuf>;
  sandbox: SandboxPolicy;
  serviceTier?: string | null;
  thread: Thread;
  turnsBackwardsCursor?: string | null;
};

export type ThreadRevertParams = { beforeTurnId: string; threadId: string };

export type ThreadRevertResponse = {
  itemsBackwardsCursor?: string | null;
  thread: Thread;
  turnsBackwardsCursor?: string | null;
};

export type ThreadRevertedNotification = { threadId: string };

export type ThreadSampleCancelParams = { requestId: RequestId };

export type ThreadSampleCancelResponse = Record<string, unknown>;

export type ThreadSampleParams = {
  disableMcp?: boolean;
  effort?: ReasoningEffort | null;
  expectedTurnId: string;
  interruptOnCancel?: boolean | null;
  prompt: string;
  responsesapiClientMetadata?: Record<string, string> | null;
  threadId: string;
  threadSource?: ThreadSource | null;
  turnTrigger?: string | null;
};

export type ThreadSampleResponse = {
  cachedInputTokens?: number | null;
  inputTokens?: number | null;
  outputTokens?: number | null;
  responseId: string;
  sourceTurnId: string;
  text?: string | null;
};

export type ThreadSearchOccurrence = {
  itemId: string;
  snippet: string;
  snippetMatchRange: ThreadSearchTextRange;
  turnCursor: string;
  turnId: string;
};

export type ThreadSearchOccurrencesParams = {
  cursor?: string | null;
  limit?: number | null;
  searchTerm: string;
  threadId: string;
};

export type ThreadSearchOccurrencesResponse = {
  data: Array<ThreadSearchOccurrence>;
  nextCursor?: string | null;
};

export type ThreadSearchParams = {
  archived?: boolean | null;
  cursor?: string | null;
  limit?: number | null;
  searchTerm: string;
  sortDirection?: SortDirection | null;
  sortKey?: ThreadSearchSortKey | null;
  sourceKinds?: Array<ThreadSourceKind> | null;
};

export type ThreadSearchResponse = {
  backwardsCursor?: string | null;
  data: Array<ThreadSearchResult>;
  nextCursor?: string | null;
};

export type ThreadSearchResult = { snippet: string; thread: Thread };

export type ThreadSearchSortKey = "created_at" | "updated_at" | "recency_at";

export type ThreadSearchTextRange = { end: number; start: number };

export type ThreadSection = {
  appearance?: ThreadSectionAppearance | null;
  id: string;
  name: string;
};

export type ThreadSectionAppearance = {
  color?: string | null;
  icon?: string | null;
};

export type ThreadSectionChangedNotification = {
  sectionId: string | null;
  threadId: string;
};

export type ThreadSectionCreateParams = {
  appearance?: ThreadSectionAppearance | null;
  name: string;
};

export type ThreadSectionCreateResponse = { section: ThreadSection };

export type ThreadSectionDeleteParams = { sectionId: string };

export type ThreadSectionDeleteResponse = Record<string, unknown>;

export type ThreadSectionListParams = {
  cursor?: string | null;
  limit?: number | null;
};

export type ThreadSectionListResponse = {
  data: Array<ThreadSection>;
  nextCursor?: string | null;
};

export type ThreadSectionMoveParams = {
  beforeThreadId?: string | null;
  sectionId: string | null;
  threadId: string;
};

export type ThreadSectionMoveResponse = Record<string, unknown>;

export type ThreadSectionUpdateParams = {
  appearance?: ThreadSectionAppearance | null;
  name: string;
  sectionId: string;
};

export type ThreadSectionUpdateResponse = { section: ThreadSection };

export type ThreadSetNameParams = { name: string; threadId: string };

export type ThreadSetNameResponse = Record<string, unknown>;

export type ThreadSettings = {
  activePermissionProfile?: ActivePermissionProfile | null;
  approvalPolicy: AskForApproval;
  approvalsReviewer: ApprovalsReviewer;
  collaborationMode: CollaborationMode;
  cwd: AbsolutePathBuf;
  disabledPluginIds?: Array<string>;
  effort?: ReasoningEffort | null;
  model: string;
  modelProvider: string;
  multiAgentMode?: MultiAgentMode;
  personality?: Personality | null;
  sandboxPolicy: SandboxPolicy;
  serviceTier?: string | null;
  summary?: ReasoningSummary | null;
};

export type ThreadSettingsUpdateParams = {
  approvalPolicy?: AskForApproval | null;
  approvalsReviewer?: ApprovalsReviewer | null;
  collaborationMode?: CollaborationMode | null;
  cwd?: string | null;
  disabledPluginIds?: Array<string> | null;
  effort?: ReasoningEffort | null;
  model?: string | null;
  multiAgentMode?: MultiAgentMode | null;
  permissions?: string | null;
  personality?: Personality | null;
  sandboxPolicy?: SandboxPolicy | null;
  serviceTier?: string | null;
  summary?: ReasoningSummary | null;
  threadId: string;
};

export type ThreadSettingsUpdateResponse = Record<string, unknown>;

export type ThreadSettingsUpdatedNotification = {
  threadId: string;
  threadSettings: ThreadSettings;
};

export type ThreadShellCommandParams = {
  command: string;
  threadId: string;
  timeoutMs?: number | null;
};

export type ThreadShellCommandResponse = Record<string, unknown>;

export type ThreadSortKey =
  | "created_at"
  | "updated_at"
  | "recency_at"
  | "section_position";

export type ThreadSource = string;

export type ThreadSourceKind =
  | "cli"
  | "vscode"
  | "exec"
  | "appServer"
  | "subAgent"
  | "subAgentReview"
  | "subAgentCompact"
  | "subAgentThreadSpawn"
  | "subAgentOther"
  | "unknown";

export type ThreadStartParams = {
  allowProviderModelFallback?: boolean;
  approvalPolicy?: AskForApproval | null;
  approvalsReviewer?: ApprovalsReviewer | null;
  baseInstructions?: string | null;
  config?: Record<string, unknown> | null;
  cwd?: string | null;
  daybreakEnabled?: boolean | null;
  developerInstructions?: string | null;
  dynamicTools?: Array<DynamicToolSpec> | null;
  environments?: Array<TurnEnvironmentParams> | null;
  ephemeral?: boolean | null;
  experimentalRawEvents?: boolean;
  historyMode?: ThreadHistoryMode | null;
  mockExperimentalField?: string | null;
  model?: string | null;
  modelProvider?: string | null;
  multiAgentMode?: MultiAgentMode | null;
  permissions?: string | null;
  personality?: Personality | null;
  projectId?: string | null;
  runtimeWorkspaceRoots?: Array<AbsolutePathBuf> | null;
  sandbox?: SandboxMode | null;
  selectedCapabilityRoots?: Array<SelectedCapabilityRoot> | null;
  serviceName?: string | null;
  serviceTier?: string | null;
  sessionStartSource?: ThreadStartSource | null;
  threadSource?: ThreadSource | null;
};

export type ThreadStartResponse = {
  activePermissionProfile?: ActivePermissionProfile | null;
  approvalPolicy: AskForApproval;
  approvalsReviewer: ApprovalsReviewer;
  cwd: AbsolutePathBuf;
  disabledPluginIds?: Array<string>;
  instructionSources?: Array<LegacyAppPathString>;
  model: string;
  modelProvider: string;
  multiAgentMode?: MultiAgentMode;
  reasoningEffort?: ReasoningEffort | null;
  runtimeWorkspaceRoots?: Array<AbsolutePathBuf>;
  sandbox: SandboxPolicy;
  serviceTier?: string | null;
  thread: Thread;
};

export type ThreadStartSource = "startup" | "clear";

export type ThreadStartedNotification = { thread: Thread };

export type ThreadStatus =
  | { type: "notLoaded" }
  | { type: "idle" }
  | { type: "systemError" }
  | { activeFlags: Array<ThreadActiveFlag>; type: "active" };

export type ThreadStatusChangedNotification = {
  status: ThreadStatus;
  threadId: string;
};

export type ThreadTimelineEntry =
  | { item: ThreadItem; position: number; turnId: string; type: "item" }
  | { item: ThreadRealtimeItem; position: number; type: "realtime" }
  | {
      position: number;
      started_at?: number | null;
      turn_id: string;
      type: "turnStarted";
    }
  | {
      completed_at?: number | null;
      duration_ms?: number | null;
      error?: TurnError | null;
      position: number;
      started_at?: number | null;
      status: TurnStatus;
      turn_id: string;
      type: "turnCompleted";
    };

export type ThreadTimelineListParams = {
  cursor?: string | null;
  limit?: number | null;
  threadId: string;
};

export type ThreadTimelineListResponse = {
  activeRealtimeSessionAtPageStart?: string | null;
  data: Array<ThreadTimelineEntry>;
  nextCursor?: string | null;
};

export type ThreadTokenUsage = {
  last: TokenUsageBreakdown;
  modelContextWindow?: number | null;
  total: TokenUsageBreakdown;
};

export type ThreadTokenUsageUpdatedNotification = {
  threadId: string;
  tokenUsage: ThreadTokenUsage;
  turnId: string;
};

export type ThreadTurnsListParams = {
  cursor?: string | null;
  itemsView?: TurnItemsView | null;
  limit?: number | null;
  sortDirection?: SortDirection | null;
  threadId: string;
};

export type ThreadTurnsListResponse = {
  backwardsCursor?: string | null;
  data: Array<Turn>;
  nextCursor?: string | null;
};

export type ThreadUnarchiveParams = { threadId: string };

export type ThreadUnarchiveResponse = { thread: Thread };

export type ThreadUnarchivedNotification = { threadId: string };

export type ThreadUnreadPosition =
  | { type: "threadStart" }
  | { turnId: string; type: "turn" };

export type ThreadUnsubscribeParams = { threadId: string };

export type ThreadUnsubscribeResponse = { status: ThreadUnsubscribeStatus };

export type ThreadUnsubscribeStatus =
  | "notLoaded"
  | "notSubscribed"
  | "unsubscribed";

export type ThreadUsage = {
  estimatedUsageCreditsMicros: number;
  estimatedUsageUsdMicros?: number | null;
  groups: Array<ThreadUsageBreakdownGroup>;
  nativeUsageUsdMicros?: number | null;
  threadId: string;
};

export type ThreadUsageBreakdownGroup = {
  cachedInputTokens?: number | null;
  estimatedUsageCreditsMicros: number;
  inputTokens?: number | null;
  model?: string | null;
  netNewInputTokens?: number | null;
  outputTokens?: number | null;
  reasoningEffort?: string | null;
  speed?: string | null;
  totalTokens?: number | null;
};

export type TokenUsageBreakdown = {
  cacheWriteInputTokens?: number;
  cachedInputTokens: number;
  inputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
};

export type Tool = {
  _meta?: unknown;
  annotations?: unknown;
  description?: string | null;
  icons?: Array<unknown> | null;
  inputSchema: unknown;
  name: string;
  outputSchema?: unknown;
  title?: string | null;
};

export type ToolExposureSurface = "code_mode" | "deferred" | "direct";

export type ToolsV2 = { web_search?: WebSearchToolConfig | null };

export type Turn = {
  completedAt?: number | null;
  durationMs?: number | null;
  error?: TurnError | null;
  id: string;
  items: Array<ThreadItem>;
  itemsView?: TurnItemsView;
  rootTurnId?: string | null;
  startedAt?: number | null;
  status: TurnStatus;
};

export type TurnCompletedNotification = { threadId: string; turn: Turn };

export type TurnDiffUpdatedNotification = {
  diff: string;
  threadId: string;
  turnId: string;
};

export type TurnEnvironmentParams = {
  cwd: LegacyAppPathString;
  environmentId: string;
  runtimeWorkspaceRoots?: Array<LegacyAppPathString> | null;
};

export type TurnError = {
  additionalDetails?: string | null;
  codexErrorInfo?: CodexErrorInfo | null;
  message: string;
  misalignment?: MisalignmentErrorDetails | null;
};

export type TurnInterruptParams = { threadId: string; turnId: string };

export type TurnInterruptResponse = Record<string, unknown>;

export type TurnItemsView = "notLoaded" | "summary" | "full";

export type TurnModerationMetadataNotification = {
  metadata: unknown;
  threadId: string;
  turnId: string;
};

export type TurnPlanStep = { status: TurnPlanStepStatus; step: string };

export type TurnPlanStepStatus = "pending" | "inProgress" | "completed";

export type TurnPlanUpdatedNotification = {
  explanation?: string | null;
  plan: Array<TurnPlanStep>;
  threadId: string;
  turnId: string;
};

export type TurnSettingsUpdateParams = {
  approvalsReviewer?: ApprovalsReviewer | null;
  effort?: ReasoningEffort | null;
  model?: string | null;
  serviceTier?: string | null;
  summary?: ReasoningSummary | null;
  threadId: string;
  turnId: string;
};

export type TurnSettingsUpdateResponse = { status: TurnSettingsUpdateStatus };

export type TurnSettingsUpdateStatus = "applied" | "targetUnavailable";

export type TurnStartParams = {
  additionalContext?: Record<string, AdditionalContextEntry> | null;
  approvalPolicy?: AskForApproval | null;
  approvalsReviewer?: ApprovalsReviewer | null;
  clientUserMessageId?: string | null;
  collaborationMode?: CollaborationMode | null;
  cwd?: string | null;
  cyberAccessProgram?: CyberAccessProgram | null;
  disabledPluginIds?: Array<string> | null;
  effort?: ReasoningEffort | null;
  environments?: Array<TurnEnvironmentParams> | null;
  input: Array<UserInput>;
  model?: string | null;
  multiAgentMode?: MultiAgentMode | null;
  outputSchema?: unknown;
  parentTurnId?: string | null;
  permissions?: string | null;
  personality?: Personality | null;
  responsesapiClientMetadata?: Record<string, string> | null;
  responsesapiClientMetadataPatch?: Record<string, string> | null;
  rootTurnId?: string | null;
  runtimeWorkspaceRoots?: Array<AbsolutePathBuf> | null;
  sandboxPolicy?: SandboxPolicy | null;
  serviceTier?: string | null;
  serviceTierForTurn?: string | null;
  summary?: ReasoningSummary | null;
  threadId: string;
  toolOutput?: TurnToolOutput | null;
  turnTrigger?: string | null;
};

export type TurnStartResponse = { turn: Turn };

export type TurnStartedNotification = { threadId: string; turn: Turn };

export type TurnStatus = "completed" | "interrupted" | "failed" | "inProgress";

export type TurnSteerParams = {
  additionalContext?: Record<string, AdditionalContextEntry> | null;
  clientUserMessageId?: string | null;
  expectedTurnId: string;
  input: Array<UserInput>;
  responsesapiClientMetadata?: Record<string, string> | null;
  responsesapiClientMetadataPatch?: Record<string, string> | null;
  threadId: string;
};

export type TurnSteerResponse = { turnId: string };

export type TurnToolOutput = {
  name: string;
  namespace?: string | null;
  output: FunctionCallOutputBody;
  retain?: boolean;
};

export type TurnsPage = {
  backwardsCursor?: string | null;
  data: Array<Turn>;
  nextCursor?: string | null;
};

export type UserInput =
  | { text: string; text_elements?: Array<TextElement>; type: "text" }
  | (({ url: string } | { fileId: string }) & {
      detail?: ImageDetail | null;
      type: "image";
    })
  | { detail?: ImageDetail | null; path: string; type: "localImage" }
  | (({ url: string } | { fileId: string }) & { type: "audio" })
  | { path: string; type: "localAudio" }
  | { name: string; path: string; type: "skill" }
  | { name: string; path: string; type: "mention" };

export type UserVerificationCancelParams = { requestId: RequestId };

export type UserVerificationCancelResponse = Record<string, unknown>;

export type UserVerificationCancellationReason =
  | "userCancelled"
  | "interrupted";

export type UserVerificationDeleteParams = Record<string, never>;

export type UserVerificationDeleteResponse = Record<string, unknown>;

export type UserVerificationEnrollParams = Record<string, never>;

export type UserVerificationEnrollResponse = {
  algorithm?: string | null;
  credentialId: string;
  publicKey?: string | null;
};

export type UserVerificationErrorDetails =
  | { reason: UserVerificationInvalidRequestReason; type: "invalidRequest" }
  | { reason: UserVerificationUnavailableReason; type: "unavailable" }
  | { reason: UserVerificationCancellationReason; type: "cancelled" }
  | { reason: UserVerificationFailureReason; type: "failed" };

export type UserVerificationFailureReason =
  | "authenticationFailed"
  | "timeout"
  | "providerError"
  | "serviceError";

export type UserVerificationInvalidRequestReason = "invalidParams";

export type UserVerificationProof = { credentialId: string; signature: string };

export type UserVerificationRpcError = {
  code: number;
  data: UserVerificationErrorDetails;
  message: string;
};

export type UserVerificationStatusParams = Record<string, never>;

export type UserVerificationStatusResponse = {
  credentialId?: string | null;
  unavailableMessage?: string | null;
  unavailableReason?: UserVerificationUnavailableReason | null;
};

export type UserVerificationUnavailableReason =
  | "credentialMissing"
  | "biometricsUnavailable"
  | "providerUnavailable";

export type UserVerificationVerifyParams = {
  challenge: string;
  description: string;
  title: string;
};

export type UserVerificationVerifyResponse = { proof: UserVerificationProof };

export type Verbosity = "low" | "medium" | "high";

export type WarningNotification = { message: string; threadId?: string | null };

export type WebSearchAction =
  | { queries?: Array<string> | null; query?: string | null; type: "search" }
  | { type: "openPage"; url?: string | null }
  | { pattern?: string | null; type: "findInPage"; url?: string | null }
  | { type: "other" };

export type WebSearchContextSize = "low" | "medium" | "high";

export type WebSearchLocation = {
  city?: string | null;
  country?: string | null;
  region?: string | null;
  timezone?: string | null;
};

export type WebSearchMode = "disabled" | "cached" | "indexed" | "live";

export type WebSearchToolConfig = {
  allowed_domains?: Array<string> | null;
  context_size?: WebSearchContextSize | null;
  location?: WebSearchLocation | null;
};

export type WindowsSandboxImplementation = "elevated" | "unelevated" | "mxc";

export type WindowsSandboxReadiness =
  | "ready"
  | "notConfigured"
  | "updateRequired";

export type WindowsSandboxReadinessResponse = {
  status: WindowsSandboxReadiness;
};

export type WindowsSandboxSetupCompletedNotification = {
  error?: string | null;
  mode: WindowsSandboxSetupMode;
  success: boolean;
};

export type WindowsSandboxSetupMode = "elevated" | "unelevated";

export type WindowsSandboxSetupStartParams = {
  cwd?: AbsolutePathBuf | null;
  mode: WindowsSandboxSetupMode;
};

export type WindowsSandboxSetupStartResponse = { started: boolean };

export type WindowsWorldWritableWarningNotification = {
  extraCount: number;
  failedScan: boolean;
  samplePaths: Array<string>;
};

export type WorkspaceMessage = {
  archivedAt?: number | null;
  createdAt?: number | null;
  messageBody: string;
  messageId: string;
  messageType: WorkspaceMessageType;
};

export type WorkspaceMessageType = "headline" | "announcement" | "unknown";

export type WorkspaceRouting = {
  accountRoutingOverride: AccountRoutingOverride;
  backendOrigin: string;
  chatgptAccountId: string;
};

export type WriteStatus = "ok" | "okOverridden";

export type ClientRequestMethod =
  | "initialize"
  | "server/diagnostics"
  | "userVerification/status"
  | "userVerification/enroll"
  | "userVerification/delete"
  | "userVerification/verify"
  | "userVerification/cancel"
  | "thread/start"
  | "thread/resume"
  | "thread/fork"
  | "thread/sample"
  | "thread/sample/cancel"
  | "thread/archive"
  | "thread/delete"
  | "thread/unsubscribe"
  | "thread/increment_elicitation"
  | "thread/decrement_elicitation"
  | "thread/name/set"
  | "thread/prediction/request"
  | "thread/goal/set"
  | "thread/goal/get"
  | "thread/goal/clear"
  | "thread/queue/add"
  | "thread/queue/list"
  | "thread/queue/update"
  | "thread/queue/delete"
  | "thread/queue/reorder"
  | "thread/queue/start"
  | "thread/metadata/update"
  | "thread/attachment/add"
  | "thread/attachment/list"
  | "thread/attachmentOwner/list"
  | "thread/attachment/remove"
  | "thread/section/move"
  | "thread/settings/update"
  | "thread/memoryMode/set"
  | "thread/memory/read"
  | "memory/status"
  | "memory/reset"
  | "rollout/compress"
  | "thread/unarchive"
  | "thread/compact/start"
  | "thread/shellCommand"
  | "thread/approveGuardianDeniedAction"
  | "thread/backgroundTerminals/clean"
  | "thread/backgroundTerminals/list"
  | "thread/backgroundTerminals/terminate"
  | "thread/revert"
  | "thread/list"
  | "project/list"
  | "project/read"
  | "project/create"
  | "project/import"
  | "project/update"
  | "project/move"
  | "project/delete"
  | "threadSection/list"
  | "threadSection/create"
  | "threadSection/update"
  | "threadSection/delete"
  | "thread/search"
  | "thread/searchOccurrences"
  | "thread/loaded/list"
  | "thread/read"
  | "thread/readState/update"
  | "thread/turns/list"
  | "thread/items/list"
  | "thread/items/read"
  | "thread/guardianReview"
  | "thread/inject_items"
  | "skills/list"
  | "skills/extraRoots/set"
  | "hooks/list"
  | "marketplace/add"
  | "marketplace/remove"
  | "marketplace/upgrade"
  | "plugin/list"
  | "plugin/search"
  | "plugin/installed"
  | "plugin/reconcile"
  | "plugin/read"
  | "plugin/skill/read"
  | "plugin/share/save"
  | "plugin/share/updateTargets"
  | "plugin/share/list"
  | "plugin/share/checkout"
  | "plugin/share/delete"
  | "app/read"
  | "app/list"
  | "app/installed"
  | "fs/readFile"
  | "fs/writeFile"
  | "fs/createDirectory"
  | "fs/getMetadata"
  | "fs/readDirectory"
  | "fs/remove"
  | "fs/copy"
  | "fs/watch"
  | "fs/unwatch"
  | "skills/config/write"
  | "plugin/install"
  | "plugin/uninstall"
  | "turn/start"
  | "turn/settings/update"
  | "turn/steer"
  | "turn/interrupt"
  | "thread/realtime/start"
  | "thread/realtime/appendAudio"
  | "thread/realtime/appendText"
  | "thread/realtime/appendSpeech"
  | "thread/realtime/stop"
  | "thread/timeline/list"
  | "thread/realtime/listVoices"
  | "review/start"
  | "model/list"
  | "account/gatewayOAuth/read"
  | "account/gatewayOAuth/login"
  | "account/gatewayOAuth/cancel"
  | "modelProvider/capabilities/read"
  | "experimentalFeature/list"
  | "permissionProfile/list"
  | "experimentalFeature/enablement/set"
  | "remoteControl/enable"
  | "remoteControl/disable"
  | "remoteControl/status/read"
  | "remoteControl/pairing/start"
  | "remoteControl/pairing/status"
  | "remoteControl/client/list"
  | "remoteControl/client/revoke"
  | "collaborationMode/list"
  | "mock/experimentalMethod"
  | "environment/add"
  | "environment/info"
  | "environment/status"
  | "mcpServer/oauth/login"
  | "config/mcpServer/reload"
  | "mcpServerStatus/list"
  | "mcpServer/resource/read"
  | "mcpServer/event/stream/start"
  | "mcpServer/event/stream/stop"
  | "mcpServer/tool/call"
  | "windowsSandbox/setupStart"
  | "windowsSandbox/readiness"
  | "account/login/start"
  | "account/bedrock/discover"
  | "account/bedrock/setup"
  | "account/gcp/setup"
  | "account/bedrock/checkGovCloudRequirements"
  | "account/login/cancel"
  | "accountSession/list"
  | "accountSession/switch"
  | "account/logout"
  | "account/rateLimits/read"
  | "account/rateLimitResetCredit/consume"
  | "account/usage/read"
  | "account/workspaceMessages/read"
  | "account/sendAddCreditsNudgeEmail"
  | "feedback/upload"
  | "feedback/save"
  | "command/exec"
  | "command/exec/write"
  | "command/exec/terminate"
  | "command/exec/resize"
  | "process/spawn"
  | "process/writeStdin"
  | "process/kill"
  | "process/resizePty"
  | "config/read"
  | "externalAgentConfig/detect"
  | "externalAgentConfig/import"
  | "externalAgentConfig/import/recordHistory"
  | "externalAgentConfig/import/readHistories"
  | "config/value/write"
  | "config/batchWrite"
  | "configRequirements/read"
  | "account/read"
  | "fuzzyFileSearch"
  | "fuzzyFileSearch/sessionStart"
  | "fuzzyFileSearch/sessionUpdate"
  | "fuzzyFileSearch/sessionStop";
export type ClientRequestResults = {
  initialize: InitializeResponse;
  "server/diagnostics": ServerDiagnosticsResponse;
  "userVerification/status": UserVerificationStatusResponse;
  "userVerification/enroll": UserVerificationEnrollResponse;
  "userVerification/delete": UserVerificationDeleteResponse;
  "userVerification/verify": UserVerificationVerifyResponse;
  "userVerification/cancel": UserVerificationCancelResponse;
  "thread/start": ThreadStartResponse;
  "thread/resume": ThreadResumeResponse;
  "thread/fork": ThreadForkResponse;
  "thread/sample": ThreadSampleResponse;
  "thread/sample/cancel": ThreadSampleCancelResponse;
  "thread/archive": ThreadArchiveResponse;
  "thread/delete": ThreadDeleteResponse;
  "thread/unsubscribe": ThreadUnsubscribeResponse;
  "thread/increment_elicitation": ThreadIncrementElicitationResponse;
  "thread/decrement_elicitation": ThreadDecrementElicitationResponse;
  "thread/name/set": ThreadSetNameResponse;
  "thread/prediction/request": ThreadPredictionRequestResponse;
  "thread/goal/set": ThreadGoalSetResponse;
  "thread/goal/get": ThreadGoalGetResponse;
  "thread/goal/clear": ThreadGoalClearResponse;
  "thread/queue/add": ThreadQueueAddResponse;
  "thread/queue/list": ThreadQueueListResponse;
  "thread/queue/update": ThreadQueueUpdateResponse;
  "thread/queue/delete": ThreadQueueDeleteResponse;
  "thread/queue/reorder": ThreadQueueReorderResponse;
  "thread/queue/start": ThreadQueueStartResponse;
  "thread/metadata/update": ThreadMetadataUpdateResponse;
  "thread/attachment/add": ThreadAttachmentAddResponse;
  "thread/attachment/list": ThreadAttachmentListResponse;
  "thread/attachmentOwner/list": ThreadAttachmentOwnerListResponse;
  "thread/attachment/remove": ThreadAttachmentRemoveResponse;
  "thread/section/move": ThreadSectionMoveResponse;
  "thread/settings/update": ThreadSettingsUpdateResponse;
  "thread/memoryMode/set": ThreadMemoryModeSetResponse;
  "thread/memory/read": ThreadMemoryReadResponse;
  "memory/status": MemoryStatusResponse;
  "memory/reset": MemoryResetResponse;
  "rollout/compress": RolloutCompressResponse;
  "thread/unarchive": ThreadUnarchiveResponse;
  "thread/compact/start": ThreadCompactStartResponse;
  "thread/shellCommand": ThreadShellCommandResponse;
  "thread/approveGuardianDeniedAction": ThreadApproveGuardianDeniedActionResponse;
  "thread/backgroundTerminals/clean": ThreadBackgroundTerminalsCleanResponse;
  "thread/backgroundTerminals/list": ThreadBackgroundTerminalsListResponse;
  "thread/backgroundTerminals/terminate": ThreadBackgroundTerminalsTerminateResponse;
  "thread/revert": ThreadRevertResponse;
  "thread/list": ThreadListResponse;
  "project/list": ProjectListResponse;
  "project/read": ProjectReadResponse;
  "project/create": ProjectCreateResponse;
  "project/import": ProjectImportResponse;
  "project/update": ProjectUpdateResponse;
  "project/move": ProjectMoveResponse;
  "project/delete": ProjectDeleteResponse;
  "threadSection/list": ThreadSectionListResponse;
  "threadSection/create": ThreadSectionCreateResponse;
  "threadSection/update": ThreadSectionUpdateResponse;
  "threadSection/delete": ThreadSectionDeleteResponse;
  "thread/search": ThreadSearchResponse;
  "thread/searchOccurrences": ThreadSearchOccurrencesResponse;
  "thread/loaded/list": ThreadLoadedListResponse;
  "thread/read": ThreadReadResponse;
  "thread/readState/update": ThreadReadStateUpdateResponse;
  "thread/turns/list": ThreadTurnsListResponse;
  "thread/items/list": ThreadItemsListResponse;
  "thread/items/read": ThreadItemsReadResponse;
  "thread/guardianReview": ThreadGuardianReviewResponse;
  "thread/inject_items": ThreadInjectItemsResponse;
  "skills/list": SkillsListResponse;
  "skills/extraRoots/set": SkillsExtraRootsSetResponse;
  "hooks/list": HooksListResponse;
  "marketplace/add": MarketplaceAddResponse;
  "marketplace/remove": MarketplaceRemoveResponse;
  "marketplace/upgrade": MarketplaceUpgradeResponse;
  "plugin/list": PluginListResponse;
  "plugin/search": PluginSearchResponse;
  "plugin/installed": PluginInstalledResponse;
  "plugin/reconcile": PluginReconcileResponse;
  "plugin/read": PluginReadResponse;
  "plugin/skill/read": PluginSkillReadResponse;
  "plugin/share/save": PluginShareSaveResponse;
  "plugin/share/updateTargets": PluginShareUpdateTargetsResponse;
  "plugin/share/list": PluginShareListResponse;
  "plugin/share/checkout": PluginShareCheckoutResponse;
  "plugin/share/delete": PluginShareDeleteResponse;
  "app/read": AppsReadResponse;
  "app/list": AppsListResponse;
  "app/installed": AppsInstalledResponse;
  "fs/readFile": FsReadFileResponse;
  "fs/writeFile": FsWriteFileResponse;
  "fs/createDirectory": FsCreateDirectoryResponse;
  "fs/getMetadata": FsGetMetadataResponse;
  "fs/readDirectory": FsReadDirectoryResponse;
  "fs/remove": FsRemoveResponse;
  "fs/copy": FsCopyResponse;
  "fs/watch": FsWatchResponse;
  "fs/unwatch": FsUnwatchResponse;
  "skills/config/write": SkillsConfigWriteResponse;
  "plugin/install": PluginInstallResponse;
  "plugin/uninstall": PluginUninstallResponse;
  "turn/start": TurnStartResponse;
  "turn/settings/update": TurnSettingsUpdateResponse;
  "turn/steer": TurnSteerResponse;
  "turn/interrupt": TurnInterruptResponse;
  "thread/realtime/start": ThreadRealtimeStartResponse;
  "thread/realtime/appendAudio": ThreadRealtimeAppendAudioResponse;
  "thread/realtime/appendText": ThreadRealtimeAppendTextResponse;
  "thread/realtime/appendSpeech": ThreadRealtimeAppendSpeechResponse;
  "thread/realtime/stop": ThreadRealtimeStopResponse;
  "thread/timeline/list": ThreadTimelineListResponse;
  "thread/realtime/listVoices": ThreadRealtimeListVoicesResponse;
  "review/start": ReviewStartResponse;
  "model/list": ModelListResponse;
  "account/gatewayOAuth/read": GatewayOAuthReadResponse;
  "account/gatewayOAuth/login": GatewayOAuthLoginResponse;
  "account/gatewayOAuth/cancel": GatewayOAuthCancelResponse;
  "modelProvider/capabilities/read": ModelProviderCapabilitiesReadResponse;
  "experimentalFeature/list": ExperimentalFeatureListResponse;
  "permissionProfile/list": PermissionProfileListResponse;
  "experimentalFeature/enablement/set": ExperimentalFeatureEnablementSetResponse;
  "remoteControl/enable": RemoteControlEnableResponse;
  "remoteControl/disable": RemoteControlDisableResponse;
  "remoteControl/status/read": RemoteControlStatusReadResponse;
  "remoteControl/pairing/start": RemoteControlPairingStartResponse;
  "remoteControl/pairing/status": RemoteControlPairingStatusResponse;
  "remoteControl/client/list": RemoteControlClientsListResponse;
  "remoteControl/client/revoke": RemoteControlClientsRevokeResponse;
  "collaborationMode/list": CollaborationModeListResponse;
  "mock/experimentalMethod": MockExperimentalMethodResponse;
  "environment/add": EnvironmentAddResponse;
  "environment/info": EnvironmentInfoResponse;
  "environment/status": EnvironmentStatusResponse;
  "mcpServer/oauth/login": McpServerOauthLoginResponse;
  "config/mcpServer/reload": McpServerRefreshResponse;
  "mcpServerStatus/list": ListMcpServerStatusResponse;
  "mcpServer/resource/read": McpResourceReadResponse;
  "mcpServer/event/stream/start": McpServerEventStreamStartResponse;
  "mcpServer/event/stream/stop": McpServerEventStreamStopResponse;
  "mcpServer/tool/call": McpServerToolCallResponse;
  "windowsSandbox/setupStart": WindowsSandboxSetupStartResponse;
  "windowsSandbox/readiness": WindowsSandboxReadinessResponse;
  "account/login/start": LoginAccountResponse;
  "account/bedrock/discover": BedrockDiscoverResponse;
  "account/bedrock/setup": BedrockSetupResponse;
  "account/gcp/setup": GcpSetupResponse;
  "account/bedrock/checkGovCloudRequirements": BedrockCheckGovCloudRequirementsResponse;
  "account/login/cancel": CancelLoginAccountResponse;
  "accountSession/list": AccountSessionsResponse;
  "accountSession/switch": AccountSessionsSwitchResponse;
  "account/logout": LogoutAccountResponse;
  "account/rateLimits/read": GetAccountRateLimitsResponse;
  "account/rateLimitResetCredit/consume": ConsumeAccountRateLimitResetCreditResponse;
  "account/usage/read": GetAccountTokenUsageResponse;
  "account/workspaceMessages/read": GetWorkspaceMessagesResponse;
  "account/sendAddCreditsNudgeEmail": SendAddCreditsNudgeEmailResponse;
  "feedback/upload": FeedbackUploadResponse;
  "feedback/save": FeedbackSaveResponse;
  "command/exec": CommandExecResponse;
  "command/exec/write": CommandExecWriteResponse;
  "command/exec/terminate": CommandExecTerminateResponse;
  "command/exec/resize": CommandExecResizeResponse;
  "process/spawn": ProcessSpawnResponse;
  "process/writeStdin": ProcessWriteStdinResponse;
  "process/kill": ProcessKillResponse;
  "process/resizePty": ProcessResizePtyResponse;
  "config/read": ConfigReadResponse;
  "externalAgentConfig/detect": ExternalAgentConfigDetectResponse;
  "externalAgentConfig/import": ExternalAgentConfigImportResponse;
  "externalAgentConfig/import/recordHistory": ExternalAgentConfigImportHistoryRecordResponse;
  "externalAgentConfig/import/readHistories": ExternalAgentConfigImportHistoriesReadResponse;
  "config/value/write": ConfigWriteResponse;
  "config/batchWrite": ConfigWriteResponse;
  "configRequirements/read": ConfigRequirementsReadResponse;
  "account/read": GetAccountResponse;
  fuzzyFileSearch: FuzzyFileSearchResponse;
  "fuzzyFileSearch/sessionStart": FuzzyFileSearchSessionStartResponse;
  "fuzzyFileSearch/sessionUpdate": FuzzyFileSearchSessionUpdateResponse;
  "fuzzyFileSearch/sessionStop": FuzzyFileSearchSessionStopResponse;
};
export type ClientNotificationMethod = "initialized";
export type ServerNotificationMethod =
  | "error"
  | "thread/started"
  | "thread/status/changed"
  | "thread/readState/changed"
  | "thread/archived"
  | "thread/deleted"
  | "thread/unarchived"
  | "thread/closed"
  | "thread/reverted"
  | "skills/changed"
  | "thread/name/updated"
  | "thread/section/changed"
  | "thread/attachment/updated"
  | "thread/goal/updated"
  | "thread/prediction/updated"
  | "thread/goal/cleared"
  | "thread/queue/changed"
  | "project/changed"
  | "thread/project/updated"
  | "thread/environment/connected"
  | "thread/environment/disconnected"
  | "thread/settings/updated"
  | "thread/tokenUsage/updated"
  | "turn/started"
  | "hook/started"
  | "turn/completed"
  | "hook/completed"
  | "turn/diff/updated"
  | "turn/plan/updated"
  | "item/started"
  | "item/autoApprovalReview/started"
  | "item/autoApprovalReview/completed"
  | "autoApprovalReview/strictReviewRequired"
  | "item/completed"
  | "item/agentMessage/delta"
  | "item/plan/delta"
  | "command/exec/outputDelta"
  | "process/outputDelta"
  | "process/exited"
  | "item/commandExecution/outputDelta"
  | "item/commandExecution/terminalInteraction"
  | "item/fileChange/outputDelta"
  | "item/fileChange/patchUpdated"
  | "serverRequest/resolved"
  | "item/mcpToolCall/progress"
  | "mcpServer/oauthLogin/completed"
  | "mcpServer/startupStatus/updated"
  | "mcpServer/event/stream/notification"
  | "account/updated"
  | "account/gatewayOAuth/changed"
  | "account/rateLimits/updated"
  | "app/list/updated"
  | "remoteControl/status/changed"
  | "externalAgentConfig/import/progress"
  | "externalAgentConfig/import/completed"
  | "fs/changed"
  | "item/reasoning/summaryTextDelta"
  | "item/reasoning/summaryPartAdded"
  | "item/reasoning/textDelta"
  | "thread/compacted"
  | "model/rerouted"
  | "model/verification"
  | "modelProvider/authRecoveryStarted"
  | "modelProvider/authRecoveryCompleted"
  | "turn/moderationMetadata"
  | "model/safetyBuffering/updated"
  | "warning"
  | "guardianWarning"
  | "deprecationNotice"
  | "configWarning"
  | "fuzzyFileSearch/sessionUpdated"
  | "fuzzyFileSearch/sessionCompleted"
  | "thread/realtime/started"
  | "thread/realtime/itemAdded"
  | "thread/realtime/item/started"
  | "thread/realtime/item/transcript/delta"
  | "thread/realtime/item/completed"
  | "thread/realtime/transcript/delta"
  | "thread/realtime/transcript/done"
  | "thread/realtime/outputAudio/delta"
  | "thread/realtime/sdp"
  | "thread/realtime/error"
  | "thread/realtime/closed"
  | "windows/worldWritableWarning"
  | "windowsSandbox/setupCompleted"
  | "account/login/completed";
export type ServerRequestMethod =
  | "item/commandExecution/requestApproval"
  | "item/fileChange/requestApproval"
  | "item/tool/requestUserInput"
  | "mcpServer/elicitation/request"
  | "item/permissions/requestApproval"
  | "item/tool/call"
  | "account/chatgptAuthTokens/refresh"
  | "attestation/generate"
  | "currentTime/read"
  | "applyPatchApproval"
  | "execCommandApproval";
export type ServerRequestResults = {
  "item/commandExecution/requestApproval": CommandExecutionRequestApprovalResponse;
  "item/fileChange/requestApproval": FileChangeRequestApprovalResponse;
  "item/tool/requestUserInput": ToolRequestUserInputResponse;
  "mcpServer/elicitation/request": McpServerElicitationRequestResponse;
  "item/permissions/requestApproval": PermissionsRequestApprovalResponse;
  "item/tool/call": DynamicToolCallResponse;
  "account/chatgptAuthTokens/refresh": ChatgptAuthTokensRefreshResponse;
  "attestation/generate": AttestationGenerateResponse;
  "currentTime/read": CurrentTimeReadResponse;
  applyPatchApproval: ApplyPatchApprovalResponse;
  execCommandApproval: ExecCommandApprovalResponse;
};
