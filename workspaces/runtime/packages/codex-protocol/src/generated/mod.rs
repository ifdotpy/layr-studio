// Generated from codex-cli 0.167.1 full experimental JSON Schema; do not edit.
// Owner: provider-contract-sources.ts; SHA-256: 486c7c8daa5dc7f3ab2a1fb56de5316804caf7a4210fda1dc28d3f0f4e90cfe1

// Generated vendor layouts preserve optional/nullable fields and large unions.
#![allow(clippy::all, clippy::option_option, clippy::struct_excessive_bools)]
pub const CLI_VERSION: &str = "0.167.1";
pub const SCHEMA_SHA256: &str = "486c7c8daa5dc7f3ab2a1fb56de5316804caf7a4210fda1dc28d3f0f4e90cfe1";

// A required nullable field must still reject an absent member.
fn required<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    d: D,
) -> Result<T, D::Error> {
    T::deserialize(d)
}
fn optional<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

pub type AbsolutePathBuf = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AdditionalPermissionProfile {
    #[serde(
        rename = "fileSystem",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#file_system: Option<Option<AdditionalFileSystemPermissions>>,
    #[serde(
        rename = "network",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#network: Option<Option<AdditionalNetworkPermissions>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ApplyPatchApprovalParams {
    #[serde(rename = "callId", deserialize_with = "required")]
    pub r#call_id: String,
    #[serde(rename = "conversationId", deserialize_with = "required")]
    pub r#conversation_id: ThreadId,
    #[serde(rename = "fileChanges", deserialize_with = "required")]
    pub r#file_changes: std::collections::BTreeMap<String, FileChange>,
    #[serde(
        rename = "grantRoot",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#grant_root: Option<Option<String>>,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ApplyPatchApprovalResponse {
    #[serde(rename = "decision", deserialize_with = "required")]
    pub r#decision: ReviewDecision,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AttestationGenerateParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AttestationGenerateResponse {
    #[serde(rename = "token", deserialize_with = "required")]
    pub r#token: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatgptAuthTokensRefreshParams {
    #[serde(
        rename = "previousAccountId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#previous_account_id: Option<Option<String>>,
    #[serde(rename = "reason", deserialize_with = "required")]
    pub r#reason: ChatgptAuthTokensRefreshReason,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ChatgptAuthTokensRefreshReason {
    Variant1(Box<ChatgptAuthTokensRefreshReasonVariant1>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatgptAuthTokensRefreshResponse {
    #[serde(rename = "accessToken", deserialize_with = "required")]
    pub r#access_token: String,
    #[serde(rename = "chatgptAccountId", deserialize_with = "required")]
    pub r#chatgpt_account_id: String,
    #[serde(
        rename = "chatgptPlanType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#chatgpt_plan_type: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClientInfo {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "version", deserialize_with = "required")]
    pub r#version: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method")]
pub enum ClientNotification {
    #[serde(rename = "initialized")]
    Initialized,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method")]
pub enum ClientRequest {
    #[serde(rename = "initialize")]
    Initialize {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: InitializeParams,
    },
    #[serde(rename = "server/diagnostics")]
    ServerDiagnostics {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ServerDiagnosticsParams,
    },
    #[serde(rename = "userVerification/status")]
    UserVerificationStatus {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: UserVerificationStatusParams,
    },
    #[serde(rename = "userVerification/enroll")]
    UserVerificationEnroll {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: UserVerificationEnrollParams,
    },
    #[serde(rename = "userVerification/delete")]
    UserVerificationDelete {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: UserVerificationDeleteParams,
    },
    #[serde(rename = "userVerification/verify")]
    UserVerificationVerify {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: UserVerificationVerifyParams,
    },
    #[serde(rename = "userVerification/cancel")]
    UserVerificationCancel {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: UserVerificationCancelParams,
    },
    #[serde(rename = "thread/start")]
    ThreadStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadStartParams,
    },
    #[serde(rename = "thread/resume")]
    ThreadResume {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadResumeParams,
    },
    #[serde(rename = "thread/fork")]
    ThreadFork {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadForkParams,
    },
    #[serde(rename = "thread/sample")]
    ThreadSample {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSampleParams,
    },
    #[serde(rename = "thread/sample/cancel")]
    ThreadSampleCancel {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSampleCancelParams,
    },
    #[serde(rename = "thread/archive")]
    ThreadArchive {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadArchiveParams,
    },
    #[serde(rename = "thread/delete")]
    ThreadDelete {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadDeleteParams,
    },
    #[serde(rename = "thread/unsubscribe")]
    ThreadUnsubscribe {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadUnsubscribeParams,
    },
    #[serde(rename = "thread/increment_elicitation")]
    ThreadIncrementElicitation {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadIncrementElicitationParams,
    },
    #[serde(rename = "thread/decrement_elicitation")]
    ThreadDecrementElicitation {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadDecrementElicitationParams,
    },
    #[serde(rename = "thread/name/set")]
    ThreadNameSet {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSetNameParams,
    },
    #[serde(rename = "thread/prediction/request")]
    ThreadPredictionRequest {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadPredictionRequestParams,
    },
    #[serde(rename = "thread/goal/set")]
    ThreadGoalSet {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadGoalSetParams,
    },
    #[serde(rename = "thread/goal/get")]
    ThreadGoalGet {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadGoalGetParams,
    },
    #[serde(rename = "thread/goal/clear")]
    ThreadGoalClear {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadGoalClearParams,
    },
    #[serde(rename = "thread/queue/add")]
    ThreadQueueAdd {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadQueueAddParams,
    },
    #[serde(rename = "thread/queue/list")]
    ThreadQueueList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadQueueListParams,
    },
    #[serde(rename = "thread/queue/update")]
    ThreadQueueUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadQueueUpdateParams,
    },
    #[serde(rename = "thread/queue/delete")]
    ThreadQueueDelete {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadQueueDeleteParams,
    },
    #[serde(rename = "thread/queue/reorder")]
    ThreadQueueReorder {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadQueueReorderParams,
    },
    #[serde(rename = "thread/queue/start")]
    ThreadQueueStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadQueueStartParams,
    },
    #[serde(rename = "thread/metadata/update")]
    ThreadMetadataUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadMetadataUpdateParams,
    },
    #[serde(rename = "thread/attachment/add")]
    ThreadAttachmentAdd {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadAttachmentAddParams,
    },
    #[serde(rename = "thread/attachment/list")]
    ThreadAttachmentList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadAttachmentListParams,
    },
    #[serde(rename = "thread/attachmentOwner/list")]
    ThreadAttachmentOwnerList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadAttachmentOwnerListParams,
    },
    #[serde(rename = "thread/attachment/remove")]
    ThreadAttachmentRemove {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadAttachmentRemoveParams,
    },
    #[serde(rename = "thread/section/move")]
    ThreadSectionMove {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSectionMoveParams,
    },
    #[serde(rename = "thread/settings/update")]
    ThreadSettingsUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSettingsUpdateParams,
    },
    #[serde(rename = "thread/memoryMode/set")]
    ThreadMemoryModeSet {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadMemoryModeSetParams,
    },
    #[serde(rename = "thread/memory/read")]
    ThreadMemoryRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadMemoryReadParams,
    },
    #[serde(rename = "memory/status")]
    MemoryStatus {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: MemoryStatusParams,
    },
    #[serde(rename = "memory/reset")]
    MemoryReset {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "rollout/compress")]
    RolloutCompress {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "thread/unarchive")]
    ThreadUnarchive {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadUnarchiveParams,
    },
    #[serde(rename = "thread/compact/start")]
    ThreadCompactStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadCompactStartParams,
    },
    #[serde(rename = "thread/shellCommand")]
    ThreadShellCommand {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadShellCommandParams,
    },
    #[serde(rename = "thread/approveGuardianDeniedAction")]
    ThreadApproveGuardianDeniedAction {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadApproveGuardianDeniedActionParams,
    },
    #[serde(rename = "thread/backgroundTerminals/clean")]
    ThreadBackgroundTerminalsClean {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadBackgroundTerminalsCleanParams,
    },
    #[serde(rename = "thread/backgroundTerminals/list")]
    ThreadBackgroundTerminalsList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadBackgroundTerminalsListParams,
    },
    #[serde(rename = "thread/backgroundTerminals/terminate")]
    ThreadBackgroundTerminalsTerminate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadBackgroundTerminalsTerminateParams,
    },
    #[serde(rename = "thread/revert")]
    ThreadRevert {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRevertParams,
    },
    #[serde(rename = "thread/list")]
    ThreadList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadListParams,
    },
    #[serde(rename = "project/list")]
    ProjectList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectListParams,
    },
    #[serde(rename = "project/read")]
    ProjectRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectReadParams,
    },
    #[serde(rename = "project/create")]
    ProjectCreate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectCreateParams,
    },
    #[serde(rename = "project/import")]
    ProjectImport {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectImportParams,
    },
    #[serde(rename = "project/update")]
    ProjectUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectUpdateParams,
    },
    #[serde(rename = "project/move")]
    ProjectMove {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectMoveParams,
    },
    #[serde(rename = "project/delete")]
    ProjectDelete {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectDeleteParams,
    },
    #[serde(rename = "threadSection/list")]
    ThreadSectionList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSectionListParams,
    },
    #[serde(rename = "threadSection/create")]
    ThreadSectionCreate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSectionCreateParams,
    },
    #[serde(rename = "threadSection/update")]
    ThreadSectionUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSectionUpdateParams,
    },
    #[serde(rename = "threadSection/delete")]
    ThreadSectionDelete {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSectionDeleteParams,
    },
    #[serde(rename = "thread/search")]
    ThreadSearch {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSearchParams,
    },
    #[serde(rename = "thread/searchOccurrences")]
    ThreadSearchOccurrences {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSearchOccurrencesParams,
    },
    #[serde(rename = "thread/loaded/list")]
    ThreadLoadedList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadLoadedListParams,
    },
    #[serde(rename = "thread/read")]
    ThreadRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadReadParams,
    },
    #[serde(rename = "thread/readState/update")]
    ThreadReadStateUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadReadStateUpdateParams,
    },
    #[serde(rename = "thread/turns/list")]
    ThreadTurnsList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadTurnsListParams,
    },
    #[serde(rename = "thread/items/list")]
    ThreadItemsList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadItemsListParams,
    },
    #[serde(rename = "thread/items/read")]
    ThreadItemsRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadItemsReadParams,
    },
    #[serde(rename = "thread/guardianReview")]
    ThreadGuardianReview {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadGuardianReviewParams,
    },
    #[serde(rename = "thread/inject_items")]
    ThreadInjectItems {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadInjectItemsParams,
    },
    #[serde(rename = "skills/list")]
    SkillsList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: SkillsListParams,
    },
    #[serde(rename = "skills/extraRoots/set")]
    SkillsExtraRootsSet {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: SkillsExtraRootsSetParams,
    },
    #[serde(rename = "hooks/list")]
    HooksList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: HooksListParams,
    },
    #[serde(rename = "marketplace/add")]
    MarketplaceAdd {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: MarketplaceAddParams,
    },
    #[serde(rename = "marketplace/remove")]
    MarketplaceRemove {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: MarketplaceRemoveParams,
    },
    #[serde(rename = "marketplace/upgrade")]
    MarketplaceUpgrade {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: MarketplaceUpgradeParams,
    },
    #[serde(rename = "plugin/list")]
    PluginList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginListParams,
    },
    #[serde(rename = "plugin/search")]
    PluginSearch {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginSearchParams,
    },
    #[serde(rename = "plugin/installed")]
    PluginInstalled {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginInstalledParams,
    },
    #[serde(rename = "plugin/reconcile")]
    PluginReconcile {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginReconcileParams,
    },
    #[serde(rename = "plugin/read")]
    PluginRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginReadParams,
    },
    #[serde(rename = "plugin/skill/read")]
    PluginSkillRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginSkillReadParams,
    },
    #[serde(rename = "plugin/share/save")]
    PluginShareSave {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginShareSaveParams,
    },
    #[serde(rename = "plugin/share/updateTargets")]
    PluginShareUpdateTargets {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginShareUpdateTargetsParams,
    },
    #[serde(rename = "plugin/share/list")]
    PluginShareList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginShareListParams,
    },
    #[serde(rename = "plugin/share/checkout")]
    PluginShareCheckout {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginShareCheckoutParams,
    },
    #[serde(rename = "plugin/share/delete")]
    PluginShareDelete {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginShareDeleteParams,
    },
    #[serde(rename = "app/read")]
    AppRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AppsReadParams,
    },
    #[serde(rename = "app/list")]
    AppList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AppsListParams,
    },
    #[serde(rename = "app/installed")]
    AppInstalled {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AppsInstalledParams,
    },
    #[serde(rename = "fs/readFile")]
    FsReadFile {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsReadFileParams,
    },
    #[serde(rename = "fs/writeFile")]
    FsWriteFile {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsWriteFileParams,
    },
    #[serde(rename = "fs/createDirectory")]
    FsCreateDirectory {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsCreateDirectoryParams,
    },
    #[serde(rename = "fs/getMetadata")]
    FsGetMetadata {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsGetMetadataParams,
    },
    #[serde(rename = "fs/readDirectory")]
    FsReadDirectory {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsReadDirectoryParams,
    },
    #[serde(rename = "fs/remove")]
    FsRemove {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsRemoveParams,
    },
    #[serde(rename = "fs/copy")]
    FsCopy {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsCopyParams,
    },
    #[serde(rename = "fs/watch")]
    FsWatch {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsWatchParams,
    },
    #[serde(rename = "fs/unwatch")]
    FsUnwatch {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsUnwatchParams,
    },
    #[serde(rename = "skills/config/write")]
    SkillsConfigWrite {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: SkillsConfigWriteParams,
    },
    #[serde(rename = "plugin/install")]
    PluginInstall {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginInstallParams,
    },
    #[serde(rename = "plugin/uninstall")]
    PluginUninstall {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PluginUninstallParams,
    },
    #[serde(rename = "turn/start")]
    TurnStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnStartParams,
    },
    #[serde(rename = "turn/settings/update")]
    TurnSettingsUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnSettingsUpdateParams,
    },
    #[serde(rename = "turn/steer")]
    TurnSteer {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnSteerParams,
    },
    #[serde(rename = "turn/interrupt")]
    TurnInterrupt {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnInterruptParams,
    },
    #[serde(rename = "thread/realtime/start")]
    ThreadRealtimeStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeStartParams,
    },
    #[serde(rename = "thread/realtime/appendAudio")]
    ThreadRealtimeAppendAudio {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeAppendAudioParams,
    },
    #[serde(rename = "thread/realtime/appendText")]
    ThreadRealtimeAppendText {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeAppendTextParams,
    },
    #[serde(rename = "thread/realtime/appendSpeech")]
    ThreadRealtimeAppendSpeech {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeAppendSpeechParams,
    },
    #[serde(rename = "thread/realtime/stop")]
    ThreadRealtimeStop {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeStopParams,
    },
    #[serde(rename = "thread/timeline/list")]
    ThreadTimelineList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadTimelineListParams,
    },
    #[serde(rename = "thread/realtime/listVoices")]
    ThreadRealtimeListVoices {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeListVoicesParams,
    },
    #[serde(rename = "review/start")]
    ReviewStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ReviewStartParams,
    },
    #[serde(rename = "model/list")]
    ModelList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ModelListParams,
    },
    #[serde(rename = "account/gatewayOAuth/read")]
    AccountGatewayOAuthRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "account/gatewayOAuth/login")]
    AccountGatewayOAuthLogin {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "account/gatewayOAuth/cancel")]
    AccountGatewayOAuthCancel {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "modelProvider/capabilities/read")]
    ModelProviderCapabilitiesRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ModelProviderCapabilitiesReadParams,
    },
    #[serde(rename = "experimentalFeature/list")]
    ExperimentalFeatureList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExperimentalFeatureListParams,
    },
    #[serde(rename = "permissionProfile/list")]
    PermissionProfileList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PermissionProfileListParams,
    },
    #[serde(rename = "experimentalFeature/enablement/set")]
    ExperimentalFeatureEnablementSet {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExperimentalFeatureEnablementSetParams,
    },
    #[serde(rename = "remoteControl/enable")]
    RemoteControlEnable {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<Option<RemoteControlEnableParams>>,
    },
    #[serde(rename = "remoteControl/disable")]
    RemoteControlDisable {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<Option<RemoteControlDisableParams>>,
    },
    #[serde(rename = "remoteControl/status/read")]
    RemoteControlStatusRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "remoteControl/pairing/start")]
    RemoteControlPairingStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: RemoteControlPairingStartParams,
    },
    #[serde(rename = "remoteControl/pairing/status")]
    RemoteControlPairingStatus {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: RemoteControlPairingStatusParams,
    },
    #[serde(rename = "remoteControl/client/list")]
    RemoteControlClientList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: RemoteControlClientsListParams,
    },
    #[serde(rename = "remoteControl/client/revoke")]
    RemoteControlClientRevoke {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: RemoteControlClientsRevokeParams,
    },
    #[serde(rename = "collaborationMode/list")]
    CollaborationModeList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CollaborationModeListParams,
    },
    #[serde(rename = "mock/experimentalMethod")]
    MockExperimentalMethod {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: MockExperimentalMethodParams,
    },
    #[serde(rename = "environment/add")]
    EnvironmentAdd {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: EnvironmentAddParams,
    },
    #[serde(rename = "environment/info")]
    EnvironmentInfo {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: EnvironmentInfoParams,
    },
    #[serde(rename = "environment/status")]
    EnvironmentStatus {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: EnvironmentStatusParams,
    },
    #[serde(rename = "mcpServer/oauth/login")]
    McpServerOauthLogin {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerOauthLoginParams,
    },
    #[serde(rename = "config/mcpServer/reload")]
    ConfigMcpServerReload {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "mcpServerStatus/list")]
    McpServerStatusList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ListMcpServerStatusParams,
    },
    #[serde(rename = "mcpServer/resource/read")]
    McpServerResourceRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpResourceReadParams,
    },
    #[serde(rename = "mcpServer/event/stream/start")]
    McpServerEventStreamStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerEventStreamStartParams,
    },
    #[serde(rename = "mcpServer/event/stream/stop")]
    McpServerEventStreamStop {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerEventStreamStopParams,
    },
    #[serde(rename = "mcpServer/tool/call")]
    McpServerToolCall {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerToolCallParams,
    },
    #[serde(rename = "windowsSandbox/setupStart")]
    WindowsSandboxSetupStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: WindowsSandboxSetupStartParams,
    },
    #[serde(rename = "windowsSandbox/readiness")]
    WindowsSandboxReadiness {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "account/login/start")]
    AccountLoginStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: LoginAccountParams,
    },
    #[serde(rename = "account/bedrock/discover")]
    AccountBedrockDiscover {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: BedrockDiscoverParams,
    },
    #[serde(rename = "account/bedrock/setup")]
    AccountBedrockSetup {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: BedrockSetupParams,
    },
    #[serde(rename = "account/gcp/setup")]
    AccountGcpSetup {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: GcpSetupParams,
    },
    #[serde(rename = "account/bedrock/checkGovCloudRequirements")]
    AccountBedrockCheckGovCloudRequirements {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: BedrockCheckGovCloudRequirementsParams,
    },
    #[serde(rename = "account/login/cancel")]
    AccountLoginCancel {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CancelLoginAccountParams,
    },
    #[serde(rename = "accountSession/list")]
    AccountSessionList {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AccountSessionsListParams,
    },
    #[serde(rename = "accountSession/switch")]
    AccountSessionSwitch {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AccountSessionsSwitchParams,
    },
    #[serde(rename = "account/logout")]
    AccountLogout {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "account/rateLimits/read")]
    AccountRateLimitsRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<Option<GetAccountRateLimitsParams>>,
    },
    #[serde(rename = "account/rateLimitResetCredit/consume")]
    AccountRateLimitResetCreditConsume {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ConsumeAccountRateLimitResetCreditParams,
    },
    #[serde(rename = "account/usage/read")]
    AccountUsageRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<Option<GetAccountTokenUsageParams>>,
    },
    #[serde(rename = "account/workspaceMessages/read")]
    AccountWorkspaceMessagesRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "account/sendAddCreditsNudgeEmail")]
    AccountSendAddCreditsNudgeEmail {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: SendAddCreditsNudgeEmailParams,
    },
    #[serde(rename = "feedback/upload")]
    FeedbackUpload {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FeedbackUploadParams,
    },
    #[serde(rename = "feedback/save")]
    FeedbackSave {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FeedbackSaveParams,
    },
    #[serde(rename = "command/exec")]
    CommandExec {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CommandExecParams,
    },
    #[serde(rename = "command/exec/write")]
    CommandExecWrite {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CommandExecWriteParams,
    },
    #[serde(rename = "command/exec/terminate")]
    CommandExecTerminate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CommandExecTerminateParams,
    },
    #[serde(rename = "command/exec/resize")]
    CommandExecResize {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CommandExecResizeParams,
    },
    #[serde(rename = "process/spawn")]
    ProcessSpawn {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProcessSpawnParams,
    },
    #[serde(rename = "process/writeStdin")]
    ProcessWriteStdin {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProcessWriteStdinParams,
    },
    #[serde(rename = "process/kill")]
    ProcessKill {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProcessKillParams,
    },
    #[serde(rename = "process/resizePty")]
    ProcessResizePty {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProcessResizePtyParams,
    },
    #[serde(rename = "config/read")]
    ConfigRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ConfigReadParams,
    },
    #[serde(rename = "externalAgentConfig/detect")]
    ExternalAgentConfigDetect {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExternalAgentConfigDetectParams,
    },
    #[serde(rename = "externalAgentConfig/import")]
    ExternalAgentConfigImport {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExternalAgentConfigImportParams,
    },
    #[serde(rename = "externalAgentConfig/import/recordHistory")]
    ExternalAgentConfigImportRecordHistory {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExternalAgentConfigImportHistoryRecordParams,
    },
    #[serde(rename = "externalAgentConfig/import/readHistories")]
    ExternalAgentConfigImportReadHistories {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "config/value/write")]
    ConfigValueWrite {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ConfigValueWriteParams,
    },
    #[serde(rename = "config/batchWrite")]
    ConfigBatchWrite {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ConfigBatchWriteParams,
    },
    #[serde(rename = "configRequirements/read")]
    ConfigRequirementsRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(
            rename = "params",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#params: Option<()>,
    },
    #[serde(rename = "account/read")]
    AccountRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: GetAccountParams,
    },
    #[serde(rename = "fuzzyFileSearch")]
    FuzzyFileSearch {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FuzzyFileSearchParams,
    },
    #[serde(rename = "fuzzyFileSearch/sessionStart")]
    FuzzyFileSearchSessionStart {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FuzzyFileSearchSessionStartParams,
    },
    #[serde(rename = "fuzzyFileSearch/sessionUpdate")]
    FuzzyFileSearchSessionUpdate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FuzzyFileSearchSessionUpdateParams,
    },
    #[serde(rename = "fuzzyFileSearch/sessionStop")]
    FuzzyFileSearchSessionStop {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FuzzyFileSearchSessionStopParams,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CommandExecutionApprovalDecision {
    Variant1(Box<CommandExecutionApprovalDecisionVariant1>),
    Variant2(Box<CommandExecutionApprovalDecisionVariant2>),
    Variant3(Box<CommandExecutionApprovalDecisionVariant3>),
    Variant4(Box<CommandExecutionApprovalDecisionVariant4>),
    Variant5(Box<CommandExecutionApprovalDecisionVariant5>),
    Variant6(Box<CommandExecutionApprovalDecisionVariant6>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CommandExecutionApprovalKind {
    Variant1(Box<CommandExecutionApprovalKindVariant1>),
    Variant2(Box<CommandExecutionApprovalKindVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecutionRequestApprovalParams {
    #[serde(
        rename = "additionalPermissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#additional_permissions: Option<Option<AdditionalPermissionProfile>>,
    #[serde(
        rename = "approvalId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_id: Option<Option<String>>,
    #[serde(
        rename = "availableDecisions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#available_decisions: Option<Option<Vec<CommandExecutionApprovalDecision>>>,
    #[serde(
        rename = "command",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#command: Option<Option<String>>,
    #[serde(
        rename = "commandActions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#command_actions: Option<Option<Vec<CommandAction>>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<LegacyAppPathString>>,
    #[serde(
        rename = "environmentId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environment_id: Option<Option<String>>,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(
        rename = "kind",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#kind: Option<CommandExecutionApprovalKind>,
    #[serde(
        rename = "networkApprovalContext",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#network_approval_context: Option<Option<NetworkApprovalContext>>,
    #[serde(
        rename = "proposedExecpolicyAmendment",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#proposed_execpolicy_amendment: Option<Option<Vec<String>>>,
    #[serde(
        rename = "proposedNetworkPolicyAmendments",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#proposed_network_policy_amendments: Option<Option<Vec<NetworkPolicyAmendment>>>,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
    #[serde(rename = "startedAtMs", deserialize_with = "required")]
    pub r#started_at_ms: i64,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecutionRequestApprovalResponse {
    #[serde(rename = "decision", deserialize_with = "required")]
    pub r#decision: CommandExecutionApprovalDecision,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurrentTimeReadParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CurrentTimeReadResponse {
    #[serde(rename = "currentTimeAt", deserialize_with = "required")]
    pub r#current_time_at: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DynamicToolCallParams {
    #[serde(rename = "arguments", deserialize_with = "required")]
    pub r#arguments: studio_json::Value,
    #[serde(rename = "callId", deserialize_with = "required")]
    pub r#call_id: String,
    #[serde(
        rename = "namespace",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#namespace: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "tool", deserialize_with = "required")]
    pub r#tool: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DynamicToolCallResponse {
    #[serde(rename = "contentItems", deserialize_with = "required")]
    pub r#content_items: Vec<DynamicToolCallOutputContentItem>,
    #[serde(rename = "success", deserialize_with = "required")]
    pub r#success: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExecCommandApprovalParams {
    #[serde(
        rename = "approvalId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_id: Option<Option<String>>,
    #[serde(rename = "callId", deserialize_with = "required")]
    pub r#call_id: String,
    #[serde(rename = "command", deserialize_with = "required")]
    pub r#command: Vec<String>,
    #[serde(rename = "conversationId", deserialize_with = "required")]
    pub r#conversation_id: ThreadId,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: String,
    #[serde(rename = "parsedCmd", deserialize_with = "required")]
    pub r#parsed_cmd: Vec<ParsedCommand>,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExecCommandApprovalResponse {
    #[serde(rename = "decision", deserialize_with = "required")]
    pub r#decision: ReviewDecision,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum FileChange {
    #[serde(rename = "add")]
    Add {
        #[serde(rename = "content", deserialize_with = "required")]
        r#content: String,
    },
    #[serde(rename = "delete")]
    Delete {
        #[serde(rename = "content", deserialize_with = "required")]
        r#content: String,
    },
    #[serde(rename = "update")]
    Update {
        #[serde(
            rename = "move_path",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#move_path: Option<Option<String>>,
        #[serde(rename = "unified_diff", deserialize_with = "required")]
        r#unified_diff: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum FileChangeApprovalDecision {
    Variant1(Box<FileChangeApprovalDecisionVariant1>),
    Variant2(Box<FileChangeApprovalDecisionVariant2>),
    Variant3(Box<FileChangeApprovalDecisionVariant3>),
    Variant4(Box<FileChangeApprovalDecisionVariant4>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileChangeRequestApprovalParams {
    #[serde(
        rename = "grantRoot",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#grant_root: Option<Option<String>>,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
    #[serde(rename = "startedAtMs", deserialize_with = "required")]
    pub r#started_at_ms: i64,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileChangeRequestApprovalResponse {
    #[serde(rename = "decision", deserialize_with = "required")]
    pub r#decision: FileChangeApprovalDecision,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FuzzyFileSearchMatchType {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "directory")]
    Directory,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchParams {
    #[serde(
        rename = "cancellationToken",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cancellation_token: Option<Option<String>>,
    #[serde(rename = "query", deserialize_with = "required")]
    pub r#query: String,
    #[serde(rename = "roots", deserialize_with = "required")]
    pub r#roots: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchResponse {
    #[serde(rename = "files", deserialize_with = "required")]
    pub r#files: Vec<FuzzyFileSearchResult>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchResult {
    #[serde(rename = "file_name", deserialize_with = "required")]
    pub r#file_name: String,
    #[serde(
        rename = "indices",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#indices: Option<Option<Vec<u32>>>,
    #[serde(rename = "match_type", deserialize_with = "required")]
    pub r#match_type: FuzzyFileSearchMatchType,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: String,
    #[serde(rename = "root", deserialize_with = "required")]
    pub r#root: String,
    #[serde(rename = "score", deserialize_with = "required")]
    pub r#score: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionCompletedNotification {
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionStartParams {
    #[serde(rename = "roots", deserialize_with = "required")]
    pub r#roots: Vec<String>,
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionStartResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionStopParams {
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionStopResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionUpdateParams {
    #[serde(rename = "query", deserialize_with = "required")]
    pub r#query: String,
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionUpdateResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FuzzyFileSearchSessionUpdatedNotification {
    #[serde(rename = "files", deserialize_with = "required")]
    pub r#files: Vec<FuzzyFileSearchResult>,
    #[serde(rename = "query", deserialize_with = "required")]
    pub r#query: String,
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GrantedPermissionProfile {
    #[serde(
        rename = "fileSystem",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#file_system: Option<Option<AdditionalFileSystemPermissions>>,
    #[serde(
        rename = "network",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#network: Option<Option<AdditionalNetworkPermissions>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InitializeCapabilities {
    #[serde(
        rename = "experimentalApi",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#experimental_api: Option<bool>,
    #[serde(
        rename = "explicitGatewayOauth",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#explicit_gateway_oauth: Option<bool>,
    #[serde(
        rename = "extensions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#extensions: Option<Option<std::collections::BTreeMap<String, studio_json::Value>>>,
    #[serde(
        rename = "mcpServerOpenaiFormElicitation",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mcp_server_openai_form_elicitation: Option<bool>,
    #[serde(
        rename = "optOutNotificationMethods",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#opt_out_notification_methods: Option<Option<Vec<String>>>,
    #[serde(
        rename = "requestAttestation",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#request_attestation: Option<bool>,
    #[serde(
        rename = "requiredHumanApproval",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#required_human_approval: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InitializeParams {
    #[serde(
        rename = "capabilities",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#capabilities: Option<Option<InitializeCapabilities>>,
    #[serde(rename = "clientInfo", deserialize_with = "required")]
    pub r#client_info: ClientInfo,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InitializeResponse {
    #[serde(rename = "codexHome", deserialize_with = "required")]
    pub r#codex_home: AbsolutePathBuf,
    #[serde(rename = "platformFamily", deserialize_with = "required")]
    pub r#platform_family: String,
    #[serde(rename = "platformOs", deserialize_with = "required")]
    pub r#platform_os: String,
    #[serde(rename = "userAgent", deserialize_with = "required")]
    pub r#user_agent: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSONRPCError {
    #[serde(rename = "error", deserialize_with = "required")]
    pub r#error: JSONRPCErrorError,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: RequestId,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSONRPCErrorError {
    #[serde(rename = "code", deserialize_with = "required")]
    pub r#code: i64,
    #[serde(
        rename = "data",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#data: Option<studio_json::Value>,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum JSONRPCMessage {
    Variant1(Box<JSONRPCRequest>),
    Variant2(Box<JSONRPCNotification>),
    Variant3(Box<JSONRPCResponse>),
    Variant4(Box<JSONRPCError>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSONRPCNotification {
    #[serde(rename = "method", deserialize_with = "required")]
    pub r#method: String,
    #[serde(
        rename = "params",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#params: Option<studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSONRPCRequest {
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: RequestId,
    #[serde(rename = "method", deserialize_with = "required")]
    pub r#method: String,
    #[serde(
        rename = "params",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#params: Option<studio_json::Value>,
    #[serde(
        rename = "trace",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#trace: Option<Option<W3cTraceContext>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSONRPCResponse {
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: RequestId,
    #[serde(rename = "result", deserialize_with = "required")]
    pub r#result: studio_json::Value,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpElicitationArrayType {
    #[serde(rename = "array")]
    Array,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationBooleanSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<bool>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationBooleanType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpElicitationBooleanType {
    #[serde(rename = "boolean")]
    Boolean,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationConstOption {
    #[serde(rename = "const", deserialize_with = "required")]
    pub r#const: String,
    #[serde(rename = "title", deserialize_with = "required")]
    pub r#title: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum McpElicitationEnumSchema {
    Variant1(Box<McpElicitationSingleSelectEnumSchema>),
    Variant2(Box<McpElicitationMultiSelectEnumSchema>),
    Variant3(Box<McpElicitationLegacyTitledEnumSchema>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationLegacyTitledEnumSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<String>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "enum", deserialize_with = "required")]
    pub r#enum: Vec<String>,
    #[serde(
        rename = "enumNames",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enum_names: Option<Option<Vec<String>>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationStringType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum McpElicitationMultiSelectEnumSchema {
    Variant1(Box<McpElicitationUntitledMultiSelectEnumSchema>),
    Variant2(Box<McpElicitationTitledMultiSelectEnumSchema>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationNumberSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<f64>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "maximum",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#maximum: Option<Option<f64>>,
    #[serde(
        rename = "minimum",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#minimum: Option<Option<f64>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationNumberType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpElicitationNumberType {
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "integer")]
    Integer,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpElicitationObjectType {
    #[serde(rename = "object")]
    Object,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum McpElicitationPrimitiveSchema {
    Variant1(Box<McpElicitationEnumSchema>),
    Variant2(Box<McpElicitationStringSchema>),
    Variant3(Box<McpElicitationNumberSchema>),
    Variant4(Box<McpElicitationBooleanSchema>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationSchema {
    #[serde(
        rename = "$schema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_schema: Option<Option<String>>,
    #[serde(rename = "properties", deserialize_with = "required")]
    pub r#properties: std::collections::BTreeMap<String, McpElicitationPrimitiveSchema>,
    #[serde(
        rename = "required",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#required: Option<Option<Vec<String>>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationObjectType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum McpElicitationSingleSelectEnumSchema {
    Variant1(Box<McpElicitationUntitledSingleSelectEnumSchema>),
    Variant2(Box<McpElicitationTitledSingleSelectEnumSchema>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpElicitationStringFormat {
    #[serde(rename = "email")]
    Email,
    #[serde(rename = "uri")]
    Uri,
    #[serde(rename = "date")]
    Date,
    #[serde(rename = "date-time")]
    DateTime,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationStringSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<String>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "format",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#format: Option<Option<McpElicitationStringFormat>>,
    #[serde(
        rename = "maxLength",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#max_length: Option<Option<u32>>,
    #[serde(
        rename = "minLength",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#min_length: Option<Option<u32>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationStringType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpElicitationStringType {
    #[serde(rename = "string")]
    String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationTitledEnumItems {
    #[serde(rename = "anyOf", deserialize_with = "required")]
    pub r#any_of: Vec<McpElicitationConstOption>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationTitledMultiSelectEnumSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<Vec<String>>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "items", deserialize_with = "required")]
    pub r#items: McpElicitationTitledEnumItems,
    #[serde(
        rename = "maxItems",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#max_items: Option<Option<u64>>,
    #[serde(
        rename = "minItems",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#min_items: Option<Option<u64>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationArrayType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationTitledSingleSelectEnumSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<String>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "oneOf", deserialize_with = "required")]
    pub r#one_of: Vec<McpElicitationConstOption>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationStringType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationUntitledEnumItems {
    #[serde(rename = "enum", deserialize_with = "required")]
    pub r#enum: Vec<String>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationStringType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationUntitledMultiSelectEnumSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<Vec<String>>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "items", deserialize_with = "required")]
    pub r#items: McpElicitationUntitledEnumItems,
    #[serde(
        rename = "maxItems",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#max_items: Option<Option<u64>>,
    #[serde(
        rename = "minItems",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#min_items: Option<Option<u64>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationArrayType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationUntitledSingleSelectEnumSchema {
    #[serde(
        rename = "default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default: Option<Option<String>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "enum", deserialize_with = "required")]
    pub r#enum: Vec<String>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: McpElicitationStringType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpServerElicitationAction {
    #[serde(rename = "accept")]
    Accept,
    #[serde(rename = "decline")]
    Decline,
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode")]
pub enum McpServerElicitationRequestParams {
    #[serde(rename = "openai/userVerification")]
    OpenaiUserVerification {
        #[serde(rename = "serverName", deserialize_with = "required")]
        r#server_name: String,
        #[serde(rename = "threadId", deserialize_with = "required")]
        r#thread_id: String,
        #[serde(
            rename = "turnId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#turn_id: Option<Option<String>>,
        #[serde(
            rename = "_meta",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#_meta: Option<studio_json::Value>,
        #[serde(rename = "challenge", deserialize_with = "required")]
        r#challenge: String,
        #[serde(rename = "description", deserialize_with = "required")]
        r#description: String,
        #[serde(rename = "title", deserialize_with = "required")]
        r#title: String,
    },
    #[serde(rename = "form")]
    Form {
        #[serde(rename = "serverName", deserialize_with = "required")]
        r#server_name: String,
        #[serde(rename = "threadId", deserialize_with = "required")]
        r#thread_id: String,
        #[serde(
            rename = "turnId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#turn_id: Option<Option<String>>,
        #[serde(
            rename = "_meta",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#_meta: Option<studio_json::Value>,
        #[serde(rename = "message", deserialize_with = "required")]
        r#message: String,
        #[serde(rename = "requestedSchema", deserialize_with = "required")]
        r#requested_schema: McpElicitationSchema,
    },
    #[serde(rename = "openai/form")]
    OpenaiForm {
        #[serde(rename = "serverName", deserialize_with = "required")]
        r#server_name: String,
        #[serde(rename = "threadId", deserialize_with = "required")]
        r#thread_id: String,
        #[serde(
            rename = "turnId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#turn_id: Option<Option<String>>,
        #[serde(
            rename = "_meta",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#_meta: Option<studio_json::Value>,
        #[serde(rename = "message", deserialize_with = "required")]
        r#message: String,
        #[serde(rename = "requestedSchema", deserialize_with = "required")]
        r#requested_schema: studio_json::Value,
    },
    #[serde(rename = "openaiForm")]
    OpenaiFormVariant4 {
        #[serde(rename = "serverName", deserialize_with = "required")]
        r#server_name: String,
        #[serde(rename = "threadId", deserialize_with = "required")]
        r#thread_id: String,
        #[serde(
            rename = "turnId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#turn_id: Option<Option<String>>,
        #[serde(
            rename = "_meta",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#_meta: Option<studio_json::Value>,
        #[serde(rename = "message", deserialize_with = "required")]
        r#message: String,
        #[serde(rename = "requestedSchema", deserialize_with = "required")]
        r#requested_schema: studio_json::Value,
    },
    #[serde(rename = "url")]
    Url {
        #[serde(rename = "serverName", deserialize_with = "required")]
        r#server_name: String,
        #[serde(rename = "threadId", deserialize_with = "required")]
        r#thread_id: String,
        #[serde(
            rename = "turnId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#turn_id: Option<Option<String>>,
        #[serde(
            rename = "_meta",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#_meta: Option<studio_json::Value>,
        #[serde(rename = "elicitationId", deserialize_with = "required")]
        r#elicitation_id: String,
        #[serde(rename = "message", deserialize_with = "required")]
        r#message: String,
        #[serde(rename = "url", deserialize_with = "required")]
        r#url: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerElicitationRequestResponse {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(rename = "action", deserialize_with = "required")]
    pub r#action: McpServerElicitationAction,
    #[serde(
        rename = "content",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#content: Option<studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetworkApprovalContext {
    #[serde(rename = "host", deserialize_with = "required")]
    pub r#host: String,
    #[serde(rename = "protocol", deserialize_with = "required")]
    pub r#protocol: NetworkApprovalProtocol,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetworkPolicyAmendment {
    #[serde(rename = "action", deserialize_with = "required")]
    pub r#action: NetworkPolicyRuleAction,
    #[serde(rename = "host", deserialize_with = "required")]
    pub r#host: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkPolicyRuleAction {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ParsedCommand {
    #[serde(rename = "read")]
    Read {
        #[serde(rename = "cmd", deserialize_with = "required")]
        r#cmd: String,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: String,
    },
    #[serde(rename = "list_files")]
    ListFiles {
        #[serde(rename = "cmd", deserialize_with = "required")]
        r#cmd: String,
        #[serde(
            rename = "path",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#path: Option<Option<String>>,
    },
    #[serde(rename = "search")]
    Search {
        #[serde(rename = "cmd", deserialize_with = "required")]
        r#cmd: String,
        #[serde(
            rename = "path",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#path: Option<Option<String>>,
        #[serde(
            rename = "query",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#query: Option<Option<String>>,
    },
    #[serde(rename = "unknown")]
    Unknown {
        #[serde(rename = "cmd", deserialize_with = "required")]
        r#cmd: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PermissionGrantScope {
    #[serde(rename = "turn")]
    Turn,
    #[serde(rename = "session")]
    Session,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionsRequestApprovalParams {
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: LegacyAppPathString,
    #[serde(
        rename = "environmentId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environment_id: Option<Option<String>>,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "permissions", deserialize_with = "required")]
    pub r#permissions: RequestPermissionProfile,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
    #[serde(rename = "startedAtMs", deserialize_with = "required")]
    pub r#started_at_ms: i64,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionsRequestApprovalResponse {
    #[serde(rename = "permissions", deserialize_with = "required")]
    pub r#permissions: GrantedPermissionProfile,
    #[serde(
        rename = "scope",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#scope: Option<PermissionGrantScope>,
    #[serde(
        rename = "strictAutoReview",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#strict_auto_review: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Variant1(Box<String>),
    Variant2(Box<i64>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ReviewDecision {
    Variant1(Box<ReviewDecisionVariant1>),
    Variant2(Box<ReviewDecisionVariant2>),
    Variant3(Box<ReviewDecisionVariant3>),
    Variant4(Box<ReviewDecisionVariant4>),
    Variant5(Box<ReviewDecisionVariant5>),
    Variant6(Box<ReviewDecisionVariant6>),
    Variant7(Box<ReviewDecisionVariant7>),
    Variant8(Box<ReviewDecisionVariant8>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method")]
pub enum ServerNotification {
    #[serde(rename = "error")]
    Error {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ErrorNotification,
    },
    #[serde(rename = "thread/started")]
    ThreadStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadStartedNotification,
    },
    #[serde(rename = "thread/status/changed")]
    ThreadStatusChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadStatusChangedNotification,
    },
    #[serde(rename = "thread/readState/changed")]
    ThreadReadStateChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadReadStateChangedNotification,
    },
    #[serde(rename = "thread/archived")]
    ThreadArchived {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadArchivedNotification,
    },
    #[serde(rename = "thread/deleted")]
    ThreadDeleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadDeletedNotification,
    },
    #[serde(rename = "thread/unarchived")]
    ThreadUnarchived {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadUnarchivedNotification,
    },
    #[serde(rename = "thread/closed")]
    ThreadClosed {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadClosedNotification,
    },
    #[serde(rename = "thread/reverted")]
    ThreadReverted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRevertedNotification,
    },
    #[serde(rename = "skills/changed")]
    SkillsChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: SkillsChangedNotification,
    },
    #[serde(rename = "thread/name/updated")]
    ThreadNameUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadNameUpdatedNotification,
    },
    #[serde(rename = "thread/section/changed")]
    ThreadSectionChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSectionChangedNotification,
    },
    #[serde(rename = "thread/attachment/updated")]
    ThreadAttachmentUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadAttachmentUpdatedNotification,
    },
    #[serde(rename = "thread/goal/updated")]
    ThreadGoalUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadGoalUpdatedNotification,
    },
    #[serde(rename = "thread/prediction/updated")]
    ThreadPredictionUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadPredictionUpdatedNotification,
    },
    #[serde(rename = "thread/goal/cleared")]
    ThreadGoalCleared {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadGoalClearedNotification,
    },
    #[serde(rename = "thread/queue/changed")]
    ThreadQueueChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadQueueChangedNotification,
    },
    #[serde(rename = "project/changed")]
    ProjectChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProjectChangedNotification,
    },
    #[serde(rename = "thread/project/updated")]
    ThreadProjectUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadProjectUpdatedNotification,
    },
    #[serde(rename = "thread/environment/connected")]
    ThreadEnvironmentConnected {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: EnvironmentConnectionNotification,
    },
    #[serde(rename = "thread/environment/disconnected")]
    ThreadEnvironmentDisconnected {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: EnvironmentConnectionNotification,
    },
    #[serde(rename = "thread/settings/updated")]
    ThreadSettingsUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadSettingsUpdatedNotification,
    },
    #[serde(rename = "thread/tokenUsage/updated")]
    ThreadTokenUsageUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadTokenUsageUpdatedNotification,
    },
    #[serde(rename = "turn/started")]
    TurnStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnStartedNotification,
    },
    #[serde(rename = "hook/started")]
    HookStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: HookStartedNotification,
    },
    #[serde(rename = "turn/completed")]
    TurnCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnCompletedNotification,
    },
    #[serde(rename = "hook/completed")]
    HookCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: HookCompletedNotification,
    },
    #[serde(rename = "turn/diff/updated")]
    TurnDiffUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnDiffUpdatedNotification,
    },
    #[serde(rename = "turn/plan/updated")]
    TurnPlanUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnPlanUpdatedNotification,
    },
    #[serde(rename = "item/started")]
    ItemStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ItemStartedNotification,
    },
    #[serde(rename = "item/autoApprovalReview/started")]
    ItemAutoApprovalReviewStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ItemGuardianApprovalReviewStartedNotification,
    },
    #[serde(rename = "item/autoApprovalReview/completed")]
    ItemAutoApprovalReviewCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ItemGuardianApprovalReviewCompletedNotification,
    },
    #[serde(rename = "autoApprovalReview/strictReviewRequired")]
    AutoApprovalReviewStrictReviewRequired {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: StrictReviewRequiredNotification,
    },
    #[serde(rename = "item/completed")]
    ItemCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ItemCompletedNotification,
    },
    #[serde(rename = "item/agentMessage/delta")]
    ItemAgentMessageDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AgentMessageDeltaNotification,
    },
    #[serde(rename = "item/plan/delta")]
    ItemPlanDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PlanDeltaNotification,
    },
    #[serde(rename = "command/exec/outputDelta")]
    CommandExecOutputDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CommandExecOutputDeltaNotification,
    },
    #[serde(rename = "process/outputDelta")]
    ProcessOutputDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProcessOutputDeltaNotification,
    },
    #[serde(rename = "process/exited")]
    ProcessExited {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ProcessExitedNotification,
    },
    #[serde(rename = "item/commandExecution/outputDelta")]
    ItemCommandExecutionOutputDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CommandExecutionOutputDeltaNotification,
    },
    #[serde(rename = "item/commandExecution/terminalInteraction")]
    ItemCommandExecutionTerminalInteraction {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TerminalInteractionNotification,
    },
    #[serde(rename = "item/fileChange/outputDelta")]
    ItemFileChangeOutputDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FileChangeOutputDeltaNotification,
    },
    #[serde(rename = "item/fileChange/patchUpdated")]
    ItemFileChangePatchUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FileChangePatchUpdatedNotification,
    },
    #[serde(rename = "serverRequest/resolved")]
    ServerRequestResolved {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ServerRequestResolvedNotification,
    },
    #[serde(rename = "item/mcpToolCall/progress")]
    ItemMcpToolCallProgress {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpToolCallProgressNotification,
    },
    #[serde(rename = "mcpServer/oauthLogin/completed")]
    McpServerOauthLoginCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerOauthLoginCompletedNotification,
    },
    #[serde(rename = "mcpServer/startupStatus/updated")]
    McpServerStartupStatusUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerStatusUpdatedNotification,
    },
    #[serde(rename = "mcpServer/event/stream/notification")]
    McpServerEventStreamNotification {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerEventStreamNotification,
    },
    #[serde(rename = "account/updated")]
    AccountUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AccountUpdatedNotification,
    },
    #[serde(rename = "account/gatewayOAuth/changed")]
    AccountGatewayOAuthChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: GatewayOAuthChangedNotification,
    },
    #[serde(rename = "account/rateLimits/updated")]
    AccountRateLimitsUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AccountRateLimitsUpdatedNotification,
    },
    #[serde(rename = "app/list/updated")]
    AppListUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AppListUpdatedNotification,
    },
    #[serde(rename = "remoteControl/status/changed")]
    RemoteControlStatusChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: RemoteControlStatusChangedNotification,
    },
    #[serde(rename = "externalAgentConfig/import/progress")]
    ExternalAgentConfigImportProgress {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExternalAgentConfigImportProgressNotification,
    },
    #[serde(rename = "externalAgentConfig/import/completed")]
    ExternalAgentConfigImportCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExternalAgentConfigImportCompletedNotification,
    },
    #[serde(rename = "fs/changed")]
    FsChanged {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FsChangedNotification,
    },
    #[serde(rename = "item/reasoning/summaryTextDelta")]
    ItemReasoningSummaryTextDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ReasoningSummaryTextDeltaNotification,
    },
    #[serde(rename = "item/reasoning/summaryPartAdded")]
    ItemReasoningSummaryPartAdded {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ReasoningSummaryPartAddedNotification,
    },
    #[serde(rename = "item/reasoning/textDelta")]
    ItemReasoningTextDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ReasoningTextDeltaNotification,
    },
    #[serde(rename = "thread/compacted")]
    ThreadCompacted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ContextCompactedNotification,
    },
    #[serde(rename = "model/rerouted")]
    ModelRerouted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ModelReroutedNotification,
    },
    #[serde(rename = "model/verification")]
    ModelVerification {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ModelVerificationNotification,
    },
    #[serde(rename = "modelProvider/authRecoveryStarted")]
    ModelProviderAuthRecoveryStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AuthRecoveryNotification,
    },
    #[serde(rename = "modelProvider/authRecoveryCompleted")]
    ModelProviderAuthRecoveryCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AuthRecoveryNotification,
    },
    #[serde(rename = "turn/moderationMetadata")]
    TurnModerationMetadata {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: TurnModerationMetadataNotification,
    },
    #[serde(rename = "model/safetyBuffering/updated")]
    ModelSafetyBufferingUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ModelSafetyBufferingUpdatedNotification,
    },
    #[serde(rename = "warning")]
    Warning {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: WarningNotification,
    },
    #[serde(rename = "guardianWarning")]
    GuardianWarning {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: GuardianWarningNotification,
    },
    #[serde(rename = "deprecationNotice")]
    DeprecationNotice {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: DeprecationNoticeNotification,
    },
    #[serde(rename = "configWarning")]
    ConfigWarning {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ConfigWarningNotification,
    },
    #[serde(rename = "fuzzyFileSearch/sessionUpdated")]
    FuzzyFileSearchSessionUpdated {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FuzzyFileSearchSessionUpdatedNotification,
    },
    #[serde(rename = "fuzzyFileSearch/sessionCompleted")]
    FuzzyFileSearchSessionCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FuzzyFileSearchSessionCompletedNotification,
    },
    #[serde(rename = "thread/realtime/started")]
    ThreadRealtimeStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeStartedNotification,
    },
    #[serde(rename = "thread/realtime/itemAdded")]
    ThreadRealtimeItemAdded {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeItemAddedNotification,
    },
    #[serde(rename = "thread/realtime/item/started")]
    ThreadRealtimeItemStarted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeItemStartedNotification,
    },
    #[serde(rename = "thread/realtime/item/transcript/delta")]
    ThreadRealtimeItemTranscriptDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeItemTranscriptDeltaNotification,
    },
    #[serde(rename = "thread/realtime/item/completed")]
    ThreadRealtimeItemCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeItemCompletedNotification,
    },
    #[serde(rename = "thread/realtime/transcript/delta")]
    ThreadRealtimeTranscriptDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeTranscriptDeltaNotification,
    },
    #[serde(rename = "thread/realtime/transcript/done")]
    ThreadRealtimeTranscriptDone {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeTranscriptDoneNotification,
    },
    #[serde(rename = "thread/realtime/outputAudio/delta")]
    ThreadRealtimeOutputAudioDelta {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeOutputAudioDeltaNotification,
    },
    #[serde(rename = "thread/realtime/sdp")]
    ThreadRealtimeSdp {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeSdpNotification,
    },
    #[serde(rename = "thread/realtime/error")]
    ThreadRealtimeError {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeErrorNotification,
    },
    #[serde(rename = "thread/realtime/closed")]
    ThreadRealtimeClosed {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ThreadRealtimeClosedNotification,
    },
    #[serde(rename = "windows/worldWritableWarning")]
    WindowsWorldWritableWarning {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: WindowsWorldWritableWarningNotification,
    },
    #[serde(rename = "windowsSandbox/setupCompleted")]
    WindowsSandboxSetupCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: WindowsSandboxSetupCompletedNotification,
    },
    #[serde(rename = "account/login/completed")]
    AccountLoginCompleted {
        #[serde(
            rename = "emittedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#emitted_at_ms: Option<i64>,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AccountLoginCompletedNotification,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method")]
pub enum ServerRequest {
    #[serde(rename = "item/commandExecution/requestApproval")]
    ItemCommandExecutionRequestApproval {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CommandExecutionRequestApprovalParams,
    },
    #[serde(rename = "item/fileChange/requestApproval")]
    ItemFileChangeRequestApproval {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: FileChangeRequestApprovalParams,
    },
    #[serde(rename = "item/tool/requestUserInput")]
    ItemToolRequestUserInput {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ToolRequestUserInputParams,
    },
    #[serde(rename = "mcpServer/elicitation/request")]
    McpServerElicitationRequest {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: McpServerElicitationRequestParams,
    },
    #[serde(rename = "item/permissions/requestApproval")]
    ItemPermissionsRequestApproval {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: PermissionsRequestApprovalParams,
    },
    #[serde(rename = "item/tool/call")]
    ItemToolCall {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: DynamicToolCallParams,
    },
    #[serde(rename = "account/chatgptAuthTokens/refresh")]
    AccountChatgptAuthTokensRefresh {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ChatgptAuthTokensRefreshParams,
    },
    #[serde(rename = "attestation/generate")]
    AttestationGenerate {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: AttestationGenerateParams,
    },
    #[serde(rename = "currentTime/read")]
    CurrentTimeRead {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: CurrentTimeReadParams,
    },
    #[serde(rename = "applyPatchApproval")]
    ApplyPatchApproval {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ApplyPatchApprovalParams,
    },
    #[serde(rename = "execCommandApproval")]
    ExecCommandApproval {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: RequestId,
        #[serde(rename = "params", deserialize_with = "required")]
        r#params: ExecCommandApprovalParams,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRequestUserInputAnswer {
    #[serde(rename = "answers", deserialize_with = "required")]
    pub r#answers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRequestUserInputOption {
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(rename = "label", deserialize_with = "required")]
    pub r#label: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRequestUserInputParams {
    #[serde(
        rename = "autoResolutionMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#auto_resolution_ms: Option<Option<u64>>,
    #[serde(rename = "isBlocking", deserialize_with = "required")]
    pub r#is_blocking: bool,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "questions", deserialize_with = "required")]
    pub r#questions: Vec<ToolRequestUserInputQuestion>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRequestUserInputQuestion {
    #[serde(rename = "header", deserialize_with = "required")]
    pub r#header: String,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(
        rename = "isOther",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#is_other: Option<bool>,
    #[serde(
        rename = "isSecret",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#is_secret: Option<bool>,
    #[serde(
        rename = "options",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#options: Option<Option<Vec<ToolRequestUserInputOption>>>,
    #[serde(rename = "question", deserialize_with = "required")]
    pub r#question: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRequestUserInputResponse {
    #[serde(rename = "answers", deserialize_with = "required")]
    pub r#answers: std::collections::BTreeMap<String, ToolRequestUserInputAnswer>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct W3cTraceContext {
    #[serde(
        rename = "traceparent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#traceparent: Option<Option<String>>,
    #[serde(
        rename = "tracestate",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tracestate: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Account {
    #[serde(rename = "apiKey")]
    ApiKey,
    #[serde(rename = "chatgpt")]
    Chatgpt {
        #[serde(rename = "email", deserialize_with = "required")]
        r#email: Option<String>,
        #[serde(rename = "planType", deserialize_with = "required")]
        r#plan_type: PlanType,
        #[serde(
            rename = "workspaceName",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#workspace_name: Option<Option<String>>,
    },
    #[serde(rename = "amazonBedrock")]
    AmazonBedrock {
        #[serde(
            rename = "usesCodexManagedCredentials",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#uses_codex_managed_credentials: Option<bool>,
    },
    #[serde(rename = "gcp")]
    Gcp,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountLoginCompletedNotification {
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<String>>,
    #[serde(
        rename = "loginId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#login_id: Option<Option<String>>,
    #[serde(
        rename = "onboardingEntrypoint",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#onboarding_entrypoint: Option<Option<DesktopOnboardingEntrypoint>>,
    #[serde(rename = "success", deserialize_with = "required")]
    pub r#success: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountRateLimitsUpdatedNotification {
    #[serde(rename = "rateLimits", deserialize_with = "required")]
    pub r#rate_limits: RateLimitSnapshot,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AccountRoutingOverride {
    #[serde(rename = "NO_CONSTRAINT")]
    NOCONSTRAINT,
    #[serde(rename = "us")]
    Us,
    #[serde(rename = "us_cr")]
    UsCr,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountSession {
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#display_name: Option<Option<String>>,
    #[serde(
        rename = "email",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#email: Option<Option<String>>,
    #[serde(
        rename = "imageUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#image_url: Option<Option<String>>,
    #[serde(rename = "isActive", deserialize_with = "required")]
    pub r#is_active: bool,
    #[serde(rename = "lastUsedAt", deserialize_with = "required")]
    pub r#last_used_at: i64,
    #[serde(
        rename = "selectedWorkspaceAccountId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#selected_workspace_account_id: Option<Option<String>>,
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
    #[serde(
        rename = "userId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#user_id: Option<Option<String>>,
    #[serde(rename = "workspaces", deserialize_with = "required")]
    pub r#workspaces: Vec<AccountSessionWorkspace>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountSessionWorkspace {
    #[serde(rename = "accountId", deserialize_with = "required")]
    pub r#account_id: String,
    #[serde(
        rename = "imageUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#image_url: Option<Option<String>>,
    #[serde(
        rename = "kind",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#kind: Option<Option<AccountSessionWorkspaceKind>>,
    #[serde(
        rename = "name",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#name: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AccountSessionWorkspaceKind {
    #[serde(rename = "personal")]
    Personal,
    #[serde(rename = "workspace")]
    Workspace,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountSessionsListParams {
    #[serde(
        rename = "refreshWorkspaceMetadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#refresh_workspace_metadata: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountSessionsResponse {
    #[serde(
        rename = "activeSessionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#active_session_id: Option<Option<String>>,
    #[serde(rename = "sessions", deserialize_with = "required")]
    pub r#sessions: Vec<AccountSession>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountSessionsSwitchParams {
    #[serde(rename = "accountId", deserialize_with = "required")]
    pub r#account_id: String,
    #[serde(
        rename = "allowBrowserLogin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_browser_login: Option<bool>,
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum AccountSessionsSwitchResponse {
    Variant1(Box<AccountSessionsSwitchResponseVariant1>),
    Variant2(Box<AccountSessionsSwitchResponseVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountTokenUsageDailyBucket {
    #[serde(rename = "startDate", deserialize_with = "required")]
    pub r#start_date: String,
    #[serde(rename = "tokens", deserialize_with = "required")]
    pub r#tokens: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountTokenUsageSummary {
    #[serde(
        rename = "currentStreakDays",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#current_streak_days: Option<Option<i64>>,
    #[serde(
        rename = "lifetimeTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#lifetime_tokens: Option<Option<i64>>,
    #[serde(
        rename = "longestRunningTurnSec",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#longest_running_turn_sec: Option<Option<i64>>,
    #[serde(
        rename = "longestStreakDays",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#longest_streak_days: Option<Option<i64>>,
    #[serde(
        rename = "peakDailyTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#peak_daily_tokens: Option<Option<i64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountUpdatedNotification {
    #[serde(
        rename = "authMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#auth_mode: Option<Option<AuthMode>>,
    #[serde(
        rename = "planType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#plan_type: Option<Option<PlanType>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActivePermissionProfile {
    #[serde(
        rename = "extends",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#extends: Option<Option<String>>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AddCreditsNudgeCreditType {
    #[serde(rename = "credits")]
    Credits,
    #[serde(rename = "usage_limit")]
    UsageLimit,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AddCreditsNudgeEmailStatus {
    #[serde(rename = "sent")]
    Sent,
    #[serde(rename = "cooldown_active")]
    CooldownActive,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AdditionalContextEntry {
    #[serde(rename = "kind", deserialize_with = "required")]
    pub r#kind: AdditionalContextKind,
    #[serde(
        rename = "name",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#name: Option<Option<String>>,
    #[serde(rename = "value", deserialize_with = "required")]
    pub r#value: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AdditionalContextKind {
    #[serde(rename = "untrusted")]
    Untrusted,
    #[serde(rename = "application")]
    Application,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AdditionalFileSystemPermissions {
    #[serde(
        rename = "entries",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#entries: Option<Option<Vec<FileSystemSandboxEntry>>>,
    #[serde(
        rename = "globScanMaxDepth",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#glob_scan_max_depth: Option<Option<u64>>,
    #[serde(
        rename = "read",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#read: Option<Option<Vec<LegacyAppPathString>>>,
    #[serde(
        rename = "write",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#write: Option<Option<Vec<LegacyAppPathString>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AdditionalNetworkPermissions {
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enabled: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AgentMessageDelivery {
    #[serde(rename = "async")]
    Async,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentMessageDeltaNotification {
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AgentMessageInputContent {
    #[serde(rename = "input_text")]
    InputText {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "encrypted_content")]
    EncryptedContent {
        #[serde(rename = "encrypted_content", deserialize_with = "required")]
        r#encrypted_content: String,
    },
}

pub type AgentPath = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AllowDenyRequirement {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnalyticsConfig {
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enabled: Option<Option<bool>>,
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppBranding {
    #[serde(
        rename = "category",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#category: Option<Option<String>>,
    #[serde(
        rename = "developer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer: Option<Option<String>>,
    #[serde(rename = "isDiscoverableApp", deserialize_with = "required")]
    pub r#is_discoverable_app: bool,
    #[serde(
        rename = "privacyPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#privacy_policy: Option<Option<String>>,
    #[serde(
        rename = "termsOfService",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#terms_of_service: Option<Option<String>>,
    #[serde(
        rename = "website",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#website: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    #[serde(
        rename = "approvals_reviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "default_tools_approval_mode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_tools_approval_mode: Option<Option<AppToolApproval>>,
    #[serde(
        rename = "default_tools_enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_tools_enabled: Option<Option<bool>>,
    #[serde(
        rename = "destructive_enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#destructive_enabled: Option<Option<bool>>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enabled: Option<bool>,
    #[serde(
        rename = "links",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#links: Option<Option<AppLinksConfig>>,
    #[serde(
        rename = "omit_tools_from",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#omit_tools_from: Option<Option<Vec<ToolExposureSurface>>>,
    #[serde(
        rename = "open_world_enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#open_world_enabled: Option<Option<bool>>,
    #[serde(
        rename = "tools",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tools: Option<Option<AppToolsConfig>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppInfo {
    #[serde(
        rename = "appMetadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#app_metadata: Option<Option<AppMetadata>>,
    #[serde(
        rename = "branding",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#branding: Option<Option<AppBranding>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "distributionChannel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#distribution_channel: Option<Option<String>>,
    #[serde(
        rename = "iconAssets",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_assets: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "iconDarkAssets",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_dark_assets: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(
        rename = "installUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#install_url: Option<Option<String>>,
    #[serde(
        rename = "isAccessible",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#is_accessible: Option<bool>,
    #[serde(
        rename = "isEnabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#is_enabled: Option<bool>,
    #[serde(
        rename = "labels",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#labels: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "logoUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo_url: Option<Option<String>>,
    #[serde(
        rename = "logoUrlDark",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo_url_dark: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "pluginDisplayNames",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#plugin_display_names: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppLinkConfig {
    #[serde(
        rename = "approvals_reviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "default_tools_approval_mode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_tools_approval_mode: Option<Option<AppToolApproval>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppLinksConfig {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppListUpdatedNotification {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<AppInfo>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppMetadata {
    #[serde(
        rename = "categories",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#categories: Option<Option<Vec<String>>>,
    #[serde(
        rename = "developer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer: Option<Option<String>>,
    #[serde(
        rename = "firstPartyRequiresInstall",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#first_party_requires_install: Option<Option<bool>>,
    #[serde(
        rename = "review",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#review: Option<Option<AppReview>>,
    #[serde(
        rename = "screenshots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#screenshots: Option<Option<Vec<AppScreenshot>>>,
    #[serde(
        rename = "seoDescription",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#seo_description: Option<Option<String>>,
    #[serde(
        rename = "showInComposerWhenUnlinked",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#show_in_composer_when_unlinked: Option<Option<bool>>,
    #[serde(
        rename = "subCategories",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sub_categories: Option<Option<Vec<String>>>,
    #[serde(
        rename = "version",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#version: Option<Option<String>>,
    #[serde(
        rename = "versionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#version_id: Option<Option<String>>,
    #[serde(
        rename = "versionNotes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#version_notes: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppReview {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppScreenshot {
    #[serde(
        rename = "fileId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#file_id: Option<Option<String>>,
    #[serde(
        rename = "url",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#url: Option<Option<String>>,
    #[serde(rename = "userPrompt", deserialize_with = "required")]
    pub r#user_prompt: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppSummary {
    #[serde(
        rename = "category",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#category: Option<Option<String>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(
        rename = "installUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#install_url: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppTemplateSummary {
    #[serde(
        rename = "canonicalConnectorId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#canonical_connector_id: Option<Option<String>>,
    #[serde(
        rename = "category",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#category: Option<Option<String>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "logoUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo_url: Option<Option<String>>,
    #[serde(
        rename = "logoUrlDark",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo_url_dark: Option<Option<String>>,
    #[serde(rename = "materializedAppIds", deserialize_with = "required")]
    pub r#materialized_app_ids: Vec<String>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<AppTemplateUnavailableReason>>,
    #[serde(rename = "templateId", deserialize_with = "required")]
    pub r#template_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AppTemplateUnavailableReason {
    #[serde(rename = "NOT_CONFIGURED_FOR_WORKSPACE")]
    NOTCONFIGUREDFORWORKSPACE,
    #[serde(rename = "NO_ACTIVE_WORKSPACE")]
    NOACTIVEWORKSPACE,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AppToolApproval {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "prompt")]
    Prompt,
    #[serde(rename = "writes")]
    Writes,
    #[serde(rename = "approve")]
    Approve,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppToolConfig {
    #[serde(
        rename = "approval_mode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_mode: Option<Option<AppToolApproval>>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enabled: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppToolSummary {
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(
        rename = "disabledReason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_reason: Option<Option<String>>,
    #[serde(
        rename = "isEnabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#is_enabled: Option<bool>,
    #[serde(
        rename = "isReadOnly",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#is_read_only: Option<bool>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppToolsConfig {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ApplicationNetworkRequirements {
    #[serde(rename = "domains", deserialize_with = "required")]
    pub r#domains: std::collections::BTreeMap<String, NetworkDomainPermission>,
    #[serde(rename = "enabled", deserialize_with = "required")]
    pub r#enabled: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ApplicationRequirements {
    #[serde(
        rename = "network",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#network: Option<Option<ApplicationNetworkRequirements>>,
    #[serde(
        rename = "storage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#storage: Option<Option<ApplicationStorageRequirements>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ApplicationStorageRequirements {
    #[serde(
        rename = "defaultProjectParent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_project_parent: Option<Option<LegacyAppPathString>>,
    #[serde(
        rename = "projectlessWorkspaceRoot",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#projectless_workspace_root: Option<Option<LegacyAppPathString>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ApprovalsReviewer {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "auto_review")]
    AutoReview,
    #[serde(rename = "guardian_subagent")]
    GuardianSubagent,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsConfig {
    #[serde(
        rename = "_default",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_default: Option<Option<AppsDefaultConfig>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsDefaultConfig {
    #[serde(
        rename = "approvals_reviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "default_tools_approval_mode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_tools_approval_mode: Option<Option<AppToolApproval>>,
    #[serde(
        rename = "destructive_enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#destructive_enabled: Option<bool>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enabled: Option<bool>,
    #[serde(
        rename = "open_world_enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#open_world_enabled: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsInstalledParams {
    #[serde(
        rename = "forceRefresh",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#force_refresh: Option<bool>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsInstalledResponse {
    #[serde(rename = "apps", deserialize_with = "required")]
    pub r#apps: Vec<InstalledApp>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "forceRefetch",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#force_refetch: Option<bool>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<AppInfo>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsReadParams {
    #[serde(rename = "appIds", deserialize_with = "required")]
    pub r#app_ids: Vec<String>,
    #[serde(
        rename = "includeTools",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_tools: Option<bool>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppsReadResponse {
    #[serde(rename = "apps", deserialize_with = "required")]
    pub r#apps: Vec<ConnectorMetadata>,
    #[serde(rename = "missingAppIds", deserialize_with = "required")]
    pub r#missing_app_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum AskForApproval {
    Variant1(Box<AskForApprovalVariant1>),
    Variant2(Box<AskForApprovalVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsyncUserInputQuestion {
    #[serde(
        rename = "options",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#options: Option<Option<Vec<String>>>,
    #[serde(rename = "title", deserialize_with = "required")]
    pub r#title: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum AuthMode {
    Variant1(Box<AuthModeVariant1>),
    Variant2(Box<AuthModeVariant2>),
    Variant3(Box<AuthModeVariant3>),
    Variant4(Box<AuthModeVariant4>),
    Variant5(Box<AuthModeVariant5>),
    Variant6(Box<AuthModeVariant6>),
    Variant7(Box<AuthModeVariant7>),
    Variant8(Box<AuthModeVariant8>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AuthRecoveryNotification {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(rename = "provider", deserialize_with = "required")]
    pub r#provider: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum AutoCompactTokenLimitScope {
    Variant1(Box<AutoCompactTokenLimitScopeVariant1>),
    Variant2(Box<AutoCompactTokenLimitScopeVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AutoReviewDecisionSource {
    #[serde(rename = "agent")]
    Agent,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AutoReviewRequirements {
    #[serde(
        rename = "ignoreRules",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ignore_rules: Option<Option<Vec<String>>>,
    #[serde(
        rename = "requiredOnModels",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#required_on_models: Option<Option<Vec<String>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AwsCredentialType {
    #[serde(rename = "accessKeys")]
    AccessKeys,
    #[serde(rename = "bedrockApiKey")]
    BedrockApiKey,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BedrockAwsProfile {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "region",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#region: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BedrockCheckGovCloudRequirementsParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BedrockCheckGovCloudRequirementsResponse {
    #[serde(rename = "isGovCloud", deserialize_with = "required")]
    pub r#is_gov_cloud: bool,
    #[serde(rename = "shouldWarn", deserialize_with = "required")]
    pub r#should_warn: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BedrockDiscoverParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BedrockDiscoverResponse {
    #[serde(rename = "environmentCredentials", deserialize_with = "required")]
    pub r#environment_credentials: Vec<BedrockEnvironmentCredential>,
    #[serde(rename = "profiles", deserialize_with = "required")]
    pub r#profiles: Vec<BedrockAwsProfile>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BedrockEnvironmentCredential {
    #[serde(
        rename = "region",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#region: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: AwsCredentialType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum BedrockSetupParams {
    #[serde(rename = "profile")]
    Profile {
        #[serde(rename = "profile", deserialize_with = "required")]
        r#profile: String,
        #[serde(rename = "region", deserialize_with = "required")]
        r#region: String,
    },
    #[serde(rename = "environment")]
    Environment {
        #[serde(rename = "region", deserialize_with = "required")]
        r#region: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BedrockSetupResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrowserUseAccessApprovalLifetime {
    #[serde(rename = "turn")]
    Turn,
    #[serde(rename = "thread")]
    Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrowserUseConfig {
    #[serde(
        rename = "allow_history_access",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_history_access: Option<Option<bool>>,
    #[serde(
        rename = "default_origin_policy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_origin_policy: Option<Option<BrowserUseOriginPolicyConfig>>,
    #[serde(
        rename = "origins",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origins: Option<Option<std::collections::BTreeMap<String, BrowserUseOriginPolicyConfig>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrowserUseExtensionRequirements {
    #[serde(
        rename = "requestHeaders",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#request_headers: Option<Option<Vec<RequestHeader>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrowserUseOriginPolicy {
    #[serde(
        rename = "access",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#access: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "accessApprovalLifetime",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#access_approval_lifetime: Option<Option<BrowserUseAccessApprovalLifetime>>,
    #[serde(
        rename = "autoReview",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#auto_review: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "downloads",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#downloads: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "fullCdpAccess",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#full_cdp_access: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "persistentApproval",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#persistent_approval: Option<Option<bool>>,
    #[serde(
        rename = "uploads",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#uploads: Option<Option<AllowDenyRequirement>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrowserUseOriginPolicyConfig {
    #[serde(
        rename = "access",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#access: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "downloads",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#downloads: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "full_cdp_access",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#full_cdp_access: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "uploads",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#uploads: Option<Option<AllowDenyRequirement>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BrowserUseRequirements {
    #[serde(
        rename = "allowGlobalPersistentApproval",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_global_persistent_approval: Option<Option<bool>>,
    #[serde(
        rename = "allowHistoryAccess",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_history_access: Option<Option<bool>>,
    #[serde(
        rename = "allowWebmcp",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_webmcp: Option<Option<bool>>,
    #[serde(
        rename = "defaultOriginPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_origin_policy: Option<Option<BrowserUseOriginPolicy>>,
    #[serde(
        rename = "disableAutoReview",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disable_auto_review: Option<Option<bool>>,
    #[serde(
        rename = "extension",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#extension: Option<Option<BrowserUseExtensionRequirements>>,
    #[serde(
        rename = "origins",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origins: Option<Option<std::collections::BTreeMap<String, BrowserUseOriginPolicy>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ByteRange {
    #[serde(rename = "end", deserialize_with = "required")]
    pub r#end: u64,
    #[serde(rename = "start", deserialize_with = "required")]
    pub r#start: u64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CancelLoginAccountParams {
    #[serde(rename = "loginId", deserialize_with = "required")]
    pub r#login_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CancelLoginAccountResponse {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: CancelLoginAccountStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CancelLoginAccountStatus {
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "notFound")]
    NotFound,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum CapabilityRootLocation {
    #[serde(rename = "environment")]
    Environment {
        #[serde(rename = "environmentId", deserialize_with = "required")]
        r#environment_id: String,
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CliAuthCredentialsStoreMode {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "keyring")]
    Keyring,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "ephemeral")]
    Ephemeral,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CodexErrorInfo {
    Variant1(Box<CodexErrorInfoVariant1>),
    Variant2(Box<CodexErrorInfoVariant2>),
    Variant3(Box<CodexErrorInfoVariant3>),
    Variant4(Box<CodexErrorInfoVariant4>),
    Variant5(Box<CodexErrorInfoVariant5>),
    Variant6(Box<CodexErrorInfoVariant6>),
    Variant7(Box<CodexErrorInfoVariant7>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CodexResponseHandoffMode {
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "commentary")]
    Commentary,
    #[serde(rename = "bemTags")]
    BemTags,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CollabAgentState {
    #[serde(
        rename = "message",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#message: Option<Option<String>>,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: CollabAgentStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CollabAgentStatus {
    #[serde(rename = "pendingInit")]
    PendingInit,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "errored")]
    Errored,
    #[serde(rename = "shutdown")]
    Shutdown,
    #[serde(rename = "notFound")]
    NotFound,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CollabAgentTool {
    #[serde(rename = "spawnAgent")]
    SpawnAgent,
    #[serde(rename = "sendInput")]
    SendInput,
    #[serde(rename = "resumeAgent")]
    ResumeAgent,
    #[serde(rename = "wait")]
    Wait,
    #[serde(rename = "closeAgent")]
    CloseAgent,
    #[serde(rename = "sendMessage")]
    SendMessage,
    #[serde(rename = "followupTask")]
    FollowupTask,
    #[serde(rename = "interruptAgent")]
    InterruptAgent,
    #[serde(rename = "listAgents")]
    ListAgents,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CollabAgentToolCallStatus {
    #[serde(rename = "inProgress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "interrupted")]
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CollaborationMode {
    #[serde(rename = "mode", deserialize_with = "required")]
    pub r#mode: ModeKind,
    #[serde(rename = "settings", deserialize_with = "required")]
    pub r#settings: Settings,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CollaborationModeListParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CollaborationModeListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<CollaborationModeMask>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CollaborationModeMask {
    #[serde(
        rename = "mode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mode: Option<Option<ModeKind>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "reasoning_effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reasoning_effort: Option<Option<Option<ReasoningEffort>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum CommandAction {
    #[serde(rename = "read")]
    Read {
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: LegacyAppPathString,
    },
    #[serde(rename = "listFiles")]
    ListFiles {
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
        #[serde(
            rename = "path",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#path: Option<Option<String>>,
    },
    #[serde(rename = "search")]
    Search {
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
        #[serde(
            rename = "path",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#path: Option<Option<String>>,
        #[serde(
            rename = "query",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#query: Option<Option<String>>,
    },
    #[serde(rename = "unknown")]
    Unknown {
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecOutputDeltaNotification {
    #[serde(rename = "capReached", deserialize_with = "required")]
    pub r#cap_reached: bool,
    #[serde(rename = "deltaBase64", deserialize_with = "required")]
    pub r#delta_base64: String,
    #[serde(rename = "processId", deserialize_with = "required")]
    pub r#process_id: String,
    #[serde(rename = "stream", deserialize_with = "required")]
    pub r#stream: CommandExecOutputStream,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CommandExecOutputStream {
    Variant1(Box<CommandExecOutputStreamVariant1>),
    Variant2(Box<CommandExecOutputStreamVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecParams {
    #[serde(rename = "command", deserialize_with = "required")]
    pub r#command: Vec<String>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "disableOutputCap",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disable_output_cap: Option<bool>,
    #[serde(
        rename = "disableTimeout",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disable_timeout: Option<bool>,
    #[serde(
        rename = "env",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#env: Option<Option<std::collections::BTreeMap<String, Option<String>>>>,
    #[serde(
        rename = "outputBytesCap",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#output_bytes_cap: Option<Option<u64>>,
    #[serde(
        rename = "permissionProfile",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#permission_profile: Option<Option<String>>,
    #[serde(
        rename = "processId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#process_id: Option<Option<String>>,
    #[serde(
        rename = "sandboxPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox_policy: Option<Option<SandboxPolicy>>,
    #[serde(
        rename = "size",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#size: Option<Option<CommandExecTerminalSize>>,
    #[serde(
        rename = "streamStdin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#stream_stdin: Option<bool>,
    #[serde(
        rename = "streamStdoutStderr",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#stream_stdout_stderr: Option<bool>,
    #[serde(
        rename = "timeoutMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#timeout_ms: Option<Option<i64>>,
    #[serde(
        rename = "tty",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tty: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecResizeParams {
    #[serde(rename = "processId", deserialize_with = "required")]
    pub r#process_id: String,
    #[serde(rename = "size", deserialize_with = "required")]
    pub r#size: CommandExecTerminalSize,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecResizeResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecResponse {
    #[serde(rename = "exitCode", deserialize_with = "required")]
    pub r#exit_code: i32,
    #[serde(rename = "stderr", deserialize_with = "required")]
    pub r#stderr: String,
    #[serde(rename = "stdout", deserialize_with = "required")]
    pub r#stdout: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecTerminalSize {
    #[serde(rename = "cols", deserialize_with = "required")]
    pub r#cols: u16,
    #[serde(rename = "rows", deserialize_with = "required")]
    pub r#rows: u16,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecTerminateParams {
    #[serde(rename = "processId", deserialize_with = "required")]
    pub r#process_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecTerminateResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecWriteParams {
    #[serde(
        rename = "closeStdin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#close_stdin: Option<bool>,
    #[serde(
        rename = "deltaBase64",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#delta_base64: Option<Option<String>>,
    #[serde(rename = "processId", deserialize_with = "required")]
    pub r#process_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecWriteResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecutionOutputDeltaNotification {
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionSource {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "userShell")]
    UserShell,
    #[serde(rename = "unifiedExecStartup")]
    UnifiedExecStartup,
    #[serde(rename = "unifiedExecInteraction")]
    UnifiedExecInteraction,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionStatus {
    #[serde(rename = "inProgress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "declined")]
    Declined,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandMigration {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseConfig {
    #[serde(
        rename = "default_app_access",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_app_access: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "macos",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#macos: Option<Option<ComputerUseMacosConfig>>,
    #[serde(
        rename = "windows",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#windows: Option<Option<ComputerUseWindowsConfig>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseMacosConfig {
    #[serde(
        rename = "bundle_ids",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#bundle_ids: Option<Option<std::collections::BTreeMap<String, AllowDenyRequirement>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseMacosRequirements {
    #[serde(
        rename = "bundleIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#bundle_ids: Option<Option<std::collections::BTreeMap<String, AllowDenyRequirement>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseRequirements {
    #[serde(
        rename = "allowLockedComputerUse",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_locked_computer_use: Option<Option<bool>>,
    #[serde(
        rename = "allowPersistentApproval",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_persistent_approval: Option<Option<bool>>,
    #[serde(
        rename = "defaultAppAccess",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_app_access: Option<Option<AllowDenyRequirement>>,
    #[serde(
        rename = "macos",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#macos: Option<Option<ComputerUseMacosRequirements>>,
    #[serde(
        rename = "windows",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#windows: Option<Option<ComputerUseWindowsRequirements>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseWindowsConfig {
    #[serde(
        rename = "aumids",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#aumids: Option<Option<std::collections::BTreeMap<String, AllowDenyRequirement>>>,
    #[serde(
        rename = "exes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exes: Option<Option<Vec<ComputerUseWindowsExeConfig>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseWindowsExeConfig {
    #[serde(rename = "access", deserialize_with = "required")]
    pub r#access: AllowDenyRequirement,
    #[serde(
        rename = "binary_name",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#binary_name: Option<Option<String>>,
    #[serde(rename = "product_name", deserialize_with = "required")]
    pub r#product_name: String,
    #[serde(rename = "publisher_name", deserialize_with = "required")]
    pub r#publisher_name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseWindowsExeRequirement {
    #[serde(rename = "access", deserialize_with = "required")]
    pub r#access: AllowDenyRequirement,
    #[serde(
        rename = "binaryName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#binary_name: Option<Option<String>>,
    #[serde(rename = "productName", deserialize_with = "required")]
    pub r#product_name: String,
    #[serde(rename = "publisherName", deserialize_with = "required")]
    pub r#publisher_name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ComputerUseWindowsRequirements {
    #[serde(
        rename = "aumids",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#aumids: Option<Option<std::collections::BTreeMap<String, AllowDenyRequirement>>>,
    #[serde(
        rename = "exes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exes: Option<Option<Vec<ComputerUseWindowsExeRequirement>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(
        rename = "analytics",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#analytics: Option<Option<AnalyticsConfig>>,
    #[serde(
        rename = "approval_policy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_policy: Option<Option<AskForApproval>>,
    #[serde(
        rename = "approvals_reviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "apps",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#apps: Option<Option<AppsConfig>>,
    #[serde(
        rename = "browser_use",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#browser_use: Option<Option<BrowserUseConfig>>,
    #[serde(
        rename = "compact_prompt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#compact_prompt: Option<Option<String>>,
    #[serde(
        rename = "computer_use",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#computer_use: Option<Option<ComputerUseConfig>>,
    #[serde(
        rename = "desktop",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#desktop: Option<Option<std::collections::BTreeMap<String, studio_json::Value>>>,
    #[serde(
        rename = "developer_instructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer_instructions: Option<Option<String>>,
    #[serde(
        rename = "forced_chatgpt_workspace_id",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#forced_chatgpt_workspace_id: Option<Option<ForcedChatgptWorkspaceIds>>,
    #[serde(
        rename = "forced_login_method",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#forced_login_method: Option<Option<ForcedLoginMethod>>,
    #[serde(
        rename = "instructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#instructions: Option<Option<String>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "model_auto_compact_token_limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_auto_compact_token_limit: Option<Option<i64>>,
    #[serde(
        rename = "model_auto_compact_token_limit_scope",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_auto_compact_token_limit_scope: Option<Option<AutoCompactTokenLimitScope>>,
    #[serde(
        rename = "model_context_window",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_context_window: Option<Option<i64>>,
    #[serde(
        rename = "model_provider",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_provider: Option<Option<String>>,
    #[serde(
        rename = "model_reasoning_effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_reasoning_effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "model_reasoning_summary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_reasoning_summary: Option<Option<ReasoningSummary>>,
    #[serde(
        rename = "model_verbosity",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_verbosity: Option<Option<Verbosity>>,
    #[serde(
        rename = "review_model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#review_model: Option<Option<String>>,
    #[serde(
        rename = "sandbox_mode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox_mode: Option<Option<SandboxMode>>,
    #[serde(
        rename = "sandbox_workspace_write",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox_workspace_write: Option<Option<SandboxWorkspaceWrite>>,
    #[serde(
        rename = "service_tier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(
        rename = "tools",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tools: Option<Option<ToolsV2>>,
    #[serde(
        rename = "web_search",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#web_search: Option<Option<WebSearchMode>>,
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigBatchWriteParams {
    #[serde(rename = "edits", deserialize_with = "required")]
    pub r#edits: Vec<ConfigEdit>,
    #[serde(
        rename = "expectedVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#expected_version: Option<Option<String>>,
    #[serde(
        rename = "filePath",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#file_path: Option<Option<String>>,
    #[serde(
        rename = "reloadUserConfig",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reload_user_config: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigEdit {
    #[serde(rename = "keyPath", deserialize_with = "required")]
    pub r#key_path: String,
    #[serde(rename = "mergeStrategy", deserialize_with = "required")]
    pub r#merge_strategy: MergeStrategy,
    #[serde(rename = "value", deserialize_with = "required")]
    pub r#value: studio_json::Value,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigLayer {
    #[serde(rename = "config", deserialize_with = "required")]
    pub r#config: studio_json::Value,
    #[serde(
        rename = "disabledReason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_reason: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: ConfigLayerSource,
    #[serde(rename = "version", deserialize_with = "required")]
    pub r#version: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigLayerMetadata {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: ConfigLayerSource,
    #[serde(rename = "version", deserialize_with = "required")]
    pub r#version: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ConfigLayerSource {
    #[serde(rename = "packagedDefaults")]
    PackagedDefaults {
        #[serde(rename = "file", deserialize_with = "required")]
        r#file: AbsolutePathBuf,
    },
    #[serde(rename = "mdm")]
    Mdm {
        #[serde(rename = "domain", deserialize_with = "required")]
        r#domain: String,
        #[serde(rename = "key", deserialize_with = "required")]
        r#key: String,
    },
    #[serde(rename = "system")]
    System {
        #[serde(rename = "file", deserialize_with = "required")]
        r#file: AbsolutePathBuf,
    },
    #[serde(rename = "enterpriseManaged")]
    EnterpriseManaged {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
    },
    #[serde(rename = "user")]
    User {
        #[serde(rename = "file", deserialize_with = "required")]
        r#file: AbsolutePathBuf,
        #[serde(
            rename = "profile",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#profile: Option<Option<String>>,
    },
    #[serde(rename = "project")]
    Project {
        #[serde(rename = "dotCodexFolder", deserialize_with = "required")]
        r#dot_codex_folder: AbsolutePathBuf,
    },
    #[serde(rename = "sessionFlags")]
    SessionFlags,
    #[serde(rename = "legacyManagedConfigTomlFromFile")]
    LegacyManagedConfigTomlFromFile {
        #[serde(rename = "file", deserialize_with = "required")]
        r#file: AbsolutePathBuf,
    },
    #[serde(rename = "legacyManagedConfigTomlFromMdm")]
    LegacyManagedConfigTomlFromMdm,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigReadParams {
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "includeLayers",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_layers: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigReadResponse {
    #[serde(rename = "config", deserialize_with = "required")]
    pub r#config: Config,
    #[serde(
        rename = "layers",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#layers: Option<Option<Vec<ConfigLayer>>>,
    #[serde(rename = "origins", deserialize_with = "required")]
    pub r#origins: std::collections::BTreeMap<String, ConfigLayerMetadata>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigRequirements {
    #[serde(
        rename = "additionalDeveloperInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#additional_developer_instructions: Option<Option<String>>,
    #[serde(
        rename = "allowAppshots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_appshots: Option<Option<bool>>,
    #[serde(
        rename = "allowBrowserAndComputerUse",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_browser_and_computer_use: Option<Option<bool>>,
    #[serde(
        rename = "allowLoginShell",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_login_shell: Option<Option<bool>>,
    #[serde(
        rename = "allowManagedHooksOnly",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_managed_hooks_only: Option<Option<bool>>,
    #[serde(
        rename = "allowRemoteControl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_remote_control: Option<Option<bool>>,
    #[serde(
        rename = "allowedApprovalPolicies",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_approval_policies: Option<Option<Vec<AskForApproval>>>,
    #[serde(
        rename = "allowedApprovalsReviewers",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_approvals_reviewers: Option<Option<Vec<ApprovalsReviewer>>>,
    #[serde(
        rename = "allowedLoginMethods",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_login_methods: Option<Option<Vec<ForcedLoginMethod>>>,
    #[serde(
        rename = "allowedPermissionProfiles",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_permission_profiles: Option<Option<std::collections::BTreeMap<String, bool>>>,
    #[serde(
        rename = "allowedSandboxModes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_sandbox_modes: Option<Option<Vec<SandboxMode>>>,
    #[serde(
        rename = "allowedWebSearchModes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_web_search_modes: Option<Option<Vec<WebSearchMode>>>,
    #[serde(
        rename = "allowedWindowsSandboxImplementations",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_windows_sandbox_implementations:
        Option<Option<Vec<WindowsSandboxImplementation>>>,
    #[serde(
        rename = "application",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#application: Option<Option<ApplicationRequirements>>,
    #[serde(
        rename = "autoReview",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#auto_review: Option<Option<AutoReviewRequirements>>,
    #[serde(
        rename = "browserUse",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#browser_use: Option<Option<BrowserUseRequirements>>,
    #[serde(
        rename = "chatgptBaseUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#chatgpt_base_url: Option<Option<String>>,
    #[serde(
        rename = "checkForUpdateOnStartup",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#check_for_update_on_startup: Option<Option<bool>>,
    #[serde(
        rename = "cliAuthCredentialsStore",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cli_auth_credentials_store: Option<Option<CliAuthCredentialsStoreMode>>,
    #[serde(
        rename = "computerUse",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#computer_use: Option<Option<ComputerUseRequirements>>,
    #[serde(
        rename = "defaultPermissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_permissions: Option<Option<String>>,
    #[serde(
        rename = "enforceResidency",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enforce_residency: Option<Option<ResidencyRequirement>>,
    #[serde(
        rename = "featureRequirements",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#feature_requirements: Option<Option<std::collections::BTreeMap<String, bool>>>,
    #[serde(
        rename = "feedback",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#feedback: Option<Option<FeedbackRequirements>>,
    #[serde(
        rename = "hooks",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#hooks: Option<Option<ManagedHooksRequirements>>,
    #[serde(
        rename = "inAppBrowser",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#in_app_browser: Option<Option<InAppBrowserRequirements>>,
    #[serde(
        rename = "logDir",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#log_dir: Option<Option<String>>,
    #[serde(
        rename = "modelCatalogJson",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_catalog_json: Option<Option<String>>,
    #[serde(
        rename = "modelProvider",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_provider: Option<Option<String>>,
    #[serde(
        rename = "modelProviders",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_providers: Option<Option<std::collections::BTreeMap<String, studio_json::Value>>>,
    #[serde(
        rename = "models",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#models: Option<Option<ModelsRequirements>>,
    #[serde(
        rename = "network",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#network: Option<Option<NetworkRequirements>>,
    #[serde(
        rename = "otel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#otel: Option<Option<OtelRequirements>>,
    #[serde(
        rename = "sqliteHome",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sqlite_home: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigRequirementsReadResponse {
    #[serde(
        rename = "requirements",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#requirements: Option<Option<ConfigRequirements>>,
    #[serde(
        rename = "supportsIndependentSpeedModes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#supports_independent_speed_modes: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigValueWriteParams {
    #[serde(
        rename = "expectedVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#expected_version: Option<Option<String>>,
    #[serde(
        rename = "filePath",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#file_path: Option<Option<String>>,
    #[serde(rename = "keyPath", deserialize_with = "required")]
    pub r#key_path: String,
    #[serde(rename = "mergeStrategy", deserialize_with = "required")]
    pub r#merge_strategy: MergeStrategy,
    #[serde(rename = "value", deserialize_with = "required")]
    pub r#value: studio_json::Value,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigWarningNotification {
    #[serde(
        rename = "details",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#details: Option<Option<String>>,
    #[serde(
        rename = "path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#path: Option<Option<String>>,
    #[serde(
        rename = "range",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#range: Option<Option<TextRange>>,
    #[serde(rename = "summary", deserialize_with = "required")]
    pub r#summary: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigWriteResponse {
    #[serde(rename = "filePath", deserialize_with = "required")]
    pub r#file_path: AbsolutePathBuf,
    #[serde(
        rename = "overriddenMetadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#overridden_metadata: Option<Option<OverriddenMetadata>>,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: WriteStatus,
    #[serde(rename = "version", deserialize_with = "required")]
    pub r#version: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfigurationReasoning {
    #[serde(rename = "effort", deserialize_with = "required")]
    pub r#effort: ReasoningEffort,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ConfiguredHookHandler {
    #[serde(rename = "command")]
    Command {
        #[serde(
            rename = "additionalContextLimit",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#additional_context_limit: Option<Option<u64>>,
        #[serde(rename = "async", deserialize_with = "required")]
        r#async: bool,
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
        #[serde(
            rename = "commandWindows",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#command_windows: Option<Option<String>>,
        #[serde(
            rename = "statusMessage",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status_message: Option<Option<String>>,
        #[serde(
            rename = "timeoutSec",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#timeout_sec: Option<Option<u64>>,
    },
    #[serde(rename = "mcp_tool")]
    McpTool {
        #[serde(rename = "input", deserialize_with = "required")]
        r#input: std::collections::BTreeMap<String, studio_json::Value>,
        #[serde(rename = "server", deserialize_with = "required")]
        r#server: String,
        #[serde(
            rename = "statusMessage",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status_message: Option<Option<String>>,
        #[serde(
            rename = "timeoutSec",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#timeout_sec: Option<Option<u64>>,
        #[serde(rename = "tool", deserialize_with = "required")]
        r#tool: String,
    },
    #[serde(rename = "prompt")]
    Prompt,
    #[serde(rename = "agent")]
    Agent,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfiguredHookMatcherGroup {
    #[serde(rename = "hooks", deserialize_with = "required")]
    pub r#hooks: Vec<ConfiguredHookHandler>,
    #[serde(
        rename = "matcher",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#matcher: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConnectorMetadata {
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "distributionChannel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#distribution_channel: Option<Option<String>>,
    #[serde(
        rename = "iconUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_url: Option<Option<String>>,
    #[serde(
        rename = "iconUrlDark",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_url_dark: Option<Option<String>>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(
        rename = "installUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#install_url: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "pluginDisplayNames",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#plugin_display_names: Option<Vec<String>>,
    #[serde(
        rename = "toolSummaries",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tool_summaries: Option<Option<Vec<AppToolSummary>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ConsumeAccountRateLimitResetCreditOutcome {
    Variant1(Box<ConsumeAccountRateLimitResetCreditOutcomeVariant1>),
    Variant2(Box<ConsumeAccountRateLimitResetCreditOutcomeVariant2>),
    Variant3(Box<ConsumeAccountRateLimitResetCreditOutcomeVariant3>),
    Variant4(Box<ConsumeAccountRateLimitResetCreditOutcomeVariant4>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConsumeAccountRateLimitResetCreditParams {
    #[serde(
        rename = "creditId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#credit_id: Option<Option<String>>,
    #[serde(rename = "idempotencyKey", deserialize_with = "required")]
    pub r#idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConsumeAccountRateLimitResetCreditResponse {
    #[serde(rename = "outcome", deserialize_with = "required")]
    pub r#outcome: ConsumeAccountRateLimitResetCreditOutcome,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ContentItem {
    #[serde(rename = "input_text")]
    InputText {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "input_image")]
    InputImage {
        #[serde(
            rename = "detail",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#detail: Option<Option<ImageDetail>>,
    },
    #[serde(rename = "input_audio")]
    InputAudio,
    #[serde(rename = "output_text")]
    OutputText {
        #[serde(
            rename = "annotations",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#annotations: Option<Option<Vec<studio_json::Value>>>,
        #[serde(
            rename = "logprobs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#logprobs: Option<Option<Vec<studio_json::Value>>>,
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContextCompactedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConversationTextRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "developer")]
    Developer,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CreditsSnapshot {
    #[serde(
        rename = "balance",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#balance: Option<Option<String>>,
    #[serde(rename = "hasCredits", deserialize_with = "required")]
    pub r#has_credits: bool,
    #[serde(rename = "unlimited", deserialize_with = "required")]
    pub r#unlimited: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CyberAccessProgram {
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "daybreakBlue")]
    DaybreakBlue,
    #[serde(rename = "daybreakRed")]
    DaybreakRed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeprecationNoticeNotification {
    #[serde(
        rename = "details",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#details: Option<Option<String>>,
    #[serde(rename = "summary", deserialize_with = "required")]
    pub r#summary: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DesktopOnboardingEntrypoint {
    #[serde(rename = "life_sciences")]
    LifeSciences,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum DynamicToolCallOutputContentItem {
    #[serde(rename = "inputText")]
    InputText {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "inputImage")]
    InputImage {
        #[serde(rename = "imageUrl", deserialize_with = "required")]
        r#image_url: String,
    },
    #[serde(rename = "inputAudio")]
    InputAudio {
        #[serde(rename = "audioUrl", deserialize_with = "required")]
        r#audio_url: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DynamicToolCallStatus {
    #[serde(rename = "inProgress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum DynamicToolNamespaceTool {
    #[serde(rename = "function")]
    Function {
        #[serde(
            rename = "deferLoading",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#defer_loading: Option<bool>,
        #[serde(rename = "description", deserialize_with = "required")]
        r#description: String,
        #[serde(rename = "inputSchema", deserialize_with = "required")]
        r#input_schema: studio_json::Value,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum DynamicToolSpec {
    #[serde(rename = "function")]
    Function {
        #[serde(
            rename = "deferLoading",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#defer_loading: Option<bool>,
        #[serde(rename = "description", deserialize_with = "required")]
        r#description: String,
        #[serde(rename = "inputSchema", deserialize_with = "required")]
        r#input_schema: studio_json::Value,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
    },
    #[serde(rename = "namespace")]
    Namespace {
        #[serde(rename = "description", deserialize_with = "required")]
        r#description: String,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(rename = "tools", deserialize_with = "required")]
        r#tools: Vec<DynamicToolNamespaceTool>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentAddParams {
    #[serde(
        rename = "authBearerToken",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#auth_bearer_token: Option<Option<String>>,
    #[serde(
        rename = "connectTimeoutMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#connect_timeout_ms: Option<Option<u64>>,
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
    #[serde(rename = "execServerUrl", deserialize_with = "required")]
    pub r#exec_server_url: String,
    #[serde(
        rename = "skills",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#skills: Option<Option<EnvironmentSkillsParams>>,
    #[serde(
        rename = "websocketRequestId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#websocket_request_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentAddResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentConnectionNotification {
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentInfoParams {
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentInfoResponse {
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<PathUri>>,
    #[serde(rename = "shell", deserialize_with = "required")]
    pub r#shell: EnvironmentShellInfo,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentShellInfo {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentSkillsParams {
    #[serde(
        rename = "required",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#required: Option<Option<Vec<String>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum EnvironmentStatusKind {
    Variant1(Box<EnvironmentStatusKindVariant1>),
    Variant2(Box<EnvironmentStatusKindVariant2>),
    Variant3(Box<EnvironmentStatusKindVariant3>),
    Variant4(Box<EnvironmentStatusKindVariant4>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentStatusParams {
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentStatusResponse {
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<String>>,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: EnvironmentStatusKind,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ErrorNotification {
    #[serde(rename = "error", deserialize_with = "required")]
    pub r#error: TurnError,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
    #[serde(rename = "willRetry", deserialize_with = "required")]
    pub r#will_retry: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExperimentalFeature {
    #[serde(
        rename = "announcement",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#announcement: Option<Option<String>>,
    #[serde(rename = "defaultEnabled", deserialize_with = "required")]
    pub r#default_enabled: bool,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#display_name: Option<Option<String>>,
    #[serde(rename = "enabled", deserialize_with = "required")]
    pub r#enabled: bool,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "stage", deserialize_with = "required")]
    pub r#stage: ExperimentalFeatureStage,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExperimentalFeatureEnablementSetParams {
    #[serde(rename = "enablement", deserialize_with = "required")]
    pub r#enablement: std::collections::BTreeMap<String, bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExperimentalFeatureEnablementSetResponse {
    #[serde(rename = "enablement", deserialize_with = "required")]
    pub r#enablement: std::collections::BTreeMap<String, bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExperimentalFeatureListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExperimentalFeatureListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ExperimentalFeature>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ExperimentalFeatureStage {
    Variant1(Box<ExperimentalFeatureStageVariant1>),
    Variant2(Box<ExperimentalFeatureStageVariant2>),
    Variant3(Box<ExperimentalFeatureStageVariant3>),
    Variant4(Box<ExperimentalFeatureStageVariant4>),
    Variant5(Box<ExperimentalFeatureStageVariant5>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigDetectParams {
    #[serde(
        rename = "cwds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwds: Option<Option<Vec<String>>>,
    #[serde(
        rename = "includeHome",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_home: Option<bool>,
    #[serde(
        rename = "maxSessionAgeDays",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#max_session_age_days: Option<Option<u32>>,
    #[serde(
        rename = "maxSessions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#max_sessions: Option<Option<u32>>,
    #[serde(
        rename = "migrationSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#migration_source: Option<Option<String>>,
    #[serde(
        rename = "source",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigDetectResponse {
    #[serde(
        rename = "connectors",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#connectors: Option<Vec<ExternalAgentDetectedConnectorCandidate>>,
    #[serde(rename = "items", deserialize_with = "required")]
    pub r#items: Vec<ExternalAgentConfigMigrationItem>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportCompletedNotification {
    #[serde(rename = "importId", deserialize_with = "required")]
    pub r#import_id: String,
    #[serde(rename = "itemTypeResults", deserialize_with = "required")]
    pub r#item_type_results: Vec<ExternalAgentConfigImportTypeResult>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportHistoriesReadResponse {
    #[serde(rename = "connectors", deserialize_with = "required")]
    pub r#connectors: Vec<ExternalAgentImportedConnectorCandidate>,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ExternalAgentConfigImportHistory>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportHistory {
    #[serde(rename = "completedAtMs", deserialize_with = "required")]
    pub r#completed_at_ms: i64,
    #[serde(rename = "failures", deserialize_with = "required")]
    pub r#failures: Vec<ExternalAgentConfigImportItemTypeFailure>,
    #[serde(rename = "importId", deserialize_with = "required")]
    pub r#import_id: String,
    #[serde(
        rename = "providerId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#provider_id: Option<Option<String>>,
    #[serde(rename = "successes", deserialize_with = "required")]
    pub r#successes: Vec<ExternalAgentConfigImportItemTypeSuccess>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportHistoryRecordParams {
    #[serde(rename = "itemTypeResults", deserialize_with = "required")]
    pub r#item_type_results: Vec<ExternalAgentConfigImportHistoryRecordTypeResultParams>,
    #[serde(rename = "providerId", deserialize_with = "required")]
    pub r#provider_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportHistoryRecordResponse {
    #[serde(rename = "importId", deserialize_with = "required")]
    pub r#import_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportHistoryRecordSuccessParams {
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(rename = "itemType", deserialize_with = "required")]
    pub r#item_type: ExternalAgentConfigMigrationItemType,
    #[serde(
        rename = "source",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source: Option<Option<String>>,
    #[serde(
        rename = "target",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#target: Option<Option<String>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportHistoryRecordTypeResultParams {
    #[serde(rename = "failures", deserialize_with = "required")]
    pub r#failures: Vec<ExternalAgentConfigImportItemTypeFailure>,
    #[serde(rename = "itemType", deserialize_with = "required")]
    pub r#item_type: ExternalAgentConfigMigrationItemType,
    #[serde(rename = "successes", deserialize_with = "required")]
    pub r#successes: Vec<ExternalAgentConfigImportHistoryRecordSuccessParams>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportItemTypeFailure {
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "errorType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error_type: Option<Option<String>>,
    #[serde(rename = "failureStage", deserialize_with = "required")]
    pub r#failure_stage: String,
    #[serde(rename = "itemType", deserialize_with = "required")]
    pub r#item_type: ExternalAgentConfigMigrationItemType,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(
        rename = "source",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source: Option<Option<String>>,
    #[serde(
        rename = "subErrorType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sub_error_type: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportItemTypeSuccess {
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(rename = "itemType", deserialize_with = "required")]
    pub r#item_type: ExternalAgentConfigMigrationItemType,
    #[serde(
        rename = "source",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source: Option<Option<String>>,
    #[serde(
        rename = "target",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#target: Option<Option<String>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportParams {
    #[serde(rename = "migrationItems", deserialize_with = "required")]
    pub r#migration_items: Vec<ExternalAgentConfigMigrationItem>,
    #[serde(
        rename = "migrationSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#migration_source: Option<Option<String>>,
    #[serde(
        rename = "providerId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#provider_id: Option<Option<String>>,
    #[serde(
        rename = "source",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportProgressNotification {
    #[serde(rename = "importId", deserialize_with = "required")]
    pub r#import_id: String,
    #[serde(rename = "itemTypeResults", deserialize_with = "required")]
    pub r#item_type_results: Vec<ExternalAgentConfigImportTypeResult>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportResponse {
    #[serde(rename = "importId", deserialize_with = "required")]
    pub r#import_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigImportTypeResult {
    #[serde(rename = "failures", deserialize_with = "required")]
    pub r#failures: Vec<ExternalAgentConfigImportItemTypeFailure>,
    #[serde(rename = "itemType", deserialize_with = "required")]
    pub r#item_type: ExternalAgentConfigMigrationItemType,
    #[serde(rename = "successes", deserialize_with = "required")]
    pub r#successes: Vec<ExternalAgentConfigImportItemTypeSuccess>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentConfigMigrationItem {
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(
        rename = "details",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#details: Option<Option<MigrationDetails>>,
    #[serde(rename = "itemType", deserialize_with = "required")]
    pub r#item_type: ExternalAgentConfigMigrationItemType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExternalAgentConfigMigrationItemType {
    #[serde(rename = "AGENTS_MD")]
    AGENTSMD,
    #[serde(rename = "CONFIG")]
    CONFIG,
    #[serde(rename = "SKILLS")]
    SKILLS,
    #[serde(rename = "PLUGINS")]
    PLUGINS,
    #[serde(rename = "MCP_SERVER_CONFIG")]
    MCPSERVERCONFIG,
    #[serde(rename = "SUBAGENTS")]
    SUBAGENTS,
    #[serde(rename = "HOOKS")]
    HOOKS,
    #[serde(rename = "COMMANDS")]
    COMMANDS,
    #[serde(rename = "MEMORY")]
    MEMORY,
    #[serde(rename = "SESSIONS")]
    SESSIONS,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentDetectedConnectorCandidate {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "sessionCount", deserialize_with = "required")]
    pub r#session_count: u32,
    #[serde(rename = "source", deserialize_with = "required")]
    pub r#source: ExternalAgentDetectedConnectorSource,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExternalAgentDetectedConnectorSource {
    #[serde(rename = "remoteMcpServersConfig")]
    RemoteMcpServersConfig,
    #[serde(rename = "sessionToolUse")]
    SessionToolUse,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentImportedConnectorCandidate {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "sessionCount", deserialize_with = "required")]
    pub r#session_count: u32,
    #[serde(rename = "source", deserialize_with = "required")]
    pub r#source: ExternalAgentImportedConnectorSource,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExternalAgentImportedConnectorSource {
    #[serde(rename = "remoteMcpServersConfig")]
    RemoteMcpServersConfig,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedbackRequirements {
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enabled: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedbackSaveParams {
    #[serde(rename = "classification", deserialize_with = "required")]
    pub r#classification: String,
    #[serde(
        rename = "extraLogFiles",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#extra_log_files: Option<Option<Vec<String>>>,
    #[serde(
        rename = "includeLogs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_logs: Option<bool>,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
    #[serde(rename = "savePath", deserialize_with = "required")]
    pub r#save_path: LegacyAppPathString,
    #[serde(
        rename = "tags",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tags: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedbackSaveResponse {
    #[serde(
        rename = "promptHash",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#prompt_hash: Option<Option<String>>,
    #[serde(rename = "savedPath", deserialize_with = "required")]
    pub r#saved_path: LegacyAppPathString,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedbackUploadParams {
    #[serde(rename = "classification", deserialize_with = "required")]
    pub r#classification: String,
    #[serde(
        rename = "extraLogFiles",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#extra_log_files: Option<Option<Vec<String>>>,
    #[serde(
        rename = "includeLogs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_logs: Option<bool>,
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
    #[serde(
        rename = "tags",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tags: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedbackUploadResponse {
    #[serde(
        rename = "promptHash",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#prompt_hash: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileChangeOutputDeltaNotification {
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileChangePatchUpdatedNotification {
    #[serde(rename = "changes", deserialize_with = "required")]
    pub r#changes: Vec<FileUpdateChange>,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileSystemAccessMode {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum FileSystemPath {
    #[serde(rename = "path")]
    Path {
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: LegacyAppPathString,
    },
    #[serde(rename = "glob_pattern")]
    GlobPattern {
        #[serde(rename = "pattern", deserialize_with = "required")]
        r#pattern: String,
    },
    #[serde(rename = "special")]
    Special {
        #[serde(rename = "value", deserialize_with = "required")]
        r#value: FileSystemSpecialPath,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileSystemSandboxEntry {
    #[serde(rename = "access", deserialize_with = "required")]
    pub r#access: FileSystemAccessMode,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: FileSystemPath,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum FileSystemSpecialPath {
    #[serde(rename = "root")]
    Root,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "project_roots")]
    ProjectRoots {
        #[serde(
            rename = "subpath",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#subpath: Option<Option<LegacyAppPathString>>,
    },
    #[serde(rename = "tmpdir")]
    Tmpdir,
    #[serde(rename = "slash_tmp")]
    SlashTmp,
    #[serde(rename = "unknown")]
    Unknown {
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: String,
        #[serde(
            rename = "subpath",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#subpath: Option<Option<LegacyAppPathString>>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileUpdateChange {
    #[serde(rename = "diff", deserialize_with = "required")]
    pub r#diff: String,
    #[serde(rename = "kind", deserialize_with = "required")]
    pub r#kind: PatchChangeKind,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ForcedChatgptWorkspaceIds {
    Variant1(Box<String>),
    Variant2(Box<Vec<String>>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ForcedLoginMethod {
    #[serde(rename = "chatgpt")]
    Chatgpt,
    #[serde(rename = "api")]
    Api,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsChangedNotification {
    #[serde(rename = "changedPaths", deserialize_with = "required")]
    pub r#changed_paths: Vec<AbsolutePathBuf>,
    #[serde(rename = "watchId", deserialize_with = "required")]
    pub r#watch_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsCopyParams {
    #[serde(rename = "destinationPath", deserialize_with = "required")]
    pub r#destination_path: AbsolutePathBuf,
    #[serde(
        rename = "recursive",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#recursive: Option<bool>,
    #[serde(rename = "sourcePath", deserialize_with = "required")]
    pub r#source_path: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsCopyResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsCreateDirectoryParams {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
    #[serde(
        rename = "recursive",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#recursive: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsCreateDirectoryResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsGetMetadataParams {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsGetMetadataResponse {
    #[serde(rename = "createdAtMs", deserialize_with = "required")]
    pub r#created_at_ms: i64,
    #[serde(rename = "isDirectory", deserialize_with = "required")]
    pub r#is_directory: bool,
    #[serde(rename = "isFile", deserialize_with = "required")]
    pub r#is_file: bool,
    #[serde(rename = "isSymlink", deserialize_with = "required")]
    pub r#is_symlink: bool,
    #[serde(rename = "modifiedAtMs", deserialize_with = "required")]
    pub r#modified_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsReadDirectoryEntry {
    #[serde(rename = "fileName", deserialize_with = "required")]
    pub r#file_name: String,
    #[serde(rename = "isDirectory", deserialize_with = "required")]
    pub r#is_directory: bool,
    #[serde(rename = "isFile", deserialize_with = "required")]
    pub r#is_file: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsReadDirectoryParams {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsReadDirectoryResponse {
    #[serde(rename = "entries", deserialize_with = "required")]
    pub r#entries: Vec<FsReadDirectoryEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsReadFileParams {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsReadFileResponse {
    #[serde(rename = "dataBase64", deserialize_with = "required")]
    pub r#data_base64: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsRemoveParams {
    #[serde(
        rename = "force",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#force: Option<Option<bool>>,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
    #[serde(
        rename = "recursive",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#recursive: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsRemoveResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsUnwatchParams {
    #[serde(rename = "watchId", deserialize_with = "required")]
    pub r#watch_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsUnwatchResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsWatchParams {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
    #[serde(rename = "watchId", deserialize_with = "required")]
    pub r#watch_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsWatchResponse {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsWriteFileParams {
    #[serde(rename = "dataBase64", deserialize_with = "required")]
    pub r#data_base64: String,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FsWriteFileResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum FunctionCallOutputBody {
    Variant1(Box<String>),
    Variant2(Box<Vec<FunctionCallOutputContentItem>>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum FunctionCallOutputContentItem {
    #[serde(rename = "input_text")]
    InputText {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "input_image")]
    InputImage {
        #[serde(
            rename = "detail",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#detail: Option<Option<ImageDetail>>,
    },
    #[serde(rename = "input_audio")]
    InputAudio {
        #[serde(rename = "audio_url", deserialize_with = "required")]
        r#audio_url: String,
    },
    #[serde(rename = "encrypted_content")]
    EncryptedContent {
        #[serde(rename = "encrypted_content", deserialize_with = "required")]
        r#encrypted_content: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GatewayOAuthCancelResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GatewayOAuthChangedNotification {
    #[serde(
        rename = "authUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#auth_url: Option<Option<String>>,
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<String>>,
    #[serde(rename = "providerId", deserialize_with = "required")]
    pub r#provider_id: String,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: GatewayOAuthStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GatewayOAuthLoginResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GatewayOAuthReadResponse {
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<String>>,
    #[serde(rename = "providerId", deserialize_with = "required")]
    pub r#provider_id: String,
    #[serde(rename = "providerName", deserialize_with = "required")]
    pub r#provider_name: String,
    #[serde(rename = "required", deserialize_with = "required")]
    pub r#required: bool,
    #[serde(
        rename = "status",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#status: Option<Option<GatewayOAuthStatus>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GatewayOAuthStatus {
    #[serde(rename = "notReady")]
    NotReady,
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GcpSetupParams {
    #[serde(
        rename = "credentialFile",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#credential_file: Option<Option<String>>,
    #[serde(
        rename = "location",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#location: Option<Option<String>>,
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GcpSetupResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetAccountParams {
    #[serde(
        rename = "refreshToken",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#refresh_token: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetAccountRateLimitsParams {
    #[serde(
        rename = "excludeResetCreditDetails",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exclude_reset_credit_details: Option<bool>,
    #[serde(
        rename = "supportsLunaReserve",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#supports_luna_reserve: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetAccountRateLimitsResponse {
    #[serde(
        rename = "accountId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#account_id: Option<Option<String>>,
    #[serde(
        rename = "ordinaryUsageAllowed",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ordinary_usage_allowed: Option<Option<bool>>,
    #[serde(
        rename = "rateLimitResetCredits",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#rate_limit_reset_credits: Option<Option<RateLimitResetCreditsSummary>>,
    #[serde(
        rename = "rateLimitUpsell",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#rate_limit_upsell: Option<studio_json::Value>,
    #[serde(rename = "rateLimits", deserialize_with = "required")]
    pub r#rate_limits: RateLimitSnapshot,
    #[serde(
        rename = "rateLimitsByLimitId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#rate_limits_by_limit_id:
        Option<Option<std::collections::BTreeMap<String, RateLimitSnapshot>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetAccountResponse {
    #[serde(
        rename = "account",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#account: Option<Option<Account>>,
    #[serde(rename = "requiresOpenaiAuth", deserialize_with = "required")]
    pub r#requires_openai_auth: bool,
    #[serde(
        rename = "workspaceRouting",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#workspace_routing: Option<Option<WorkspaceRouting>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetAccountTokenUsageParams {
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetAccountTokenUsageResponse {
    #[serde(
        rename = "dailyUsageBuckets",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#daily_usage_buckets: Option<Option<Vec<AccountTokenUsageDailyBucket>>>,
    #[serde(rename = "summary", deserialize_with = "required")]
    pub r#summary: AccountTokenUsageSummary,
    #[serde(
        rename = "threadUsage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_usage: Option<Option<ThreadUsage>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetWorkspaceMessagesResponse {
    #[serde(rename = "featureEnabled", deserialize_with = "required")]
    pub r#feature_enabled: bool,
    #[serde(rename = "messages", deserialize_with = "required")]
    pub r#messages: Vec<WorkspaceMessage>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GitInfo {
    #[serde(
        rename = "branch",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#branch: Option<Option<String>>,
    #[serde(
        rename = "originUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origin_url: Option<Option<String>>,
    #[serde(
        rename = "sha",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sha: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuardianApprovalReview {
    #[serde(
        rename = "rationale",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#rationale: Option<Option<String>>,
    #[serde(
        rename = "riskLevel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#risk_level: Option<Option<GuardianRiskLevel>>,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: GuardianApprovalReviewStatus,
    #[serde(
        rename = "userAuthorization",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#user_authorization: Option<Option<GuardianUserAuthorization>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum GuardianApprovalReviewAction {
    #[serde(rename = "command")]
    Command {
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
        #[serde(rename = "cwd", deserialize_with = "required")]
        r#cwd: LegacyAppPathString,
        #[serde(rename = "source", deserialize_with = "required")]
        r#source: GuardianCommandSource,
    },
    #[serde(rename = "execve")]
    Execve {
        #[serde(rename = "argv", deserialize_with = "required")]
        r#argv: Vec<String>,
        #[serde(rename = "cwd", deserialize_with = "required")]
        r#cwd: AbsolutePathBuf,
        #[serde(rename = "program", deserialize_with = "required")]
        r#program: String,
        #[serde(rename = "source", deserialize_with = "required")]
        r#source: GuardianCommandSource,
    },
    #[serde(rename = "writeStdin")]
    WriteStdin {
        #[serde(rename = "approvalId", deserialize_with = "required")]
        r#approval_id: String,
        #[serde(rename = "cwd", deserialize_with = "required")]
        r#cwd: LegacyAppPathString,
        #[serde(rename = "processId", deserialize_with = "required")]
        r#process_id: String,
        #[serde(rename = "stdin", deserialize_with = "required")]
        r#stdin: String,
    },
    #[serde(rename = "applyPatch")]
    ApplyPatch {
        #[serde(rename = "cwd", deserialize_with = "required")]
        r#cwd: LegacyAppPathString,
        #[serde(rename = "files", deserialize_with = "required")]
        r#files: Vec<LegacyAppPathString>,
    },
    #[serde(rename = "networkAccess")]
    NetworkAccess {
        #[serde(rename = "host", deserialize_with = "required")]
        r#host: String,
        #[serde(rename = "port", deserialize_with = "required")]
        r#port: u16,
        #[serde(rename = "protocol", deserialize_with = "required")]
        r#protocol: NetworkApprovalProtocol,
        #[serde(rename = "target", deserialize_with = "required")]
        r#target: String,
    },
    #[serde(rename = "mcpToolCall")]
    McpToolCall {
        #[serde(
            rename = "connectorId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#connector_id: Option<Option<String>>,
        #[serde(
            rename = "connectorName",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#connector_name: Option<Option<String>>,
        #[serde(rename = "server", deserialize_with = "required")]
        r#server: String,
        #[serde(rename = "toolName", deserialize_with = "required")]
        r#tool_name: String,
        #[serde(
            rename = "toolTitle",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#tool_title: Option<Option<String>>,
    },
    #[serde(rename = "requestPermissions")]
    RequestPermissions {
        #[serde(rename = "permissions", deserialize_with = "required")]
        r#permissions: RequestPermissionProfile,
        #[serde(
            rename = "reason",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#reason: Option<Option<String>>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GuardianApprovalReviewStatus {
    #[serde(rename = "inProgress")]
    InProgress,
    #[serde(rename = "approved")]
    Approved,
    #[serde(rename = "denied")]
    Denied,
    #[serde(rename = "timedOut")]
    TimedOut,
    #[serde(rename = "aborted")]
    Aborted,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GuardianAssessmentOutcome {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GuardianCommandSource {
    #[serde(rename = "shell")]
    Shell,
    #[serde(rename = "unifiedExec")]
    UnifiedExec,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GuardianRiskLevel {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "critical")]
    Critical,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GuardianUserAuthorization {
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuardianWarningNotification {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HookCompletedNotification {
    #[serde(rename = "run", deserialize_with = "required")]
    pub r#run: HookRunSummary,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "turnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turn_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HookErrorInfo {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookEventName {
    #[serde(rename = "preModelRequest")]
    PreModelRequest,
    #[serde(rename = "postModelRequest")]
    PostModelRequest,
    #[serde(rename = "preToolUse")]
    PreToolUse,
    #[serde(rename = "permissionRequest")]
    PermissionRequest,
    #[serde(rename = "postToolUse")]
    PostToolUse,
    #[serde(rename = "preCompact")]
    PreCompact,
    #[serde(rename = "postCompact")]
    PostCompact,
    #[serde(rename = "sessionStart")]
    SessionStart,
    #[serde(rename = "sessionEnd")]
    SessionEnd,
    #[serde(rename = "userPromptSubmit")]
    UserPromptSubmit,
    #[serde(rename = "subagentStart")]
    SubagentStart,
    #[serde(rename = "subagentStop")]
    SubagentStop,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "interrupt")]
    Interrupt,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookExecutionMode {
    #[serde(rename = "sync")]
    Sync,
    #[serde(rename = "async")]
    Async,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookHandlerType {
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "mcpTool")]
    McpTool,
    #[serde(rename = "prompt")]
    Prompt,
    #[serde(rename = "agent")]
    Agent,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "handlerType")]
pub enum HookMetadata {
    #[serde(rename = "command")]
    Command {
        #[serde(
            rename = "additionalContextLimit",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#additional_context_limit: Option<Option<u64>>,
        #[serde(rename = "currentHash", deserialize_with = "required")]
        r#current_hash: String,
        #[serde(rename = "displayOrder", deserialize_with = "required")]
        r#display_order: i64,
        #[serde(rename = "enabled", deserialize_with = "required")]
        r#enabled: bool,
        #[serde(
            rename = "environmentId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#environment_id: Option<Option<String>>,
        #[serde(rename = "eventName", deserialize_with = "required")]
        r#event_name: HookEventName,
        #[serde(rename = "isManaged", deserialize_with = "required")]
        r#is_managed: bool,
        #[serde(rename = "key", deserialize_with = "required")]
        r#key: String,
        #[serde(
            rename = "matcher",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#matcher: Option<Option<String>>,
        #[serde(
            rename = "pluginId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#plugin_id: Option<Option<String>>,
        #[serde(rename = "source", deserialize_with = "required")]
        r#source: HookSource,
        #[serde(rename = "sourcePath", deserialize_with = "required")]
        r#source_path: AbsolutePathBuf,
        #[serde(
            rename = "statusMessage",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status_message: Option<Option<String>>,
        #[serde(rename = "timeoutSec", deserialize_with = "required")]
        r#timeout_sec: u64,
        #[serde(rename = "trustStatus", deserialize_with = "required")]
        r#trust_status: HookTrustStatus,
        #[serde(
            rename = "async",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#async: Option<bool>,
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
    },
    #[serde(rename = "mcpTool")]
    McpTool {
        #[serde(
            rename = "additionalContextLimit",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#additional_context_limit: Option<Option<u64>>,
        #[serde(rename = "currentHash", deserialize_with = "required")]
        r#current_hash: String,
        #[serde(rename = "displayOrder", deserialize_with = "required")]
        r#display_order: i64,
        #[serde(rename = "enabled", deserialize_with = "required")]
        r#enabled: bool,
        #[serde(
            rename = "environmentId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#environment_id: Option<Option<String>>,
        #[serde(rename = "eventName", deserialize_with = "required")]
        r#event_name: HookEventName,
        #[serde(rename = "isManaged", deserialize_with = "required")]
        r#is_managed: bool,
        #[serde(rename = "key", deserialize_with = "required")]
        r#key: String,
        #[serde(
            rename = "matcher",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#matcher: Option<Option<String>>,
        #[serde(
            rename = "pluginId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#plugin_id: Option<Option<String>>,
        #[serde(rename = "source", deserialize_with = "required")]
        r#source: HookSource,
        #[serde(rename = "sourcePath", deserialize_with = "required")]
        r#source_path: AbsolutePathBuf,
        #[serde(
            rename = "statusMessage",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status_message: Option<Option<String>>,
        #[serde(rename = "timeoutSec", deserialize_with = "required")]
        r#timeout_sec: u64,
        #[serde(rename = "trustStatus", deserialize_with = "required")]
        r#trust_status: HookTrustStatus,
        #[serde(rename = "server", deserialize_with = "required")]
        r#server: String,
        #[serde(rename = "tool", deserialize_with = "required")]
        r#tool: String,
    },
    #[serde(rename = "prompt")]
    Prompt {
        #[serde(
            rename = "additionalContextLimit",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#additional_context_limit: Option<Option<u64>>,
        #[serde(rename = "currentHash", deserialize_with = "required")]
        r#current_hash: String,
        #[serde(rename = "displayOrder", deserialize_with = "required")]
        r#display_order: i64,
        #[serde(rename = "enabled", deserialize_with = "required")]
        r#enabled: bool,
        #[serde(
            rename = "environmentId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#environment_id: Option<Option<String>>,
        #[serde(rename = "eventName", deserialize_with = "required")]
        r#event_name: HookEventName,
        #[serde(rename = "isManaged", deserialize_with = "required")]
        r#is_managed: bool,
        #[serde(rename = "key", deserialize_with = "required")]
        r#key: String,
        #[serde(
            rename = "matcher",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#matcher: Option<Option<String>>,
        #[serde(
            rename = "pluginId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#plugin_id: Option<Option<String>>,
        #[serde(rename = "source", deserialize_with = "required")]
        r#source: HookSource,
        #[serde(rename = "sourcePath", deserialize_with = "required")]
        r#source_path: AbsolutePathBuf,
        #[serde(
            rename = "statusMessage",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status_message: Option<Option<String>>,
        #[serde(rename = "timeoutSec", deserialize_with = "required")]
        r#timeout_sec: u64,
        #[serde(rename = "trustStatus", deserialize_with = "required")]
        r#trust_status: HookTrustStatus,
    },
    #[serde(rename = "agent")]
    Agent {
        #[serde(
            rename = "additionalContextLimit",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#additional_context_limit: Option<Option<u64>>,
        #[serde(rename = "currentHash", deserialize_with = "required")]
        r#current_hash: String,
        #[serde(rename = "displayOrder", deserialize_with = "required")]
        r#display_order: i64,
        #[serde(rename = "enabled", deserialize_with = "required")]
        r#enabled: bool,
        #[serde(
            rename = "environmentId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#environment_id: Option<Option<String>>,
        #[serde(rename = "eventName", deserialize_with = "required")]
        r#event_name: HookEventName,
        #[serde(rename = "isManaged", deserialize_with = "required")]
        r#is_managed: bool,
        #[serde(rename = "key", deserialize_with = "required")]
        r#key: String,
        #[serde(
            rename = "matcher",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#matcher: Option<Option<String>>,
        #[serde(
            rename = "pluginId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#plugin_id: Option<Option<String>>,
        #[serde(rename = "source", deserialize_with = "required")]
        r#source: HookSource,
        #[serde(rename = "sourcePath", deserialize_with = "required")]
        r#source_path: AbsolutePathBuf,
        #[serde(
            rename = "statusMessage",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status_message: Option<Option<String>>,
        #[serde(rename = "timeoutSec", deserialize_with = "required")]
        r#timeout_sec: u64,
        #[serde(rename = "trustStatus", deserialize_with = "required")]
        r#trust_status: HookTrustStatus,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HookMigration {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HookOutputEntry {
    #[serde(rename = "kind", deserialize_with = "required")]
    pub r#kind: HookOutputEntryKind,
    #[serde(rename = "text", deserialize_with = "required")]
    pub r#text: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookOutputEntryKind {
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "feedback")]
    Feedback,
    #[serde(rename = "context")]
    Context,
    #[serde(rename = "error")]
    Error,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HookPromptFragment {
    #[serde(rename = "hookRunId", deserialize_with = "required")]
    pub r#hook_run_id: String,
    #[serde(rename = "text", deserialize_with = "required")]
    pub r#text: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookRunStatus {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "blocked")]
    Blocked,
    #[serde(rename = "stopped")]
    Stopped,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HookRunSummary {
    #[serde(
        rename = "completedAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#completed_at: Option<Option<i64>>,
    #[serde(rename = "displayOrder", deserialize_with = "required")]
    pub r#display_order: i64,
    #[serde(
        rename = "durationMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#duration_ms: Option<Option<i64>>,
    #[serde(rename = "entries", deserialize_with = "required")]
    pub r#entries: Vec<HookOutputEntry>,
    #[serde(rename = "eventName", deserialize_with = "required")]
    pub r#event_name: HookEventName,
    #[serde(rename = "executionMode", deserialize_with = "required")]
    pub r#execution_mode: HookExecutionMode,
    #[serde(rename = "handlerType", deserialize_with = "required")]
    pub r#handler_type: HookHandlerType,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "scope", deserialize_with = "required")]
    pub r#scope: HookScope,
    #[serde(
        rename = "source",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source: Option<HookSource>,
    #[serde(rename = "sourcePath", deserialize_with = "required")]
    pub r#source_path: LegacyAppPathString,
    #[serde(rename = "startedAt", deserialize_with = "required")]
    pub r#started_at: i64,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: HookRunStatus,
    #[serde(
        rename = "statusMessage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#status_message: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookScope {
    #[serde(rename = "thread")]
    Thread,
    #[serde(rename = "turn")]
    Turn,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookSource {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "mdm")]
    Mdm,
    #[serde(rename = "sessionFlags")]
    SessionFlags,
    #[serde(rename = "plugin")]
    Plugin,
    #[serde(rename = "cloudRequirements")]
    CloudRequirements,
    #[serde(rename = "cloudManagedConfig")]
    CloudManagedConfig,
    #[serde(rename = "legacyManagedConfigFile")]
    LegacyManagedConfigFile,
    #[serde(rename = "legacyManagedConfigMdm")]
    LegacyManagedConfigMdm,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HookStartedNotification {
    #[serde(rename = "run", deserialize_with = "required")]
    pub r#run: HookRunSummary,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "turnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turn_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookTrustStatus {
    #[serde(rename = "managed")]
    Managed,
    #[serde(rename = "untrusted")]
    Untrusted,
    #[serde(rename = "trusted")]
    Trusted,
    #[serde(rename = "modified")]
    Modified,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HooksListEntry {
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: String,
    #[serde(rename = "errors", deserialize_with = "required")]
    pub r#errors: Vec<HookErrorInfo>,
    #[serde(rename = "hooks", deserialize_with = "required")]
    pub r#hooks: Vec<HookMetadata>,
    #[serde(rename = "warnings", deserialize_with = "required")]
    pub r#warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HooksListParams {
    #[serde(
        rename = "cwds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwds: Option<Vec<String>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HooksListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<HooksListEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ImageDetail {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "original")]
    Original,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ImageGenerationFailure {
    #[serde(rename = "usageLimitExceeded")]
    UsageLimitExceeded {
        #[serde(rename = "limitId", deserialize_with = "required")]
        r#limit_id: String,
        #[serde(
            rename = "resetsAt",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#resets_at: Option<Option<i64>>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImageGenerationInputImage {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
    #[serde(rename = "sha256", deserialize_with = "required")]
    pub r#sha256: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InAppBrowserRequirements {
    #[serde(
        rename = "allowExternalBrowserSettingsImport",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_external_browser_settings_import: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum InputModality {
    Variant1(Box<InputModalityVariant1>),
    Variant2(Box<InputModalityVariant2>),
    Variant3(Box<InputModalityVariant3>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InstalledApp {
    #[serde(rename = "callable", deserialize_with = "required")]
    pub r#callable: bool,
    #[serde(rename = "enabled", deserialize_with = "required")]
    pub r#enabled: bool,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(
        rename = "runtimeName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_name: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InternalChatMessageMetadataPassthrough {
    #[serde(
        rename = "turn_id",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turn_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemCompletedNotification {
    #[serde(rename = "completedAtMs", deserialize_with = "required")]
    pub r#completed_at_ms: i64,
    #[serde(rename = "item", deserialize_with = "required")]
    pub r#item: ThreadItem,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemGuardianApprovalReviewCompletedNotification {
    #[serde(rename = "action", deserialize_with = "required")]
    pub r#action: GuardianApprovalReviewAction,
    #[serde(rename = "completedAtMs", deserialize_with = "required")]
    pub r#completed_at_ms: i64,
    #[serde(rename = "decisionSource", deserialize_with = "required")]
    pub r#decision_source: AutoReviewDecisionSource,
    #[serde(rename = "review", deserialize_with = "required")]
    pub r#review: GuardianApprovalReview,
    #[serde(rename = "reviewId", deserialize_with = "required")]
    pub r#review_id: String,
    #[serde(rename = "startedAtMs", deserialize_with = "required")]
    pub r#started_at_ms: i64,
    #[serde(
        rename = "targetItemId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#target_item_id: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemGuardianApprovalReviewStartedNotification {
    #[serde(rename = "action", deserialize_with = "required")]
    pub r#action: GuardianApprovalReviewAction,
    #[serde(rename = "review", deserialize_with = "required")]
    pub r#review: GuardianApprovalReview,
    #[serde(rename = "reviewId", deserialize_with = "required")]
    pub r#review_id: String,
    #[serde(rename = "startedAtMs", deserialize_with = "required")]
    pub r#started_at_ms: i64,
    #[serde(
        rename = "targetItemId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#target_item_id: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemStartedNotification {
    #[serde(rename = "item", deserialize_with = "required")]
    pub r#item: ThreadItem,
    #[serde(rename = "startedAtMs", deserialize_with = "required")]
    pub r#started_at_ms: i64,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

pub type LegacyAppPathString = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ListMcpServerStatusParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "detail",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#detail: Option<Option<McpServerStatusDetail>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "serverName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#server_name: Option<Option<String>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ListMcpServerStatusResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<McpServerStatus>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum LocalShellAction {
    #[serde(rename = "exec")]
    Exec {
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: Vec<String>,
        #[serde(
            rename = "env",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#env: Option<Option<std::collections::BTreeMap<String, String>>>,
        #[serde(
            rename = "timeout_ms",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#timeout_ms: Option<Option<u64>>,
        #[serde(
            rename = "user",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#user: Option<Option<String>>,
        #[serde(
            rename = "working_directory",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#working_directory: Option<Option<String>>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LocalShellStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "incomplete")]
    Incomplete,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum LoginAccountParams {
    #[serde(rename = "apiKey")]
    ApiKey {
        #[serde(rename = "apiKey", deserialize_with = "required")]
        r#api_key: String,
    },
    #[serde(rename = "chatgpt")]
    Chatgpt {
        #[serde(
            rename = "appBrand",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#app_brand: Option<Option<LoginAppBrand>>,
        #[serde(
            rename = "codexStreamlinedLogin",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#codex_streamlined_login: Option<bool>,
        #[serde(
            rename = "useHostedLoginSuccessPage",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#use_hosted_login_success_page: Option<bool>,
    },
    #[serde(rename = "chatgptDeviceCode")]
    ChatgptDeviceCode,
    #[serde(rename = "chatgptAuthTokens")]
    ChatgptAuthTokens {
        #[serde(rename = "accessToken", deserialize_with = "required")]
        r#access_token: String,
        #[serde(rename = "chatgptAccountId", deserialize_with = "required")]
        r#chatgpt_account_id: String,
        #[serde(
            rename = "chatgptPlanType",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#chatgpt_plan_type: Option<Option<String>>,
    },
    #[serde(rename = "amazonBedrock")]
    AmazonBedrock {
        #[serde(rename = "apiKey", deserialize_with = "required")]
        r#api_key: String,
        #[serde(rename = "region", deserialize_with = "required")]
        r#region: String,
    },
    #[serde(rename = "amazonBedrockAccessKeys")]
    AmazonBedrockAccessKeys {
        #[serde(rename = "accessKeyId", deserialize_with = "required")]
        r#access_key_id: String,
        #[serde(rename = "region", deserialize_with = "required")]
        r#region: String,
        #[serde(rename = "secretAccessKey", deserialize_with = "required")]
        r#secret_access_key: String,
        #[serde(
            rename = "sessionToken",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#session_token: Option<Option<String>>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum LoginAccountResponse {
    #[serde(rename = "apiKey")]
    ApiKey,
    #[serde(rename = "chatgpt")]
    Chatgpt {
        #[serde(rename = "authUrl", deserialize_with = "required")]
        r#auth_url: String,
        #[serde(rename = "loginId", deserialize_with = "required")]
        r#login_id: String,
    },
    #[serde(rename = "chatgptDeviceCode")]
    ChatgptDeviceCode {
        #[serde(rename = "loginId", deserialize_with = "required")]
        r#login_id: String,
        #[serde(rename = "userCode", deserialize_with = "required")]
        r#user_code: String,
        #[serde(rename = "verificationUrl", deserialize_with = "required")]
        r#verification_url: String,
    },
    #[serde(rename = "chatgptAuthTokens")]
    ChatgptAuthTokens,
    #[serde(rename = "amazonBedrock")]
    AmazonBedrock,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LoginAppBrand {
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "chatgpt")]
    Chatgpt,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LogoutAccountResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ManagedHooksRequirements {
    #[serde(
        rename = "Interrupt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#interrupt: Option<Vec<ConfiguredHookMatcherGroup>>,
    #[serde(rename = "PermissionRequest", deserialize_with = "required")]
    pub r#permission_request: Vec<ConfiguredHookMatcherGroup>,
    #[serde(rename = "PostCompact", deserialize_with = "required")]
    pub r#post_compact: Vec<ConfiguredHookMatcherGroup>,
    #[serde(
        rename = "PostModelRequest",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#post_model_request: Option<Vec<ConfiguredHookMatcherGroup>>,
    #[serde(rename = "PostToolUse", deserialize_with = "required")]
    pub r#post_tool_use: Vec<ConfiguredHookMatcherGroup>,
    #[serde(rename = "PreCompact", deserialize_with = "required")]
    pub r#pre_compact: Vec<ConfiguredHookMatcherGroup>,
    #[serde(
        rename = "PreModelRequest",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#pre_model_request: Option<Vec<ConfiguredHookMatcherGroup>>,
    #[serde(rename = "PreToolUse", deserialize_with = "required")]
    pub r#pre_tool_use: Vec<ConfiguredHookMatcherGroup>,
    #[serde(
        rename = "SessionEnd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#session_end: Option<Vec<ConfiguredHookMatcherGroup>>,
    #[serde(rename = "SessionStart", deserialize_with = "required")]
    pub r#session_start: Vec<ConfiguredHookMatcherGroup>,
    #[serde(rename = "Stop", deserialize_with = "required")]
    pub r#stop: Vec<ConfiguredHookMatcherGroup>,
    #[serde(rename = "SubagentStart", deserialize_with = "required")]
    pub r#subagent_start: Vec<ConfiguredHookMatcherGroup>,
    #[serde(rename = "SubagentStop", deserialize_with = "required")]
    pub r#subagent_stop: Vec<ConfiguredHookMatcherGroup>,
    #[serde(rename = "UserPromptSubmit", deserialize_with = "required")]
    pub r#user_prompt_submit: Vec<ConfiguredHookMatcherGroup>,
    #[serde(
        rename = "managedDir",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#managed_dir: Option<Option<String>>,
    #[serde(
        rename = "windowsManagedDir",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#windows_managed_dir: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceAddParams {
    #[serde(
        rename = "refName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ref_name: Option<Option<String>>,
    #[serde(rename = "source", deserialize_with = "required")]
    pub r#source: String,
    #[serde(
        rename = "sparsePaths",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sparse_paths: Option<Option<Vec<String>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceAddResponse {
    #[serde(rename = "alreadyAdded", deserialize_with = "required")]
    pub r#already_added: bool,
    #[serde(rename = "installedRoot", deserialize_with = "required")]
    pub r#installed_root: AbsolutePathBuf,
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceInterface {
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#display_name: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceLoadErrorInfo {
    #[serde(rename = "marketplacePath", deserialize_with = "required")]
    pub r#marketplace_path: AbsolutePathBuf,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceRemoveParams {
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceRemoveResponse {
    #[serde(
        rename = "installedRoot",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#installed_root: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceUpgradeErrorInfo {
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceUpgradeParams {
    #[serde(
        rename = "marketplaceName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_name: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketplaceUpgradeResponse {
    #[serde(rename = "errors", deserialize_with = "required")]
    pub r#errors: Vec<MarketplaceUpgradeErrorInfo>,
    #[serde(rename = "selectedMarketplaces", deserialize_with = "required")]
    pub r#selected_marketplaces: Vec<String>,
    #[serde(rename = "upgradedRoots", deserialize_with = "required")]
    pub r#upgraded_roots: Vec<AbsolutePathBuf>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpAppDisplayMode {
    #[serde(rename = "inline")]
    Inline,
    #[serde(rename = "fullscreen")]
    Fullscreen,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpAppUi {
    #[serde(rename = "preferredModelDisplayMode", deserialize_with = "required")]
    pub r#preferred_model_display_mode: McpAppDisplayMode,
    #[serde(rename = "resourceUri", deserialize_with = "required")]
    pub r#resource_uri: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpAuthStatus {
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "notLoggedIn")]
    NotLoggedIn,
    #[serde(rename = "bearerToken")]
    BearerToken,
    #[serde(rename = "oAuth")]
    OAuth,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpResourceReadParams {
    #[serde(
        rename = "connectorId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#connector_id: Option<Option<String>>,
    #[serde(
        rename = "originCallId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origin_call_id: Option<Option<String>>,
    #[serde(rename = "server", deserialize_with = "required")]
    pub r#server: String,
    #[serde(
        rename = "target",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#target: Option<Option<McpResourceReadTarget>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
    #[serde(rename = "uri", deserialize_with = "required")]
    pub r#uri: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpResourceReadResponse {
    #[serde(rename = "contents", deserialize_with = "required")]
    pub r#contents: Vec<ResourceContent>,
    #[serde(
        rename = "originCallId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origin_call_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpResourceReadTarget {
    #[serde(rename = "connectorId", deserialize_with = "required")]
    pub r#connector_id: String,
    #[serde(rename = "linkId", deserialize_with = "required")]
    pub r#link_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpServerConnectionStatus {
    #[serde(rename = "notStarted")]
    NotStarted,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "authenticationRequired")]
    AuthenticationRequired,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "disabled")]
    Disabled,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerEventNotification {
    #[serde(rename = "method", deserialize_with = "required")]
    pub r#method: String,
    #[serde(rename = "params", deserialize_with = "required")]
    pub r#params: studio_json::Value,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerEventStreamNotification {
    #[serde(rename = "notification", deserialize_with = "required")]
    pub r#notification: McpServerEventNotification,
    #[serde(rename = "subscriptionId", deserialize_with = "required")]
    pub r#subscription_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerEventStreamStartParams {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(rename = "arguments", deserialize_with = "required")]
    pub r#arguments: studio_json::Value,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "server", deserialize_with = "required")]
    pub r#server: String,
    #[serde(rename = "subscriptionId", deserialize_with = "required")]
    pub r#subscription_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerEventStreamStartResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerEventStreamStopParams {
    #[serde(rename = "subscriptionId", deserialize_with = "required")]
    pub r#subscription_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerEventStreamStopResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerInfo {
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "icons",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icons: Option<Option<Vec<studio_json::Value>>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "version", deserialize_with = "required")]
    pub r#version: String,
    #[serde(
        rename = "websiteUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#website_url: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerMigration {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpServerOauthClientRegistration {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "cimd")]
    Cimd,
    #[serde(rename = "dcr")]
    Dcr,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerOauthLoginCompletedNotification {
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<String>>,
    #[serde(
        rename = "loginId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#login_id: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "success", deserialize_with = "required")]
    pub r#success: bool,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerOauthLoginParams {
    #[serde(
        rename = "clientRegistration",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#client_registration: Option<Option<McpServerOauthClientRegistration>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "scopes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#scopes: Option<Option<Vec<String>>>,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
    #[serde(
        rename = "timeoutSecs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#timeout_secs: Option<Option<i64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerOauthLoginResponse {
    #[serde(rename = "authorizationUrl", deserialize_with = "required")]
    pub r#authorization_url: String,
    #[serde(
        rename = "loginId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#login_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerRefreshResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpServerStartupFailureReason {
    #[serde(rename = "reauthenticationRequired")]
    ReauthenticationRequired,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpServerStartupState {
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerStatus {
    #[serde(rename = "authStatus", deserialize_with = "required")]
    pub r#auth_status: McpAuthStatus,
    #[serde(
        rename = "httpOrigin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#http_origin: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "pluginId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#plugin_id: Option<Option<String>>,
    #[serde(rename = "resourceTemplates", deserialize_with = "required")]
    pub r#resource_templates: Vec<ResourceTemplate>,
    #[serde(rename = "resources", deserialize_with = "required")]
    pub r#resources: Vec<Resource>,
    #[serde(
        rename = "runtimeStatus",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_status: Option<Option<McpServerConnectionStatus>>,
    #[serde(
        rename = "serverCapabilities",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#server_capabilities: Option<studio_json::Value>,
    #[serde(
        rename = "serverInfo",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#server_info: Option<Option<McpServerInfo>>,
    #[serde(
        rename = "toolListMeta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tool_list_meta: Option<studio_json::Value>,
    #[serde(rename = "tools", deserialize_with = "required")]
    pub r#tools: std::collections::BTreeMap<String, Tool>,
    #[serde(
        rename = "toolsError",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tools_error: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpServerStatusDetail {
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "toolsAndAuthOnly")]
    ToolsAndAuthOnly,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerStatusUpdatedNotification {
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<String>>,
    #[serde(
        rename = "failureReason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#failure_reason: Option<Option<McpServerStartupFailureReason>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: McpServerStartupState,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerToolCallParams {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(
        rename = "arguments",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#arguments: Option<studio_json::Value>,
    #[serde(rename = "server", deserialize_with = "required")]
    pub r#server: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "tool", deserialize_with = "required")]
    pub r#tool: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpServerToolCallResponse {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(rename = "content", deserialize_with = "required")]
    pub r#content: Vec<studio_json::Value>,
    #[serde(
        rename = "isError",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#is_error: Option<Option<bool>>,
    #[serde(
        rename = "structuredContent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#structured_content: Option<studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpToolCallAppContext {
    #[serde(
        rename = "actionName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#action_name: Option<Option<String>>,
    #[serde(
        rename = "appName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#app_name: Option<Option<String>>,
    #[serde(rename = "connectorId", deserialize_with = "required")]
    pub r#connector_id: String,
    #[serde(
        rename = "linkId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#link_id: Option<Option<String>>,
    #[serde(
        rename = "resourceUri",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#resource_uri: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpToolCallError {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpToolCallProgressNotification {
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpToolCallResult {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(rename = "content", deserialize_with = "required")]
    pub r#content: Vec<studio_json::Value>,
    #[serde(
        rename = "structuredContent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#structured_content: Option<studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpToolCallStatus {
    #[serde(rename = "inProgress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryCitation {
    #[serde(rename = "entries", deserialize_with = "required")]
    pub r#entries: Vec<MemoryCitationEntry>,
    #[serde(rename = "threadIds", deserialize_with = "required")]
    pub r#thread_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryCitationEntry {
    #[serde(rename = "lineEnd", deserialize_with = "required")]
    pub r#line_end: u32,
    #[serde(rename = "lineStart", deserialize_with = "required")]
    pub r#line_start: u32,
    #[serde(rename = "note", deserialize_with = "required")]
    pub r#note: String,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryResetResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryStatusParams {
    #[serde(
        rename = "minConsolidatedThreads",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#min_consolidated_threads: Option<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryStatusResponse {
    #[serde(rename = "v2ConsolidatedThreads", deserialize_with = "required")]
    pub r#v2_consolidated_threads: u32,
    #[serde(rename = "v2Ready", deserialize_with = "required")]
    pub r#v2_ready: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MergeStrategy {
    #[serde(rename = "replace")]
    Replace,
    #[serde(rename = "upsert")]
    Upsert,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MessagePhase {
    Variant1(Box<MessagePhaseVariant1>),
    Variant2(Box<MessagePhaseVariant2>),
    Variant3(Box<MessagePhaseVariant3>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MigrationDetails {
    #[serde(
        rename = "commands",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#commands: Option<Vec<CommandMigration>>,
    #[serde(
        rename = "hooks",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#hooks: Option<Vec<HookMigration>>,
    #[serde(
        rename = "mcpServers",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mcp_servers: Option<Vec<McpServerMigration>>,
    #[serde(
        rename = "memory",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#memory: Option<Vec<String>>,
    #[serde(
        rename = "plugins",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#plugins: Option<Vec<PluginsMigration>>,
    #[serde(
        rename = "sessions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sessions: Option<Vec<SessionMigration>>,
    #[serde(
        rename = "skills",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#skills: Option<Vec<SkillMigration>>,
    #[serde(
        rename = "subagents",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#subagents: Option<Vec<SubagentMigration>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MisalignmentErrorDetails {
    #[serde(
        rename = "detailedExplanation",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#detailed_explanation: Option<Option<String>>,
    #[serde(
        rename = "errorType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error_type: Option<Option<String>>,
    #[serde(
        rename = "reviewTarget",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#review_target: Option<Option<String>>,
    #[serde(
        rename = "steer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#steer: Option<Option<MisalignmentSteer>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MisalignmentSteer {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MockExperimentalMethodParams {
    #[serde(
        rename = "value",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#value: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MockExperimentalMethodResponse {
    #[serde(
        rename = "echoed",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#echoed: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModeKind {
    #[serde(rename = "plan")]
    Plan,
    #[serde(rename = "default")]
    Default,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Model {
    #[serde(
        rename = "additionalSpeedTiers",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#additional_speed_tiers: Option<Vec<String>>,
    #[serde(
        rename = "availabilityNux",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#availability_nux: Option<Option<ModelAvailabilityNux>>,
    #[serde(
        rename = "availableAccessPrograms",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#available_access_programs: Option<Option<ModelAccessPrograms>>,
    #[serde(rename = "defaultReasoningEffort", deserialize_with = "required")]
    pub r#default_reasoning_effort: ReasoningEffort,
    #[serde(
        rename = "defaultServiceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_service_tier: Option<Option<String>>,
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(rename = "displayName", deserialize_with = "required")]
    pub r#display_name: String,
    #[serde(rename = "hidden", deserialize_with = "required")]
    pub r#hidden: bool,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(
        rename = "inputModalities",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#input_modalities: Option<Vec<InputModality>>,
    #[serde(rename = "isDefault", deserialize_with = "required")]
    pub r#is_default: bool,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(
        rename = "modelSpecialty",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_specialty: Option<Option<String>>,
    #[serde(
        rename = "multiAgentVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_version: Option<Option<MultiAgentVersion>>,
    #[serde(
        rename = "serviceTiers",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tiers: Option<Vec<ModelServiceTier>>,
    #[serde(rename = "supportedReasoningEfforts", deserialize_with = "required")]
    pub r#supported_reasoning_efforts: Vec<ReasoningEffortOption>,
    #[serde(
        rename = "supportsPersonality",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#supports_personality: Option<bool>,
    #[serde(
        rename = "upgrade",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#upgrade: Option<Option<String>>,
    #[serde(
        rename = "upgradeInfo",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#upgrade_info: Option<Option<ModelUpgradeInfo>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelAccessPrograms {
    #[serde(rename = "cyber", deserialize_with = "required")]
    pub r#cyber: Vec<CyberAccessProgram>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelAvailabilityNux {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "includeHidden",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_hidden: Option<Option<bool>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<Model>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelProviderCapabilitiesReadParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelProviderCapabilitiesReadResponse {
    #[serde(rename = "imageGeneration", deserialize_with = "required")]
    pub r#image_generation: bool,
    #[serde(rename = "webSearch", deserialize_with = "required")]
    pub r#web_search: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ModelProviderError {
    #[serde(rename = "modelProviderMismatch")]
    ModelProviderMismatch {
        #[serde(rename = "currentModelProvider", deserialize_with = "required")]
        r#current_model_provider: String,
        #[serde(rename = "modelProvider", deserialize_with = "required")]
        r#model_provider: String,
    },
    #[serde(rename = "modelProviderNotConfigured")]
    ModelProviderNotConfigured {
        #[serde(rename = "modelProvider", deserialize_with = "required")]
        r#model_provider: String,
    },
    #[serde(rename = "modelProviderPolicyConflict")]
    ModelProviderPolicyConflict {
        #[serde(rename = "modelProvider", deserialize_with = "required")]
        r#model_provider: String,
        #[serde(rename = "requiredModelProvider", deserialize_with = "required")]
        r#required_model_provider: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModelRerouteReason {
    #[serde(rename = "highRiskCyberActivity")]
    HighRiskCyberActivity,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelReroutedNotification {
    #[serde(rename = "fromModel", deserialize_with = "required")]
    pub r#from_model: String,
    #[serde(rename = "reason", deserialize_with = "required")]
    pub r#reason: ModelRerouteReason,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "toModel", deserialize_with = "required")]
    pub r#to_model: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelSafetyBufferingUpdatedNotification {
    #[serde(
        rename = "fasterModel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#faster_model: Option<Option<String>>,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(rename = "reasons", deserialize_with = "required")]
    pub r#reasons: Vec<String>,
    #[serde(rename = "showBufferingUi", deserialize_with = "required")]
    pub r#show_buffering_ui: bool,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
    #[serde(rename = "useCases", deserialize_with = "required")]
    pub r#use_cases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelServiceTier {
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelUpgradeInfo {
    #[serde(
        rename = "migrationMarkdown",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#migration_markdown: Option<Option<String>>,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(
        rename = "modelLink",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_link: Option<Option<String>>,
    #[serde(
        rename = "retirementAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#retirement_at: Option<Option<i64>>,
    #[serde(
        rename = "upgradeCopy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#upgrade_copy: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModelVerification {
    #[serde(rename = "trustedAccessForCyber")]
    TrustedAccessForCyber,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelVerificationNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
    #[serde(rename = "verifications", deserialize_with = "required")]
    pub r#verifications: Vec<ModelVerification>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelsRequirements {
    #[serde(
        rename = "newThread",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#new_thread: Option<Option<NewThreadModelDefaults>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MultiAgentMode {
    Variant1(Box<MultiAgentModeVariant1>),
    Variant2(Box<MultiAgentModeVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MultiAgentVersion {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "v1")]
    V1,
    #[serde(rename = "v2")]
    V2,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkAccess {
    #[serde(rename = "restricted")]
    Restricted,
    #[serde(rename = "enabled")]
    Enabled,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkApprovalProtocol {
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "https")]
    Https,
    #[serde(rename = "socks5Tcp")]
    Socks5Tcp,
    #[serde(rename = "socks5Udp")]
    Socks5Udp,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkDomainPermission {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetworkRequirements {
    #[serde(
        rename = "allowLocalBinding",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_local_binding: Option<Option<bool>>,
    #[serde(
        rename = "allowUnixSockets",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_unix_sockets: Option<Option<Vec<String>>>,
    #[serde(
        rename = "allowUpstreamProxy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_upstream_proxy: Option<Option<bool>>,
    #[serde(
        rename = "allowedDomains",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_domains: Option<Option<Vec<String>>>,
    #[serde(
        rename = "dangerouslyAllowAllUnixSockets",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#dangerously_allow_all_unix_sockets: Option<Option<bool>>,
    #[serde(
        rename = "dangerouslyAllowNonLoopbackProxy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#dangerously_allow_non_loopback_proxy: Option<Option<bool>>,
    #[serde(
        rename = "deniedDomains",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#denied_domains: Option<Option<Vec<String>>>,
    #[serde(
        rename = "domains",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#domains: Option<Option<std::collections::BTreeMap<String, NetworkDomainPermission>>>,
    #[serde(
        rename = "enabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#enabled: Option<Option<bool>>,
    #[serde(
        rename = "httpPort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#http_port: Option<Option<u16>>,
    #[serde(
        rename = "managedAllowedDomainsOnly",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#managed_allowed_domains_only: Option<Option<bool>>,
    #[serde(
        rename = "reviewDomains",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#review_domains: Option<Option<Vec<String>>>,
    #[serde(
        rename = "socksPort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#socks_port: Option<Option<u16>>,
    #[serde(
        rename = "unixSockets",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#unix_sockets:
        Option<Option<std::collections::BTreeMap<String, NetworkUnixSocketPermission>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkUnixSocketPermission {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NewThreadModelDefaults {
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "modelReasoningEffort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_reasoning_effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NonSteerableTurnKind {
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "compact")]
    Compact,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum NullableGetAccountRateLimitsParams {
    Variant1(Box<GetAccountRateLimitsParams>),
    Variant2(Box<()>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum NullableGetAccountTokenUsageParams {
    Variant1(Box<GetAccountTokenUsageParams>),
    Variant2(Box<()>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum NullableRemoteControlDisableParams {
    Variant1(Box<RemoteControlDisableParams>),
    Variant2(Box<()>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum NullableRemoteControlEnableParams {
    Variant1(Box<RemoteControlEnableParams>),
    Variant2(Box<()>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum OtelExporter {
    Variant1(Box<OtelExporterVariant1>),
    Variant2(Box<OtelExporterVariant2>),
    Variant3(Box<OtelExporterVariant3>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OtelHttpProtocol {
    #[serde(rename = "binary")]
    Binary,
    #[serde(rename = "json")]
    Json,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OtelRequirements {
    #[serde(
        rename = "environment",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environment: Option<Option<String>>,
    #[serde(
        rename = "exporter",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exporter: Option<Option<OtelExporter>>,
    #[serde(
        rename = "log_agent_responses",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#log_agent_responses: Option<Option<bool>>,
    #[serde(
        rename = "log_guardian_assessments",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#log_guardian_assessments: Option<Option<bool>>,
    #[serde(
        rename = "log_sse_deltas",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#log_sse_deltas: Option<Option<bool>>,
    #[serde(
        rename = "log_user_prompt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#log_user_prompt: Option<Option<bool>>,
    #[serde(
        rename = "metrics_exporter",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#metrics_exporter: Option<Option<OtelExporter>>,
    #[serde(
        rename = "span_attributes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#span_attributes: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "tool_result",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tool_result: Option<Option<OtelToolResultLogConfig>>,
    #[serde(
        rename = "trace_exporter",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#trace_exporter: Option<Option<OtelExporter>>,
    #[serde(
        rename = "tracestate",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tracestate: Option<
        Option<std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>>,
    >,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OtelTlsConfig {
    #[serde(
        rename = "ca-certificate",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ca_certificate: Option<Option<String>>,
    #[serde(
        rename = "client-certificate",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#client_certificate: Option<Option<String>>,
    #[serde(
        rename = "client-private-key",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#client_private_key: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OtelToolResultLogConfig {
    #[serde(rename = "max_bytes", deserialize_with = "required")]
    pub r#max_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OverriddenMetadata {
    #[serde(rename = "effectiveValue", deserialize_with = "required")]
    pub r#effective_value: studio_json::Value,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(rename = "overridingLayer", deserialize_with = "required")]
    pub r#overriding_layer: ConfigLayerMetadata,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PatchApplyStatus {
    #[serde(rename = "inProgress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "declined")]
    Declined,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum PatchChangeKind {
    #[serde(rename = "add")]
    Add,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "update")]
    Update {
        #[serde(
            rename = "move_path",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#move_path: Option<Option<String>>,
    },
}

pub type PathUri = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionProfileListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionProfileListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<PermissionProfileSummary>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionProfileSummary {
    #[serde(rename = "allowed", deserialize_with = "required")]
    pub r#allowed: bool,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Personality {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "friendly")]
    Friendly,
    #[serde(rename = "pragmatic")]
    Pragmatic,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanDeltaNotification {
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PlanType {
    #[serde(rename = "free")]
    Free,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "plus")]
    Plus,
    #[serde(rename = "pro")]
    Pro,
    #[serde(rename = "prolite")]
    Prolite,
    #[serde(rename = "promax")]
    Promax,
    #[serde(rename = "team")]
    Team,
    #[serde(rename = "self_serve_business_prolite")]
    SelfServeBusinessProlite,
    #[serde(rename = "self_serve_business_usage_based")]
    SelfServeBusinessUsageBased,
    #[serde(rename = "business")]
    Business,
    #[serde(rename = "ent26")]
    Ent26,
    #[serde(rename = "enterprise_cbp_automation")]
    EnterpriseCbpAutomation,
    #[serde(rename = "enterprise_cbp_usage_based")]
    EnterpriseCbpUsageBased,
    #[serde(rename = "enterprise")]
    Enterprise,
    #[serde(rename = "edu")]
    Edu,
    #[serde(rename = "edu_plus")]
    EduPlus,
    #[serde(rename = "edu_pro")]
    EduPro,
    #[serde(rename = "sci")]
    Sci,
    #[serde(rename = "sci_plus")]
    SciPlus,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginAuthPolicy {
    #[serde(rename = "ON_INSTALL")]
    ONINSTALL,
    #[serde(rename = "ON_USE")]
    ONUSE,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum PluginAvailability {
    Variant1(Box<PluginAvailabilityVariant1>),
    Variant2(Box<PluginAvailabilityVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginDetail {
    #[serde(rename = "appTemplates", deserialize_with = "required")]
    pub r#app_templates: Vec<AppTemplateSummary>,
    #[serde(rename = "apps", deserialize_with = "required")]
    pub r#apps: Vec<AppSummary>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(rename = "hooks", deserialize_with = "required")]
    pub r#hooks: Vec<PluginHookSummary>,
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
    #[serde(
        rename = "marketplacePath",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_path: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "mcpServers", deserialize_with = "required")]
    pub r#mcp_servers: Vec<String>,
    #[serde(
        rename = "onboardingSkill",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#onboarding_skill: Option<Option<SkillSummary>>,
    #[serde(
        rename = "scheduledTasks",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#scheduled_tasks: Option<Option<Vec<ScheduledTaskSummary>>>,
    #[serde(
        rename = "shareUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#share_url: Option<Option<String>>,
    #[serde(rename = "skills", deserialize_with = "required")]
    pub r#skills: Vec<SkillSummary>,
    #[serde(rename = "summary", deserialize_with = "required")]
    pub r#summary: PluginSummary,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginDisabledReason {
    #[serde(rename = "disabled_by_admin")]
    DisabledByAdmin,
    #[serde(rename = "plan_not_eligible")]
    PlanNotEligible,
    #[serde(rename = "required_app_unavailable")]
    RequiredAppUnavailable,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginHookSummary {
    #[serde(rename = "eventName", deserialize_with = "required")]
    pub r#event_name: HookEventName,
    #[serde(rename = "key", deserialize_with = "required")]
    pub r#key: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginInstallParams {
    #[serde(
        rename = "installAttemptId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#install_attempt_id: Option<Option<String>>,
    #[serde(
        rename = "marketplacePath",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_path: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "pluginName", deserialize_with = "required")]
    pub r#plugin_name: String,
    #[serde(
        rename = "remoteMarketplaceName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#remote_marketplace_name: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginInstallPolicy {
    #[serde(rename = "NOT_AVAILABLE")]
    NOTAVAILABLE,
    #[serde(rename = "AVAILABLE")]
    AVAILABLE,
    #[serde(rename = "INSTALLED_BY_DEFAULT")]
    INSTALLEDBYDEFAULT,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginInstallPolicySource {
    #[serde(rename = "WORKSPACE_SETTING")]
    WORKSPACESETTING,
    #[serde(rename = "IMPLICIT_CANONICAL_APP")]
    IMPLICITCANONICALAPP,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginInstallResponse {
    #[serde(rename = "appsNeedingAuth", deserialize_with = "required")]
    pub r#apps_needing_auth: Vec<AppSummary>,
    #[serde(rename = "authPolicy", deserialize_with = "required")]
    pub r#auth_policy: PluginAuthPolicy,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginInstalledParams {
    #[serde(
        rename = "cwds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwds: Option<Option<Vec<AbsolutePathBuf>>>,
    #[serde(
        rename = "installSuggestionPluginNames",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#install_suggestion_plugin_names: Option<Option<Vec<String>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginInstalledResponse {
    #[serde(
        rename = "marketplaceLoadErrors",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_load_errors: Option<Vec<MarketplaceLoadErrorInfo>>,
    #[serde(rename = "marketplaces", deserialize_with = "required")]
    pub r#marketplaces: Vec<PluginMarketplaceEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginInterface {
    #[serde(
        rename = "brandColor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#brand_color: Option<Option<String>>,
    #[serde(rename = "capabilities", deserialize_with = "required")]
    pub r#capabilities: Vec<String>,
    #[serde(
        rename = "category",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#category: Option<Option<String>>,
    #[serde(
        rename = "composerIcon",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#composer_icon: Option<Option<AbsolutePathBuf>>,
    #[serde(
        rename = "composerIconUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#composer_icon_url: Option<Option<String>>,
    #[serde(
        rename = "defaultPrompt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_prompt: Option<Option<Vec<String>>>,
    #[serde(
        rename = "developerName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer_name: Option<Option<String>>,
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#display_name: Option<Option<String>>,
    #[serde(
        rename = "logo",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo: Option<Option<AbsolutePathBuf>>,
    #[serde(
        rename = "logoDark",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo_dark: Option<Option<AbsolutePathBuf>>,
    #[serde(
        rename = "logoUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo_url: Option<Option<String>>,
    #[serde(
        rename = "logoUrlDark",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#logo_url_dark: Option<Option<String>>,
    #[serde(
        rename = "longDescription",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#long_description: Option<Option<String>>,
    #[serde(
        rename = "privacyPolicyUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#privacy_policy_url: Option<Option<String>>,
    #[serde(rename = "screenshotUrls", deserialize_with = "required")]
    pub r#screenshot_urls: Vec<String>,
    #[serde(rename = "screenshots", deserialize_with = "required")]
    pub r#screenshots: Vec<AbsolutePathBuf>,
    #[serde(
        rename = "shortDescription",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#short_description: Option<Option<String>>,
    #[serde(
        rename = "termsOfServiceUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#terms_of_service_url: Option<Option<String>>,
    #[serde(
        rename = "websiteUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#website_url: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginListMarketplaceKind {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "vertical")]
    Vertical,
    #[serde(rename = "workspace-directory")]
    WorkspaceDirectory,
    #[serde(rename = "shared-with-me")]
    SharedWithMe,
    #[serde(rename = "created-by-me-remote")]
    CreatedByMeRemote,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginListParams {
    #[serde(
        rename = "cwds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwds: Option<Option<Vec<AbsolutePathBuf>>>,
    #[serde(
        rename = "forceRefetch",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#force_refetch: Option<bool>,
    #[serde(
        rename = "marketplaceKinds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_kinds: Option<Option<Vec<PluginListMarketplaceKind>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginListResponse {
    #[serde(
        rename = "featuredPluginIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#featured_plugin_ids: Option<Vec<String>>,
    #[serde(
        rename = "marketplaceLoadErrors",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_load_errors: Option<Vec<MarketplaceLoadErrorInfo>>,
    #[serde(rename = "marketplaces", deserialize_with = "required")]
    pub r#marketplaces: Vec<PluginMarketplaceEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginMarketplaceEntry {
    #[serde(
        rename = "interface",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#interface: Option<Option<MarketplaceInterface>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#path: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "plugins", deserialize_with = "required")]
    pub r#plugins: Vec<PluginSummary>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginReadParams {
    #[serde(
        rename = "marketplacePath",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_path: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "pluginName", deserialize_with = "required")]
    pub r#plugin_name: String,
    #[serde(
        rename = "remoteMarketplaceName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#remote_marketplace_name: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginReadResponse {
    #[serde(rename = "plugin", deserialize_with = "required")]
    pub r#plugin: PluginDetail,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginReconcileChangedPlugin {
    #[serde(rename = "hasApps", deserialize_with = "required")]
    pub r#has_apps: bool,
    #[serde(rename = "hasHooks", deserialize_with = "required")]
    pub r#has_hooks: bool,
    #[serde(rename = "hasMcps", deserialize_with = "required")]
    pub r#has_mcps: bool,
    #[serde(rename = "hasSkills", deserialize_with = "required")]
    pub r#has_skills: bool,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginReconcileParams {
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginReconcileResponse {
    #[serde(rename = "changedPlugins", deserialize_with = "required")]
    pub r#changed_plugins: Vec<PluginReconcileChangedPlugin>,
    #[serde(
        rename = "failedMaterializationRemotePluginIds",
        deserialize_with = "required"
    )]
    pub r#failed_materialization_remote_plugin_ids: Vec<String>,
    #[serde(rename = "failedRemotePluginIds", deserialize_with = "required")]
    pub r#failed_remote_plugin_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginSearchParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "cwds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwds: Option<Option<Vec<AbsolutePathBuf>>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "scope",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#scope: Option<Option<PluginSearchScope>>,
    #[serde(rename = "searchTerm", deserialize_with = "required")]
    pub r#search_term: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginSearchResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<PluginSearchResult>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginSearchResult {
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
    #[serde(
        rename = "marketplacePath",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#marketplace_path: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "plugin", deserialize_with = "required")]
    pub r#plugin: PluginSummary,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginSearchScope {
    #[serde(rename = "global")]
    Global,
    #[serde(rename = "workspace")]
    Workspace,
    #[serde(rename = "personal")]
    Personal,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareCheckoutParams {
    #[serde(rename = "remotePluginId", deserialize_with = "required")]
    pub r#remote_plugin_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareCheckoutResponse {
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
    #[serde(rename = "marketplacePath", deserialize_with = "required")]
    pub r#marketplace_path: AbsolutePathBuf,
    #[serde(rename = "pluginId", deserialize_with = "required")]
    pub r#plugin_id: String,
    #[serde(rename = "pluginName", deserialize_with = "required")]
    pub r#plugin_name: String,
    #[serde(rename = "pluginPath", deserialize_with = "required")]
    pub r#plugin_path: AbsolutePathBuf,
    #[serde(rename = "remotePluginId", deserialize_with = "required")]
    pub r#remote_plugin_id: String,
    #[serde(
        rename = "remoteVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#remote_version: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareContext {
    #[serde(
        rename = "canPublishToWorkspace",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#can_publish_to_workspace: Option<Option<bool>>,
    #[serde(
        rename = "creatorAccountUserId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#creator_account_user_id: Option<Option<String>>,
    #[serde(
        rename = "creatorName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#creator_name: Option<Option<String>>,
    #[serde(
        rename = "discoverability",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#discoverability: Option<Option<PluginShareDiscoverability>>,
    #[serde(rename = "remotePluginId", deserialize_with = "required")]
    pub r#remote_plugin_id: String,
    #[serde(
        rename = "remoteVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#remote_version: Option<Option<String>>,
    #[serde(
        rename = "sharePrincipals",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#share_principals: Option<Option<Vec<PluginSharePrincipal>>>,
    #[serde(
        rename = "shareUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#share_url: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareDeleteParams {
    #[serde(rename = "remotePluginId", deserialize_with = "required")]
    pub r#remote_plugin_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareDeleteResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginShareDiscoverability {
    #[serde(rename = "LISTED")]
    LISTED,
    #[serde(rename = "UNLISTED")]
    UNLISTED,
    #[serde(rename = "PRIVATE")]
    PRIVATE,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareListItem {
    #[serde(
        rename = "localPluginPath",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#local_plugin_path: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "plugin", deserialize_with = "required")]
    pub r#plugin: PluginSummary,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareListParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<PluginShareListItem>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginSharePrincipal {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "principalId", deserialize_with = "required")]
    pub r#principal_id: String,
    #[serde(rename = "principalType", deserialize_with = "required")]
    pub r#principal_type: PluginSharePrincipalType,
    #[serde(rename = "role", deserialize_with = "required")]
    pub r#role: PluginSharePrincipalRole,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginSharePrincipalRole {
    #[serde(rename = "reader")]
    Reader,
    #[serde(rename = "editor")]
    Editor,
    #[serde(rename = "owner")]
    Owner,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginSharePrincipalType {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "workspace")]
    Workspace,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareSaveParams {
    #[serde(
        rename = "discoverability",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#discoverability: Option<Option<PluginShareDiscoverability>>,
    #[serde(rename = "pluginPath", deserialize_with = "required")]
    pub r#plugin_path: AbsolutePathBuf,
    #[serde(
        rename = "remotePluginId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#remote_plugin_id: Option<Option<String>>,
    #[serde(
        rename = "shareTargets",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#share_targets: Option<Option<Vec<PluginShareTarget>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareSaveResponse {
    #[serde(
        rename = "canPublishToWorkspace",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#can_publish_to_workspace: Option<Option<bool>>,
    #[serde(rename = "remotePluginId", deserialize_with = "required")]
    pub r#remote_plugin_id: String,
    #[serde(rename = "shareUrl", deserialize_with = "required")]
    pub r#share_url: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareTarget {
    #[serde(rename = "principalId", deserialize_with = "required")]
    pub r#principal_id: String,
    #[serde(rename = "principalType", deserialize_with = "required")]
    pub r#principal_type: PluginSharePrincipalType,
    #[serde(rename = "role", deserialize_with = "required")]
    pub r#role: PluginShareTargetRole,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginShareTargetRole {
    #[serde(rename = "reader")]
    Reader,
    #[serde(rename = "editor")]
    Editor,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginShareUpdateDiscoverability {
    #[serde(rename = "UNLISTED")]
    UNLISTED,
    #[serde(rename = "PRIVATE")]
    PRIVATE,
    #[serde(rename = "LISTED")]
    LISTED,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareUpdateTargetsParams {
    #[serde(rename = "discoverability", deserialize_with = "required")]
    pub r#discoverability: PluginShareUpdateDiscoverability,
    #[serde(rename = "remotePluginId", deserialize_with = "required")]
    pub r#remote_plugin_id: String,
    #[serde(rename = "shareTargets", deserialize_with = "required")]
    pub r#share_targets: Vec<PluginShareTarget>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginShareUpdateTargetsResponse {
    #[serde(rename = "discoverability", deserialize_with = "required")]
    pub r#discoverability: PluginShareDiscoverability,
    #[serde(rename = "principals", deserialize_with = "required")]
    pub r#principals: Vec<PluginSharePrincipal>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginSkillReadParams {
    #[serde(rename = "remoteMarketplaceName", deserialize_with = "required")]
    pub r#remote_marketplace_name: String,
    #[serde(rename = "remotePluginId", deserialize_with = "required")]
    pub r#remote_plugin_id: String,
    #[serde(rename = "skillName", deserialize_with = "required")]
    pub r#skill_name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginSkillReadResponse {
    #[serde(
        rename = "contents",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#contents: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum PluginSource {
    #[serde(rename = "local")]
    Local {
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: AbsolutePathBuf,
    },
    #[serde(rename = "git")]
    Git {
        #[serde(
            rename = "path",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#path: Option<Option<String>>,
        #[serde(
            rename = "refName",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#ref_name: Option<Option<String>>,
        #[serde(
            rename = "sha",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#sha: Option<Option<String>>,
        #[serde(rename = "url", deserialize_with = "required")]
        r#url: String,
    },
    #[serde(rename = "npm")]
    Npm {
        #[serde(rename = "package", deserialize_with = "required")]
        r#package: String,
        #[serde(
            rename = "registry",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#registry: Option<Option<String>>,
        #[serde(
            rename = "version",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#version: Option<Option<String>>,
    },
    #[serde(rename = "remote")]
    Remote,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginSummary {
    #[serde(rename = "authPolicy", deserialize_with = "required")]
    pub r#auth_policy: PluginAuthPolicy,
    #[serde(
        rename = "availability",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#availability: Option<PluginAvailability>,
    #[serde(
        rename = "disabledReason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_reason: Option<Option<PluginDisabledReason>>,
    #[serde(
        rename = "eligiblePlanTypes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#eligible_plan_types: Option<Option<Vec<String>>>,
    #[serde(rename = "enabled", deserialize_with = "required")]
    pub r#enabled: bool,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "installPolicy", deserialize_with = "required")]
    pub r#install_policy: PluginInstallPolicy,
    #[serde(
        rename = "installPolicySource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#install_policy_source: Option<Option<PluginInstallPolicySource>>,
    #[serde(rename = "installed", deserialize_with = "required")]
    pub r#installed: bool,
    #[serde(
        rename = "installedAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#installed_at: Option<Option<i64>>,
    #[serde(
        rename = "interface",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#interface: Option<Option<PluginInterface>>,
    #[serde(
        rename = "keywords",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#keywords: Option<Vec<String>>,
    #[serde(
        rename = "localVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#local_version: Option<Option<String>>,
    #[serde(
        rename = "mustShowInstallationInterstitial",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#must_show_installation_interstitial: Option<Option<bool>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "remotePluginId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#remote_plugin_id: Option<Option<String>>,
    #[serde(
        rename = "shareContext",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#share_context: Option<Option<PluginShareContext>>,
    #[serde(rename = "source", deserialize_with = "required")]
    pub r#source: PluginSource,
    #[serde(
        rename = "version",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#version: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginUninstallParams {
    #[serde(rename = "pluginId", deserialize_with = "required")]
    pub r#plugin_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginUninstallResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginsMigration {
    #[serde(rename = "marketplaceName", deserialize_with = "required")]
    pub r#marketplace_name: String,
    #[serde(rename = "pluginNames", deserialize_with = "required")]
    pub r#plugin_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessExitedNotification {
    #[serde(rename = "exitCode", deserialize_with = "required")]
    pub r#exit_code: i32,
    #[serde(
        rename = "outputDrainTimedOut",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#output_drain_timed_out: Option<Option<bool>>,
    #[serde(rename = "processHandle", deserialize_with = "required")]
    pub r#process_handle: String,
    #[serde(rename = "stderr", deserialize_with = "required")]
    pub r#stderr: String,
    #[serde(rename = "stderrCapReached", deserialize_with = "required")]
    pub r#stderr_cap_reached: bool,
    #[serde(rename = "stdout", deserialize_with = "required")]
    pub r#stdout: String,
    #[serde(rename = "stdoutCapReached", deserialize_with = "required")]
    pub r#stdout_cap_reached: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessKillParams {
    #[serde(rename = "processHandle", deserialize_with = "required")]
    pub r#process_handle: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessKillResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessOutputDeltaNotification {
    #[serde(rename = "capReached", deserialize_with = "required")]
    pub r#cap_reached: bool,
    #[serde(rename = "deltaBase64", deserialize_with = "required")]
    pub r#delta_base64: String,
    #[serde(rename = "processHandle", deserialize_with = "required")]
    pub r#process_handle: String,
    #[serde(rename = "stream", deserialize_with = "required")]
    pub r#stream: ProcessOutputStream,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ProcessOutputStream {
    Variant1(Box<ProcessOutputStreamVariant1>),
    Variant2(Box<ProcessOutputStreamVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessResizePtyParams {
    #[serde(rename = "processHandle", deserialize_with = "required")]
    pub r#process_handle: String,
    #[serde(rename = "size", deserialize_with = "required")]
    pub r#size: ProcessTerminalSize,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessResizePtyResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessSpawnParams {
    #[serde(rename = "command", deserialize_with = "required")]
    pub r#command: Vec<String>,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: AbsolutePathBuf,
    #[serde(
        rename = "env",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#env: Option<Option<std::collections::BTreeMap<String, Option<String>>>>,
    #[serde(
        rename = "outputBytesCap",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#output_bytes_cap: Option<Option<u64>>,
    #[serde(rename = "processHandle", deserialize_with = "required")]
    pub r#process_handle: String,
    #[serde(
        rename = "size",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#size: Option<Option<ProcessTerminalSize>>,
    #[serde(
        rename = "streamStdin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#stream_stdin: Option<bool>,
    #[serde(
        rename = "streamStdoutStderr",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#stream_stdout_stderr: Option<bool>,
    #[serde(
        rename = "timeoutMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#timeout_ms: Option<Option<i64>>,
    #[serde(
        rename = "tty",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tty: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessSpawnResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessTerminalSize {
    #[serde(rename = "cols", deserialize_with = "required")]
    pub r#cols: u16,
    #[serde(rename = "rows", deserialize_with = "required")]
    pub r#rows: u16,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessWriteStdinParams {
    #[serde(
        rename = "closeStdin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#close_stdin: Option<bool>,
    #[serde(
        rename = "deltaBase64",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#delta_base64: Option<Option<String>>,
    #[serde(rename = "processHandle", deserialize_with = "required")]
    pub r#process_handle: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProcessWriteStdinResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Project {
    #[serde(rename = "createdAt", deserialize_with = "required")]
    pub r#created_at: i64,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "metadata", deserialize_with = "required")]
    pub r#metadata: std::collections::BTreeMap<String, String>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "position", deserialize_with = "required")]
    pub r#position: i64,
    #[serde(
        rename = "recencyAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#recency_at: Option<Option<i64>>,
    #[serde(rename = "roots", deserialize_with = "required")]
    pub r#roots: Vec<ProjectRoot>,
    #[serde(rename = "updatedAt", deserialize_with = "required")]
    pub r#updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProjectChangeType {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "updated")]
    Updated,
    #[serde(rename = "deleted")]
    Deleted,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectChangedNotification {
    #[serde(rename = "changeType", deserialize_with = "required")]
    pub r#change_type: ProjectChangeType,
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectCreateParams {
    #[serde(rename = "idempotencyKey", deserialize_with = "required")]
    pub r#idempotency_key: String,
    #[serde(
        rename = "metadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#metadata: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "roots", deserialize_with = "required")]
    pub r#roots: Vec<ProjectRoot>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectCreateResponse {
    #[serde(rename = "project", deserialize_with = "required")]
    pub r#project: Project,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectDeleteParams {
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectDeleteResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectImportParams {
    #[serde(rename = "idempotencyKey", deserialize_with = "required")]
    pub r#idempotency_key: String,
    #[serde(
        rename = "metadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#metadata: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "roots", deserialize_with = "required")]
    pub r#roots: Vec<ProjectRoot>,
    #[serde(
        rename = "threads",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#threads: Option<Option<Vec<String>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectImportResponse {
    #[serde(rename = "project", deserialize_with = "required")]
    pub r#project: Project,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "sortDirection",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_direction: Option<Option<SortDirection>>,
    #[serde(
        rename = "sortKey",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_key: Option<Option<ProjectSortKey>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<Project>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectMoveParams {
    #[serde(
        rename = "beforeProjectId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#before_project_id: Option<Option<String>>,
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectMoveResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectReadParams {
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectReadResponse {
    #[serde(rename = "project", deserialize_with = "required")]
    pub r#project: Project,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectRoot {
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProjectSortKey {
    #[serde(rename = "position")]
    Position,
    #[serde(rename = "recencyAt")]
    RecencyAt,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectUpdateParams {
    #[serde(
        rename = "metadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#metadata: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "name",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#name: Option<Option<String>>,
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: String,
    #[serde(
        rename = "roots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#roots: Option<Option<Vec<ProjectRoot>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectUpdateResponse {
    #[serde(rename = "project", deserialize_with = "required")]
    pub r#project: Project,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct QueuedSubmission {
    #[serde(rename = "clientUserMessageId", deserialize_with = "required")]
    pub r#client_user_message_id: String,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "input", deserialize_with = "required")]
    pub r#input: Vec<UserInput>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RateLimitReachedType {
    #[serde(rename = "rate_limit_reached")]
    RateLimitReached,
    #[serde(rename = "workspace_owner_credits_depleted")]
    WorkspaceOwnerCreditsDepleted,
    #[serde(rename = "workspace_member_credits_depleted")]
    WorkspaceMemberCreditsDepleted,
    #[serde(rename = "workspace_owner_usage_limit_reached")]
    WorkspaceOwnerUsageLimitReached,
    #[serde(rename = "workspace_member_usage_limit_reached")]
    WorkspaceMemberUsageLimitReached,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RateLimitResetCredit {
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "expiresAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#expires_at: Option<Option<i64>>,
    #[serde(rename = "grantedAt", deserialize_with = "required")]
    pub r#granted_at: i64,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "resetType", deserialize_with = "required")]
    pub r#reset_type: RateLimitResetType,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: RateLimitResetCreditStatus,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RateLimitResetCreditStatus {
    #[serde(rename = "available")]
    Available,
    #[serde(rename = "redeeming")]
    Redeeming,
    #[serde(rename = "redeemed")]
    Redeemed,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RateLimitResetCreditsSummary {
    #[serde(rename = "availableCount", deserialize_with = "required")]
    pub r#available_count: i64,
    #[serde(
        rename = "credits",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#credits: Option<Option<Vec<RateLimitResetCredit>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RateLimitResetType {
    #[serde(rename = "codexRateLimits")]
    CodexRateLimits,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RateLimitSnapshot {
    #[serde(
        rename = "credits",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#credits: Option<Option<CreditsSnapshot>>,
    #[serde(
        rename = "individualLimit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#individual_limit: Option<Option<SpendControlLimitSnapshot>>,
    #[serde(
        rename = "limitId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit_id: Option<Option<String>>,
    #[serde(
        rename = "limitName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit_name: Option<Option<String>>,
    #[serde(
        rename = "normalModelSlug",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#normal_model_slug: Option<Option<String>>,
    #[serde(
        rename = "planType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#plan_type: Option<Option<PlanType>>,
    #[serde(
        rename = "primary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#primary: Option<Option<RateLimitWindow>>,
    #[serde(
        rename = "rateLimitReachedType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#rate_limit_reached_type: Option<Option<RateLimitReachedType>>,
    #[serde(
        rename = "secondary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#secondary: Option<Option<RateLimitWindow>>,
    #[serde(
        rename = "spendControlReached",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#spend_control_reached: Option<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RateLimitWindow {
    #[serde(
        rename = "resetsAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#resets_at: Option<Option<i64>>,
    #[serde(rename = "usedPercent", deserialize_with = "required")]
    pub r#used_percent: i32,
    #[serde(
        rename = "windowDurationMins",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#window_duration_mins: Option<Option<i64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawResponseCompletedNotification {
    #[serde(rename = "responseId", deserialize_with = "required")]
    pub r#response_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
    #[serde(
        rename = "usage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#usage: Option<Option<TokenUsageBreakdown>>,
    #[serde(
        rename = "usageMetadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#usage_metadata: Option<Option<ResponseUsageMetadata>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawResponseItemCompletedNotification {
    #[serde(rename = "item", deserialize_with = "required")]
    pub r#item: ResponseItem,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RealtimeConversationVersion {
    #[serde(rename = "v1")]
    V1,
    #[serde(rename = "v2")]
    V2,
    #[serde(rename = "v3")]
    V3,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RealtimeOutputModality {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "audio")]
    Audio,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RealtimeVoice {
    #[serde(rename = "alloy")]
    Alloy,
    #[serde(rename = "arbor")]
    Arbor,
    #[serde(rename = "ash")]
    Ash,
    #[serde(rename = "aube")]
    Aube,
    #[serde(rename = "ballad")]
    Ballad,
    #[serde(rename = "breeze")]
    Breeze,
    #[serde(rename = "bubbie")]
    Bubbie,
    #[serde(rename = "cedar")]
    Cedar,
    #[serde(rename = "coral")]
    Coral,
    #[serde(rename = "cove")]
    Cove,
    #[serde(rename = "echo")]
    Echo,
    #[serde(rename = "ember")]
    Ember,
    #[serde(rename = "haetsal")]
    Haetsal,
    #[serde(rename = "hanul")]
    Hanul,
    #[serde(rename = "himari")]
    Himari,
    #[serde(rename = "juniper")]
    Juniper,
    #[serde(rename = "leher")]
    Leher,
    #[serde(rename = "maple")]
    Maple,
    #[serde(rename = "marin")]
    Marin,
    #[serde(rename = "miko")]
    Miko,
    #[serde(rename = "neer")]
    Neer,
    #[serde(rename = "porto")]
    Porto,
    #[serde(rename = "rindo")]
    Rindo,
    #[serde(rename = "rio")]
    Rio,
    #[serde(rename = "rivage")]
    Rivage,
    #[serde(rename = "sage")]
    Sage,
    #[serde(rename = "selva")]
    Selva,
    #[serde(rename = "shimmer")]
    Shimmer,
    #[serde(rename = "sol")]
    Sol,
    #[serde(rename = "sonna")]
    Sonna,
    #[serde(rename = "spruce")]
    Spruce,
    #[serde(rename = "tinta")]
    Tinta,
    #[serde(rename = "vale")]
    Vale,
    #[serde(rename = "verse")]
    Verse,
    #[serde(rename = "viola")]
    Viola,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RealtimeVoicesList {
    #[serde(rename = "defaultV1", deserialize_with = "required")]
    pub r#default_v1: RealtimeVoice,
    #[serde(rename = "defaultV2", deserialize_with = "required")]
    pub r#default_v2: RealtimeVoice,
    #[serde(rename = "v1", deserialize_with = "required")]
    pub r#v1: Vec<RealtimeVoice>,
    #[serde(rename = "v2", deserialize_with = "required")]
    pub r#v2: Vec<RealtimeVoice>,
    #[serde(
        rename = "v3",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#v3: Option<Vec<RealtimeVoice>>,
}

pub type ReasoningEffort = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningEffortOption {
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(rename = "reasoningEffort", deserialize_with = "required")]
    pub r#reasoning_effort: ReasoningEffort,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ReasoningItemContent {
    #[serde(rename = "reasoning_text")]
    ReasoningText {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "text")]
    Text {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ReasoningItemReasoningSummary {
    #[serde(rename = "summary_text")]
    SummaryText {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ReasoningSummary {
    Variant1(Box<ReasoningSummaryVariant1>),
    Variant2(Box<ReasoningSummaryVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningSummaryPartAddedNotification {
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "summaryIndex", deserialize_with = "required")]
    pub r#summary_index: i64,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningSummaryTextDeltaNotification {
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "summaryIndex", deserialize_with = "required")]
    pub r#summary_index: i64,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningTextDeltaNotification {
    #[serde(rename = "contentIndex", deserialize_with = "required")]
    pub r#content_index: i64,
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlClient {
    #[serde(
        rename = "appVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#app_version: Option<Option<String>>,
    #[serde(rename = "clientId", deserialize_with = "required")]
    pub r#client_id: String,
    #[serde(
        rename = "deviceModel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#device_model: Option<Option<String>>,
    #[serde(
        rename = "deviceType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#device_type: Option<Option<String>>,
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#display_name: Option<Option<String>>,
    #[serde(
        rename = "lastSeenAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#last_seen_at: Option<Option<i64>>,
    #[serde(
        rename = "osVersion",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#os_version: Option<Option<String>>,
    #[serde(
        rename = "platform",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#platform: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RemoteControlClientsListOrder {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlClientsListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "order",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#order: Option<Option<RemoteControlClientsListOrder>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlClientsListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<RemoteControlClient>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlClientsRevokeParams {
    #[serde(rename = "clientId", deserialize_with = "required")]
    pub r#client_id: String,
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlClientsRevokeResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RemoteControlConnectionStatus {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "errored")]
    Errored,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlDisableParams {
    #[serde(
        rename = "ephemeral",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ephemeral: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlDisableResponse {
    #[serde(
        rename = "environmentId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environment_id: Option<Option<String>>,
    #[serde(rename = "installationId", deserialize_with = "required")]
    pub r#installation_id: String,
    #[serde(rename = "serverName", deserialize_with = "required")]
    pub r#server_name: String,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: RemoteControlConnectionStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlEnableParams {
    #[serde(
        rename = "ephemeral",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ephemeral: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlEnableResponse {
    #[serde(
        rename = "environmentId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environment_id: Option<Option<String>>,
    #[serde(rename = "installationId", deserialize_with = "required")]
    pub r#installation_id: String,
    #[serde(rename = "serverName", deserialize_with = "required")]
    pub r#server_name: String,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: RemoteControlConnectionStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlPairingStartParams {
    #[serde(
        rename = "manualCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#manual_code: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlPairingStartResponse {
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
    #[serde(rename = "expiresAt", deserialize_with = "required")]
    pub r#expires_at: i64,
    #[serde(
        rename = "manualPairingCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#manual_pairing_code: Option<Option<String>>,
    #[serde(rename = "pairingCode", deserialize_with = "required")]
    pub r#pairing_code: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlPairingStatusParams {
    #[serde(
        rename = "manualPairingCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#manual_pairing_code: Option<Option<String>>,
    #[serde(
        rename = "pairingCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#pairing_code: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlPairingStatusResponse {
    #[serde(rename = "claimed", deserialize_with = "required")]
    pub r#claimed: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlStatusChangedNotification {
    #[serde(
        rename = "environmentId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environment_id: Option<Option<String>>,
    #[serde(rename = "installationId", deserialize_with = "required")]
    pub r#installation_id: String,
    #[serde(rename = "serverName", deserialize_with = "required")]
    pub r#server_name: String,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: RemoteControlConnectionStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteControlStatusReadResponse {
    #[serde(
        rename = "environmentId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environment_id: Option<Option<String>>,
    #[serde(rename = "installationId", deserialize_with = "required")]
    pub r#installation_id: String,
    #[serde(rename = "serverName", deserialize_with = "required")]
    pub r#server_name: String,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: RemoteControlConnectionStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RequestHeader {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "value", deserialize_with = "required")]
    pub r#value: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestPermissionProfile {
    #[serde(
        rename = "fileSystem",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#file_system: Option<Option<AdditionalFileSystemPermissions>>,
    #[serde(
        rename = "network",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#network: Option<Option<AdditionalNetworkPermissions>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ResidencyRequirement {
    #[serde(rename = "us")]
    Us,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Resource {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(
        rename = "annotations",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#annotations: Option<studio_json::Value>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "icons",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icons: Option<Option<Vec<studio_json::Value>>>,
    #[serde(
        rename = "mimeType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mime_type: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "size",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#size: Option<Option<i64>>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "uri", deserialize_with = "required")]
    pub r#uri: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ResourceContent {
    Variant1(Box<ResourceContentVariant1>),
    Variant2(Box<ResourceContentVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceTemplate {
    #[serde(
        rename = "annotations",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#annotations: Option<studio_json::Value>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "mimeType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mime_type: Option<Option<String>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
    #[serde(rename = "uriTemplate", deserialize_with = "required")]
    pub r#uri_template: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ResponseItem {
    #[serde(rename = "additional_tools")]
    AdditionalTools {
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(rename = "role", deserialize_with = "required")]
        r#role: String,
        #[serde(rename = "tools", deserialize_with = "required")]
        r#tools: Vec<studio_json::Value>,
    },
    #[serde(rename = "message")]
    Message {
        #[serde(rename = "content", deserialize_with = "required")]
        r#content: Vec<ContentItem>,
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(
            rename = "phase",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#phase: Option<Option<MessagePhase>>,
        #[serde(rename = "role", deserialize_with = "required")]
        r#role: String,
        #[serde(
            rename = "status",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status: Option<Option<String>>,
    },
    #[serde(rename = "agent_message")]
    AgentMessage {
        #[serde(rename = "author", deserialize_with = "required")]
        r#author: String,
        #[serde(rename = "content", deserialize_with = "required")]
        r#content: Vec<AgentMessageInputContent>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(rename = "recipient", deserialize_with = "required")]
        r#recipient: String,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        #[serde(
            rename = "content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#content: Option<Option<Vec<ReasoningItemContent>>>,
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(rename = "summary", deserialize_with = "required")]
        r#summary: Vec<ReasoningItemReasoningSummary>,
    },
    #[serde(rename = "local_shell_call")]
    LocalShellCall {
        #[serde(rename = "action", deserialize_with = "required")]
        r#action: LocalShellAction,
        #[serde(
            rename = "call_id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#call_id: Option<Option<String>>,
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: LocalShellStatus,
    },
    #[serde(rename = "function_call")]
    FunctionCall {
        #[serde(rename = "arguments", deserialize_with = "required")]
        r#arguments: String,
        #[serde(rename = "call_id", deserialize_with = "required")]
        r#call_id: String,
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "encrypted_function_args",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_function_args: Option<Option<Vec<String>>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(
            rename = "namespace",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#namespace: Option<Option<String>>,
        #[serde(
            rename = "status",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status: Option<Option<String>>,
    },
    #[serde(rename = "tool_search_call")]
    ToolSearchCall {
        #[serde(rename = "arguments", deserialize_with = "required")]
        r#arguments: studio_json::Value,
        #[serde(
            rename = "call_id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#call_id: Option<Option<String>>,
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(rename = "execution", deserialize_with = "required")]
        r#execution: String,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(
            rename = "status",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status: Option<Option<String>>,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput {
        #[serde(
            rename = "call_id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#call_id: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(
            rename = "name",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#name: Option<Option<String>>,
        #[serde(
            rename = "namespace",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#namespace: Option<Option<String>>,
        #[serde(rename = "output", deserialize_with = "required")]
        r#output: FunctionCallOutputBody,
    },
    #[serde(rename = "custom_tool_call")]
    CustomToolCall {
        #[serde(rename = "call_id", deserialize_with = "required")]
        r#call_id: String,
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(rename = "input", deserialize_with = "required")]
        r#input: String,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(
            rename = "namespace",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#namespace: Option<Option<String>>,
        #[serde(
            rename = "status",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status: Option<Option<String>>,
    },
    #[serde(rename = "custom_tool_call_output")]
    CustomToolCallOutput {
        #[serde(rename = "call_id", deserialize_with = "required")]
        r#call_id: String,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(
            rename = "name",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#name: Option<Option<String>>,
        #[serde(rename = "output", deserialize_with = "required")]
        r#output: FunctionCallOutputBody,
    },
    #[serde(rename = "tool_search_output")]
    ToolSearchOutput {
        #[serde(
            rename = "call_id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#call_id: Option<Option<String>>,
        #[serde(rename = "execution", deserialize_with = "required")]
        r#execution: String,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: String,
        #[serde(rename = "tools", deserialize_with = "required")]
        r#tools: Vec<studio_json::Value>,
    },
    #[serde(rename = "web_search_call")]
    WebSearchCall {
        #[serde(
            rename = "action",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#action: Option<Option<ResponsesApiWebSearchAction>>,
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(
            rename = "status",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#status: Option<Option<String>>,
    },
    #[serde(rename = "image_generation_call")]
    ImageGenerationCall {
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
        #[serde(rename = "result", deserialize_with = "required")]
        r#result: String,
        #[serde(
            rename = "revised_prompt",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#revised_prompt: Option<Option<String>>,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: String,
    },
    #[serde(rename = "compaction")]
    Compaction {
        #[serde(rename = "encrypted_content", deserialize_with = "required")]
        r#encrypted_content: String,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
    },
    #[serde(rename = "configuration_update")]
    ConfigurationUpdate {
        #[serde(rename = "reasoning", deserialize_with = "required")]
        r#reasoning: ConfigurationReasoning,
    },
    #[serde(rename = "compaction_trigger")]
    CompactionTrigger,
    #[serde(rename = "context_compaction")]
    ContextCompaction {
        #[serde(
            rename = "encrypted_content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#encrypted_content: Option<Option<String>>,
        #[serde(
            rename = "id",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#id: Option<Option<String>>,
        #[serde(
            rename = "internal_chat_message_metadata_passthrough",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#internal_chat_message_metadata_passthrough:
            Option<Option<InternalChatMessageMetadataPassthrough>>,
    },
    #[serde(rename = "other")]
    Other,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResponseUsageMetadata {
    #[serde(
        rename = "amount",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#amount: Option<Option<String>>,
    #[serde(
        rename = "metadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#metadata: Option<studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesApiWebSearchAction {
    #[serde(rename = "search")]
    Search {
        #[serde(
            rename = "queries",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#queries: Option<Option<Vec<String>>>,
        #[serde(
            rename = "query",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#query: Option<Option<String>>,
    },
    #[serde(rename = "open_page")]
    OpenPage {
        #[serde(
            rename = "url",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#url: Option<Option<String>>,
    },
    #[serde(rename = "find_in_page")]
    FindInPage {
        #[serde(
            rename = "pattern",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#pattern: Option<Option<String>>,
        #[serde(
            rename = "url",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#url: Option<Option<String>>,
    },
    #[serde(rename = "other")]
    Other,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReviewDelivery {
    #[serde(rename = "inline")]
    Inline,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewStartParams {
    #[serde(
        rename = "delivery",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#delivery: Option<Option<ReviewDelivery>>,
    #[serde(rename = "target", deserialize_with = "required")]
    pub r#target: ReviewTarget,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewStartResponse {
    #[serde(rename = "reviewThreadId", deserialize_with = "required")]
    pub r#review_thread_id: String,
    #[serde(rename = "turn", deserialize_with = "required")]
    pub r#turn: Turn,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ReviewTarget {
    #[serde(rename = "uncommittedChanges")]
    UncommittedChanges,
    #[serde(rename = "baseBranch")]
    BaseBranch {
        #[serde(rename = "branch", deserialize_with = "required")]
        r#branch: String,
    },
    #[serde(rename = "commit")]
    Commit {
        #[serde(rename = "sha", deserialize_with = "required")]
        r#sha: String,
        #[serde(
            rename = "title",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#title: Option<Option<String>>,
    },
    #[serde(rename = "custom")]
    Custom {
        #[serde(rename = "instructions", deserialize_with = "required")]
        r#instructions: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RolloutCompressResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum SandboxPolicy {
    #[serde(rename = "dangerFullAccess")]
    DangerFullAccess,
    #[serde(rename = "readOnly")]
    ReadOnly {
        #[serde(
            rename = "networkAccess",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#network_access: Option<bool>,
    },
    #[serde(rename = "externalSandbox")]
    ExternalSandbox {
        #[serde(
            rename = "networkAccess",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#network_access: Option<NetworkAccess>,
    },
    #[serde(rename = "workspaceWrite")]
    WorkspaceWrite {
        #[serde(
            rename = "excludeSlashTmp",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#exclude_slash_tmp: Option<bool>,
        #[serde(
            rename = "excludeTmpdirEnvVar",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#exclude_tmpdir_env_var: Option<bool>,
        #[serde(
            rename = "networkAccess",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#network_access: Option<bool>,
        #[serde(
            rename = "writableRoots",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#writable_roots: Option<Vec<AbsolutePathBuf>>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SandboxWorkspaceWrite {
    #[serde(
        rename = "exclude_slash_tmp",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exclude_slash_tmp: Option<bool>,
    #[serde(
        rename = "exclude_tmpdir_env_var",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exclude_tmpdir_env_var: Option<bool>,
    #[serde(
        rename = "network_access",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#network_access: Option<bool>,
    #[serde(
        rename = "writable_roots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#writable_roots: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ScheduledTaskSchedule {
    #[serde(rename = "hourly")]
    Hourly {
        #[serde(
            rename = "days",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#days: Option<Option<Vec<ScheduledTaskWeekday>>>,
        #[serde(rename = "intervalHours", deserialize_with = "required")]
        r#interval_hours: u32,
    },
    #[serde(rename = "daily")]
    Daily {
        #[serde(rename = "time", deserialize_with = "required")]
        r#time: String,
    },
    #[serde(rename = "weekdays")]
    Weekdays {
        #[serde(rename = "time", deserialize_with = "required")]
        r#time: String,
    },
    #[serde(rename = "weekly")]
    Weekly {
        #[serde(rename = "days", deserialize_with = "required")]
        r#days: Vec<ScheduledTaskWeekday>,
        #[serde(rename = "time", deserialize_with = "required")]
        r#time: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScheduledTaskSummary {
    #[serde(rename = "key", deserialize_with = "required")]
    pub r#key: String,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "prompt", deserialize_with = "required")]
    pub r#prompt: String,
    #[serde(rename = "schedule", deserialize_with = "required")]
    pub r#schedule: ScheduledTaskSchedule,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ScheduledTaskWeekday {
    #[serde(rename = "MO")]
    MO,
    #[serde(rename = "TU")]
    TU,
    #[serde(rename = "WE")]
    WE,
    #[serde(rename = "TH")]
    TH,
    #[serde(rename = "FR")]
    FR,
    #[serde(rename = "SA")]
    SA,
    #[serde(rename = "SU")]
    SU,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SelectedCapabilityRoot {
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "location", deserialize_with = "required")]
    pub r#location: CapabilityRootLocation,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SendAddCreditsNudgeEmailParams {
    #[serde(rename = "creditType", deserialize_with = "required")]
    pub r#credit_type: AddCreditsNudgeCreditType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SendAddCreditsNudgeEmailResponse {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: AddCreditsNudgeEmailStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServerDiagnosticsGauge {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "value", deserialize_with = "required")]
    pub r#value: u64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServerDiagnosticsParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServerDiagnosticsProcess {
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: u32,
    #[serde(
        rename = "physicalFootprintBytes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#physical_footprint_bytes: Option<Option<u64>>,
    #[serde(
        rename = "residentMemoryBytes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#resident_memory_bytes: Option<Option<u64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServerDiagnosticsResponse {
    #[serde(rename = "gauges", deserialize_with = "required")]
    pub r#gauges: Vec<ServerDiagnosticsGauge>,
    #[serde(rename = "process", deserialize_with = "required")]
    pub r#process: ServerDiagnosticsProcess,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServerRequestResolvedNotification {
    #[serde(rename = "requestId", deserialize_with = "required")]
    pub r#request_id: RequestId,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionMigration {
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: String,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: String,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SessionSource {
    Variant1(Box<SessionSourceVariant1>),
    Variant2(Box<SessionSourceVariant2>),
    Variant3(Box<SessionSourceVariant3>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(
        rename = "developer_instructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer_instructions: Option<Option<String>>,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(
        rename = "reasoning_effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reasoning_effort: Option<Option<ReasoningEffort>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillDependencies {
    #[serde(rename = "tools", deserialize_with = "required")]
    pub r#tools: Vec<SkillToolDependency>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillErrorInfo {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillInterface {
    #[serde(
        rename = "brandColor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#brand_color: Option<Option<String>>,
    #[serde(
        rename = "defaultPrompt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#default_prompt: Option<Option<String>>,
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#display_name: Option<Option<String>>,
    #[serde(
        rename = "iconLarge",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_large: Option<Option<AbsolutePathBuf>>,
    #[serde(
        rename = "iconLargeUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_large_url: Option<Option<String>>,
    #[serde(
        rename = "iconSmall",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_small: Option<Option<AbsolutePathBuf>>,
    #[serde(
        rename = "iconSmallUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon_small_url: Option<Option<String>>,
    #[serde(
        rename = "shortDescription",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#short_description: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillMetadata {
    #[serde(
        rename = "dependencies",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#dependencies: Option<Option<SkillDependencies>>,
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(rename = "enabled", deserialize_with = "required")]
    pub r#enabled: bool,
    #[serde(
        rename = "interface",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#interface: Option<Option<SkillInterface>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "path", deserialize_with = "required")]
    pub r#path: LegacyAppPathString,
    #[serde(
        rename = "pluginId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#plugin_id: Option<Option<String>>,
    #[serde(rename = "scope", deserialize_with = "required")]
    pub r#scope: SkillScope,
    #[serde(
        rename = "shortDescription",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#short_description: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillMigration {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SkillScope {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "repo")]
    Repo,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "admin")]
    Admin,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillSummary {
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(rename = "enabled", deserialize_with = "required")]
    pub r#enabled: bool,
    #[serde(
        rename = "interface",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#interface: Option<Option<SkillInterface>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#path: Option<Option<LegacyAppPathString>>,
    #[serde(
        rename = "shortDescription",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#short_description: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillToolDependency {
    #[serde(
        rename = "command",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#command: Option<Option<String>>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "transport",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#transport: Option<Option<String>>,
    #[serde(rename = "type", deserialize_with = "required")]
    pub r#type: String,
    #[serde(
        rename = "url",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#url: Option<Option<String>>,
    #[serde(rename = "value", deserialize_with = "required")]
    pub r#value: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsChangedNotification {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsConfigWriteParams {
    #[serde(rename = "enabled", deserialize_with = "required")]
    pub r#enabled: bool,
    #[serde(
        rename = "name",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#name: Option<Option<String>>,
    #[serde(
        rename = "path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#path: Option<Option<AbsolutePathBuf>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsConfigWriteResponse {
    #[serde(rename = "effectiveEnabled", deserialize_with = "required")]
    pub r#effective_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsExtraRootsSetParams {
    #[serde(rename = "extraRoots", deserialize_with = "required")]
    pub r#extra_roots: Vec<AbsolutePathBuf>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsExtraRootsSetResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsListEntry {
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: String,
    #[serde(rename = "errors", deserialize_with = "required")]
    pub r#errors: Vec<SkillErrorInfo>,
    #[serde(rename = "skills", deserialize_with = "required")]
    pub r#skills: Vec<SkillMetadata>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsListParams {
    #[serde(
        rename = "cwds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwds: Option<Vec<String>>,
    #[serde(
        rename = "forceReload",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#force_reload: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillsListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<SkillsListEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SortDirection {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpendControlLimitSnapshot {
    #[serde(rename = "limit", deserialize_with = "required")]
    pub r#limit: String,
    #[serde(rename = "remainingPercent", deserialize_with = "required")]
    pub r#remaining_percent: i32,
    #[serde(rename = "resetsAt", deserialize_with = "required")]
    pub r#resets_at: i64,
    #[serde(rename = "used", deserialize_with = "required")]
    pub r#used: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StrictReviewRequiredNotification {
    #[serde(rename = "startedAtMs", deserialize_with = "required")]
    pub r#started_at_ms: i64,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SubAgentActivityKind {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "interacted")]
    Interacted,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "completed")]
    Completed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SubAgentSource {
    Variant1(Box<SubAgentSourceVariant1>),
    Variant2(Box<SubAgentSourceVariant2>),
    Variant3(Box<SubAgentSourceVariant3>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubagentMigration {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TerminalInteractionNotification {
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "processId", deserialize_with = "required")]
    pub r#process_id: String,
    #[serde(rename = "stdin", deserialize_with = "required")]
    pub r#stdin: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextElement {
    #[serde(rename = "byteRange", deserialize_with = "required")]
    pub r#byte_range: ByteRange,
    #[serde(
        rename = "placeholder",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#placeholder: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextPosition {
    #[serde(rename = "column", deserialize_with = "required")]
    pub r#column: u64,
    #[serde(rename = "line", deserialize_with = "required")]
    pub r#line: u64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextRange {
    #[serde(rename = "end", deserialize_with = "required")]
    pub r#end: TextPosition,
    #[serde(rename = "start", deserialize_with = "required")]
    pub r#start: TextPosition,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Thread {
    #[serde(
        rename = "agentNickname",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#agent_nickname: Option<Option<String>>,
    #[serde(
        rename = "agentRole",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#agent_role: Option<Option<String>>,
    #[serde(
        rename = "canAcceptDirectInput",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#can_accept_direct_input: Option<Option<bool>>,
    #[serde(rename = "cliVersion", deserialize_with = "required")]
    pub r#cli_version: String,
    #[serde(rename = "createdAt", deserialize_with = "required")]
    pub r#created_at: i64,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: AbsolutePathBuf,
    #[serde(
        rename = "daybreakEnabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#daybreak_enabled: Option<Option<bool>>,
    #[serde(
        rename = "environments",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environments: Option<Option<Vec<ThreadEnvironment>>>,
    #[serde(rename = "ephemeral", deserialize_with = "required")]
    pub r#ephemeral: bool,
    #[serde(
        rename = "extra",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#extra: Option<Option<ThreadExtra>>,
    #[serde(
        rename = "forkedFromId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#forked_from_id: Option<Option<String>>,
    #[serde(
        rename = "gitInfo",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#git_info: Option<Option<GitInfo>>,
    #[serde(
        rename = "historyMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#history_mode: Option<ThreadHistoryMode>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(rename = "modelProvider", deserialize_with = "required")]
    pub r#model_provider: String,
    #[serde(
        rename = "name",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#name: Option<Option<String>>,
    #[serde(
        rename = "originator",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#originator: Option<Option<String>>,
    #[serde(
        rename = "parentThreadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#parent_thread_id: Option<Option<String>>,
    #[serde(
        rename = "path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#path: Option<Option<String>>,
    #[serde(rename = "preview", deserialize_with = "required")]
    pub r#preview: String,
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: Option<String>,
    #[serde(
        rename = "reasoningEffort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reasoning_effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "recencyAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#recency_at: Option<Option<i64>>,
    #[serde(
        rename = "section",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#section: Option<Option<ThreadSection>>,
    #[serde(
        rename = "sectionEnteredAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#section_entered_at: Option<Option<i64>>,
    #[serde(rename = "sessionId", deserialize_with = "required")]
    pub r#session_id: String,
    #[serde(rename = "source", deserialize_with = "required")]
    pub r#source: SessionSource,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: ThreadStatus,
    #[serde(
        rename = "threadSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_source: Option<Option<ThreadSource>>,
    #[serde(rename = "turns", deserialize_with = "required")]
    pub r#turns: Vec<Turn>,
    #[serde(rename = "updatedAt", deserialize_with = "required")]
    pub r#updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadActiveFlag {
    #[serde(rename = "waitingOnApproval")]
    WaitingOnApproval,
    #[serde(rename = "waitingOnUserInput")]
    WaitingOnUserInput,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadApproveGuardianDeniedActionParams {
    #[serde(rename = "event", deserialize_with = "required")]
    pub r#event: studio_json::Value,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadApproveGuardianDeniedActionResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadArchiveParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadArchiveResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadArchivedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachment {
    #[serde(rename = "attachmentType", deserialize_with = "required")]
    pub r#attachment_type: String,
    #[serde(rename = "createdAt", deserialize_with = "required")]
    pub r#created_at: i64,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "identityKey", deserialize_with = "required")]
    pub r#identity_key: String,
    #[serde(rename = "payload", deserialize_with = "required")]
    pub r#payload: studio_json::Value,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadAttachmentAddOutcome {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "existing")]
    Existing,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentAddParams {
    #[serde(rename = "attachmentType", deserialize_with = "required")]
    pub r#attachment_type: String,
    #[serde(rename = "identityKey", deserialize_with = "required")]
    pub r#identity_key: String,
    #[serde(rename = "payload", deserialize_with = "required")]
    pub r#payload: studio_json::Value,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentAddResponse {
    #[serde(rename = "attachment", deserialize_with = "required")]
    pub r#attachment: ThreadAttachment,
    #[serde(rename = "outcome", deserialize_with = "required")]
    pub r#outcome: ThreadAttachmentAddOutcome,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadAttachment>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadAttachmentOperation {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "deleted")]
    Deleted,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentOwner {
    #[serde(rename = "archived", deserialize_with = "required")]
    pub r#archived: bool,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentOwnerListParams {
    #[serde(
        rename = "archived",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#archived: Option<Option<bool>>,
    #[serde(rename = "attachmentType", deserialize_with = "required")]
    pub r#attachment_type: String,
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(rename = "identityKey", deserialize_with = "required")]
    pub r#identity_key: String,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentOwnerListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadAttachmentOwner>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentRemoveParams {
    #[serde(rename = "attachmentType", deserialize_with = "required")]
    pub r#attachment_type: String,
    #[serde(rename = "identityKey", deserialize_with = "required")]
    pub r#identity_key: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentRemoveResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadAttachmentUpdatedNotification {
    #[serde(rename = "attachmentId", deserialize_with = "required")]
    pub r#attachment_id: String,
    #[serde(rename = "attachmentType", deserialize_with = "required")]
    pub r#attachment_type: String,
    #[serde(rename = "identityKey", deserialize_with = "required")]
    pub r#identity_key: String,
    #[serde(rename = "operation", deserialize_with = "required")]
    pub r#operation: ThreadAttachmentOperation,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadBackgroundTerminal {
    #[serde(rename = "command", deserialize_with = "required")]
    pub r#command: String,
    #[serde(
        rename = "cpuPercent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cpu_percent: Option<Option<f64>>,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: LegacyAppPathString,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(
        rename = "osPid",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#os_pid: Option<Option<u32>>,
    #[serde(rename = "processId", deserialize_with = "required")]
    pub r#process_id: String,
    #[serde(
        rename = "rssKb",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#rss_kb: Option<Option<u64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadBackgroundTerminalsCleanParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadBackgroundTerminalsCleanResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadBackgroundTerminalsListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadBackgroundTerminalsListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadBackgroundTerminal>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadBackgroundTerminalsTerminateParams {
    #[serde(rename = "processId", deserialize_with = "required")]
    pub r#process_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadBackgroundTerminalsTerminateResponse {
    #[serde(rename = "terminated", deserialize_with = "required")]
    pub r#terminated: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadClosedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadCompactStartParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadCompactStartResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadDecrementElicitationParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadDecrementElicitationResponse {
    #[serde(rename = "count", deserialize_with = "required")]
    pub r#count: i64,
    #[serde(rename = "paused", deserialize_with = "required")]
    pub r#paused: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadDeleteParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadDeleteResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadDeletedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadEnvironment {
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: LegacyAppPathString,
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
    #[serde(rename = "runtimeWorkspaceRoots", deserialize_with = "required")]
    pub r#runtime_workspace_roots: Vec<LegacyAppPathString>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadExtra {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadForkParams {
    #[serde(
        rename = "approvalPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_policy: Option<Option<AskForApproval>>,
    #[serde(
        rename = "approvalsReviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "attachToSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#attach_to_source: Option<bool>,
    #[serde(
        rename = "baseInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#base_instructions: Option<Option<String>>,
    #[serde(
        rename = "beforeTurnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#before_turn_id: Option<Option<String>>,
    #[serde(
        rename = "config",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#config: Option<Option<std::collections::BTreeMap<String, studio_json::Value>>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "deferGoalContinuation",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#defer_goal_continuation: Option<bool>,
    #[serde(
        rename = "developerInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer_instructions: Option<Option<String>>,
    #[serde(
        rename = "ephemeral",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ephemeral: Option<bool>,
    #[serde(
        rename = "excludeTurns",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exclude_turns: Option<bool>,
    #[serde(
        rename = "experimentalDisablePredictionMcp",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#experimental_disable_prediction_mcp: Option<bool>,
    #[serde(
        rename = "experimentalPredictionMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#experimental_prediction_mode: Option<bool>,
    #[serde(
        rename = "lastTurnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#last_turn_id: Option<Option<String>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "modelProvider",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_provider: Option<Option<String>>,
    #[serde(
        rename = "path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#path: Option<Option<String>>,
    #[serde(
        rename = "permissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#permissions: Option<Option<String>>,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Option<Vec<AbsolutePathBuf>>>,
    #[serde(
        rename = "sandbox",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox: Option<Option<SandboxMode>>,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "threadSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_source: Option<Option<ThreadSource>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadForkResponse {
    #[serde(
        rename = "activePermissionProfile",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#active_permission_profile: Option<Option<ActivePermissionProfile>>,
    #[serde(rename = "approvalPolicy", deserialize_with = "required")]
    pub r#approval_policy: AskForApproval,
    #[serde(rename = "approvalsReviewer", deserialize_with = "required")]
    pub r#approvals_reviewer: ApprovalsReviewer,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: AbsolutePathBuf,
    #[serde(
        rename = "disabledPluginIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_plugin_ids: Option<Vec<String>>,
    #[serde(
        rename = "instructionSources",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#instruction_sources: Option<Vec<LegacyAppPathString>>,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(rename = "modelProvider", deserialize_with = "required")]
    pub r#model_provider: String,
    #[serde(
        rename = "multiAgentMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_mode: Option<MultiAgentMode>,
    #[serde(
        rename = "reasoningEffort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reasoning_effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Vec<AbsolutePathBuf>>,
    #[serde(rename = "sandbox", deserialize_with = "required")]
    pub r#sandbox: SandboxPolicy,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoal {
    #[serde(rename = "createdAt", deserialize_with = "required")]
    pub r#created_at: i64,
    #[serde(rename = "objective", deserialize_with = "required")]
    pub r#objective: String,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: ThreadGoalStatus,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "timeUsedSeconds", deserialize_with = "required")]
    pub r#time_used_seconds: i64,
    #[serde(
        rename = "tokenBudget",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#token_budget: Option<Option<i64>>,
    #[serde(rename = "tokensUsed", deserialize_with = "required")]
    pub r#tokens_used: i64,
    #[serde(rename = "updatedAt", deserialize_with = "required")]
    pub r#updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalClearParams {
    #[serde(
        rename = "origin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origin: Option<Option<ThreadGoalMutationOrigin>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalClearResponse {
    #[serde(rename = "cleared", deserialize_with = "required")]
    pub r#cleared: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalClearedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalGetParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalGetResponse {
    #[serde(
        rename = "goal",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#goal: Option<Option<ThreadGoal>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadGoalMutationOrigin {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "automatic")]
    Automatic,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalSetParams {
    #[serde(
        rename = "objective",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#objective: Option<Option<String>>,
    #[serde(
        rename = "origin",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origin: Option<Option<ThreadGoalMutationOrigin>>,
    #[serde(
        rename = "status",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#status: Option<Option<ThreadGoalStatus>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "tokenBudget",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#token_budget: Option<Option<i64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalSetResponse {
    #[serde(rename = "goal", deserialize_with = "required")]
    pub r#goal: ThreadGoal,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadGoalStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "blocked")]
    Blocked,
    #[serde(rename = "usageLimited")]
    UsageLimited,
    #[serde(rename = "budgetLimited")]
    BudgetLimited,
    #[serde(rename = "complete")]
    Complete,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGoalUpdatedNotification {
    #[serde(rename = "goal", deserialize_with = "required")]
    pub r#goal: ThreadGoal,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "turnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turn_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGuardianAssessment {
    #[serde(rename = "outcome", deserialize_with = "required")]
    pub r#outcome: GuardianAssessmentOutcome,
    #[serde(rename = "rationale", deserialize_with = "required")]
    pub r#rationale: String,
    #[serde(rename = "riskLevel", deserialize_with = "required")]
    pub r#risk_level: GuardianRiskLevel,
    #[serde(rename = "userAuthorization", deserialize_with = "required")]
    pub r#user_authorization: GuardianUserAuthorization,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGuardianReviewParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadGuardianReviewResponse {
    #[serde(
        rename = "assessment",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#assessment: Option<Option<ThreadGuardianAssessment>>,
    #[serde(rename = "callId", deserialize_with = "required")]
    pub r#call_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadHistoryMode {
    #[serde(rename = "legacy")]
    Legacy,
    #[serde(rename = "paginated")]
    Paginated,
}

pub type ThreadId = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadIncrementElicitationParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadIncrementElicitationResponse {
    #[serde(rename = "count", deserialize_with = "required")]
    pub r#count: i64,
    #[serde(rename = "paused", deserialize_with = "required")]
    pub r#paused: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadInjectItemsParams {
    #[serde(rename = "items", deserialize_with = "required")]
    pub r#items: Vec<studio_json::Value>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadInjectItemsResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadItem {
    #[serde(rename = "userMessage")]
    UserMessage {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "clientId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#client_id: Option<Option<String>>,
        #[serde(rename = "content", deserialize_with = "required")]
        r#content: Vec<UserInput>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
    },
    #[serde(rename = "hookPrompt")]
    HookPrompt {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "fragments", deserialize_with = "required")]
        r#fragments: Vec<HookPromptFragment>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
    },
    #[serde(rename = "agentMessage")]
    AgentMessage {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "delivery",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#delivery: Option<Option<AgentMessageDelivery>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(
            rename = "memoryCitation",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#memory_citation: Option<Option<MemoryCitation>>,
        #[serde(
            rename = "phase",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#phase: Option<Option<MessagePhase>>,
        #[serde(
            rename = "questions",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#questions: Option<Option<Vec<AsyncUserInputQuestion>>>,
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "functionCallOutput")]
    FunctionCallOutput {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(
            rename = "namespace",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#namespace: Option<Option<String>>,
        #[serde(rename = "output", deserialize_with = "required")]
        r#output: FunctionCallOutputBody,
    },
    #[serde(rename = "plan")]
    Plan {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "content",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#content: Option<Vec<String>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(
            rename = "summary",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#summary: Option<Vec<String>>,
    },
    #[serde(rename = "commandExecution")]
    CommandExecution {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "aggregatedOutput",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#aggregated_output: Option<Option<String>>,
        #[serde(rename = "command", deserialize_with = "required")]
        r#command: String,
        #[serde(rename = "commandActions", deserialize_with = "required")]
        r#command_actions: Vec<CommandAction>,
        #[serde(rename = "cwd", deserialize_with = "required")]
        r#cwd: LegacyAppPathString,
        #[serde(
            rename = "durationMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#duration_ms: Option<Option<i64>>,
        #[serde(
            rename = "exitCode",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#exit_code: Option<Option<i32>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(
            rename = "pluginId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#plugin_id: Option<Option<String>>,
        #[serde(
            rename = "processId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#process_id: Option<Option<String>>,
        #[serde(
            rename = "scriptPath",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#script_path: Option<Option<String>>,
        #[serde(
            rename = "source",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#source: Option<CommandExecutionSource>,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: CommandExecutionStatus,
    },
    #[serde(rename = "fileChange")]
    FileChange {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "changes", deserialize_with = "required")]
        r#changes: Vec<FileUpdateChange>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: PatchApplyStatus,
    },
    #[serde(rename = "mcpToolCall")]
    McpToolCall {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "appContext",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#app_context: Option<Option<McpToolCallAppContext>>,
        #[serde(rename = "arguments", deserialize_with = "required")]
        r#arguments: studio_json::Value,
        #[serde(
            rename = "durationMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#duration_ms: Option<Option<i64>>,
        #[serde(
            rename = "error",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#error: Option<Option<McpToolCallError>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(
            rename = "mcpAppResourceUri",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#mcp_app_resource_uri: Option<Option<String>>,
        #[serde(
            rename = "mcpAppUi",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#mcp_app_ui: Option<Option<McpAppUi>>,
        #[serde(
            rename = "pluginId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#plugin_id: Option<Option<String>>,
        #[serde(
            rename = "readOnlyHint",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#read_only_hint: Option<Option<bool>>,
        #[serde(
            rename = "result",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#result: Option<Option<McpToolCallResult>>,
        #[serde(rename = "server", deserialize_with = "required")]
        r#server: String,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: McpToolCallStatus,
        #[serde(rename = "tool", deserialize_with = "required")]
        r#tool: String,
    },
    #[serde(rename = "dynamicToolCall")]
    DynamicToolCall {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "arguments", deserialize_with = "required")]
        r#arguments: studio_json::Value,
        #[serde(
            rename = "contentItems",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#content_items: Option<Option<Vec<DynamicToolCallOutputContentItem>>>,
        #[serde(
            rename = "durationMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#duration_ms: Option<Option<i64>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(
            rename = "namespace",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#namespace: Option<Option<String>>,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: DynamicToolCallStatus,
        #[serde(
            rename = "success",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#success: Option<Option<bool>>,
        #[serde(rename = "tool", deserialize_with = "required")]
        r#tool: String,
    },
    #[serde(rename = "collabAgentToolCall")]
    CollabAgentToolCall {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "agentsStates", deserialize_with = "required")]
        r#agents_states: std::collections::BTreeMap<String, CollabAgentState>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(
            rename = "model",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#model: Option<Option<String>>,
        #[serde(
            rename = "prompt",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#prompt: Option<Option<String>>,
        #[serde(
            rename = "reasoningEffort",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#reasoning_effort: Option<Option<ReasoningEffort>>,
        #[serde(rename = "receiverThreadIds", deserialize_with = "required")]
        r#receiver_thread_ids: Vec<String>,
        #[serde(rename = "senderThreadId", deserialize_with = "required")]
        r#sender_thread_id: String,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: CollabAgentToolCallStatus,
        #[serde(rename = "tool", deserialize_with = "required")]
        r#tool: CollabAgentTool,
    },
    #[serde(rename = "subAgentActivity")]
    SubAgentActivity {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "agentPath", deserialize_with = "required")]
        r#agent_path: String,
        #[serde(rename = "agentThreadId", deserialize_with = "required")]
        r#agent_thread_id: String,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "kind", deserialize_with = "required")]
        r#kind: SubAgentActivityKind,
        #[serde(
            rename = "model",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#model: Option<Option<String>>,
        #[serde(
            rename = "reasoningEffort",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#reasoning_effort: Option<Option<ReasoningEffort>>,
    },
    #[serde(rename = "webSearch")]
    WebSearch {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "action",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#action: Option<Option<WebSearchAction>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "query", deserialize_with = "required")]
        r#query: String,
        #[serde(
            rename = "results",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#results: Option<Option<Vec<studio_json::Value>>>,
    },
    #[serde(rename = "imageView")]
    ImageView {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: LegacyAppPathString,
    },
    #[serde(rename = "sleep")]
    Sleep {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "durationMs", deserialize_with = "required")]
        r#duration_ms: u64,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
    },
    #[serde(rename = "imageGeneration")]
    ImageGeneration {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "failure",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#failure: Option<Option<ImageGenerationFailure>>,
        #[serde(
            rename = "generationId",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#generation_id: Option<Option<String>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(
            rename = "inputImages",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#input_images: Option<Vec<ImageGenerationInputImage>>,
        #[serde(
            rename = "revisedPrompt",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#revised_prompt: Option<Option<String>>,
        #[serde(
            rename = "savedPath",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#saved_path: Option<Option<AbsolutePathBuf>>,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: String,
        #[serde(
            rename = "transparentBackground",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#transparent_background: Option<Option<bool>>,
    },
    #[serde(rename = "enteredReviewMode")]
    EnteredReviewMode {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "review", deserialize_with = "required")]
        r#review: String,
    },
    #[serde(rename = "exitedReviewMode")]
    ExitedReviewMode {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "review", deserialize_with = "required")]
        r#review: String,
    },
    #[serde(rename = "contextCompaction")]
    ContextCompaction {
        #[serde(
            rename = "completedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at_ms: Option<Option<i64>>,
        #[serde(
            rename = "startedAtMs",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at_ms: Option<Option<i64>>,
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadItemEntry {
    #[serde(
        rename = "completedAtMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#completed_at_ms: Option<Option<i64>>,
    #[serde(rename = "item", deserialize_with = "required")]
    pub r#item: ThreadItem,
    #[serde(
        rename = "startedAtMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#started_at_ms: Option<Option<i64>>,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadItemsListAnchor {
    #[serde(rename = "item")]
    Item {
        #[serde(rename = "itemId", deserialize_with = "required")]
        r#item_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ThreadItemsListCursor {
    Variant1(Box<String>),
    Variant2(Box<ThreadItemsListAnchor>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadItemsListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<ThreadItemsListCursor>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "sortDirection",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_direction: Option<Option<SortDirection>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "turnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turn_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadItemsListResponse {
    #[serde(
        rename = "backwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#backwards_cursor: Option<Option<String>>,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadItemEntry>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadItemsReadParams {
    #[serde(rename = "itemIds", deserialize_with = "required")]
    pub r#item_ids: Vec<String>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadItemsReadResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadItemEntry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ThreadListCwdFilter {
    Variant1(Box<String>),
    Variant2(Box<Vec<String>>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadListParams {
    #[serde(
        rename = "ancestorThreadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ancestor_thread_id: Option<Option<String>>,
    #[serde(
        rename = "archived",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#archived: Option<Option<bool>>,
    #[serde(
        rename = "attachedToThreadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#attached_to_thread_id: Option<Option<String>>,
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<ThreadListCwdFilter>>,
    #[serde(
        rename = "excludedThreadIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#excluded_thread_ids: Option<Option<Vec<String>>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "modelProviders",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_providers: Option<Option<Vec<String>>>,
    #[serde(
        rename = "originators",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#originators: Option<Option<Vec<String>>>,
    #[serde(
        rename = "parentThreadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#parent_thread_id: Option<Option<String>>,
    #[serde(
        rename = "projectId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#project_id: Option<Option<String>>,
    #[serde(
        rename = "searchTerm",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#search_term: Option<Option<String>>,
    #[serde(
        rename = "sectionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#section_id: Option<Option<String>>,
    #[serde(
        rename = "sortDirection",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_direction: Option<Option<SortDirection>>,
    #[serde(
        rename = "sortKey",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_key: Option<Option<ThreadSortKey>>,
    #[serde(
        rename = "sourceKinds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source_kinds: Option<Option<Vec<ThreadSourceKind>>>,
    #[serde(
        rename = "useStateDbOnly",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#use_state_db_only: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadListResponse {
    #[serde(
        rename = "backwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#backwards_cursor: Option<Option<String>>,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<Thread>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
    #[serde(
        rename = "readStates",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#read_states: Option<Option<std::collections::BTreeMap<String, ThreadReadState>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadLoadedListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadLoadedListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<String>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadMemoryMode {
    #[serde(rename = "enabled")]
    Enabled,
    #[serde(rename = "disabled")]
    Disabled,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMemoryModeSetParams {
    #[serde(rename = "mode", deserialize_with = "required")]
    pub r#mode: ThreadMemoryMode,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMemoryModeSetResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMemoryReadParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMemoryReadResponse {
    #[serde(
        rename = "summary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#summary: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMetadataGitInfoUpdateParams {
    #[serde(
        rename = "branch",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#branch: Option<Option<String>>,
    #[serde(
        rename = "originUrl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#origin_url: Option<Option<String>>,
    #[serde(
        rename = "sha",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sha: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMetadataUpdateParams {
    #[serde(
        rename = "daybreakEnabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#daybreak_enabled: Option<Option<bool>>,
    #[serde(
        rename = "gitInfo",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#git_info: Option<Option<ThreadMetadataGitInfoUpdateParams>>,
    #[serde(
        rename = "projectId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#project_id: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMetadataUpdateResponse {
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadNameUpdatedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "threadName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_name: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadPredictionRequestParams {
    #[serde(rename = "sourceTurnId", deserialize_with = "required")]
    pub r#source_turn_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadPredictionRequestResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadPredictionResult {
    #[serde(rename = "completed")]
    Completed {
        #[serde(
            rename = "text",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#text: Option<Option<String>>,
    },
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadPredictionUpdatedNotification {
    #[serde(rename = "result", deserialize_with = "required")]
    pub r#result: ThreadPredictionResult,
    #[serde(rename = "sourceTurnId", deserialize_with = "required")]
    pub r#source_turn_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadProjectUpdatedNotification {
    #[serde(rename = "projectId", deserialize_with = "required")]
    pub r#project_id: Option<String>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueAddParams {
    #[serde(rename = "clientUserMessageId", deserialize_with = "required")]
    pub r#client_user_message_id: String,
    #[serde(rename = "input", deserialize_with = "required")]
    pub r#input: Vec<UserInput>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueAddResponse {
    #[serde(rename = "queuedSubmission", deserialize_with = "required")]
    pub r#queued_submission: QueuedSubmission,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueChangedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueDeleteParams {
    #[serde(rename = "queuedSubmissionId", deserialize_with = "required")]
    pub r#queued_submission_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueDeleteResponse {
    #[serde(rename = "deleted", deserialize_with = "required")]
    pub r#deleted: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<QueuedSubmission>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueReorderParams {
    #[serde(rename = "queuedSubmissionIds", deserialize_with = "required")]
    pub r#queued_submission_ids: Vec<String>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueReorderResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueStartParams {
    #[serde(
        rename = "queuedSubmissionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#queued_submission_id: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueStartResponse {
    #[serde(rename = "turn", deserialize_with = "required")]
    pub r#turn: Turn,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueUpdateParams {
    #[serde(rename = "input", deserialize_with = "required")]
    pub r#input: Vec<UserInput>,
    #[serde(rename = "queuedSubmissionId", deserialize_with = "required")]
    pub r#queued_submission_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadQueueUpdateResponse {
    #[serde(rename = "queuedSubmission", deserialize_with = "required")]
    pub r#queued_submission: QueuedSubmission,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadReadParams {
    #[serde(
        rename = "includeTurns",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_turns: Option<bool>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadReadResponse {
    #[serde(
        rename = "readState",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#read_state: Option<Option<ThreadReadState>>,
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadReadState {
    #[serde(
        rename = "firstUnread",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#first_unread: Option<Option<ThreadUnreadPosition>>,
    #[serde(rename = "revision", deserialize_with = "required")]
    pub r#revision: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadReadStateChangedNotification {
    #[serde(rename = "readState", deserialize_with = "required")]
    pub r#read_state: ThreadReadState,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadReadStateOperation {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "unread")]
    Unread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadReadStateUpdateParams {
    #[serde(rename = "expectedRevision", deserialize_with = "required")]
    pub r#expected_revision: String,
    #[serde(rename = "operation", deserialize_with = "required")]
    pub r#operation: ThreadReadStateOperation,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadReadStateUpdateResponse {
    #[serde(rename = "readState", deserialize_with = "required")]
    pub r#read_state: ThreadReadState,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeAppendAudioParams {
    #[serde(rename = "audio", deserialize_with = "required")]
    pub r#audio: ThreadRealtimeAudioChunk,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeAppendAudioResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeAppendSpeechParams {
    #[serde(rename = "text", deserialize_with = "required")]
    pub r#text: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeAppendSpeechResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeAppendTextParams {
    #[serde(
        rename = "role",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#role: Option<ConversationTextRole>,
    #[serde(rename = "text", deserialize_with = "required")]
    pub r#text: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeAppendTextResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeAudioChunk {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: String,
    #[serde(
        rename = "itemId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#item_id: Option<Option<String>>,
    #[serde(rename = "numChannels", deserialize_with = "required")]
    pub r#num_channels: u16,
    #[serde(rename = "sampleRate", deserialize_with = "required")]
    pub r#sample_rate: u32,
    #[serde(
        rename = "samplesPerChannel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#samples_per_channel: Option<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadRealtimeBemItemPresentation {
    #[serde(rename = "wholeItem")]
    WholeItem,
    #[serde(rename = "inlineMarkdown")]
    InlineMarkdown,
    #[serde(rename = "inlineVisualization")]
    InlineVisualization {
        #[serde(rename = "index", deserialize_with = "required")]
        r#index: u32,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeClosedNotification {
    #[serde(
        rename = "reason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reason: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeErrorNotification {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeInitialItem {
    #[serde(rename = "role", deserialize_with = "required")]
    pub r#role: ConversationTextRole,
    #[serde(rename = "text", deserialize_with = "required")]
    pub r#text: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadRealtimeItem {
    #[serde(rename = "realtimeSessionStarted")]
    RealtimeSessionStarted {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "realtimeSessionId", deserialize_with = "required")]
        r#realtime_session_id: String,
    },
    #[serde(rename = "transcriptSegment")]
    TranscriptSegment {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "realtimeSessionId", deserialize_with = "required")]
        r#realtime_session_id: String,
        #[serde(rename = "role", deserialize_with = "required")]
        r#role: ThreadRealtimeTranscriptRole,
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
    },
    #[serde(rename = "bemItemPromoted")]
    BemItemPromoted {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "realtimeSessionId", deserialize_with = "required")]
        r#realtime_session_id: String,
        #[serde(rename = "item_id", deserialize_with = "required")]
        r#item_id: String,
        #[serde(rename = "presentation", deserialize_with = "required")]
        r#presentation: ThreadRealtimeBemItemPresentation,
        #[serde(rename = "turn_id", deserialize_with = "required")]
        r#turn_id: String,
    },
    #[serde(rename = "realtimeSessionClosed")]
    RealtimeSessionClosed {
        #[serde(rename = "id", deserialize_with = "required")]
        r#id: String,
        #[serde(rename = "realtimeSessionId", deserialize_with = "required")]
        r#realtime_session_id: String,
        #[serde(rename = "outcome", deserialize_with = "required")]
        r#outcome: ThreadRealtimeSessionOutcome,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeItemAddedNotification {
    #[serde(rename = "item", deserialize_with = "required")]
    pub r#item: studio_json::Value,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeItemCompletedNotification {
    #[serde(rename = "item", deserialize_with = "required")]
    pub r#item: ThreadRealtimeItem,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeItemStartedNotification {
    #[serde(rename = "item", deserialize_with = "required")]
    pub r#item: ThreadRealtimeItem,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeItemTranscriptDeltaNotification {
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeListVoicesParams {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeListVoicesResponse {
    #[serde(rename = "voices", deserialize_with = "required")]
    pub r#voices: RealtimeVoicesList,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeOutputAudioDeltaNotification {
    #[serde(rename = "audio", deserialize_with = "required")]
    pub r#audio: ThreadRealtimeAudioChunk,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeSdpNotification {
    #[serde(rename = "sdp", deserialize_with = "required")]
    pub r#sdp: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadRealtimeSessionOutcome {
    #[serde(rename = "ended")]
    Ended,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeStartParams {
    #[serde(
        rename = "backendReasoningStatus",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#backend_reasoning_status: Option<bool>,
    #[serde(
        rename = "clientManagedHandoffs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#client_managed_handoffs: Option<Option<bool>>,
    #[serde(
        rename = "codexResponseHandoffChannelPrefixes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#codex_response_handoff_channel_prefixes:
        Option<Option<std::collections::BTreeMap<String, Vec<String>>>>,
    #[serde(
        rename = "codexResponseHandoffMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#codex_response_handoff_mode: Option<Option<CodexResponseHandoffMode>>,
    #[serde(
        rename = "codexResponseItemPrefix",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#codex_response_item_prefix: Option<Option<String>>,
    #[serde(
        rename = "codexResponsesAsItems",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#codex_responses_as_items: Option<Option<bool>>,
    #[serde(
        rename = "delegationAckFiller",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#delegation_ack_filler: Option<Option<bool>>,
    #[serde(
        rename = "flushTranscriptTailOnSessionEnd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#flush_transcript_tail_on_session_end: Option<Option<bool>>,
    #[serde(
        rename = "includeStartupContext",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#include_startup_context: Option<Option<bool>>,
    #[serde(
        rename = "initialItems",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#initial_items: Option<Option<Vec<ThreadRealtimeInitialItem>>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(rename = "outputModality", deserialize_with = "required")]
    pub r#output_modality: RealtimeOutputModality,
    #[serde(
        rename = "prompt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#prompt: Option<Option<String>>,
    #[serde(
        rename = "realtimeEndInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#realtime_end_instructions: Option<Option<String>>,
    #[serde(
        rename = "realtimeSessionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#realtime_session_id: Option<Option<String>>,
    #[serde(
        rename = "realtimeStartInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#realtime_start_instructions: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "transport",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#transport: Option<Option<ThreadRealtimeStartTransport>>,
    #[serde(
        rename = "version",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#version: Option<Option<RealtimeConversationVersion>>,
    #[serde(
        rename = "voice",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#voice: Option<Option<RealtimeVoice>>,
    #[serde(
        rename = "voiceSessionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#voice_session_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeStartResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadRealtimeStartTransport {
    #[serde(rename = "websocket")]
    Websocket,
    #[serde(rename = "webrtc")]
    Webrtc {
        #[serde(rename = "sdp", deserialize_with = "required")]
        r#sdp: String,
    },
    #[serde(rename = "existingCall")]
    ExistingCall {
        #[serde(rename = "callId", deserialize_with = "required")]
        r#call_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeStartedNotification {
    #[serde(
        rename = "realtimeSessionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#realtime_session_id: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "version", deserialize_with = "required")]
    pub r#version: RealtimeConversationVersion,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeStopParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeStopResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeTranscriptDeltaNotification {
    #[serde(rename = "delta", deserialize_with = "required")]
    pub r#delta: String,
    #[serde(rename = "role", deserialize_with = "required")]
    pub r#role: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRealtimeTranscriptDoneNotification {
    #[serde(rename = "role", deserialize_with = "required")]
    pub r#role: String,
    #[serde(rename = "text", deserialize_with = "required")]
    pub r#text: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadRealtimeTranscriptRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadResumeInitialTurnsPageParams {
    #[serde(
        rename = "itemsView",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#items_view: Option<Option<TurnItemsView>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "sortDirection",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_direction: Option<Option<SortDirection>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadResumeParams {
    #[serde(
        rename = "approvalPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_policy: Option<Option<AskForApproval>>,
    #[serde(
        rename = "approvalsReviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "baseInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#base_instructions: Option<Option<String>>,
    #[serde(
        rename = "config",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#config: Option<Option<std::collections::BTreeMap<String, studio_json::Value>>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "developerInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer_instructions: Option<Option<String>>,
    #[serde(
        rename = "excludeTurns",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#exclude_turns: Option<bool>,
    #[serde(
        rename = "history",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#history: Option<Option<Vec<ResponseItem>>>,
    #[serde(
        rename = "initialTurnsPage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#initial_turns_page: Option<Option<ThreadResumeInitialTurnsPageParams>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "modelProvider",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_provider: Option<Option<String>>,
    #[serde(
        rename = "path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#path: Option<Option<String>>,
    #[serde(
        rename = "permissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#permissions: Option<Option<String>>,
    #[serde(
        rename = "personality",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#personality: Option<Option<Personality>>,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Option<Vec<AbsolutePathBuf>>>,
    #[serde(
        rename = "sandbox",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox: Option<Option<SandboxMode>>,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadResumeResponse {
    #[serde(
        rename = "activePermissionProfile",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#active_permission_profile: Option<Option<ActivePermissionProfile>>,
    #[serde(rename = "approvalPolicy", deserialize_with = "required")]
    pub r#approval_policy: AskForApproval,
    #[serde(rename = "approvalsReviewer", deserialize_with = "required")]
    pub r#approvals_reviewer: ApprovalsReviewer,
    #[serde(
        rename = "collaborationMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#collaboration_mode: Option<Option<CollaborationMode>>,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: AbsolutePathBuf,
    #[serde(
        rename = "disabledPluginIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_plugin_ids: Option<Vec<String>>,
    #[serde(
        rename = "initialTurnsPage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#initial_turns_page: Option<Option<TurnsPage>>,
    #[serde(
        rename = "instructionSources",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#instruction_sources: Option<Vec<LegacyAppPathString>>,
    #[serde(
        rename = "itemsBackwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#items_backwards_cursor: Option<Option<String>>,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(rename = "modelProvider", deserialize_with = "required")]
    pub r#model_provider: String,
    #[serde(
        rename = "multiAgentMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_mode: Option<MultiAgentMode>,
    #[serde(
        rename = "reasoningEffort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reasoning_effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Vec<AbsolutePathBuf>>,
    #[serde(rename = "sandbox", deserialize_with = "required")]
    pub r#sandbox: SandboxPolicy,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
    #[serde(
        rename = "turnsBackwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turns_backwards_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRevertParams {
    #[serde(rename = "beforeTurnId", deserialize_with = "required")]
    pub r#before_turn_id: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRevertResponse {
    #[serde(
        rename = "itemsBackwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#items_backwards_cursor: Option<Option<String>>,
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
    #[serde(
        rename = "turnsBackwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turns_backwards_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRevertedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadSampleCancelParams {
    #[serde(rename = "requestId", deserialize_with = "required")]
    pub r#request_id: RequestId,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSampleCancelResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSampleParams {
    #[serde(
        rename = "disableMcp",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disable_mcp: Option<bool>,
    #[serde(
        rename = "effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#effort: Option<Option<ReasoningEffort>>,
    #[serde(rename = "expectedTurnId", deserialize_with = "required")]
    pub r#expected_turn_id: String,
    #[serde(
        rename = "interruptOnCancel",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#interrupt_on_cancel: Option<Option<bool>>,
    #[serde(rename = "prompt", deserialize_with = "required")]
    pub r#prompt: String,
    #[serde(
        rename = "responsesapiClientMetadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#responsesapi_client_metadata: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "threadSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_source: Option<Option<ThreadSource>>,
    #[serde(
        rename = "turnTrigger",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turn_trigger: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSampleResponse {
    #[serde(
        rename = "cachedInputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cached_input_tokens: Option<Option<i64>>,
    #[serde(
        rename = "inputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#input_tokens: Option<Option<i64>>,
    #[serde(
        rename = "outputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#output_tokens: Option<Option<i64>>,
    #[serde(rename = "responseId", deserialize_with = "required")]
    pub r#response_id: String,
    #[serde(rename = "sourceTurnId", deserialize_with = "required")]
    pub r#source_turn_id: String,
    #[serde(
        rename = "text",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#text: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSearchOccurrence {
    #[serde(rename = "itemId", deserialize_with = "required")]
    pub r#item_id: String,
    #[serde(rename = "snippet", deserialize_with = "required")]
    pub r#snippet: String,
    #[serde(rename = "snippetMatchRange", deserialize_with = "required")]
    pub r#snippet_match_range: ThreadSearchTextRange,
    #[serde(rename = "turnCursor", deserialize_with = "required")]
    pub r#turn_cursor: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSearchOccurrencesParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(rename = "searchTerm", deserialize_with = "required")]
    pub r#search_term: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSearchOccurrencesResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadSearchOccurrence>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSearchParams {
    #[serde(
        rename = "archived",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#archived: Option<Option<bool>>,
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(rename = "searchTerm", deserialize_with = "required")]
    pub r#search_term: String,
    #[serde(
        rename = "sortDirection",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_direction: Option<Option<SortDirection>>,
    #[serde(
        rename = "sortKey",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_key: Option<Option<ThreadSearchSortKey>>,
    #[serde(
        rename = "sourceKinds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#source_kinds: Option<Option<Vec<ThreadSourceKind>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSearchResponse {
    #[serde(
        rename = "backwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#backwards_cursor: Option<Option<String>>,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadSearchResult>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSearchResult {
    #[serde(rename = "snippet", deserialize_with = "required")]
    pub r#snippet: String,
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadSearchSortKey {
    #[serde(rename = "created_at")]
    CreatedAt,
    #[serde(rename = "updated_at")]
    UpdatedAt,
    #[serde(rename = "recency_at")]
    RecencyAt,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSearchTextRange {
    #[serde(rename = "end", deserialize_with = "required")]
    pub r#end: u32,
    #[serde(rename = "start", deserialize_with = "required")]
    pub r#start: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSection {
    #[serde(
        rename = "appearance",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#appearance: Option<Option<ThreadSectionAppearance>>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionAppearance {
    #[serde(
        rename = "color",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#color: Option<Option<String>>,
    #[serde(
        rename = "icon",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icon: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionChangedNotification {
    #[serde(rename = "sectionId", deserialize_with = "required")]
    pub r#section_id: Option<String>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionCreateParams {
    #[serde(
        rename = "appearance",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#appearance: Option<Option<ThreadSectionAppearance>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionCreateResponse {
    #[serde(rename = "section", deserialize_with = "required")]
    pub r#section: ThreadSection,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionDeleteParams {
    #[serde(rename = "sectionId", deserialize_with = "required")]
    pub r#section_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionDeleteResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionListResponse {
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadSection>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionMoveParams {
    #[serde(
        rename = "beforeThreadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#before_thread_id: Option<Option<String>>,
    #[serde(rename = "sectionId", deserialize_with = "required")]
    pub r#section_id: Option<String>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionMoveResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionUpdateParams {
    #[serde(
        rename = "appearance",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#appearance: Option<Option<ThreadSectionAppearance>>,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "sectionId", deserialize_with = "required")]
    pub r#section_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSectionUpdateResponse {
    #[serde(rename = "section", deserialize_with = "required")]
    pub r#section: ThreadSection,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSetNameParams {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSetNameResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSettings {
    #[serde(
        rename = "activePermissionProfile",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#active_permission_profile: Option<Option<ActivePermissionProfile>>,
    #[serde(rename = "approvalPolicy", deserialize_with = "required")]
    pub r#approval_policy: AskForApproval,
    #[serde(rename = "approvalsReviewer", deserialize_with = "required")]
    pub r#approvals_reviewer: ApprovalsReviewer,
    #[serde(rename = "collaborationMode", deserialize_with = "required")]
    pub r#collaboration_mode: CollaborationMode,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: AbsolutePathBuf,
    #[serde(
        rename = "disabledPluginIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_plugin_ids: Option<Vec<String>>,
    #[serde(
        rename = "effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#effort: Option<Option<ReasoningEffort>>,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(rename = "modelProvider", deserialize_with = "required")]
    pub r#model_provider: String,
    #[serde(
        rename = "multiAgentMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_mode: Option<MultiAgentMode>,
    #[serde(
        rename = "personality",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#personality: Option<Option<Personality>>,
    #[serde(rename = "sandboxPolicy", deserialize_with = "required")]
    pub r#sandbox_policy: SandboxPolicy,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(
        rename = "summary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#summary: Option<Option<ReasoningSummary>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSettingsUpdateParams {
    #[serde(
        rename = "approvalPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_policy: Option<Option<AskForApproval>>,
    #[serde(
        rename = "approvalsReviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "collaborationMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#collaboration_mode: Option<Option<CollaborationMode>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "disabledPluginIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_plugin_ids: Option<Option<Vec<String>>>,
    #[serde(
        rename = "effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "multiAgentMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_mode: Option<Option<MultiAgentMode>>,
    #[serde(
        rename = "permissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#permissions: Option<Option<String>>,
    #[serde(
        rename = "personality",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#personality: Option<Option<Personality>>,
    #[serde(
        rename = "sandboxPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox_policy: Option<Option<SandboxPolicy>>,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(
        rename = "summary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#summary: Option<Option<ReasoningSummary>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSettingsUpdateResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadSettingsUpdatedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "threadSettings", deserialize_with = "required")]
    pub r#thread_settings: ThreadSettings,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadShellCommandParams {
    #[serde(rename = "command", deserialize_with = "required")]
    pub r#command: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "timeoutMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#timeout_ms: Option<Option<i64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadShellCommandResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadSortKey {
    #[serde(rename = "created_at")]
    CreatedAt,
    #[serde(rename = "updated_at")]
    UpdatedAt,
    #[serde(rename = "recency_at")]
    RecencyAt,
    #[serde(rename = "section_position")]
    SectionPosition,
}

pub type ThreadSource = String;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadSourceKind {
    #[serde(rename = "cli")]
    Cli,
    #[serde(rename = "vscode")]
    Vscode,
    #[serde(rename = "exec")]
    Exec,
    #[serde(rename = "appServer")]
    AppServer,
    #[serde(rename = "subAgent")]
    SubAgent,
    #[serde(rename = "subAgentReview")]
    SubAgentReview,
    #[serde(rename = "subAgentCompact")]
    SubAgentCompact,
    #[serde(rename = "subAgentThreadSpawn")]
    SubAgentThreadSpawn,
    #[serde(rename = "subAgentOther")]
    SubAgentOther,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadStartParams {
    #[serde(
        rename = "allowProviderModelFallback",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allow_provider_model_fallback: Option<bool>,
    #[serde(
        rename = "approvalPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_policy: Option<Option<AskForApproval>>,
    #[serde(
        rename = "approvalsReviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "baseInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#base_instructions: Option<Option<String>>,
    #[serde(
        rename = "config",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#config: Option<Option<std::collections::BTreeMap<String, studio_json::Value>>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "daybreakEnabled",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#daybreak_enabled: Option<Option<bool>>,
    #[serde(
        rename = "developerInstructions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#developer_instructions: Option<Option<String>>,
    #[serde(
        rename = "dynamicTools",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#dynamic_tools: Option<Option<Vec<DynamicToolSpec>>>,
    #[serde(
        rename = "environments",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environments: Option<Option<Vec<TurnEnvironmentParams>>>,
    #[serde(
        rename = "ephemeral",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#ephemeral: Option<Option<bool>>,
    #[serde(
        rename = "experimentalRawEvents",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#experimental_raw_events: Option<bool>,
    #[serde(
        rename = "historyMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#history_mode: Option<Option<ThreadHistoryMode>>,
    #[serde(
        rename = "mockExperimentalField",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mock_experimental_field: Option<Option<String>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "modelProvider",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_provider: Option<Option<String>>,
    #[serde(
        rename = "multiAgentMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_mode: Option<Option<MultiAgentMode>>,
    #[serde(
        rename = "permissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#permissions: Option<Option<String>>,
    #[serde(
        rename = "personality",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#personality: Option<Option<Personality>>,
    #[serde(
        rename = "projectId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#project_id: Option<Option<String>>,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Option<Vec<AbsolutePathBuf>>>,
    #[serde(
        rename = "sandbox",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox: Option<Option<SandboxMode>>,
    #[serde(
        rename = "selectedCapabilityRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#selected_capability_roots: Option<Option<Vec<SelectedCapabilityRoot>>>,
    #[serde(
        rename = "serviceName",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_name: Option<Option<String>>,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(
        rename = "sessionStartSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#session_start_source: Option<Option<ThreadStartSource>>,
    #[serde(
        rename = "threadSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_source: Option<Option<ThreadSource>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadStartResponse {
    #[serde(
        rename = "activePermissionProfile",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#active_permission_profile: Option<Option<ActivePermissionProfile>>,
    #[serde(rename = "approvalPolicy", deserialize_with = "required")]
    pub r#approval_policy: AskForApproval,
    #[serde(rename = "approvalsReviewer", deserialize_with = "required")]
    pub r#approvals_reviewer: ApprovalsReviewer,
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: AbsolutePathBuf,
    #[serde(
        rename = "disabledPluginIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_plugin_ids: Option<Vec<String>>,
    #[serde(
        rename = "instructionSources",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#instruction_sources: Option<Vec<LegacyAppPathString>>,
    #[serde(rename = "model", deserialize_with = "required")]
    pub r#model: String,
    #[serde(rename = "modelProvider", deserialize_with = "required")]
    pub r#model_provider: String,
    #[serde(
        rename = "multiAgentMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_mode: Option<MultiAgentMode>,
    #[serde(
        rename = "reasoningEffort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reasoning_effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Vec<AbsolutePathBuf>>,
    #[serde(rename = "sandbox", deserialize_with = "required")]
    pub r#sandbox: SandboxPolicy,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadStartSource {
    #[serde(rename = "startup")]
    Startup,
    #[serde(rename = "clear")]
    Clear,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadStartedNotification {
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadStatus {
    #[serde(rename = "notLoaded")]
    NotLoaded,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "systemError")]
    SystemError,
    #[serde(rename = "active")]
    Active {
        #[serde(rename = "activeFlags", deserialize_with = "required")]
        r#active_flags: Vec<ThreadActiveFlag>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadStatusChangedNotification {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: ThreadStatus,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadTimelineEntry {
    #[serde(rename = "item")]
    Item {
        #[serde(rename = "item", deserialize_with = "required")]
        r#item: ThreadItem,
        #[serde(rename = "position", deserialize_with = "required")]
        r#position: u64,
        #[serde(rename = "turnId", deserialize_with = "required")]
        r#turn_id: String,
    },
    #[serde(rename = "realtime")]
    Realtime {
        #[serde(rename = "item", deserialize_with = "required")]
        r#item: ThreadRealtimeItem,
        #[serde(rename = "position", deserialize_with = "required")]
        r#position: u64,
    },
    #[serde(rename = "turnStarted")]
    TurnStarted {
        #[serde(rename = "position", deserialize_with = "required")]
        r#position: u64,
        #[serde(
            rename = "started_at",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at: Option<Option<i64>>,
        #[serde(rename = "turn_id", deserialize_with = "required")]
        r#turn_id: String,
    },
    #[serde(rename = "turnCompleted")]
    TurnCompleted {
        #[serde(
            rename = "completed_at",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#completed_at: Option<Option<i64>>,
        #[serde(
            rename = "duration_ms",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#duration_ms: Option<Option<i64>>,
        #[serde(
            rename = "error",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#error: Option<Option<TurnError>>,
        #[serde(rename = "position", deserialize_with = "required")]
        r#position: u64,
        #[serde(
            rename = "started_at",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#started_at: Option<Option<i64>>,
        #[serde(rename = "status", deserialize_with = "required")]
        r#status: TurnStatus,
        #[serde(rename = "turn_id", deserialize_with = "required")]
        r#turn_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadTimelineListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadTimelineListResponse {
    #[serde(
        rename = "activeRealtimeSessionAtPageStart",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#active_realtime_session_at_page_start: Option<Option<String>>,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<ThreadTimelineEntry>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadTokenUsage {
    #[serde(rename = "last", deserialize_with = "required")]
    pub r#last: TokenUsageBreakdown,
    #[serde(
        rename = "modelContextWindow",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model_context_window: Option<Option<i64>>,
    #[serde(rename = "total", deserialize_with = "required")]
    pub r#total: TokenUsageBreakdown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadTokenUsageUpdatedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "tokenUsage", deserialize_with = "required")]
    pub r#token_usage: ThreadTokenUsage,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadTurnsListParams {
    #[serde(
        rename = "cursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cursor: Option<Option<String>>,
    #[serde(
        rename = "itemsView",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#items_view: Option<Option<TurnItemsView>>,
    #[serde(
        rename = "limit",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#limit: Option<Option<u32>>,
    #[serde(
        rename = "sortDirection",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sort_direction: Option<Option<SortDirection>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadTurnsListResponse {
    #[serde(
        rename = "backwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#backwards_cursor: Option<Option<String>>,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<Turn>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadUnarchiveParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadUnarchiveResponse {
    #[serde(rename = "thread", deserialize_with = "required")]
    pub r#thread: Thread,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadUnarchivedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ThreadUnreadPosition {
    #[serde(rename = "threadStart")]
    ThreadStart,
    #[serde(rename = "turn")]
    Turn {
        #[serde(rename = "turnId", deserialize_with = "required")]
        r#turn_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadUnsubscribeParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadUnsubscribeResponse {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: ThreadUnsubscribeStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ThreadUnsubscribeStatus {
    #[serde(rename = "notLoaded")]
    NotLoaded,
    #[serde(rename = "notSubscribed")]
    NotSubscribed,
    #[serde(rename = "unsubscribed")]
    Unsubscribed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadUsage {
    #[serde(rename = "estimatedUsageCreditsMicros", deserialize_with = "required")]
    pub r#estimated_usage_credits_micros: i64,
    #[serde(
        rename = "estimatedUsageUsdMicros",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#estimated_usage_usd_micros: Option<Option<i64>>,
    #[serde(rename = "groups", deserialize_with = "required")]
    pub r#groups: Vec<ThreadUsageBreakdownGroup>,
    #[serde(
        rename = "nativeUsageUsdMicros",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#native_usage_usd_micros: Option<Option<i64>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadUsageBreakdownGroup {
    #[serde(
        rename = "cachedInputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cached_input_tokens: Option<Option<i64>>,
    #[serde(rename = "estimatedUsageCreditsMicros", deserialize_with = "required")]
    pub r#estimated_usage_credits_micros: i64,
    #[serde(
        rename = "inputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#input_tokens: Option<Option<i64>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "netNewInputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#net_new_input_tokens: Option<Option<i64>>,
    #[serde(
        rename = "outputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#output_tokens: Option<Option<i64>>,
    #[serde(
        rename = "reasoningEffort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#reasoning_effort: Option<Option<String>>,
    #[serde(
        rename = "speed",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#speed: Option<Option<String>>,
    #[serde(
        rename = "totalTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#total_tokens: Option<Option<i64>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TokenUsageBreakdown {
    #[serde(
        rename = "cacheWriteInputTokens",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cache_write_input_tokens: Option<i64>,
    #[serde(rename = "cachedInputTokens", deserialize_with = "required")]
    pub r#cached_input_tokens: i64,
    #[serde(rename = "inputTokens", deserialize_with = "required")]
    pub r#input_tokens: i64,
    #[serde(rename = "outputTokens", deserialize_with = "required")]
    pub r#output_tokens: i64,
    #[serde(rename = "reasoningOutputTokens", deserialize_with = "required")]
    pub r#reasoning_output_tokens: i64,
    #[serde(rename = "totalTokens", deserialize_with = "required")]
    pub r#total_tokens: i64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tool {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(
        rename = "annotations",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#annotations: Option<studio_json::Value>,
    #[serde(
        rename = "description",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#description: Option<Option<String>>,
    #[serde(
        rename = "icons",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#icons: Option<Option<Vec<studio_json::Value>>>,
    #[serde(rename = "inputSchema", deserialize_with = "required")]
    pub r#input_schema: studio_json::Value,
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "outputSchema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#output_schema: Option<studio_json::Value>,
    #[serde(
        rename = "title",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#title: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ToolExposureSurface {
    Variant1(Box<ToolExposureSurfaceVariant1>),
    Variant2(Box<ToolExposureSurfaceVariant2>),
    Variant3(Box<ToolExposureSurfaceVariant3>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolsV2 {
    #[serde(
        rename = "web_search",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#web_search: Option<Option<WebSearchToolConfig>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Turn {
    #[serde(
        rename = "completedAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#completed_at: Option<Option<i64>>,
    #[serde(
        rename = "durationMs",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#duration_ms: Option<Option<i64>>,
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<TurnError>>,
    #[serde(rename = "id", deserialize_with = "required")]
    pub r#id: String,
    #[serde(rename = "items", deserialize_with = "required")]
    pub r#items: Vec<ThreadItem>,
    #[serde(
        rename = "itemsView",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#items_view: Option<TurnItemsView>,
    #[serde(
        rename = "rootTurnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#root_turn_id: Option<Option<String>>,
    #[serde(
        rename = "startedAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#started_at: Option<Option<i64>>,
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: TurnStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnCompletedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turn", deserialize_with = "required")]
    pub r#turn: Turn,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnDiffUpdatedNotification {
    #[serde(rename = "diff", deserialize_with = "required")]
    pub r#diff: String,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnEnvironmentParams {
    #[serde(rename = "cwd", deserialize_with = "required")]
    pub r#cwd: LegacyAppPathString,
    #[serde(rename = "environmentId", deserialize_with = "required")]
    pub r#environment_id: String,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Option<Vec<LegacyAppPathString>>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnError {
    #[serde(
        rename = "additionalDetails",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#additional_details: Option<Option<String>>,
    #[serde(
        rename = "codexErrorInfo",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#codex_error_info: Option<Option<CodexErrorInfo>>,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(
        rename = "misalignment",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#misalignment: Option<Option<MisalignmentErrorDetails>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnInterruptParams {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnInterruptResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum TurnItemsView {
    Variant1(Box<TurnItemsViewVariant1>),
    Variant2(Box<TurnItemsViewVariant2>),
    Variant3(Box<TurnItemsViewVariant3>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnModerationMetadataNotification {
    #[serde(rename = "metadata", deserialize_with = "required")]
    pub r#metadata: studio_json::Value,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnPlanStep {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: TurnPlanStepStatus,
    #[serde(rename = "step", deserialize_with = "required")]
    pub r#step: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TurnPlanStepStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "inProgress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnPlanUpdatedNotification {
    #[serde(
        rename = "explanation",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#explanation: Option<Option<String>>,
    #[serde(rename = "plan", deserialize_with = "required")]
    pub r#plan: Vec<TurnPlanStep>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnSettingsUpdateParams {
    #[serde(
        rename = "approvalsReviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(
        rename = "summary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#summary: Option<Option<ReasoningSummary>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnSettingsUpdateResponse {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: TurnSettingsUpdateStatus,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum TurnSettingsUpdateStatus {
    Variant1(Box<TurnSettingsUpdateStatusVariant1>),
    Variant2(Box<TurnSettingsUpdateStatusVariant2>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnStartParams {
    #[serde(
        rename = "additionalContext",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#additional_context:
        Option<Option<std::collections::BTreeMap<String, AdditionalContextEntry>>>,
    #[serde(
        rename = "approvalPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approval_policy: Option<Option<AskForApproval>>,
    #[serde(
        rename = "approvalsReviewer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#approvals_reviewer: Option<Option<ApprovalsReviewer>>,
    #[serde(
        rename = "clientUserMessageId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#client_user_message_id: Option<Option<String>>,
    #[serde(
        rename = "collaborationMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#collaboration_mode: Option<Option<CollaborationMode>>,
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<String>>,
    #[serde(
        rename = "cyberAccessProgram",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cyber_access_program: Option<Option<CyberAccessProgram>>,
    #[serde(
        rename = "disabledPluginIds",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#disabled_plugin_ids: Option<Option<Vec<String>>>,
    #[serde(
        rename = "effort",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#effort: Option<Option<ReasoningEffort>>,
    #[serde(
        rename = "environments",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#environments: Option<Option<Vec<TurnEnvironmentParams>>>,
    #[serde(rename = "input", deserialize_with = "required")]
    pub r#input: Vec<UserInput>,
    #[serde(
        rename = "model",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#model: Option<Option<String>>,
    #[serde(
        rename = "multiAgentMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#multi_agent_mode: Option<Option<MultiAgentMode>>,
    #[serde(
        rename = "outputSchema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#output_schema: Option<studio_json::Value>,
    #[serde(
        rename = "parentTurnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#parent_turn_id: Option<Option<String>>,
    #[serde(
        rename = "permissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#permissions: Option<Option<String>>,
    #[serde(
        rename = "personality",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#personality: Option<Option<Personality>>,
    #[serde(
        rename = "responsesapiClientMetadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#responsesapi_client_metadata: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "responsesapiClientMetadataPatch",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#responsesapi_client_metadata_patch:
        Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "rootTurnId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#root_turn_id: Option<Option<String>>,
    #[serde(
        rename = "runtimeWorkspaceRoots",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#runtime_workspace_roots: Option<Option<Vec<AbsolutePathBuf>>>,
    #[serde(
        rename = "sandboxPolicy",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#sandbox_policy: Option<Option<SandboxPolicy>>,
    #[serde(
        rename = "serviceTier",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier: Option<Option<String>>,
    #[serde(
        rename = "serviceTierForTurn",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#service_tier_for_turn: Option<Option<String>>,
    #[serde(
        rename = "summary",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#summary: Option<Option<ReasoningSummary>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(
        rename = "toolOutput",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tool_output: Option<Option<TurnToolOutput>>,
    #[serde(
        rename = "turnTrigger",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#turn_trigger: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnStartResponse {
    #[serde(rename = "turn", deserialize_with = "required")]
    pub r#turn: Turn,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnStartedNotification {
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
    #[serde(rename = "turn", deserialize_with = "required")]
    pub r#turn: Turn,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TurnStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "inProgress")]
    InProgress,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnSteerParams {
    #[serde(
        rename = "additionalContext",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#additional_context:
        Option<Option<std::collections::BTreeMap<String, AdditionalContextEntry>>>,
    #[serde(
        rename = "clientUserMessageId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#client_user_message_id: Option<Option<String>>,
    #[serde(rename = "expectedTurnId", deserialize_with = "required")]
    pub r#expected_turn_id: String,
    #[serde(rename = "input", deserialize_with = "required")]
    pub r#input: Vec<UserInput>,
    #[serde(
        rename = "responsesapiClientMetadata",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#responsesapi_client_metadata: Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(
        rename = "responsesapiClientMetadataPatch",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#responsesapi_client_metadata_patch:
        Option<Option<std::collections::BTreeMap<String, String>>>,
    #[serde(rename = "threadId", deserialize_with = "required")]
    pub r#thread_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnSteerResponse {
    #[serde(rename = "turnId", deserialize_with = "required")]
    pub r#turn_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnToolOutput {
    #[serde(rename = "name", deserialize_with = "required")]
    pub r#name: String,
    #[serde(
        rename = "namespace",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#namespace: Option<Option<String>>,
    #[serde(rename = "output", deserialize_with = "required")]
    pub r#output: FunctionCallOutputBody,
    #[serde(
        rename = "retain",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#retain: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnsPage {
    #[serde(
        rename = "backwardsCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#backwards_cursor: Option<Option<String>>,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: Vec<Turn>,
    #[serde(
        rename = "nextCursor",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#next_cursor: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum UserInput {
    #[serde(rename = "text")]
    Text {
        #[serde(rename = "text", deserialize_with = "required")]
        r#text: String,
        #[serde(
            rename = "text_elements",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#text_elements: Option<Vec<TextElement>>,
    },
    #[serde(rename = "image")]
    Image {
        #[serde(
            rename = "detail",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#detail: Option<Option<ImageDetail>>,
    },
    #[serde(rename = "localImage")]
    LocalImage {
        #[serde(
            rename = "detail",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#detail: Option<Option<ImageDetail>>,
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: String,
    },
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "localAudio")]
    LocalAudio {
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: String,
    },
    #[serde(rename = "skill")]
    Skill {
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: String,
    },
    #[serde(rename = "mention")]
    Mention {
        #[serde(rename = "name", deserialize_with = "required")]
        r#name: String,
        #[serde(rename = "path", deserialize_with = "required")]
        r#path: String,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVerificationCancelParams {
    #[serde(rename = "requestId", deserialize_with = "required")]
    pub r#request_id: RequestId,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserVerificationCancelResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UserVerificationCancellationReason {
    #[serde(rename = "userCancelled")]
    UserCancelled,
    #[serde(rename = "interrupted")]
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVerificationDeleteParams {}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserVerificationDeleteResponse {
    #[serde(flatten)]
    pub additional_properties: std::collections::BTreeMap<String, studio_json::Value>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVerificationEnrollParams {}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserVerificationEnrollResponse {
    #[serde(
        rename = "algorithm",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#algorithm: Option<Option<String>>,
    #[serde(rename = "credentialId", deserialize_with = "required")]
    pub r#credential_id: String,
    #[serde(
        rename = "publicKey",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#public_key: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum UserVerificationErrorDetails {
    #[serde(rename = "invalidRequest")]
    InvalidRequest {
        #[serde(rename = "reason", deserialize_with = "required")]
        r#reason: UserVerificationInvalidRequestReason,
    },
    #[serde(rename = "unavailable")]
    Unavailable {
        #[serde(rename = "reason", deserialize_with = "required")]
        r#reason: UserVerificationUnavailableReason,
    },
    #[serde(rename = "cancelled")]
    Cancelled {
        #[serde(rename = "reason", deserialize_with = "required")]
        r#reason: UserVerificationCancellationReason,
    },
    #[serde(rename = "failed")]
    Failed {
        #[serde(rename = "reason", deserialize_with = "required")]
        r#reason: UserVerificationFailureReason,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UserVerificationFailureReason {
    #[serde(rename = "authenticationFailed")]
    AuthenticationFailed,
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "providerError")]
    ProviderError,
    #[serde(rename = "serviceError")]
    ServiceError,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UserVerificationInvalidRequestReason {
    #[serde(rename = "invalidParams")]
    InvalidParams,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVerificationProof {
    #[serde(rename = "credentialId", deserialize_with = "required")]
    pub r#credential_id: String,
    #[serde(rename = "signature", deserialize_with = "required")]
    pub r#signature: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVerificationRpcError {
    #[serde(rename = "code", deserialize_with = "required")]
    pub r#code: i64,
    #[serde(rename = "data", deserialize_with = "required")]
    pub r#data: UserVerificationErrorDetails,
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVerificationStatusParams {}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserVerificationStatusResponse {
    #[serde(
        rename = "credentialId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#credential_id: Option<Option<String>>,
    #[serde(
        rename = "unavailableMessage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#unavailable_message: Option<Option<String>>,
    #[serde(
        rename = "unavailableReason",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#unavailable_reason: Option<Option<UserVerificationUnavailableReason>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UserVerificationUnavailableReason {
    #[serde(rename = "credentialMissing")]
    CredentialMissing,
    #[serde(rename = "biometricsUnavailable")]
    BiometricsUnavailable,
    #[serde(rename = "providerUnavailable")]
    ProviderUnavailable,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserVerificationVerifyParams {
    #[serde(rename = "challenge", deserialize_with = "required")]
    pub r#challenge: String,
    #[serde(rename = "description", deserialize_with = "required")]
    pub r#description: String,
    #[serde(rename = "title", deserialize_with = "required")]
    pub r#title: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserVerificationVerifyResponse {
    #[serde(rename = "proof", deserialize_with = "required")]
    pub r#proof: UserVerificationProof,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Verbosity {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WarningNotification {
    #[serde(rename = "message", deserialize_with = "required")]
    pub r#message: String,
    #[serde(
        rename = "threadId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#thread_id: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum WebSearchAction {
    #[serde(rename = "search")]
    Search {
        #[serde(
            rename = "queries",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#queries: Option<Option<Vec<String>>>,
        #[serde(
            rename = "query",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#query: Option<Option<String>>,
    },
    #[serde(rename = "openPage")]
    OpenPage {
        #[serde(
            rename = "url",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#url: Option<Option<String>>,
    },
    #[serde(rename = "findInPage")]
    FindInPage {
        #[serde(
            rename = "pattern",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#pattern: Option<Option<String>>,
        #[serde(
            rename = "url",
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional"
        )]
        r#url: Option<Option<String>>,
    },
    #[serde(rename = "other")]
    Other,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WebSearchContextSize {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebSearchLocation {
    #[serde(
        rename = "city",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#city: Option<Option<String>>,
    #[serde(
        rename = "country",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#country: Option<Option<String>>,
    #[serde(
        rename = "region",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#region: Option<Option<String>>,
    #[serde(
        rename = "timezone",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#timezone: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WebSearchMode {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "cached")]
    Cached,
    #[serde(rename = "indexed")]
    Indexed,
    #[serde(rename = "live")]
    Live,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebSearchToolConfig {
    #[serde(
        rename = "allowed_domains",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#allowed_domains: Option<Option<Vec<String>>>,
    #[serde(
        rename = "context_size",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#context_size: Option<Option<WebSearchContextSize>>,
    #[serde(
        rename = "location",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#location: Option<Option<WebSearchLocation>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WindowsSandboxImplementation {
    #[serde(rename = "elevated")]
    Elevated,
    #[serde(rename = "unelevated")]
    Unelevated,
    #[serde(rename = "mxc")]
    Mxc,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WindowsSandboxReadiness {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "notConfigured")]
    NotConfigured,
    #[serde(rename = "updateRequired")]
    UpdateRequired,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowsSandboxReadinessResponse {
    #[serde(rename = "status", deserialize_with = "required")]
    pub r#status: WindowsSandboxReadiness,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowsSandboxSetupCompletedNotification {
    #[serde(
        rename = "error",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#error: Option<Option<String>>,
    #[serde(rename = "mode", deserialize_with = "required")]
    pub r#mode: WindowsSandboxSetupMode,
    #[serde(rename = "success", deserialize_with = "required")]
    pub r#success: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WindowsSandboxSetupMode {
    #[serde(rename = "elevated")]
    Elevated,
    #[serde(rename = "unelevated")]
    Unelevated,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowsSandboxSetupStartParams {
    #[serde(
        rename = "cwd",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#cwd: Option<Option<AbsolutePathBuf>>,
    #[serde(rename = "mode", deserialize_with = "required")]
    pub r#mode: WindowsSandboxSetupMode,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowsSandboxSetupStartResponse {
    #[serde(rename = "started", deserialize_with = "required")]
    pub r#started: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowsWorldWritableWarningNotification {
    #[serde(rename = "extraCount", deserialize_with = "required")]
    pub r#extra_count: u64,
    #[serde(rename = "failedScan", deserialize_with = "required")]
    pub r#failed_scan: bool,
    #[serde(rename = "samplePaths", deserialize_with = "required")]
    pub r#sample_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceMessage {
    #[serde(
        rename = "archivedAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#archived_at: Option<Option<i64>>,
    #[serde(
        rename = "createdAt",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#created_at: Option<Option<i64>>,
    #[serde(rename = "messageBody", deserialize_with = "required")]
    pub r#message_body: String,
    #[serde(rename = "messageId", deserialize_with = "required")]
    pub r#message_id: String,
    #[serde(rename = "messageType", deserialize_with = "required")]
    pub r#message_type: WorkspaceMessageType,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WorkspaceMessageType {
    #[serde(rename = "headline")]
    Headline,
    #[serde(rename = "announcement")]
    Announcement,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceRouting {
    #[serde(rename = "accountRoutingOverride", deserialize_with = "required")]
    pub r#account_routing_override: AccountRoutingOverride,
    #[serde(rename = "backendOrigin", deserialize_with = "required")]
    pub r#backend_origin: String,
    #[serde(rename = "chatgptAccountId", deserialize_with = "required")]
    pub r#chatgpt_account_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WriteStatus {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "okOverridden")]
    OkOverridden,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChatgptAuthTokensRefreshReasonVariant1 {
    #[serde(rename = "unauthorized")]
    Unauthorized,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionApprovalDecisionVariant1 {
    #[serde(rename = "accept")]
    Accept,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionApprovalDecisionVariant2 {
    #[serde(rename = "acceptForSession")]
    AcceptForSession,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandExecutionApprovalDecisionVariant3 {
    #[serde(
        rename = "acceptWithExecpolicyAmendment",
        deserialize_with = "required"
    )]
    pub r#accept_with_execpolicy_amendment:
        CommandExecutionApprovalDecisionVariant3AcceptWithExecpolicyAmendment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandExecutionApprovalDecisionVariant4 {
    #[serde(rename = "applyNetworkPolicyAmendment", deserialize_with = "required")]
    pub r#apply_network_policy_amendment:
        CommandExecutionApprovalDecisionVariant4ApplyNetworkPolicyAmendment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionApprovalDecisionVariant5 {
    #[serde(rename = "decline")]
    Decline,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionApprovalDecisionVariant6 {
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionApprovalKindVariant1 {
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "writeStdin")]
    WriteStdin,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecutionApprovalKindVariant2 {
    #[serde(rename = "humanCommand")]
    HumanCommand,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileChangeApprovalDecisionVariant1 {
    #[serde(rename = "accept")]
    Accept,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileChangeApprovalDecisionVariant2 {
    #[serde(rename = "acceptForSession")]
    AcceptForSession,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileChangeApprovalDecisionVariant3 {
    #[serde(rename = "decline")]
    Decline,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileChangeApprovalDecisionVariant4 {
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReviewDecisionVariant1 {
    #[serde(rename = "approved")]
    Approved,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionVariant2 {
    #[serde(
        rename = "approved_execpolicy_amendment",
        deserialize_with = "required"
    )]
    pub r#approved_execpolicy_amendment: ReviewDecisionVariant2ApprovedExecpolicyAmendment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReviewDecisionVariant3 {
    #[serde(rename = "approved_for_session")]
    ApprovedForSession,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReviewDecisionVariant4 {
    #[serde(rename = "approved_mcp_policy_amendment")]
    ApprovedMcpPolicyAmendment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionVariant5 {
    #[serde(rename = "network_policy_amendment", deserialize_with = "required")]
    pub r#network_policy_amendment: ReviewDecisionVariant5NetworkPolicyAmendment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionVariant6 {
    #[serde(rename = "denied", deserialize_with = "required")]
    pub r#denied: ReviewDecisionVariant6Denied,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReviewDecisionVariant7 {
    #[serde(rename = "timed_out")]
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReviewDecisionVariant8 {
    #[serde(rename = "abort")]
    Abort,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountSessionsSwitchResponseVariant1 {
    #[serde(
        rename = "activeSessionId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#active_session_id: Option<Option<String>>,
    #[serde(rename = "sessions", deserialize_with = "required")]
    pub r#sessions: Vec<AccountSession>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountSessionsSwitchResponseVariant2 {
    #[serde(rename = "auth_url", deserialize_with = "required")]
    pub r#auth_url: String,
    #[serde(rename = "login_id", deserialize_with = "required")]
    pub r#login_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AskForApprovalVariant1 {
    #[serde(rename = "untrusted")]
    Untrusted,
    #[serde(rename = "on-request")]
    OnRequest,
    #[serde(rename = "never")]
    Never,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskForApprovalVariant2 {
    #[serde(rename = "granular", deserialize_with = "required")]
    pub r#granular: AskForApprovalVariant2Granular,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant1 {
    #[serde(rename = "apikey")]
    Apikey,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant2 {
    #[serde(rename = "chatgpt")]
    Chatgpt,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant3 {
    #[serde(rename = "chatgptAuthTokens")]
    ChatgptAuthTokens,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant4 {
    #[serde(rename = "headers")]
    Headers,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant5 {
    #[serde(rename = "agentIdentity")]
    AgentIdentity,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant6 {
    #[serde(rename = "personalAccessToken")]
    PersonalAccessToken,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant7 {
    #[serde(rename = "bedrockApiKey")]
    BedrockApiKey,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthModeVariant8 {
    #[serde(rename = "bedrockAccessKeys")]
    BedrockAccessKeys,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AutoCompactTokenLimitScopeVariant1 {
    #[serde(rename = "total")]
    Total,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AutoCompactTokenLimitScopeVariant2 {
    #[serde(rename = "body_after_prefix")]
    BodyAfterPrefix,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CodexErrorInfoVariant1 {
    #[serde(rename = "contextWindowExceeded")]
    ContextWindowExceeded,
    #[serde(rename = "sessionBudgetExceeded")]
    SessionBudgetExceeded,
    #[serde(rename = "usageLimitExceeded")]
    UsageLimitExceeded,
    #[serde(rename = "rateLimitExceeded")]
    RateLimitExceeded,
    #[serde(rename = "flexUnavailable")]
    FlexUnavailable,
    #[serde(rename = "serverOverloaded")]
    ServerOverloaded,
    #[serde(rename = "modelNotFound")]
    ModelNotFound,
    #[serde(rename = "policyCapacityUnavailable")]
    PolicyCapacityUnavailable,
    #[serde(rename = "cyberPolicy")]
    CyberPolicy,
    #[serde(rename = "misalignmentPolicyViolation")]
    MisalignmentPolicyViolation,
    #[serde(rename = "tooManyDenials")]
    TooManyDenials,
    #[serde(rename = "internalServerError")]
    InternalServerError,
    #[serde(rename = "unauthorized")]
    Unauthorized,
    #[serde(rename = "badRequest")]
    BadRequest,
    #[serde(rename = "threadRollbackFailed")]
    ThreadRollbackFailed,
    #[serde(rename = "sandboxError")]
    SandboxError,
    #[serde(rename = "other")]
    Other,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexErrorInfoVariant2 {
    #[serde(rename = "httpConnectionFailed", deserialize_with = "required")]
    pub r#http_connection_failed: CodexErrorInfoVariant2HttpConnectionFailed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexErrorInfoVariant3 {
    #[serde(
        rename = "responseStreamConnectionFailed",
        deserialize_with = "required"
    )]
    pub r#response_stream_connection_failed: CodexErrorInfoVariant3ResponseStreamConnectionFailed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexErrorInfoVariant4 {
    #[serde(rename = "responseStreamDisconnected", deserialize_with = "required")]
    pub r#response_stream_disconnected: CodexErrorInfoVariant4ResponseStreamDisconnected,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexErrorInfoVariant5 {
    #[serde(
        rename = "responseTooManyFailedAttempts",
        deserialize_with = "required"
    )]
    pub r#response_too_many_failed_attempts: CodexErrorInfoVariant5ResponseTooManyFailedAttempts,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexErrorInfoVariant6 {
    #[serde(rename = "activeTurnNotSteerable", deserialize_with = "required")]
    pub r#active_turn_not_steerable: CodexErrorInfoVariant6ActiveTurnNotSteerable,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CodexErrorInfoVariant7 {
    Variant1(Box<String>),
    Variant2(Box<std::collections::BTreeMap<String, studio_json::Value>>),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecOutputStreamVariant1 {
    #[serde(rename = "stdout")]
    Stdout,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CommandExecOutputStreamVariant2 {
    #[serde(rename = "stderr")]
    Stderr,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConsumeAccountRateLimitResetCreditOutcomeVariant1 {
    #[serde(rename = "reset")]
    Reset,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConsumeAccountRateLimitResetCreditOutcomeVariant2 {
    #[serde(rename = "nothingToReset")]
    NothingToReset,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConsumeAccountRateLimitResetCreditOutcomeVariant3 {
    #[serde(rename = "noCredit")]
    NoCredit,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConsumeAccountRateLimitResetCreditOutcomeVariant4 {
    #[serde(rename = "alreadyRedeemed")]
    AlreadyRedeemed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EnvironmentStatusKindVariant1 {
    #[serde(rename = "ready")]
    Ready,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EnvironmentStatusKindVariant2 {
    #[serde(rename = "pending")]
    Pending,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EnvironmentStatusKindVariant3 {
    #[serde(rename = "disconnected")]
    Disconnected,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EnvironmentStatusKindVariant4 {
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExperimentalFeatureStageVariant1 {
    #[serde(rename = "beta")]
    Beta,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExperimentalFeatureStageVariant2 {
    #[serde(rename = "underDevelopment")]
    UnderDevelopment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExperimentalFeatureStageVariant3 {
    #[serde(rename = "stable")]
    Stable,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExperimentalFeatureStageVariant4 {
    #[serde(rename = "deprecated")]
    Deprecated,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExperimentalFeatureStageVariant5 {
    #[serde(rename = "removed")]
    Removed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InputModalityVariant1 {
    #[serde(rename = "text")]
    Text,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InputModalityVariant2 {
    #[serde(rename = "image")]
    Image,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InputModalityVariant3 {
    #[serde(rename = "audio")]
    Audio,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MessagePhaseVariant1 {
    #[serde(rename = "commentary")]
    Commentary,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MessagePhaseVariant2 {
    #[serde(rename = "partial_answer")]
    PartialAnswer,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MessagePhaseVariant3 {
    #[serde(rename = "final_answer")]
    FinalAnswer,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MultiAgentModeVariant1 {
    #[serde(rename = "explicitRequestOnly")]
    ExplicitRequestOnly,
    #[serde(rename = "proactive")]
    Proactive,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiAgentModeVariant2 {
    #[serde(rename = "custom", deserialize_with = "required")]
    pub r#custom: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OtelExporterVariant1 {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "statsig")]
    Statsig,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtelExporterVariant2 {
    #[serde(rename = "otlp-http", deserialize_with = "required")]
    pub r#otlp_http: OtelExporterVariant2OtlpHttp,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OtelExporterVariant3 {
    #[serde(rename = "otlp-grpc", deserialize_with = "required")]
    pub r#otlp_grpc: OtelExporterVariant3OtlpGrpc,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginAvailabilityVariant1 {
    #[serde(rename = "DISABLED_BY_ADMIN")]
    DISABLEDBYADMIN,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PluginAvailabilityVariant2 {
    #[serde(rename = "AVAILABLE")]
    AVAILABLE,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProcessOutputStreamVariant1 {
    #[serde(rename = "stdout")]
    Stdout,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProcessOutputStreamVariant2 {
    #[serde(rename = "stderr")]
    Stderr,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReasoningSummaryVariant1 {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "concise")]
    Concise,
    #[serde(rename = "detailed")]
    Detailed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReasoningSummaryVariant2 {
    #[serde(rename = "none")]
    None,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceContentVariant1 {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(
        rename = "mimeType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mime_type: Option<Option<String>>,
    #[serde(rename = "text", deserialize_with = "required")]
    pub r#text: String,
    #[serde(rename = "uri", deserialize_with = "required")]
    pub r#uri: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceContentVariant2 {
    #[serde(
        rename = "_meta",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#_meta: Option<studio_json::Value>,
    #[serde(rename = "blob", deserialize_with = "required")]
    pub r#blob: String,
    #[serde(
        rename = "mimeType",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#mime_type: Option<Option<String>>,
    #[serde(rename = "uri", deserialize_with = "required")]
    pub r#uri: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SessionSourceVariant1 {
    #[serde(rename = "cli")]
    Cli,
    #[serde(rename = "vscode")]
    Vscode,
    #[serde(rename = "exec")]
    Exec,
    #[serde(rename = "appServer")]
    AppServer,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSourceVariant2 {
    #[serde(rename = "custom", deserialize_with = "required")]
    pub r#custom: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSourceVariant3 {
    #[serde(rename = "subAgent", deserialize_with = "required")]
    pub r#sub_agent: SubAgentSource,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SubAgentSourceVariant1 {
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "memory_consolidation")]
    MemoryConsolidation,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubAgentSourceVariant2 {
    #[serde(rename = "thread_spawn", deserialize_with = "required")]
    pub r#thread_spawn: SubAgentSourceVariant2ThreadSpawn,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubAgentSourceVariant3 {
    #[serde(rename = "other", deserialize_with = "required")]
    pub r#other: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ToolExposureSurfaceVariant1 {
    #[serde(rename = "code_mode")]
    CodeMode,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ToolExposureSurfaceVariant2 {
    #[serde(rename = "deferred")]
    Deferred,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ToolExposureSurfaceVariant3 {
    #[serde(rename = "direct")]
    Direct,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TurnItemsViewVariant1 {
    #[serde(rename = "notLoaded")]
    NotLoaded,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TurnItemsViewVariant2 {
    #[serde(rename = "summary")]
    Summary,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TurnItemsViewVariant3 {
    #[serde(rename = "full")]
    Full,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TurnSettingsUpdateStatusVariant1 {
    #[serde(rename = "applied")]
    Applied,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TurnSettingsUpdateStatusVariant2 {
    #[serde(rename = "targetUnavailable")]
    TargetUnavailable,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecutionApprovalDecisionVariant3AcceptWithExecpolicyAmendment {
    #[serde(rename = "execpolicy_amendment", deserialize_with = "required")]
    pub r#execpolicy_amendment: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandExecutionApprovalDecisionVariant4ApplyNetworkPolicyAmendment {
    #[serde(rename = "network_policy_amendment", deserialize_with = "required")]
    pub r#network_policy_amendment: NetworkPolicyAmendment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewDecisionVariant2ApprovedExecpolicyAmendment {
    #[serde(
        rename = "proposed_execpolicy_amendment",
        deserialize_with = "required"
    )]
    pub r#proposed_execpolicy_amendment: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewDecisionVariant5NetworkPolicyAmendment {
    #[serde(rename = "network_policy_amendment", deserialize_with = "required")]
    pub r#network_policy_amendment: NetworkPolicyAmendment,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewDecisionVariant6Denied {
    #[serde(rename = "rejection", deserialize_with = "required")]
    pub r#rejection: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AskForApprovalVariant2Granular {
    #[serde(rename = "mcp_elicitations", deserialize_with = "required")]
    pub r#mcp_elicitations: bool,
    #[serde(
        rename = "request_permissions",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#request_permissions: Option<bool>,
    #[serde(rename = "rules", deserialize_with = "required")]
    pub r#rules: bool,
    #[serde(rename = "sandbox_approval", deserialize_with = "required")]
    pub r#sandbox_approval: bool,
    #[serde(
        rename = "skill_approval",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#skill_approval: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexErrorInfoVariant2HttpConnectionFailed {
    #[serde(
        rename = "httpStatusCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#http_status_code: Option<Option<u16>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexErrorInfoVariant3ResponseStreamConnectionFailed {
    #[serde(
        rename = "httpStatusCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#http_status_code: Option<Option<u16>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexErrorInfoVariant4ResponseStreamDisconnected {
    #[serde(
        rename = "httpStatusCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#http_status_code: Option<Option<u16>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexErrorInfoVariant5ResponseTooManyFailedAttempts {
    #[serde(
        rename = "httpStatusCode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#http_status_code: Option<Option<u16>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodexErrorInfoVariant6ActiveTurnNotSteerable {
    #[serde(rename = "turnKind", deserialize_with = "required")]
    pub r#turn_kind: NonSteerableTurnKind,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OtelExporterVariant2OtlpHttp {
    #[serde(rename = "endpoint", deserialize_with = "required")]
    pub r#endpoint: String,
    #[serde(rename = "headers", deserialize_with = "required")]
    pub r#headers: std::collections::BTreeMap<String, String>,
    #[serde(rename = "protocol", deserialize_with = "required")]
    pub r#protocol: OtelHttpProtocol,
    #[serde(
        rename = "tls",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tls: Option<Option<OtelTlsConfig>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OtelExporterVariant3OtlpGrpc {
    #[serde(rename = "endpoint", deserialize_with = "required")]
    pub r#endpoint: String,
    #[serde(rename = "headers", deserialize_with = "required")]
    pub r#headers: std::collections::BTreeMap<String, String>,
    #[serde(
        rename = "tls",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#tls: Option<Option<OtelTlsConfig>>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubAgentSourceVariant2ThreadSpawn {
    #[serde(
        rename = "agent_nickname",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#agent_nickname: Option<Option<String>>,
    #[serde(
        rename = "agent_path",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#agent_path: Option<Option<AgentPath>>,
    #[serde(
        rename = "agent_role",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional"
    )]
    pub r#agent_role: Option<Option<String>>,
    #[serde(rename = "depth", deserialize_with = "required")]
    pub r#depth: i32,
    #[serde(rename = "parent_thread_id", deserialize_with = "required")]
    pub r#parent_thread_id: ThreadId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ClientRequestMethod {
    #[serde(rename = "initialize")]
    Initialize,
    #[serde(rename = "server/diagnostics")]
    ServerDiagnostics,
    #[serde(rename = "userVerification/status")]
    UserVerificationStatus,
    #[serde(rename = "userVerification/enroll")]
    UserVerificationEnroll,
    #[serde(rename = "userVerification/delete")]
    UserVerificationDelete,
    #[serde(rename = "userVerification/verify")]
    UserVerificationVerify,
    #[serde(rename = "userVerification/cancel")]
    UserVerificationCancel,
    #[serde(rename = "thread/start")]
    ThreadStart,
    #[serde(rename = "thread/resume")]
    ThreadResume,
    #[serde(rename = "thread/fork")]
    ThreadFork,
    #[serde(rename = "thread/sample")]
    ThreadSample,
    #[serde(rename = "thread/sample/cancel")]
    ThreadSampleCancel,
    #[serde(rename = "thread/archive")]
    ThreadArchive,
    #[serde(rename = "thread/delete")]
    ThreadDelete,
    #[serde(rename = "thread/unsubscribe")]
    ThreadUnsubscribe,
    #[serde(rename = "thread/increment_elicitation")]
    ThreadIncrementElicitation,
    #[serde(rename = "thread/decrement_elicitation")]
    ThreadDecrementElicitation,
    #[serde(rename = "thread/name/set")]
    ThreadNameSet,
    #[serde(rename = "thread/prediction/request")]
    ThreadPredictionRequest,
    #[serde(rename = "thread/goal/set")]
    ThreadGoalSet,
    #[serde(rename = "thread/goal/get")]
    ThreadGoalGet,
    #[serde(rename = "thread/goal/clear")]
    ThreadGoalClear,
    #[serde(rename = "thread/queue/add")]
    ThreadQueueAdd,
    #[serde(rename = "thread/queue/list")]
    ThreadQueueList,
    #[serde(rename = "thread/queue/update")]
    ThreadQueueUpdate,
    #[serde(rename = "thread/queue/delete")]
    ThreadQueueDelete,
    #[serde(rename = "thread/queue/reorder")]
    ThreadQueueReorder,
    #[serde(rename = "thread/queue/start")]
    ThreadQueueStart,
    #[serde(rename = "thread/metadata/update")]
    ThreadMetadataUpdate,
    #[serde(rename = "thread/attachment/add")]
    ThreadAttachmentAdd,
    #[serde(rename = "thread/attachment/list")]
    ThreadAttachmentList,
    #[serde(rename = "thread/attachmentOwner/list")]
    ThreadAttachmentOwnerList,
    #[serde(rename = "thread/attachment/remove")]
    ThreadAttachmentRemove,
    #[serde(rename = "thread/section/move")]
    ThreadSectionMove,
    #[serde(rename = "thread/settings/update")]
    ThreadSettingsUpdate,
    #[serde(rename = "thread/memoryMode/set")]
    ThreadMemoryModeSet,
    #[serde(rename = "thread/memory/read")]
    ThreadMemoryRead,
    #[serde(rename = "memory/status")]
    MemoryStatus,
    #[serde(rename = "memory/reset")]
    MemoryReset,
    #[serde(rename = "rollout/compress")]
    RolloutCompress,
    #[serde(rename = "thread/unarchive")]
    ThreadUnarchive,
    #[serde(rename = "thread/compact/start")]
    ThreadCompactStart,
    #[serde(rename = "thread/shellCommand")]
    ThreadShellCommand,
    #[serde(rename = "thread/approveGuardianDeniedAction")]
    ThreadApproveGuardianDeniedAction,
    #[serde(rename = "thread/backgroundTerminals/clean")]
    ThreadBackgroundTerminalsClean,
    #[serde(rename = "thread/backgroundTerminals/list")]
    ThreadBackgroundTerminalsList,
    #[serde(rename = "thread/backgroundTerminals/terminate")]
    ThreadBackgroundTerminalsTerminate,
    #[serde(rename = "thread/revert")]
    ThreadRevert,
    #[serde(rename = "thread/list")]
    ThreadList,
    #[serde(rename = "project/list")]
    ProjectList,
    #[serde(rename = "project/read")]
    ProjectRead,
    #[serde(rename = "project/create")]
    ProjectCreate,
    #[serde(rename = "project/import")]
    ProjectImport,
    #[serde(rename = "project/update")]
    ProjectUpdate,
    #[serde(rename = "project/move")]
    ProjectMove,
    #[serde(rename = "project/delete")]
    ProjectDelete,
    #[serde(rename = "threadSection/list")]
    ThreadSectionList,
    #[serde(rename = "threadSection/create")]
    ThreadSectionCreate,
    #[serde(rename = "threadSection/update")]
    ThreadSectionUpdate,
    #[serde(rename = "threadSection/delete")]
    ThreadSectionDelete,
    #[serde(rename = "thread/search")]
    ThreadSearch,
    #[serde(rename = "thread/searchOccurrences")]
    ThreadSearchOccurrences,
    #[serde(rename = "thread/loaded/list")]
    ThreadLoadedList,
    #[serde(rename = "thread/read")]
    ThreadRead,
    #[serde(rename = "thread/readState/update")]
    ThreadReadStateUpdate,
    #[serde(rename = "thread/turns/list")]
    ThreadTurnsList,
    #[serde(rename = "thread/items/list")]
    ThreadItemsList,
    #[serde(rename = "thread/items/read")]
    ThreadItemsRead,
    #[serde(rename = "thread/guardianReview")]
    ThreadGuardianReview,
    #[serde(rename = "thread/inject_items")]
    ThreadInjectItems,
    #[serde(rename = "skills/list")]
    SkillsList,
    #[serde(rename = "skills/extraRoots/set")]
    SkillsExtraRootsSet,
    #[serde(rename = "hooks/list")]
    HooksList,
    #[serde(rename = "marketplace/add")]
    MarketplaceAdd,
    #[serde(rename = "marketplace/remove")]
    MarketplaceRemove,
    #[serde(rename = "marketplace/upgrade")]
    MarketplaceUpgrade,
    #[serde(rename = "plugin/list")]
    PluginList,
    #[serde(rename = "plugin/search")]
    PluginSearch,
    #[serde(rename = "plugin/installed")]
    PluginInstalled,
    #[serde(rename = "plugin/reconcile")]
    PluginReconcile,
    #[serde(rename = "plugin/read")]
    PluginRead,
    #[serde(rename = "plugin/skill/read")]
    PluginSkillRead,
    #[serde(rename = "plugin/share/save")]
    PluginShareSave,
    #[serde(rename = "plugin/share/updateTargets")]
    PluginShareUpdateTargets,
    #[serde(rename = "plugin/share/list")]
    PluginShareList,
    #[serde(rename = "plugin/share/checkout")]
    PluginShareCheckout,
    #[serde(rename = "plugin/share/delete")]
    PluginShareDelete,
    #[serde(rename = "app/read")]
    AppRead,
    #[serde(rename = "app/list")]
    AppList,
    #[serde(rename = "app/installed")]
    AppInstalled,
    #[serde(rename = "fs/readFile")]
    FsReadFile,
    #[serde(rename = "fs/writeFile")]
    FsWriteFile,
    #[serde(rename = "fs/createDirectory")]
    FsCreateDirectory,
    #[serde(rename = "fs/getMetadata")]
    FsGetMetadata,
    #[serde(rename = "fs/readDirectory")]
    FsReadDirectory,
    #[serde(rename = "fs/remove")]
    FsRemove,
    #[serde(rename = "fs/copy")]
    FsCopy,
    #[serde(rename = "fs/watch")]
    FsWatch,
    #[serde(rename = "fs/unwatch")]
    FsUnwatch,
    #[serde(rename = "skills/config/write")]
    SkillsConfigWrite,
    #[serde(rename = "plugin/install")]
    PluginInstall,
    #[serde(rename = "plugin/uninstall")]
    PluginUninstall,
    #[serde(rename = "turn/start")]
    TurnStart,
    #[serde(rename = "turn/settings/update")]
    TurnSettingsUpdate,
    #[serde(rename = "turn/steer")]
    TurnSteer,
    #[serde(rename = "turn/interrupt")]
    TurnInterrupt,
    #[serde(rename = "thread/realtime/start")]
    ThreadRealtimeStart,
    #[serde(rename = "thread/realtime/appendAudio")]
    ThreadRealtimeAppendAudio,
    #[serde(rename = "thread/realtime/appendText")]
    ThreadRealtimeAppendText,
    #[serde(rename = "thread/realtime/appendSpeech")]
    ThreadRealtimeAppendSpeech,
    #[serde(rename = "thread/realtime/stop")]
    ThreadRealtimeStop,
    #[serde(rename = "thread/timeline/list")]
    ThreadTimelineList,
    #[serde(rename = "thread/realtime/listVoices")]
    ThreadRealtimeListVoices,
    #[serde(rename = "review/start")]
    ReviewStart,
    #[serde(rename = "model/list")]
    ModelList,
    #[serde(rename = "account/gatewayOAuth/read")]
    AccountGatewayOAuthRead,
    #[serde(rename = "account/gatewayOAuth/login")]
    AccountGatewayOAuthLogin,
    #[serde(rename = "account/gatewayOAuth/cancel")]
    AccountGatewayOAuthCancel,
    #[serde(rename = "modelProvider/capabilities/read")]
    ModelProviderCapabilitiesRead,
    #[serde(rename = "experimentalFeature/list")]
    ExperimentalFeatureList,
    #[serde(rename = "permissionProfile/list")]
    PermissionProfileList,
    #[serde(rename = "experimentalFeature/enablement/set")]
    ExperimentalFeatureEnablementSet,
    #[serde(rename = "remoteControl/enable")]
    RemoteControlEnable,
    #[serde(rename = "remoteControl/disable")]
    RemoteControlDisable,
    #[serde(rename = "remoteControl/status/read")]
    RemoteControlStatusRead,
    #[serde(rename = "remoteControl/pairing/start")]
    RemoteControlPairingStart,
    #[serde(rename = "remoteControl/pairing/status")]
    RemoteControlPairingStatus,
    #[serde(rename = "remoteControl/client/list")]
    RemoteControlClientList,
    #[serde(rename = "remoteControl/client/revoke")]
    RemoteControlClientRevoke,
    #[serde(rename = "collaborationMode/list")]
    CollaborationModeList,
    #[serde(rename = "mock/experimentalMethod")]
    MockExperimentalMethod,
    #[serde(rename = "environment/add")]
    EnvironmentAdd,
    #[serde(rename = "environment/info")]
    EnvironmentInfo,
    #[serde(rename = "environment/status")]
    EnvironmentStatus,
    #[serde(rename = "mcpServer/oauth/login")]
    McpServerOauthLogin,
    #[serde(rename = "config/mcpServer/reload")]
    ConfigMcpServerReload,
    #[serde(rename = "mcpServerStatus/list")]
    McpServerStatusList,
    #[serde(rename = "mcpServer/resource/read")]
    McpServerResourceRead,
    #[serde(rename = "mcpServer/event/stream/start")]
    McpServerEventStreamStart,
    #[serde(rename = "mcpServer/event/stream/stop")]
    McpServerEventStreamStop,
    #[serde(rename = "mcpServer/tool/call")]
    McpServerToolCall,
    #[serde(rename = "windowsSandbox/setupStart")]
    WindowsSandboxSetupStart,
    #[serde(rename = "windowsSandbox/readiness")]
    WindowsSandboxReadiness,
    #[serde(rename = "account/login/start")]
    AccountLoginStart,
    #[serde(rename = "account/bedrock/discover")]
    AccountBedrockDiscover,
    #[serde(rename = "account/bedrock/setup")]
    AccountBedrockSetup,
    #[serde(rename = "account/gcp/setup")]
    AccountGcpSetup,
    #[serde(rename = "account/bedrock/checkGovCloudRequirements")]
    AccountBedrockCheckGovCloudRequirements,
    #[serde(rename = "account/login/cancel")]
    AccountLoginCancel,
    #[serde(rename = "accountSession/list")]
    AccountSessionList,
    #[serde(rename = "accountSession/switch")]
    AccountSessionSwitch,
    #[serde(rename = "account/logout")]
    AccountLogout,
    #[serde(rename = "account/rateLimits/read")]
    AccountRateLimitsRead,
    #[serde(rename = "account/rateLimitResetCredit/consume")]
    AccountRateLimitResetCreditConsume,
    #[serde(rename = "account/usage/read")]
    AccountUsageRead,
    #[serde(rename = "account/workspaceMessages/read")]
    AccountWorkspaceMessagesRead,
    #[serde(rename = "account/sendAddCreditsNudgeEmail")]
    AccountSendAddCreditsNudgeEmail,
    #[serde(rename = "feedback/upload")]
    FeedbackUpload,
    #[serde(rename = "feedback/save")]
    FeedbackSave,
    #[serde(rename = "command/exec")]
    CommandExec,
    #[serde(rename = "command/exec/write")]
    CommandExecWrite,
    #[serde(rename = "command/exec/terminate")]
    CommandExecTerminate,
    #[serde(rename = "command/exec/resize")]
    CommandExecResize,
    #[serde(rename = "process/spawn")]
    ProcessSpawn,
    #[serde(rename = "process/writeStdin")]
    ProcessWriteStdin,
    #[serde(rename = "process/kill")]
    ProcessKill,
    #[serde(rename = "process/resizePty")]
    ProcessResizePty,
    #[serde(rename = "config/read")]
    ConfigRead,
    #[serde(rename = "externalAgentConfig/detect")]
    ExternalAgentConfigDetect,
    #[serde(rename = "externalAgentConfig/import")]
    ExternalAgentConfigImport,
    #[serde(rename = "externalAgentConfig/import/recordHistory")]
    ExternalAgentConfigImportRecordHistory,
    #[serde(rename = "externalAgentConfig/import/readHistories")]
    ExternalAgentConfigImportReadHistories,
    #[serde(rename = "config/value/write")]
    ConfigValueWrite,
    #[serde(rename = "config/batchWrite")]
    ConfigBatchWrite,
    #[serde(rename = "configRequirements/read")]
    ConfigRequirementsRead,
    #[serde(rename = "account/read")]
    AccountRead,
    #[serde(rename = "fuzzyFileSearch")]
    FuzzyFileSearch,
    #[serde(rename = "fuzzyFileSearch/sessionStart")]
    FuzzyFileSearchSessionStart,
    #[serde(rename = "fuzzyFileSearch/sessionUpdate")]
    FuzzyFileSearchSessionUpdate,
    #[serde(rename = "fuzzyFileSearch/sessionStop")]
    FuzzyFileSearchSessionStop,
}
impl ClientRequestMethod {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "initialize" => Some(Self::Initialize),
            "server/diagnostics" => Some(Self::ServerDiagnostics),
            "userVerification/status" => Some(Self::UserVerificationStatus),
            "userVerification/enroll" => Some(Self::UserVerificationEnroll),
            "userVerification/delete" => Some(Self::UserVerificationDelete),
            "userVerification/verify" => Some(Self::UserVerificationVerify),
            "userVerification/cancel" => Some(Self::UserVerificationCancel),
            "thread/start" => Some(Self::ThreadStart),
            "thread/resume" => Some(Self::ThreadResume),
            "thread/fork" => Some(Self::ThreadFork),
            "thread/sample" => Some(Self::ThreadSample),
            "thread/sample/cancel" => Some(Self::ThreadSampleCancel),
            "thread/archive" => Some(Self::ThreadArchive),
            "thread/delete" => Some(Self::ThreadDelete),
            "thread/unsubscribe" => Some(Self::ThreadUnsubscribe),
            "thread/increment_elicitation" => Some(Self::ThreadIncrementElicitation),
            "thread/decrement_elicitation" => Some(Self::ThreadDecrementElicitation),
            "thread/name/set" => Some(Self::ThreadNameSet),
            "thread/prediction/request" => Some(Self::ThreadPredictionRequest),
            "thread/goal/set" => Some(Self::ThreadGoalSet),
            "thread/goal/get" => Some(Self::ThreadGoalGet),
            "thread/goal/clear" => Some(Self::ThreadGoalClear),
            "thread/queue/add" => Some(Self::ThreadQueueAdd),
            "thread/queue/list" => Some(Self::ThreadQueueList),
            "thread/queue/update" => Some(Self::ThreadQueueUpdate),
            "thread/queue/delete" => Some(Self::ThreadQueueDelete),
            "thread/queue/reorder" => Some(Self::ThreadQueueReorder),
            "thread/queue/start" => Some(Self::ThreadQueueStart),
            "thread/metadata/update" => Some(Self::ThreadMetadataUpdate),
            "thread/attachment/add" => Some(Self::ThreadAttachmentAdd),
            "thread/attachment/list" => Some(Self::ThreadAttachmentList),
            "thread/attachmentOwner/list" => Some(Self::ThreadAttachmentOwnerList),
            "thread/attachment/remove" => Some(Self::ThreadAttachmentRemove),
            "thread/section/move" => Some(Self::ThreadSectionMove),
            "thread/settings/update" => Some(Self::ThreadSettingsUpdate),
            "thread/memoryMode/set" => Some(Self::ThreadMemoryModeSet),
            "thread/memory/read" => Some(Self::ThreadMemoryRead),
            "memory/status" => Some(Self::MemoryStatus),
            "memory/reset" => Some(Self::MemoryReset),
            "rollout/compress" => Some(Self::RolloutCompress),
            "thread/unarchive" => Some(Self::ThreadUnarchive),
            "thread/compact/start" => Some(Self::ThreadCompactStart),
            "thread/shellCommand" => Some(Self::ThreadShellCommand),
            "thread/approveGuardianDeniedAction" => Some(Self::ThreadApproveGuardianDeniedAction),
            "thread/backgroundTerminals/clean" => Some(Self::ThreadBackgroundTerminalsClean),
            "thread/backgroundTerminals/list" => Some(Self::ThreadBackgroundTerminalsList),
            "thread/backgroundTerminals/terminate" => {
                Some(Self::ThreadBackgroundTerminalsTerminate)
            }
            "thread/revert" => Some(Self::ThreadRevert),
            "thread/list" => Some(Self::ThreadList),
            "project/list" => Some(Self::ProjectList),
            "project/read" => Some(Self::ProjectRead),
            "project/create" => Some(Self::ProjectCreate),
            "project/import" => Some(Self::ProjectImport),
            "project/update" => Some(Self::ProjectUpdate),
            "project/move" => Some(Self::ProjectMove),
            "project/delete" => Some(Self::ProjectDelete),
            "threadSection/list" => Some(Self::ThreadSectionList),
            "threadSection/create" => Some(Self::ThreadSectionCreate),
            "threadSection/update" => Some(Self::ThreadSectionUpdate),
            "threadSection/delete" => Some(Self::ThreadSectionDelete),
            "thread/search" => Some(Self::ThreadSearch),
            "thread/searchOccurrences" => Some(Self::ThreadSearchOccurrences),
            "thread/loaded/list" => Some(Self::ThreadLoadedList),
            "thread/read" => Some(Self::ThreadRead),
            "thread/readState/update" => Some(Self::ThreadReadStateUpdate),
            "thread/turns/list" => Some(Self::ThreadTurnsList),
            "thread/items/list" => Some(Self::ThreadItemsList),
            "thread/items/read" => Some(Self::ThreadItemsRead),
            "thread/guardianReview" => Some(Self::ThreadGuardianReview),
            "thread/inject_items" => Some(Self::ThreadInjectItems),
            "skills/list" => Some(Self::SkillsList),
            "skills/extraRoots/set" => Some(Self::SkillsExtraRootsSet),
            "hooks/list" => Some(Self::HooksList),
            "marketplace/add" => Some(Self::MarketplaceAdd),
            "marketplace/remove" => Some(Self::MarketplaceRemove),
            "marketplace/upgrade" => Some(Self::MarketplaceUpgrade),
            "plugin/list" => Some(Self::PluginList),
            "plugin/search" => Some(Self::PluginSearch),
            "plugin/installed" => Some(Self::PluginInstalled),
            "plugin/reconcile" => Some(Self::PluginReconcile),
            "plugin/read" => Some(Self::PluginRead),
            "plugin/skill/read" => Some(Self::PluginSkillRead),
            "plugin/share/save" => Some(Self::PluginShareSave),
            "plugin/share/updateTargets" => Some(Self::PluginShareUpdateTargets),
            "plugin/share/list" => Some(Self::PluginShareList),
            "plugin/share/checkout" => Some(Self::PluginShareCheckout),
            "plugin/share/delete" => Some(Self::PluginShareDelete),
            "app/read" => Some(Self::AppRead),
            "app/list" => Some(Self::AppList),
            "app/installed" => Some(Self::AppInstalled),
            "fs/readFile" => Some(Self::FsReadFile),
            "fs/writeFile" => Some(Self::FsWriteFile),
            "fs/createDirectory" => Some(Self::FsCreateDirectory),
            "fs/getMetadata" => Some(Self::FsGetMetadata),
            "fs/readDirectory" => Some(Self::FsReadDirectory),
            "fs/remove" => Some(Self::FsRemove),
            "fs/copy" => Some(Self::FsCopy),
            "fs/watch" => Some(Self::FsWatch),
            "fs/unwatch" => Some(Self::FsUnwatch),
            "skills/config/write" => Some(Self::SkillsConfigWrite),
            "plugin/install" => Some(Self::PluginInstall),
            "plugin/uninstall" => Some(Self::PluginUninstall),
            "turn/start" => Some(Self::TurnStart),
            "turn/settings/update" => Some(Self::TurnSettingsUpdate),
            "turn/steer" => Some(Self::TurnSteer),
            "turn/interrupt" => Some(Self::TurnInterrupt),
            "thread/realtime/start" => Some(Self::ThreadRealtimeStart),
            "thread/realtime/appendAudio" => Some(Self::ThreadRealtimeAppendAudio),
            "thread/realtime/appendText" => Some(Self::ThreadRealtimeAppendText),
            "thread/realtime/appendSpeech" => Some(Self::ThreadRealtimeAppendSpeech),
            "thread/realtime/stop" => Some(Self::ThreadRealtimeStop),
            "thread/timeline/list" => Some(Self::ThreadTimelineList),
            "thread/realtime/listVoices" => Some(Self::ThreadRealtimeListVoices),
            "review/start" => Some(Self::ReviewStart),
            "model/list" => Some(Self::ModelList),
            "account/gatewayOAuth/read" => Some(Self::AccountGatewayOAuthRead),
            "account/gatewayOAuth/login" => Some(Self::AccountGatewayOAuthLogin),
            "account/gatewayOAuth/cancel" => Some(Self::AccountGatewayOAuthCancel),
            "modelProvider/capabilities/read" => Some(Self::ModelProviderCapabilitiesRead),
            "experimentalFeature/list" => Some(Self::ExperimentalFeatureList),
            "permissionProfile/list" => Some(Self::PermissionProfileList),
            "experimentalFeature/enablement/set" => Some(Self::ExperimentalFeatureEnablementSet),
            "remoteControl/enable" => Some(Self::RemoteControlEnable),
            "remoteControl/disable" => Some(Self::RemoteControlDisable),
            "remoteControl/status/read" => Some(Self::RemoteControlStatusRead),
            "remoteControl/pairing/start" => Some(Self::RemoteControlPairingStart),
            "remoteControl/pairing/status" => Some(Self::RemoteControlPairingStatus),
            "remoteControl/client/list" => Some(Self::RemoteControlClientList),
            "remoteControl/client/revoke" => Some(Self::RemoteControlClientRevoke),
            "collaborationMode/list" => Some(Self::CollaborationModeList),
            "mock/experimentalMethod" => Some(Self::MockExperimentalMethod),
            "environment/add" => Some(Self::EnvironmentAdd),
            "environment/info" => Some(Self::EnvironmentInfo),
            "environment/status" => Some(Self::EnvironmentStatus),
            "mcpServer/oauth/login" => Some(Self::McpServerOauthLogin),
            "config/mcpServer/reload" => Some(Self::ConfigMcpServerReload),
            "mcpServerStatus/list" => Some(Self::McpServerStatusList),
            "mcpServer/resource/read" => Some(Self::McpServerResourceRead),
            "mcpServer/event/stream/start" => Some(Self::McpServerEventStreamStart),
            "mcpServer/event/stream/stop" => Some(Self::McpServerEventStreamStop),
            "mcpServer/tool/call" => Some(Self::McpServerToolCall),
            "windowsSandbox/setupStart" => Some(Self::WindowsSandboxSetupStart),
            "windowsSandbox/readiness" => Some(Self::WindowsSandboxReadiness),
            "account/login/start" => Some(Self::AccountLoginStart),
            "account/bedrock/discover" => Some(Self::AccountBedrockDiscover),
            "account/bedrock/setup" => Some(Self::AccountBedrockSetup),
            "account/gcp/setup" => Some(Self::AccountGcpSetup),
            "account/bedrock/checkGovCloudRequirements" => {
                Some(Self::AccountBedrockCheckGovCloudRequirements)
            }
            "account/login/cancel" => Some(Self::AccountLoginCancel),
            "accountSession/list" => Some(Self::AccountSessionList),
            "accountSession/switch" => Some(Self::AccountSessionSwitch),
            "account/logout" => Some(Self::AccountLogout),
            "account/rateLimits/read" => Some(Self::AccountRateLimitsRead),
            "account/rateLimitResetCredit/consume" => {
                Some(Self::AccountRateLimitResetCreditConsume)
            }
            "account/usage/read" => Some(Self::AccountUsageRead),
            "account/workspaceMessages/read" => Some(Self::AccountWorkspaceMessagesRead),
            "account/sendAddCreditsNudgeEmail" => Some(Self::AccountSendAddCreditsNudgeEmail),
            "feedback/upload" => Some(Self::FeedbackUpload),
            "feedback/save" => Some(Self::FeedbackSave),
            "command/exec" => Some(Self::CommandExec),
            "command/exec/write" => Some(Self::CommandExecWrite),
            "command/exec/terminate" => Some(Self::CommandExecTerminate),
            "command/exec/resize" => Some(Self::CommandExecResize),
            "process/spawn" => Some(Self::ProcessSpawn),
            "process/writeStdin" => Some(Self::ProcessWriteStdin),
            "process/kill" => Some(Self::ProcessKill),
            "process/resizePty" => Some(Self::ProcessResizePty),
            "config/read" => Some(Self::ConfigRead),
            "externalAgentConfig/detect" => Some(Self::ExternalAgentConfigDetect),
            "externalAgentConfig/import" => Some(Self::ExternalAgentConfigImport),
            "externalAgentConfig/import/recordHistory" => {
                Some(Self::ExternalAgentConfigImportRecordHistory)
            }
            "externalAgentConfig/import/readHistories" => {
                Some(Self::ExternalAgentConfigImportReadHistories)
            }
            "config/value/write" => Some(Self::ConfigValueWrite),
            "config/batchWrite" => Some(Self::ConfigBatchWrite),
            "configRequirements/read" => Some(Self::ConfigRequirementsRead),
            "account/read" => Some(Self::AccountRead),
            "fuzzyFileSearch" => Some(Self::FuzzyFileSearch),
            "fuzzyFileSearch/sessionStart" => Some(Self::FuzzyFileSearchSessionStart),
            "fuzzyFileSearch/sessionUpdate" => Some(Self::FuzzyFileSearchSessionUpdate),
            "fuzzyFileSearch/sessionStop" => Some(Self::FuzzyFileSearchSessionStop),
            _ => None,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::ServerDiagnostics => "server/diagnostics",
            Self::UserVerificationStatus => "userVerification/status",
            Self::UserVerificationEnroll => "userVerification/enroll",
            Self::UserVerificationDelete => "userVerification/delete",
            Self::UserVerificationVerify => "userVerification/verify",
            Self::UserVerificationCancel => "userVerification/cancel",
            Self::ThreadStart => "thread/start",
            Self::ThreadResume => "thread/resume",
            Self::ThreadFork => "thread/fork",
            Self::ThreadSample => "thread/sample",
            Self::ThreadSampleCancel => "thread/sample/cancel",
            Self::ThreadArchive => "thread/archive",
            Self::ThreadDelete => "thread/delete",
            Self::ThreadUnsubscribe => "thread/unsubscribe",
            Self::ThreadIncrementElicitation => "thread/increment_elicitation",
            Self::ThreadDecrementElicitation => "thread/decrement_elicitation",
            Self::ThreadNameSet => "thread/name/set",
            Self::ThreadPredictionRequest => "thread/prediction/request",
            Self::ThreadGoalSet => "thread/goal/set",
            Self::ThreadGoalGet => "thread/goal/get",
            Self::ThreadGoalClear => "thread/goal/clear",
            Self::ThreadQueueAdd => "thread/queue/add",
            Self::ThreadQueueList => "thread/queue/list",
            Self::ThreadQueueUpdate => "thread/queue/update",
            Self::ThreadQueueDelete => "thread/queue/delete",
            Self::ThreadQueueReorder => "thread/queue/reorder",
            Self::ThreadQueueStart => "thread/queue/start",
            Self::ThreadMetadataUpdate => "thread/metadata/update",
            Self::ThreadAttachmentAdd => "thread/attachment/add",
            Self::ThreadAttachmentList => "thread/attachment/list",
            Self::ThreadAttachmentOwnerList => "thread/attachmentOwner/list",
            Self::ThreadAttachmentRemove => "thread/attachment/remove",
            Self::ThreadSectionMove => "thread/section/move",
            Self::ThreadSettingsUpdate => "thread/settings/update",
            Self::ThreadMemoryModeSet => "thread/memoryMode/set",
            Self::ThreadMemoryRead => "thread/memory/read",
            Self::MemoryStatus => "memory/status",
            Self::MemoryReset => "memory/reset",
            Self::RolloutCompress => "rollout/compress",
            Self::ThreadUnarchive => "thread/unarchive",
            Self::ThreadCompactStart => "thread/compact/start",
            Self::ThreadShellCommand => "thread/shellCommand",
            Self::ThreadApproveGuardianDeniedAction => "thread/approveGuardianDeniedAction",
            Self::ThreadBackgroundTerminalsClean => "thread/backgroundTerminals/clean",
            Self::ThreadBackgroundTerminalsList => "thread/backgroundTerminals/list",
            Self::ThreadBackgroundTerminalsTerminate => "thread/backgroundTerminals/terminate",
            Self::ThreadRevert => "thread/revert",
            Self::ThreadList => "thread/list",
            Self::ProjectList => "project/list",
            Self::ProjectRead => "project/read",
            Self::ProjectCreate => "project/create",
            Self::ProjectImport => "project/import",
            Self::ProjectUpdate => "project/update",
            Self::ProjectMove => "project/move",
            Self::ProjectDelete => "project/delete",
            Self::ThreadSectionList => "threadSection/list",
            Self::ThreadSectionCreate => "threadSection/create",
            Self::ThreadSectionUpdate => "threadSection/update",
            Self::ThreadSectionDelete => "threadSection/delete",
            Self::ThreadSearch => "thread/search",
            Self::ThreadSearchOccurrences => "thread/searchOccurrences",
            Self::ThreadLoadedList => "thread/loaded/list",
            Self::ThreadRead => "thread/read",
            Self::ThreadReadStateUpdate => "thread/readState/update",
            Self::ThreadTurnsList => "thread/turns/list",
            Self::ThreadItemsList => "thread/items/list",
            Self::ThreadItemsRead => "thread/items/read",
            Self::ThreadGuardianReview => "thread/guardianReview",
            Self::ThreadInjectItems => "thread/inject_items",
            Self::SkillsList => "skills/list",
            Self::SkillsExtraRootsSet => "skills/extraRoots/set",
            Self::HooksList => "hooks/list",
            Self::MarketplaceAdd => "marketplace/add",
            Self::MarketplaceRemove => "marketplace/remove",
            Self::MarketplaceUpgrade => "marketplace/upgrade",
            Self::PluginList => "plugin/list",
            Self::PluginSearch => "plugin/search",
            Self::PluginInstalled => "plugin/installed",
            Self::PluginReconcile => "plugin/reconcile",
            Self::PluginRead => "plugin/read",
            Self::PluginSkillRead => "plugin/skill/read",
            Self::PluginShareSave => "plugin/share/save",
            Self::PluginShareUpdateTargets => "plugin/share/updateTargets",
            Self::PluginShareList => "plugin/share/list",
            Self::PluginShareCheckout => "plugin/share/checkout",
            Self::PluginShareDelete => "plugin/share/delete",
            Self::AppRead => "app/read",
            Self::AppList => "app/list",
            Self::AppInstalled => "app/installed",
            Self::FsReadFile => "fs/readFile",
            Self::FsWriteFile => "fs/writeFile",
            Self::FsCreateDirectory => "fs/createDirectory",
            Self::FsGetMetadata => "fs/getMetadata",
            Self::FsReadDirectory => "fs/readDirectory",
            Self::FsRemove => "fs/remove",
            Self::FsCopy => "fs/copy",
            Self::FsWatch => "fs/watch",
            Self::FsUnwatch => "fs/unwatch",
            Self::SkillsConfigWrite => "skills/config/write",
            Self::PluginInstall => "plugin/install",
            Self::PluginUninstall => "plugin/uninstall",
            Self::TurnStart => "turn/start",
            Self::TurnSettingsUpdate => "turn/settings/update",
            Self::TurnSteer => "turn/steer",
            Self::TurnInterrupt => "turn/interrupt",
            Self::ThreadRealtimeStart => "thread/realtime/start",
            Self::ThreadRealtimeAppendAudio => "thread/realtime/appendAudio",
            Self::ThreadRealtimeAppendText => "thread/realtime/appendText",
            Self::ThreadRealtimeAppendSpeech => "thread/realtime/appendSpeech",
            Self::ThreadRealtimeStop => "thread/realtime/stop",
            Self::ThreadTimelineList => "thread/timeline/list",
            Self::ThreadRealtimeListVoices => "thread/realtime/listVoices",
            Self::ReviewStart => "review/start",
            Self::ModelList => "model/list",
            Self::AccountGatewayOAuthRead => "account/gatewayOAuth/read",
            Self::AccountGatewayOAuthLogin => "account/gatewayOAuth/login",
            Self::AccountGatewayOAuthCancel => "account/gatewayOAuth/cancel",
            Self::ModelProviderCapabilitiesRead => "modelProvider/capabilities/read",
            Self::ExperimentalFeatureList => "experimentalFeature/list",
            Self::PermissionProfileList => "permissionProfile/list",
            Self::ExperimentalFeatureEnablementSet => "experimentalFeature/enablement/set",
            Self::RemoteControlEnable => "remoteControl/enable",
            Self::RemoteControlDisable => "remoteControl/disable",
            Self::RemoteControlStatusRead => "remoteControl/status/read",
            Self::RemoteControlPairingStart => "remoteControl/pairing/start",
            Self::RemoteControlPairingStatus => "remoteControl/pairing/status",
            Self::RemoteControlClientList => "remoteControl/client/list",
            Self::RemoteControlClientRevoke => "remoteControl/client/revoke",
            Self::CollaborationModeList => "collaborationMode/list",
            Self::MockExperimentalMethod => "mock/experimentalMethod",
            Self::EnvironmentAdd => "environment/add",
            Self::EnvironmentInfo => "environment/info",
            Self::EnvironmentStatus => "environment/status",
            Self::McpServerOauthLogin => "mcpServer/oauth/login",
            Self::ConfigMcpServerReload => "config/mcpServer/reload",
            Self::McpServerStatusList => "mcpServerStatus/list",
            Self::McpServerResourceRead => "mcpServer/resource/read",
            Self::McpServerEventStreamStart => "mcpServer/event/stream/start",
            Self::McpServerEventStreamStop => "mcpServer/event/stream/stop",
            Self::McpServerToolCall => "mcpServer/tool/call",
            Self::WindowsSandboxSetupStart => "windowsSandbox/setupStart",
            Self::WindowsSandboxReadiness => "windowsSandbox/readiness",
            Self::AccountLoginStart => "account/login/start",
            Self::AccountBedrockDiscover => "account/bedrock/discover",
            Self::AccountBedrockSetup => "account/bedrock/setup",
            Self::AccountGcpSetup => "account/gcp/setup",
            Self::AccountBedrockCheckGovCloudRequirements => {
                "account/bedrock/checkGovCloudRequirements"
            }
            Self::AccountLoginCancel => "account/login/cancel",
            Self::AccountSessionList => "accountSession/list",
            Self::AccountSessionSwitch => "accountSession/switch",
            Self::AccountLogout => "account/logout",
            Self::AccountRateLimitsRead => "account/rateLimits/read",
            Self::AccountRateLimitResetCreditConsume => "account/rateLimitResetCredit/consume",
            Self::AccountUsageRead => "account/usage/read",
            Self::AccountWorkspaceMessagesRead => "account/workspaceMessages/read",
            Self::AccountSendAddCreditsNudgeEmail => "account/sendAddCreditsNudgeEmail",
            Self::FeedbackUpload => "feedback/upload",
            Self::FeedbackSave => "feedback/save",
            Self::CommandExec => "command/exec",
            Self::CommandExecWrite => "command/exec/write",
            Self::CommandExecTerminate => "command/exec/terminate",
            Self::CommandExecResize => "command/exec/resize",
            Self::ProcessSpawn => "process/spawn",
            Self::ProcessWriteStdin => "process/writeStdin",
            Self::ProcessKill => "process/kill",
            Self::ProcessResizePty => "process/resizePty",
            Self::ConfigRead => "config/read",
            Self::ExternalAgentConfigDetect => "externalAgentConfig/detect",
            Self::ExternalAgentConfigImport => "externalAgentConfig/import",
            Self::ExternalAgentConfigImportRecordHistory => {
                "externalAgentConfig/import/recordHistory"
            }
            Self::ExternalAgentConfigImportReadHistories => {
                "externalAgentConfig/import/readHistories"
            }
            Self::ConfigValueWrite => "config/value/write",
            Self::ConfigBatchWrite => "config/batchWrite",
            Self::ConfigRequirementsRead => "configRequirements/read",
            Self::AccountRead => "account/read",
            Self::FuzzyFileSearch => "fuzzyFileSearch",
            Self::FuzzyFileSearchSessionStart => "fuzzyFileSearch/sessionStart",
            Self::FuzzyFileSearchSessionUpdate => "fuzzyFileSearch/sessionUpdate",
            Self::FuzzyFileSearchSessionStop => "fuzzyFileSearch/sessionStop",
        }
    }
    pub const fn all() -> &'static [Self] {
        &[
            Self::Initialize,
            Self::ServerDiagnostics,
            Self::UserVerificationStatus,
            Self::UserVerificationEnroll,
            Self::UserVerificationDelete,
            Self::UserVerificationVerify,
            Self::UserVerificationCancel,
            Self::ThreadStart,
            Self::ThreadResume,
            Self::ThreadFork,
            Self::ThreadSample,
            Self::ThreadSampleCancel,
            Self::ThreadArchive,
            Self::ThreadDelete,
            Self::ThreadUnsubscribe,
            Self::ThreadIncrementElicitation,
            Self::ThreadDecrementElicitation,
            Self::ThreadNameSet,
            Self::ThreadPredictionRequest,
            Self::ThreadGoalSet,
            Self::ThreadGoalGet,
            Self::ThreadGoalClear,
            Self::ThreadQueueAdd,
            Self::ThreadQueueList,
            Self::ThreadQueueUpdate,
            Self::ThreadQueueDelete,
            Self::ThreadQueueReorder,
            Self::ThreadQueueStart,
            Self::ThreadMetadataUpdate,
            Self::ThreadAttachmentAdd,
            Self::ThreadAttachmentList,
            Self::ThreadAttachmentOwnerList,
            Self::ThreadAttachmentRemove,
            Self::ThreadSectionMove,
            Self::ThreadSettingsUpdate,
            Self::ThreadMemoryModeSet,
            Self::ThreadMemoryRead,
            Self::MemoryStatus,
            Self::MemoryReset,
            Self::RolloutCompress,
            Self::ThreadUnarchive,
            Self::ThreadCompactStart,
            Self::ThreadShellCommand,
            Self::ThreadApproveGuardianDeniedAction,
            Self::ThreadBackgroundTerminalsClean,
            Self::ThreadBackgroundTerminalsList,
            Self::ThreadBackgroundTerminalsTerminate,
            Self::ThreadRevert,
            Self::ThreadList,
            Self::ProjectList,
            Self::ProjectRead,
            Self::ProjectCreate,
            Self::ProjectImport,
            Self::ProjectUpdate,
            Self::ProjectMove,
            Self::ProjectDelete,
            Self::ThreadSectionList,
            Self::ThreadSectionCreate,
            Self::ThreadSectionUpdate,
            Self::ThreadSectionDelete,
            Self::ThreadSearch,
            Self::ThreadSearchOccurrences,
            Self::ThreadLoadedList,
            Self::ThreadRead,
            Self::ThreadReadStateUpdate,
            Self::ThreadTurnsList,
            Self::ThreadItemsList,
            Self::ThreadItemsRead,
            Self::ThreadGuardianReview,
            Self::ThreadInjectItems,
            Self::SkillsList,
            Self::SkillsExtraRootsSet,
            Self::HooksList,
            Self::MarketplaceAdd,
            Self::MarketplaceRemove,
            Self::MarketplaceUpgrade,
            Self::PluginList,
            Self::PluginSearch,
            Self::PluginInstalled,
            Self::PluginReconcile,
            Self::PluginRead,
            Self::PluginSkillRead,
            Self::PluginShareSave,
            Self::PluginShareUpdateTargets,
            Self::PluginShareList,
            Self::PluginShareCheckout,
            Self::PluginShareDelete,
            Self::AppRead,
            Self::AppList,
            Self::AppInstalled,
            Self::FsReadFile,
            Self::FsWriteFile,
            Self::FsCreateDirectory,
            Self::FsGetMetadata,
            Self::FsReadDirectory,
            Self::FsRemove,
            Self::FsCopy,
            Self::FsWatch,
            Self::FsUnwatch,
            Self::SkillsConfigWrite,
            Self::PluginInstall,
            Self::PluginUninstall,
            Self::TurnStart,
            Self::TurnSettingsUpdate,
            Self::TurnSteer,
            Self::TurnInterrupt,
            Self::ThreadRealtimeStart,
            Self::ThreadRealtimeAppendAudio,
            Self::ThreadRealtimeAppendText,
            Self::ThreadRealtimeAppendSpeech,
            Self::ThreadRealtimeStop,
            Self::ThreadTimelineList,
            Self::ThreadRealtimeListVoices,
            Self::ReviewStart,
            Self::ModelList,
            Self::AccountGatewayOAuthRead,
            Self::AccountGatewayOAuthLogin,
            Self::AccountGatewayOAuthCancel,
            Self::ModelProviderCapabilitiesRead,
            Self::ExperimentalFeatureList,
            Self::PermissionProfileList,
            Self::ExperimentalFeatureEnablementSet,
            Self::RemoteControlEnable,
            Self::RemoteControlDisable,
            Self::RemoteControlStatusRead,
            Self::RemoteControlPairingStart,
            Self::RemoteControlPairingStatus,
            Self::RemoteControlClientList,
            Self::RemoteControlClientRevoke,
            Self::CollaborationModeList,
            Self::MockExperimentalMethod,
            Self::EnvironmentAdd,
            Self::EnvironmentInfo,
            Self::EnvironmentStatus,
            Self::McpServerOauthLogin,
            Self::ConfigMcpServerReload,
            Self::McpServerStatusList,
            Self::McpServerResourceRead,
            Self::McpServerEventStreamStart,
            Self::McpServerEventStreamStop,
            Self::McpServerToolCall,
            Self::WindowsSandboxSetupStart,
            Self::WindowsSandboxReadiness,
            Self::AccountLoginStart,
            Self::AccountBedrockDiscover,
            Self::AccountBedrockSetup,
            Self::AccountGcpSetup,
            Self::AccountBedrockCheckGovCloudRequirements,
            Self::AccountLoginCancel,
            Self::AccountSessionList,
            Self::AccountSessionSwitch,
            Self::AccountLogout,
            Self::AccountRateLimitsRead,
            Self::AccountRateLimitResetCreditConsume,
            Self::AccountUsageRead,
            Self::AccountWorkspaceMessagesRead,
            Self::AccountSendAddCreditsNudgeEmail,
            Self::FeedbackUpload,
            Self::FeedbackSave,
            Self::CommandExec,
            Self::CommandExecWrite,
            Self::CommandExecTerminate,
            Self::CommandExecResize,
            Self::ProcessSpawn,
            Self::ProcessWriteStdin,
            Self::ProcessKill,
            Self::ProcessResizePty,
            Self::ConfigRead,
            Self::ExternalAgentConfigDetect,
            Self::ExternalAgentConfigImport,
            Self::ExternalAgentConfigImportRecordHistory,
            Self::ExternalAgentConfigImportReadHistories,
            Self::ConfigValueWrite,
            Self::ConfigBatchWrite,
            Self::ConfigRequirementsRead,
            Self::AccountRead,
            Self::FuzzyFileSearch,
            Self::FuzzyFileSearchSessionStart,
            Self::FuzzyFileSearchSessionUpdate,
            Self::FuzzyFileSearchSessionStop,
        ]
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method", content = "result")]
pub enum ClientRequestResult {
    #[serde(rename = "initialize")]
    Initialize(Box<InitializeResponse>),
    #[serde(rename = "server/diagnostics")]
    ServerDiagnostics(Box<ServerDiagnosticsResponse>),
    #[serde(rename = "userVerification/status")]
    UserVerificationStatus(Box<UserVerificationStatusResponse>),
    #[serde(rename = "userVerification/enroll")]
    UserVerificationEnroll(Box<UserVerificationEnrollResponse>),
    #[serde(rename = "userVerification/delete")]
    UserVerificationDelete(Box<UserVerificationDeleteResponse>),
    #[serde(rename = "userVerification/verify")]
    UserVerificationVerify(Box<UserVerificationVerifyResponse>),
    #[serde(rename = "userVerification/cancel")]
    UserVerificationCancel(Box<UserVerificationCancelResponse>),
    #[serde(rename = "thread/start")]
    ThreadStart(Box<ThreadStartResponse>),
    #[serde(rename = "thread/resume")]
    ThreadResume(Box<ThreadResumeResponse>),
    #[serde(rename = "thread/fork")]
    ThreadFork(Box<ThreadForkResponse>),
    #[serde(rename = "thread/sample")]
    ThreadSample(Box<ThreadSampleResponse>),
    #[serde(rename = "thread/sample/cancel")]
    ThreadSampleCancel(Box<ThreadSampleCancelResponse>),
    #[serde(rename = "thread/archive")]
    ThreadArchive(Box<ThreadArchiveResponse>),
    #[serde(rename = "thread/delete")]
    ThreadDelete(Box<ThreadDeleteResponse>),
    #[serde(rename = "thread/unsubscribe")]
    ThreadUnsubscribe(Box<ThreadUnsubscribeResponse>),
    #[serde(rename = "thread/increment_elicitation")]
    ThreadIncrementElicitation(Box<ThreadIncrementElicitationResponse>),
    #[serde(rename = "thread/decrement_elicitation")]
    ThreadDecrementElicitation(Box<ThreadDecrementElicitationResponse>),
    #[serde(rename = "thread/name/set")]
    ThreadNameSet(Box<ThreadSetNameResponse>),
    #[serde(rename = "thread/prediction/request")]
    ThreadPredictionRequest(Box<ThreadPredictionRequestResponse>),
    #[serde(rename = "thread/goal/set")]
    ThreadGoalSet(Box<ThreadGoalSetResponse>),
    #[serde(rename = "thread/goal/get")]
    ThreadGoalGet(Box<ThreadGoalGetResponse>),
    #[serde(rename = "thread/goal/clear")]
    ThreadGoalClear(Box<ThreadGoalClearResponse>),
    #[serde(rename = "thread/queue/add")]
    ThreadQueueAdd(Box<ThreadQueueAddResponse>),
    #[serde(rename = "thread/queue/list")]
    ThreadQueueList(Box<ThreadQueueListResponse>),
    #[serde(rename = "thread/queue/update")]
    ThreadQueueUpdate(Box<ThreadQueueUpdateResponse>),
    #[serde(rename = "thread/queue/delete")]
    ThreadQueueDelete(Box<ThreadQueueDeleteResponse>),
    #[serde(rename = "thread/queue/reorder")]
    ThreadQueueReorder(Box<ThreadQueueReorderResponse>),
    #[serde(rename = "thread/queue/start")]
    ThreadQueueStart(Box<ThreadQueueStartResponse>),
    #[serde(rename = "thread/metadata/update")]
    ThreadMetadataUpdate(Box<ThreadMetadataUpdateResponse>),
    #[serde(rename = "thread/attachment/add")]
    ThreadAttachmentAdd(Box<ThreadAttachmentAddResponse>),
    #[serde(rename = "thread/attachment/list")]
    ThreadAttachmentList(Box<ThreadAttachmentListResponse>),
    #[serde(rename = "thread/attachmentOwner/list")]
    ThreadAttachmentOwnerList(Box<ThreadAttachmentOwnerListResponse>),
    #[serde(rename = "thread/attachment/remove")]
    ThreadAttachmentRemove(Box<ThreadAttachmentRemoveResponse>),
    #[serde(rename = "thread/section/move")]
    ThreadSectionMove(Box<ThreadSectionMoveResponse>),
    #[serde(rename = "thread/settings/update")]
    ThreadSettingsUpdate(Box<ThreadSettingsUpdateResponse>),
    #[serde(rename = "thread/memoryMode/set")]
    ThreadMemoryModeSet(Box<ThreadMemoryModeSetResponse>),
    #[serde(rename = "thread/memory/read")]
    ThreadMemoryRead(Box<ThreadMemoryReadResponse>),
    #[serde(rename = "memory/status")]
    MemoryStatus(Box<MemoryStatusResponse>),
    #[serde(rename = "memory/reset")]
    MemoryReset(Box<MemoryResetResponse>),
    #[serde(rename = "rollout/compress")]
    RolloutCompress(Box<RolloutCompressResponse>),
    #[serde(rename = "thread/unarchive")]
    ThreadUnarchive(Box<ThreadUnarchiveResponse>),
    #[serde(rename = "thread/compact/start")]
    ThreadCompactStart(Box<ThreadCompactStartResponse>),
    #[serde(rename = "thread/shellCommand")]
    ThreadShellCommand(Box<ThreadShellCommandResponse>),
    #[serde(rename = "thread/approveGuardianDeniedAction")]
    ThreadApproveGuardianDeniedAction(Box<ThreadApproveGuardianDeniedActionResponse>),
    #[serde(rename = "thread/backgroundTerminals/clean")]
    ThreadBackgroundTerminalsClean(Box<ThreadBackgroundTerminalsCleanResponse>),
    #[serde(rename = "thread/backgroundTerminals/list")]
    ThreadBackgroundTerminalsList(Box<ThreadBackgroundTerminalsListResponse>),
    #[serde(rename = "thread/backgroundTerminals/terminate")]
    ThreadBackgroundTerminalsTerminate(Box<ThreadBackgroundTerminalsTerminateResponse>),
    #[serde(rename = "thread/revert")]
    ThreadRevert(Box<ThreadRevertResponse>),
    #[serde(rename = "thread/list")]
    ThreadList(Box<ThreadListResponse>),
    #[serde(rename = "project/list")]
    ProjectList(Box<ProjectListResponse>),
    #[serde(rename = "project/read")]
    ProjectRead(Box<ProjectReadResponse>),
    #[serde(rename = "project/create")]
    ProjectCreate(Box<ProjectCreateResponse>),
    #[serde(rename = "project/import")]
    ProjectImport(Box<ProjectImportResponse>),
    #[serde(rename = "project/update")]
    ProjectUpdate(Box<ProjectUpdateResponse>),
    #[serde(rename = "project/move")]
    ProjectMove(Box<ProjectMoveResponse>),
    #[serde(rename = "project/delete")]
    ProjectDelete(Box<ProjectDeleteResponse>),
    #[serde(rename = "threadSection/list")]
    ThreadSectionList(Box<ThreadSectionListResponse>),
    #[serde(rename = "threadSection/create")]
    ThreadSectionCreate(Box<ThreadSectionCreateResponse>),
    #[serde(rename = "threadSection/update")]
    ThreadSectionUpdate(Box<ThreadSectionUpdateResponse>),
    #[serde(rename = "threadSection/delete")]
    ThreadSectionDelete(Box<ThreadSectionDeleteResponse>),
    #[serde(rename = "thread/search")]
    ThreadSearch(Box<ThreadSearchResponse>),
    #[serde(rename = "thread/searchOccurrences")]
    ThreadSearchOccurrences(Box<ThreadSearchOccurrencesResponse>),
    #[serde(rename = "thread/loaded/list")]
    ThreadLoadedList(Box<ThreadLoadedListResponse>),
    #[serde(rename = "thread/read")]
    ThreadRead(Box<ThreadReadResponse>),
    #[serde(rename = "thread/readState/update")]
    ThreadReadStateUpdate(Box<ThreadReadStateUpdateResponse>),
    #[serde(rename = "thread/turns/list")]
    ThreadTurnsList(Box<ThreadTurnsListResponse>),
    #[serde(rename = "thread/items/list")]
    ThreadItemsList(Box<ThreadItemsListResponse>),
    #[serde(rename = "thread/items/read")]
    ThreadItemsRead(Box<ThreadItemsReadResponse>),
    #[serde(rename = "thread/guardianReview")]
    ThreadGuardianReview(Box<ThreadGuardianReviewResponse>),
    #[serde(rename = "thread/inject_items")]
    ThreadInjectItems(Box<ThreadInjectItemsResponse>),
    #[serde(rename = "skills/list")]
    SkillsList(Box<SkillsListResponse>),
    #[serde(rename = "skills/extraRoots/set")]
    SkillsExtraRootsSet(Box<SkillsExtraRootsSetResponse>),
    #[serde(rename = "hooks/list")]
    HooksList(Box<HooksListResponse>),
    #[serde(rename = "marketplace/add")]
    MarketplaceAdd(Box<MarketplaceAddResponse>),
    #[serde(rename = "marketplace/remove")]
    MarketplaceRemove(Box<MarketplaceRemoveResponse>),
    #[serde(rename = "marketplace/upgrade")]
    MarketplaceUpgrade(Box<MarketplaceUpgradeResponse>),
    #[serde(rename = "plugin/list")]
    PluginList(Box<PluginListResponse>),
    #[serde(rename = "plugin/search")]
    PluginSearch(Box<PluginSearchResponse>),
    #[serde(rename = "plugin/installed")]
    PluginInstalled(Box<PluginInstalledResponse>),
    #[serde(rename = "plugin/reconcile")]
    PluginReconcile(Box<PluginReconcileResponse>),
    #[serde(rename = "plugin/read")]
    PluginRead(Box<PluginReadResponse>),
    #[serde(rename = "plugin/skill/read")]
    PluginSkillRead(Box<PluginSkillReadResponse>),
    #[serde(rename = "plugin/share/save")]
    PluginShareSave(Box<PluginShareSaveResponse>),
    #[serde(rename = "plugin/share/updateTargets")]
    PluginShareUpdateTargets(Box<PluginShareUpdateTargetsResponse>),
    #[serde(rename = "plugin/share/list")]
    PluginShareList(Box<PluginShareListResponse>),
    #[serde(rename = "plugin/share/checkout")]
    PluginShareCheckout(Box<PluginShareCheckoutResponse>),
    #[serde(rename = "plugin/share/delete")]
    PluginShareDelete(Box<PluginShareDeleteResponse>),
    #[serde(rename = "app/read")]
    AppRead(Box<AppsReadResponse>),
    #[serde(rename = "app/list")]
    AppList(Box<AppsListResponse>),
    #[serde(rename = "app/installed")]
    AppInstalled(Box<AppsInstalledResponse>),
    #[serde(rename = "fs/readFile")]
    FsReadFile(Box<FsReadFileResponse>),
    #[serde(rename = "fs/writeFile")]
    FsWriteFile(Box<FsWriteFileResponse>),
    #[serde(rename = "fs/createDirectory")]
    FsCreateDirectory(Box<FsCreateDirectoryResponse>),
    #[serde(rename = "fs/getMetadata")]
    FsGetMetadata(Box<FsGetMetadataResponse>),
    #[serde(rename = "fs/readDirectory")]
    FsReadDirectory(Box<FsReadDirectoryResponse>),
    #[serde(rename = "fs/remove")]
    FsRemove(Box<FsRemoveResponse>),
    #[serde(rename = "fs/copy")]
    FsCopy(Box<FsCopyResponse>),
    #[serde(rename = "fs/watch")]
    FsWatch(Box<FsWatchResponse>),
    #[serde(rename = "fs/unwatch")]
    FsUnwatch(Box<FsUnwatchResponse>),
    #[serde(rename = "skills/config/write")]
    SkillsConfigWrite(Box<SkillsConfigWriteResponse>),
    #[serde(rename = "plugin/install")]
    PluginInstall(Box<PluginInstallResponse>),
    #[serde(rename = "plugin/uninstall")]
    PluginUninstall(Box<PluginUninstallResponse>),
    #[serde(rename = "turn/start")]
    TurnStart(Box<TurnStartResponse>),
    #[serde(rename = "turn/settings/update")]
    TurnSettingsUpdate(Box<TurnSettingsUpdateResponse>),
    #[serde(rename = "turn/steer")]
    TurnSteer(Box<TurnSteerResponse>),
    #[serde(rename = "turn/interrupt")]
    TurnInterrupt(Box<TurnInterruptResponse>),
    #[serde(rename = "thread/realtime/start")]
    ThreadRealtimeStart(Box<ThreadRealtimeStartResponse>),
    #[serde(rename = "thread/realtime/appendAudio")]
    ThreadRealtimeAppendAudio(Box<ThreadRealtimeAppendAudioResponse>),
    #[serde(rename = "thread/realtime/appendText")]
    ThreadRealtimeAppendText(Box<ThreadRealtimeAppendTextResponse>),
    #[serde(rename = "thread/realtime/appendSpeech")]
    ThreadRealtimeAppendSpeech(Box<ThreadRealtimeAppendSpeechResponse>),
    #[serde(rename = "thread/realtime/stop")]
    ThreadRealtimeStop(Box<ThreadRealtimeStopResponse>),
    #[serde(rename = "thread/timeline/list")]
    ThreadTimelineList(Box<ThreadTimelineListResponse>),
    #[serde(rename = "thread/realtime/listVoices")]
    ThreadRealtimeListVoices(Box<ThreadRealtimeListVoicesResponse>),
    #[serde(rename = "review/start")]
    ReviewStart(Box<ReviewStartResponse>),
    #[serde(rename = "model/list")]
    ModelList(Box<ModelListResponse>),
    #[serde(rename = "account/gatewayOAuth/read")]
    AccountGatewayOAuthRead(Box<GatewayOAuthReadResponse>),
    #[serde(rename = "account/gatewayOAuth/login")]
    AccountGatewayOAuthLogin(Box<GatewayOAuthLoginResponse>),
    #[serde(rename = "account/gatewayOAuth/cancel")]
    AccountGatewayOAuthCancel(Box<GatewayOAuthCancelResponse>),
    #[serde(rename = "modelProvider/capabilities/read")]
    ModelProviderCapabilitiesRead(Box<ModelProviderCapabilitiesReadResponse>),
    #[serde(rename = "experimentalFeature/list")]
    ExperimentalFeatureList(Box<ExperimentalFeatureListResponse>),
    #[serde(rename = "permissionProfile/list")]
    PermissionProfileList(Box<PermissionProfileListResponse>),
    #[serde(rename = "experimentalFeature/enablement/set")]
    ExperimentalFeatureEnablementSet(Box<ExperimentalFeatureEnablementSetResponse>),
    #[serde(rename = "remoteControl/enable")]
    RemoteControlEnable(Box<RemoteControlEnableResponse>),
    #[serde(rename = "remoteControl/disable")]
    RemoteControlDisable(Box<RemoteControlDisableResponse>),
    #[serde(rename = "remoteControl/status/read")]
    RemoteControlStatusRead(Box<RemoteControlStatusReadResponse>),
    #[serde(rename = "remoteControl/pairing/start")]
    RemoteControlPairingStart(Box<RemoteControlPairingStartResponse>),
    #[serde(rename = "remoteControl/pairing/status")]
    RemoteControlPairingStatus(Box<RemoteControlPairingStatusResponse>),
    #[serde(rename = "remoteControl/client/list")]
    RemoteControlClientList(Box<RemoteControlClientsListResponse>),
    #[serde(rename = "remoteControl/client/revoke")]
    RemoteControlClientRevoke(Box<RemoteControlClientsRevokeResponse>),
    #[serde(rename = "collaborationMode/list")]
    CollaborationModeList(Box<CollaborationModeListResponse>),
    #[serde(rename = "mock/experimentalMethod")]
    MockExperimentalMethod(Box<MockExperimentalMethodResponse>),
    #[serde(rename = "environment/add")]
    EnvironmentAdd(Box<EnvironmentAddResponse>),
    #[serde(rename = "environment/info")]
    EnvironmentInfo(Box<EnvironmentInfoResponse>),
    #[serde(rename = "environment/status")]
    EnvironmentStatus(Box<EnvironmentStatusResponse>),
    #[serde(rename = "mcpServer/oauth/login")]
    McpServerOauthLogin(Box<McpServerOauthLoginResponse>),
    #[serde(rename = "config/mcpServer/reload")]
    ConfigMcpServerReload(Box<McpServerRefreshResponse>),
    #[serde(rename = "mcpServerStatus/list")]
    McpServerStatusList(Box<ListMcpServerStatusResponse>),
    #[serde(rename = "mcpServer/resource/read")]
    McpServerResourceRead(Box<McpResourceReadResponse>),
    #[serde(rename = "mcpServer/event/stream/start")]
    McpServerEventStreamStart(Box<McpServerEventStreamStartResponse>),
    #[serde(rename = "mcpServer/event/stream/stop")]
    McpServerEventStreamStop(Box<McpServerEventStreamStopResponse>),
    #[serde(rename = "mcpServer/tool/call")]
    McpServerToolCall(Box<McpServerToolCallResponse>),
    #[serde(rename = "windowsSandbox/setupStart")]
    WindowsSandboxSetupStart(Box<WindowsSandboxSetupStartResponse>),
    #[serde(rename = "windowsSandbox/readiness")]
    WindowsSandboxReadiness(Box<WindowsSandboxReadinessResponse>),
    #[serde(rename = "account/login/start")]
    AccountLoginStart(Box<LoginAccountResponse>),
    #[serde(rename = "account/bedrock/discover")]
    AccountBedrockDiscover(Box<BedrockDiscoverResponse>),
    #[serde(rename = "account/bedrock/setup")]
    AccountBedrockSetup(Box<BedrockSetupResponse>),
    #[serde(rename = "account/gcp/setup")]
    AccountGcpSetup(Box<GcpSetupResponse>),
    #[serde(rename = "account/bedrock/checkGovCloudRequirements")]
    AccountBedrockCheckGovCloudRequirements(Box<BedrockCheckGovCloudRequirementsResponse>),
    #[serde(rename = "account/login/cancel")]
    AccountLoginCancel(Box<CancelLoginAccountResponse>),
    #[serde(rename = "accountSession/list")]
    AccountSessionList(Box<AccountSessionsResponse>),
    #[serde(rename = "accountSession/switch")]
    AccountSessionSwitch(Box<AccountSessionsSwitchResponse>),
    #[serde(rename = "account/logout")]
    AccountLogout(Box<LogoutAccountResponse>),
    #[serde(rename = "account/rateLimits/read")]
    AccountRateLimitsRead(Box<GetAccountRateLimitsResponse>),
    #[serde(rename = "account/rateLimitResetCredit/consume")]
    AccountRateLimitResetCreditConsume(Box<ConsumeAccountRateLimitResetCreditResponse>),
    #[serde(rename = "account/usage/read")]
    AccountUsageRead(Box<GetAccountTokenUsageResponse>),
    #[serde(rename = "account/workspaceMessages/read")]
    AccountWorkspaceMessagesRead(Box<GetWorkspaceMessagesResponse>),
    #[serde(rename = "account/sendAddCreditsNudgeEmail")]
    AccountSendAddCreditsNudgeEmail(Box<SendAddCreditsNudgeEmailResponse>),
    #[serde(rename = "feedback/upload")]
    FeedbackUpload(Box<FeedbackUploadResponse>),
    #[serde(rename = "feedback/save")]
    FeedbackSave(Box<FeedbackSaveResponse>),
    #[serde(rename = "command/exec")]
    CommandExec(Box<CommandExecResponse>),
    #[serde(rename = "command/exec/write")]
    CommandExecWrite(Box<CommandExecWriteResponse>),
    #[serde(rename = "command/exec/terminate")]
    CommandExecTerminate(Box<CommandExecTerminateResponse>),
    #[serde(rename = "command/exec/resize")]
    CommandExecResize(Box<CommandExecResizeResponse>),
    #[serde(rename = "process/spawn")]
    ProcessSpawn(Box<ProcessSpawnResponse>),
    #[serde(rename = "process/writeStdin")]
    ProcessWriteStdin(Box<ProcessWriteStdinResponse>),
    #[serde(rename = "process/kill")]
    ProcessKill(Box<ProcessKillResponse>),
    #[serde(rename = "process/resizePty")]
    ProcessResizePty(Box<ProcessResizePtyResponse>),
    #[serde(rename = "config/read")]
    ConfigRead(Box<ConfigReadResponse>),
    #[serde(rename = "externalAgentConfig/detect")]
    ExternalAgentConfigDetect(Box<ExternalAgentConfigDetectResponse>),
    #[serde(rename = "externalAgentConfig/import")]
    ExternalAgentConfigImport(Box<ExternalAgentConfigImportResponse>),
    #[serde(rename = "externalAgentConfig/import/recordHistory")]
    ExternalAgentConfigImportRecordHistory(Box<ExternalAgentConfigImportHistoryRecordResponse>),
    #[serde(rename = "externalAgentConfig/import/readHistories")]
    ExternalAgentConfigImportReadHistories(Box<ExternalAgentConfigImportHistoriesReadResponse>),
    #[serde(rename = "config/value/write")]
    ConfigValueWrite(Box<ConfigWriteResponse>),
    #[serde(rename = "config/batchWrite")]
    ConfigBatchWrite(Box<ConfigWriteResponse>),
    #[serde(rename = "configRequirements/read")]
    ConfigRequirementsRead(Box<ConfigRequirementsReadResponse>),
    #[serde(rename = "account/read")]
    AccountRead(Box<GetAccountResponse>),
    #[serde(rename = "fuzzyFileSearch")]
    FuzzyFileSearch(Box<FuzzyFileSearchResponse>),
    #[serde(rename = "fuzzyFileSearch/sessionStart")]
    FuzzyFileSearchSessionStart(Box<FuzzyFileSearchSessionStartResponse>),
    #[serde(rename = "fuzzyFileSearch/sessionUpdate")]
    FuzzyFileSearchSessionUpdate(Box<FuzzyFileSearchSessionUpdateResponse>),
    #[serde(rename = "fuzzyFileSearch/sessionStop")]
    FuzzyFileSearchSessionStop(Box<FuzzyFileSearchSessionStopResponse>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ClientNotificationMethod {
    #[serde(rename = "initialized")]
    Initialized,
}
impl ClientNotificationMethod {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "initialized" => Some(Self::Initialized),
            _ => None,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialized => "initialized",
        }
    }
    pub const fn all() -> &'static [Self] {
        &[Self::Initialized]
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ServerNotificationMethod {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "thread/started")]
    ThreadStarted,
    #[serde(rename = "thread/status/changed")]
    ThreadStatusChanged,
    #[serde(rename = "thread/readState/changed")]
    ThreadReadStateChanged,
    #[serde(rename = "thread/archived")]
    ThreadArchived,
    #[serde(rename = "thread/deleted")]
    ThreadDeleted,
    #[serde(rename = "thread/unarchived")]
    ThreadUnarchived,
    #[serde(rename = "thread/closed")]
    ThreadClosed,
    #[serde(rename = "thread/reverted")]
    ThreadReverted,
    #[serde(rename = "skills/changed")]
    SkillsChanged,
    #[serde(rename = "thread/name/updated")]
    ThreadNameUpdated,
    #[serde(rename = "thread/section/changed")]
    ThreadSectionChanged,
    #[serde(rename = "thread/attachment/updated")]
    ThreadAttachmentUpdated,
    #[serde(rename = "thread/goal/updated")]
    ThreadGoalUpdated,
    #[serde(rename = "thread/prediction/updated")]
    ThreadPredictionUpdated,
    #[serde(rename = "thread/goal/cleared")]
    ThreadGoalCleared,
    #[serde(rename = "thread/queue/changed")]
    ThreadQueueChanged,
    #[serde(rename = "project/changed")]
    ProjectChanged,
    #[serde(rename = "thread/project/updated")]
    ThreadProjectUpdated,
    #[serde(rename = "thread/environment/connected")]
    ThreadEnvironmentConnected,
    #[serde(rename = "thread/environment/disconnected")]
    ThreadEnvironmentDisconnected,
    #[serde(rename = "thread/settings/updated")]
    ThreadSettingsUpdated,
    #[serde(rename = "thread/tokenUsage/updated")]
    ThreadTokenUsageUpdated,
    #[serde(rename = "turn/started")]
    TurnStarted,
    #[serde(rename = "hook/started")]
    HookStarted,
    #[serde(rename = "turn/completed")]
    TurnCompleted,
    #[serde(rename = "hook/completed")]
    HookCompleted,
    #[serde(rename = "turn/diff/updated")]
    TurnDiffUpdated,
    #[serde(rename = "turn/plan/updated")]
    TurnPlanUpdated,
    #[serde(rename = "item/started")]
    ItemStarted,
    #[serde(rename = "item/autoApprovalReview/started")]
    ItemAutoApprovalReviewStarted,
    #[serde(rename = "item/autoApprovalReview/completed")]
    ItemAutoApprovalReviewCompleted,
    #[serde(rename = "autoApprovalReview/strictReviewRequired")]
    AutoApprovalReviewStrictReviewRequired,
    #[serde(rename = "item/completed")]
    ItemCompleted,
    #[serde(rename = "item/agentMessage/delta")]
    ItemAgentMessageDelta,
    #[serde(rename = "item/plan/delta")]
    ItemPlanDelta,
    #[serde(rename = "command/exec/outputDelta")]
    CommandExecOutputDelta,
    #[serde(rename = "process/outputDelta")]
    ProcessOutputDelta,
    #[serde(rename = "process/exited")]
    ProcessExited,
    #[serde(rename = "item/commandExecution/outputDelta")]
    ItemCommandExecutionOutputDelta,
    #[serde(rename = "item/commandExecution/terminalInteraction")]
    ItemCommandExecutionTerminalInteraction,
    #[serde(rename = "item/fileChange/outputDelta")]
    ItemFileChangeOutputDelta,
    #[serde(rename = "item/fileChange/patchUpdated")]
    ItemFileChangePatchUpdated,
    #[serde(rename = "serverRequest/resolved")]
    ServerRequestResolved,
    #[serde(rename = "item/mcpToolCall/progress")]
    ItemMcpToolCallProgress,
    #[serde(rename = "mcpServer/oauthLogin/completed")]
    McpServerOauthLoginCompleted,
    #[serde(rename = "mcpServer/startupStatus/updated")]
    McpServerStartupStatusUpdated,
    #[serde(rename = "mcpServer/event/stream/notification")]
    McpServerEventStreamNotification,
    #[serde(rename = "account/updated")]
    AccountUpdated,
    #[serde(rename = "account/gatewayOAuth/changed")]
    AccountGatewayOAuthChanged,
    #[serde(rename = "account/rateLimits/updated")]
    AccountRateLimitsUpdated,
    #[serde(rename = "app/list/updated")]
    AppListUpdated,
    #[serde(rename = "remoteControl/status/changed")]
    RemoteControlStatusChanged,
    #[serde(rename = "externalAgentConfig/import/progress")]
    ExternalAgentConfigImportProgress,
    #[serde(rename = "externalAgentConfig/import/completed")]
    ExternalAgentConfigImportCompleted,
    #[serde(rename = "fs/changed")]
    FsChanged,
    #[serde(rename = "item/reasoning/summaryTextDelta")]
    ItemReasoningSummaryTextDelta,
    #[serde(rename = "item/reasoning/summaryPartAdded")]
    ItemReasoningSummaryPartAdded,
    #[serde(rename = "item/reasoning/textDelta")]
    ItemReasoningTextDelta,
    #[serde(rename = "thread/compacted")]
    ThreadCompacted,
    #[serde(rename = "model/rerouted")]
    ModelRerouted,
    #[serde(rename = "model/verification")]
    ModelVerification,
    #[serde(rename = "modelProvider/authRecoveryStarted")]
    ModelProviderAuthRecoveryStarted,
    #[serde(rename = "modelProvider/authRecoveryCompleted")]
    ModelProviderAuthRecoveryCompleted,
    #[serde(rename = "turn/moderationMetadata")]
    TurnModerationMetadata,
    #[serde(rename = "model/safetyBuffering/updated")]
    ModelSafetyBufferingUpdated,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "guardianWarning")]
    GuardianWarning,
    #[serde(rename = "deprecationNotice")]
    DeprecationNotice,
    #[serde(rename = "configWarning")]
    ConfigWarning,
    #[serde(rename = "fuzzyFileSearch/sessionUpdated")]
    FuzzyFileSearchSessionUpdated,
    #[serde(rename = "fuzzyFileSearch/sessionCompleted")]
    FuzzyFileSearchSessionCompleted,
    #[serde(rename = "thread/realtime/started")]
    ThreadRealtimeStarted,
    #[serde(rename = "thread/realtime/itemAdded")]
    ThreadRealtimeItemAdded,
    #[serde(rename = "thread/realtime/item/started")]
    ThreadRealtimeItemStarted,
    #[serde(rename = "thread/realtime/item/transcript/delta")]
    ThreadRealtimeItemTranscriptDelta,
    #[serde(rename = "thread/realtime/item/completed")]
    ThreadRealtimeItemCompleted,
    #[serde(rename = "thread/realtime/transcript/delta")]
    ThreadRealtimeTranscriptDelta,
    #[serde(rename = "thread/realtime/transcript/done")]
    ThreadRealtimeTranscriptDone,
    #[serde(rename = "thread/realtime/outputAudio/delta")]
    ThreadRealtimeOutputAudioDelta,
    #[serde(rename = "thread/realtime/sdp")]
    ThreadRealtimeSdp,
    #[serde(rename = "thread/realtime/error")]
    ThreadRealtimeError,
    #[serde(rename = "thread/realtime/closed")]
    ThreadRealtimeClosed,
    #[serde(rename = "windows/worldWritableWarning")]
    WindowsWorldWritableWarning,
    #[serde(rename = "windowsSandbox/setupCompleted")]
    WindowsSandboxSetupCompleted,
    #[serde(rename = "account/login/completed")]
    AccountLoginCompleted,
}
impl ServerNotificationMethod {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "error" => Some(Self::Error),
            "thread/started" => Some(Self::ThreadStarted),
            "thread/status/changed" => Some(Self::ThreadStatusChanged),
            "thread/readState/changed" => Some(Self::ThreadReadStateChanged),
            "thread/archived" => Some(Self::ThreadArchived),
            "thread/deleted" => Some(Self::ThreadDeleted),
            "thread/unarchived" => Some(Self::ThreadUnarchived),
            "thread/closed" => Some(Self::ThreadClosed),
            "thread/reverted" => Some(Self::ThreadReverted),
            "skills/changed" => Some(Self::SkillsChanged),
            "thread/name/updated" => Some(Self::ThreadNameUpdated),
            "thread/section/changed" => Some(Self::ThreadSectionChanged),
            "thread/attachment/updated" => Some(Self::ThreadAttachmentUpdated),
            "thread/goal/updated" => Some(Self::ThreadGoalUpdated),
            "thread/prediction/updated" => Some(Self::ThreadPredictionUpdated),
            "thread/goal/cleared" => Some(Self::ThreadGoalCleared),
            "thread/queue/changed" => Some(Self::ThreadQueueChanged),
            "project/changed" => Some(Self::ProjectChanged),
            "thread/project/updated" => Some(Self::ThreadProjectUpdated),
            "thread/environment/connected" => Some(Self::ThreadEnvironmentConnected),
            "thread/environment/disconnected" => Some(Self::ThreadEnvironmentDisconnected),
            "thread/settings/updated" => Some(Self::ThreadSettingsUpdated),
            "thread/tokenUsage/updated" => Some(Self::ThreadTokenUsageUpdated),
            "turn/started" => Some(Self::TurnStarted),
            "hook/started" => Some(Self::HookStarted),
            "turn/completed" => Some(Self::TurnCompleted),
            "hook/completed" => Some(Self::HookCompleted),
            "turn/diff/updated" => Some(Self::TurnDiffUpdated),
            "turn/plan/updated" => Some(Self::TurnPlanUpdated),
            "item/started" => Some(Self::ItemStarted),
            "item/autoApprovalReview/started" => Some(Self::ItemAutoApprovalReviewStarted),
            "item/autoApprovalReview/completed" => Some(Self::ItemAutoApprovalReviewCompleted),
            "autoApprovalReview/strictReviewRequired" => {
                Some(Self::AutoApprovalReviewStrictReviewRequired)
            }
            "item/completed" => Some(Self::ItemCompleted),
            "item/agentMessage/delta" => Some(Self::ItemAgentMessageDelta),
            "item/plan/delta" => Some(Self::ItemPlanDelta),
            "command/exec/outputDelta" => Some(Self::CommandExecOutputDelta),
            "process/outputDelta" => Some(Self::ProcessOutputDelta),
            "process/exited" => Some(Self::ProcessExited),
            "item/commandExecution/outputDelta" => Some(Self::ItemCommandExecutionOutputDelta),
            "item/commandExecution/terminalInteraction" => {
                Some(Self::ItemCommandExecutionTerminalInteraction)
            }
            "item/fileChange/outputDelta" => Some(Self::ItemFileChangeOutputDelta),
            "item/fileChange/patchUpdated" => Some(Self::ItemFileChangePatchUpdated),
            "serverRequest/resolved" => Some(Self::ServerRequestResolved),
            "item/mcpToolCall/progress" => Some(Self::ItemMcpToolCallProgress),
            "mcpServer/oauthLogin/completed" => Some(Self::McpServerOauthLoginCompleted),
            "mcpServer/startupStatus/updated" => Some(Self::McpServerStartupStatusUpdated),
            "mcpServer/event/stream/notification" => Some(Self::McpServerEventStreamNotification),
            "account/updated" => Some(Self::AccountUpdated),
            "account/gatewayOAuth/changed" => Some(Self::AccountGatewayOAuthChanged),
            "account/rateLimits/updated" => Some(Self::AccountRateLimitsUpdated),
            "app/list/updated" => Some(Self::AppListUpdated),
            "remoteControl/status/changed" => Some(Self::RemoteControlStatusChanged),
            "externalAgentConfig/import/progress" => Some(Self::ExternalAgentConfigImportProgress),
            "externalAgentConfig/import/completed" => {
                Some(Self::ExternalAgentConfigImportCompleted)
            }
            "fs/changed" => Some(Self::FsChanged),
            "item/reasoning/summaryTextDelta" => Some(Self::ItemReasoningSummaryTextDelta),
            "item/reasoning/summaryPartAdded" => Some(Self::ItemReasoningSummaryPartAdded),
            "item/reasoning/textDelta" => Some(Self::ItemReasoningTextDelta),
            "thread/compacted" => Some(Self::ThreadCompacted),
            "model/rerouted" => Some(Self::ModelRerouted),
            "model/verification" => Some(Self::ModelVerification),
            "modelProvider/authRecoveryStarted" => Some(Self::ModelProviderAuthRecoveryStarted),
            "modelProvider/authRecoveryCompleted" => Some(Self::ModelProviderAuthRecoveryCompleted),
            "turn/moderationMetadata" => Some(Self::TurnModerationMetadata),
            "model/safetyBuffering/updated" => Some(Self::ModelSafetyBufferingUpdated),
            "warning" => Some(Self::Warning),
            "guardianWarning" => Some(Self::GuardianWarning),
            "deprecationNotice" => Some(Self::DeprecationNotice),
            "configWarning" => Some(Self::ConfigWarning),
            "fuzzyFileSearch/sessionUpdated" => Some(Self::FuzzyFileSearchSessionUpdated),
            "fuzzyFileSearch/sessionCompleted" => Some(Self::FuzzyFileSearchSessionCompleted),
            "thread/realtime/started" => Some(Self::ThreadRealtimeStarted),
            "thread/realtime/itemAdded" => Some(Self::ThreadRealtimeItemAdded),
            "thread/realtime/item/started" => Some(Self::ThreadRealtimeItemStarted),
            "thread/realtime/item/transcript/delta" => {
                Some(Self::ThreadRealtimeItemTranscriptDelta)
            }
            "thread/realtime/item/completed" => Some(Self::ThreadRealtimeItemCompleted),
            "thread/realtime/transcript/delta" => Some(Self::ThreadRealtimeTranscriptDelta),
            "thread/realtime/transcript/done" => Some(Self::ThreadRealtimeTranscriptDone),
            "thread/realtime/outputAudio/delta" => Some(Self::ThreadRealtimeOutputAudioDelta),
            "thread/realtime/sdp" => Some(Self::ThreadRealtimeSdp),
            "thread/realtime/error" => Some(Self::ThreadRealtimeError),
            "thread/realtime/closed" => Some(Self::ThreadRealtimeClosed),
            "windows/worldWritableWarning" => Some(Self::WindowsWorldWritableWarning),
            "windowsSandbox/setupCompleted" => Some(Self::WindowsSandboxSetupCompleted),
            "account/login/completed" => Some(Self::AccountLoginCompleted),
            _ => None,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::ThreadStarted => "thread/started",
            Self::ThreadStatusChanged => "thread/status/changed",
            Self::ThreadReadStateChanged => "thread/readState/changed",
            Self::ThreadArchived => "thread/archived",
            Self::ThreadDeleted => "thread/deleted",
            Self::ThreadUnarchived => "thread/unarchived",
            Self::ThreadClosed => "thread/closed",
            Self::ThreadReverted => "thread/reverted",
            Self::SkillsChanged => "skills/changed",
            Self::ThreadNameUpdated => "thread/name/updated",
            Self::ThreadSectionChanged => "thread/section/changed",
            Self::ThreadAttachmentUpdated => "thread/attachment/updated",
            Self::ThreadGoalUpdated => "thread/goal/updated",
            Self::ThreadPredictionUpdated => "thread/prediction/updated",
            Self::ThreadGoalCleared => "thread/goal/cleared",
            Self::ThreadQueueChanged => "thread/queue/changed",
            Self::ProjectChanged => "project/changed",
            Self::ThreadProjectUpdated => "thread/project/updated",
            Self::ThreadEnvironmentConnected => "thread/environment/connected",
            Self::ThreadEnvironmentDisconnected => "thread/environment/disconnected",
            Self::ThreadSettingsUpdated => "thread/settings/updated",
            Self::ThreadTokenUsageUpdated => "thread/tokenUsage/updated",
            Self::TurnStarted => "turn/started",
            Self::HookStarted => "hook/started",
            Self::TurnCompleted => "turn/completed",
            Self::HookCompleted => "hook/completed",
            Self::TurnDiffUpdated => "turn/diff/updated",
            Self::TurnPlanUpdated => "turn/plan/updated",
            Self::ItemStarted => "item/started",
            Self::ItemAutoApprovalReviewStarted => "item/autoApprovalReview/started",
            Self::ItemAutoApprovalReviewCompleted => "item/autoApprovalReview/completed",
            Self::AutoApprovalReviewStrictReviewRequired => {
                "autoApprovalReview/strictReviewRequired"
            }
            Self::ItemCompleted => "item/completed",
            Self::ItemAgentMessageDelta => "item/agentMessage/delta",
            Self::ItemPlanDelta => "item/plan/delta",
            Self::CommandExecOutputDelta => "command/exec/outputDelta",
            Self::ProcessOutputDelta => "process/outputDelta",
            Self::ProcessExited => "process/exited",
            Self::ItemCommandExecutionOutputDelta => "item/commandExecution/outputDelta",
            Self::ItemCommandExecutionTerminalInteraction => {
                "item/commandExecution/terminalInteraction"
            }
            Self::ItemFileChangeOutputDelta => "item/fileChange/outputDelta",
            Self::ItemFileChangePatchUpdated => "item/fileChange/patchUpdated",
            Self::ServerRequestResolved => "serverRequest/resolved",
            Self::ItemMcpToolCallProgress => "item/mcpToolCall/progress",
            Self::McpServerOauthLoginCompleted => "mcpServer/oauthLogin/completed",
            Self::McpServerStartupStatusUpdated => "mcpServer/startupStatus/updated",
            Self::McpServerEventStreamNotification => "mcpServer/event/stream/notification",
            Self::AccountUpdated => "account/updated",
            Self::AccountGatewayOAuthChanged => "account/gatewayOAuth/changed",
            Self::AccountRateLimitsUpdated => "account/rateLimits/updated",
            Self::AppListUpdated => "app/list/updated",
            Self::RemoteControlStatusChanged => "remoteControl/status/changed",
            Self::ExternalAgentConfigImportProgress => "externalAgentConfig/import/progress",
            Self::ExternalAgentConfigImportCompleted => "externalAgentConfig/import/completed",
            Self::FsChanged => "fs/changed",
            Self::ItemReasoningSummaryTextDelta => "item/reasoning/summaryTextDelta",
            Self::ItemReasoningSummaryPartAdded => "item/reasoning/summaryPartAdded",
            Self::ItemReasoningTextDelta => "item/reasoning/textDelta",
            Self::ThreadCompacted => "thread/compacted",
            Self::ModelRerouted => "model/rerouted",
            Self::ModelVerification => "model/verification",
            Self::ModelProviderAuthRecoveryStarted => "modelProvider/authRecoveryStarted",
            Self::ModelProviderAuthRecoveryCompleted => "modelProvider/authRecoveryCompleted",
            Self::TurnModerationMetadata => "turn/moderationMetadata",
            Self::ModelSafetyBufferingUpdated => "model/safetyBuffering/updated",
            Self::Warning => "warning",
            Self::GuardianWarning => "guardianWarning",
            Self::DeprecationNotice => "deprecationNotice",
            Self::ConfigWarning => "configWarning",
            Self::FuzzyFileSearchSessionUpdated => "fuzzyFileSearch/sessionUpdated",
            Self::FuzzyFileSearchSessionCompleted => "fuzzyFileSearch/sessionCompleted",
            Self::ThreadRealtimeStarted => "thread/realtime/started",
            Self::ThreadRealtimeItemAdded => "thread/realtime/itemAdded",
            Self::ThreadRealtimeItemStarted => "thread/realtime/item/started",
            Self::ThreadRealtimeItemTranscriptDelta => "thread/realtime/item/transcript/delta",
            Self::ThreadRealtimeItemCompleted => "thread/realtime/item/completed",
            Self::ThreadRealtimeTranscriptDelta => "thread/realtime/transcript/delta",
            Self::ThreadRealtimeTranscriptDone => "thread/realtime/transcript/done",
            Self::ThreadRealtimeOutputAudioDelta => "thread/realtime/outputAudio/delta",
            Self::ThreadRealtimeSdp => "thread/realtime/sdp",
            Self::ThreadRealtimeError => "thread/realtime/error",
            Self::ThreadRealtimeClosed => "thread/realtime/closed",
            Self::WindowsWorldWritableWarning => "windows/worldWritableWarning",
            Self::WindowsSandboxSetupCompleted => "windowsSandbox/setupCompleted",
            Self::AccountLoginCompleted => "account/login/completed",
        }
    }
    pub const fn all() -> &'static [Self] {
        &[
            Self::Error,
            Self::ThreadStarted,
            Self::ThreadStatusChanged,
            Self::ThreadReadStateChanged,
            Self::ThreadArchived,
            Self::ThreadDeleted,
            Self::ThreadUnarchived,
            Self::ThreadClosed,
            Self::ThreadReverted,
            Self::SkillsChanged,
            Self::ThreadNameUpdated,
            Self::ThreadSectionChanged,
            Self::ThreadAttachmentUpdated,
            Self::ThreadGoalUpdated,
            Self::ThreadPredictionUpdated,
            Self::ThreadGoalCleared,
            Self::ThreadQueueChanged,
            Self::ProjectChanged,
            Self::ThreadProjectUpdated,
            Self::ThreadEnvironmentConnected,
            Self::ThreadEnvironmentDisconnected,
            Self::ThreadSettingsUpdated,
            Self::ThreadTokenUsageUpdated,
            Self::TurnStarted,
            Self::HookStarted,
            Self::TurnCompleted,
            Self::HookCompleted,
            Self::TurnDiffUpdated,
            Self::TurnPlanUpdated,
            Self::ItemStarted,
            Self::ItemAutoApprovalReviewStarted,
            Self::ItemAutoApprovalReviewCompleted,
            Self::AutoApprovalReviewStrictReviewRequired,
            Self::ItemCompleted,
            Self::ItemAgentMessageDelta,
            Self::ItemPlanDelta,
            Self::CommandExecOutputDelta,
            Self::ProcessOutputDelta,
            Self::ProcessExited,
            Self::ItemCommandExecutionOutputDelta,
            Self::ItemCommandExecutionTerminalInteraction,
            Self::ItemFileChangeOutputDelta,
            Self::ItemFileChangePatchUpdated,
            Self::ServerRequestResolved,
            Self::ItemMcpToolCallProgress,
            Self::McpServerOauthLoginCompleted,
            Self::McpServerStartupStatusUpdated,
            Self::McpServerEventStreamNotification,
            Self::AccountUpdated,
            Self::AccountGatewayOAuthChanged,
            Self::AccountRateLimitsUpdated,
            Self::AppListUpdated,
            Self::RemoteControlStatusChanged,
            Self::ExternalAgentConfigImportProgress,
            Self::ExternalAgentConfigImportCompleted,
            Self::FsChanged,
            Self::ItemReasoningSummaryTextDelta,
            Self::ItemReasoningSummaryPartAdded,
            Self::ItemReasoningTextDelta,
            Self::ThreadCompacted,
            Self::ModelRerouted,
            Self::ModelVerification,
            Self::ModelProviderAuthRecoveryStarted,
            Self::ModelProviderAuthRecoveryCompleted,
            Self::TurnModerationMetadata,
            Self::ModelSafetyBufferingUpdated,
            Self::Warning,
            Self::GuardianWarning,
            Self::DeprecationNotice,
            Self::ConfigWarning,
            Self::FuzzyFileSearchSessionUpdated,
            Self::FuzzyFileSearchSessionCompleted,
            Self::ThreadRealtimeStarted,
            Self::ThreadRealtimeItemAdded,
            Self::ThreadRealtimeItemStarted,
            Self::ThreadRealtimeItemTranscriptDelta,
            Self::ThreadRealtimeItemCompleted,
            Self::ThreadRealtimeTranscriptDelta,
            Self::ThreadRealtimeTranscriptDone,
            Self::ThreadRealtimeOutputAudioDelta,
            Self::ThreadRealtimeSdp,
            Self::ThreadRealtimeError,
            Self::ThreadRealtimeClosed,
            Self::WindowsWorldWritableWarning,
            Self::WindowsSandboxSetupCompleted,
            Self::AccountLoginCompleted,
        ]
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ServerRequestMethod {
    #[serde(rename = "item/commandExecution/requestApproval")]
    ItemCommandExecutionRequestApproval,
    #[serde(rename = "item/fileChange/requestApproval")]
    ItemFileChangeRequestApproval,
    #[serde(rename = "item/tool/requestUserInput")]
    ItemToolRequestUserInput,
    #[serde(rename = "mcpServer/elicitation/request")]
    McpServerElicitationRequest,
    #[serde(rename = "item/permissions/requestApproval")]
    ItemPermissionsRequestApproval,
    #[serde(rename = "item/tool/call")]
    ItemToolCall,
    #[serde(rename = "account/chatgptAuthTokens/refresh")]
    AccountChatgptAuthTokensRefresh,
    #[serde(rename = "attestation/generate")]
    AttestationGenerate,
    #[serde(rename = "currentTime/read")]
    CurrentTimeRead,
    #[serde(rename = "applyPatchApproval")]
    ApplyPatchApproval,
    #[serde(rename = "execCommandApproval")]
    ExecCommandApproval,
}
impl ServerRequestMethod {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "item/commandExecution/requestApproval" => {
                Some(Self::ItemCommandExecutionRequestApproval)
            }
            "item/fileChange/requestApproval" => Some(Self::ItemFileChangeRequestApproval),
            "item/tool/requestUserInput" => Some(Self::ItemToolRequestUserInput),
            "mcpServer/elicitation/request" => Some(Self::McpServerElicitationRequest),
            "item/permissions/requestApproval" => Some(Self::ItemPermissionsRequestApproval),
            "item/tool/call" => Some(Self::ItemToolCall),
            "account/chatgptAuthTokens/refresh" => Some(Self::AccountChatgptAuthTokensRefresh),
            "attestation/generate" => Some(Self::AttestationGenerate),
            "currentTime/read" => Some(Self::CurrentTimeRead),
            "applyPatchApproval" => Some(Self::ApplyPatchApproval),
            "execCommandApproval" => Some(Self::ExecCommandApproval),
            _ => None,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ItemCommandExecutionRequestApproval => "item/commandExecution/requestApproval",
            Self::ItemFileChangeRequestApproval => "item/fileChange/requestApproval",
            Self::ItemToolRequestUserInput => "item/tool/requestUserInput",
            Self::McpServerElicitationRequest => "mcpServer/elicitation/request",
            Self::ItemPermissionsRequestApproval => "item/permissions/requestApproval",
            Self::ItemToolCall => "item/tool/call",
            Self::AccountChatgptAuthTokensRefresh => "account/chatgptAuthTokens/refresh",
            Self::AttestationGenerate => "attestation/generate",
            Self::CurrentTimeRead => "currentTime/read",
            Self::ApplyPatchApproval => "applyPatchApproval",
            Self::ExecCommandApproval => "execCommandApproval",
        }
    }
    pub const fn all() -> &'static [Self] {
        &[
            Self::ItemCommandExecutionRequestApproval,
            Self::ItemFileChangeRequestApproval,
            Self::ItemToolRequestUserInput,
            Self::McpServerElicitationRequest,
            Self::ItemPermissionsRequestApproval,
            Self::ItemToolCall,
            Self::AccountChatgptAuthTokensRefresh,
            Self::AttestationGenerate,
            Self::CurrentTimeRead,
            Self::ApplyPatchApproval,
            Self::ExecCommandApproval,
        ]
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "method", content = "result")]
pub enum ServerRequestResult {
    #[serde(rename = "item/commandExecution/requestApproval")]
    ItemCommandExecutionRequestApproval(Box<CommandExecutionRequestApprovalResponse>),
    #[serde(rename = "item/fileChange/requestApproval")]
    ItemFileChangeRequestApproval(Box<FileChangeRequestApprovalResponse>),
    #[serde(rename = "item/tool/requestUserInput")]
    ItemToolRequestUserInput(Box<ToolRequestUserInputResponse>),
    #[serde(rename = "mcpServer/elicitation/request")]
    McpServerElicitationRequest(Box<McpServerElicitationRequestResponse>),
    #[serde(rename = "item/permissions/requestApproval")]
    ItemPermissionsRequestApproval(Box<PermissionsRequestApprovalResponse>),
    #[serde(rename = "item/tool/call")]
    ItemToolCall(Box<DynamicToolCallResponse>),
    #[serde(rename = "account/chatgptAuthTokens/refresh")]
    AccountChatgptAuthTokensRefresh(Box<ChatgptAuthTokensRefreshResponse>),
    #[serde(rename = "attestation/generate")]
    AttestationGenerate(Box<AttestationGenerateResponse>),
    #[serde(rename = "currentTime/read")]
    CurrentTimeRead(Box<CurrentTimeReadResponse>),
    #[serde(rename = "applyPatchApproval")]
    ApplyPatchApproval(Box<ApplyPatchApprovalResponse>),
    #[serde(rename = "execCommandApproval")]
    ExecCommandApproval(Box<ExecCommandApprovalResponse>),
}

#[cfg(test)]
mod generated_fixtures {
    use super::*;
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Samples {
        client_request: Vec<ClientRequest>,
        client_notification: Vec<ClientNotification>,
        server_notification: Vec<ServerNotification>,
        server_request: Vec<ServerRequest>,
    }
    #[test]
    fn every_vendor_method_sample_decodes() {
        let samples: Samples = studio_json::from_str(include_str!("fixtures.json")).unwrap();
        assert_eq!(
            samples.client_request.len(),
            ClientRequestMethod::all().len()
        );
        assert_eq!(
            samples.client_notification.len(),
            ClientNotificationMethod::all().len()
        );
        assert_eq!(
            samples.server_notification.len(),
            ServerNotificationMethod::all().len()
        );
        assert_eq!(
            samples.server_request.len(),
            ServerRequestMethod::all().len()
        );
    }
}
