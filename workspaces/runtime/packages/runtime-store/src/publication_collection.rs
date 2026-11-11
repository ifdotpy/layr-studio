//! Permanent publication collection codes; generated wire tags own their spellings.
//! Append new codes. Never renumber existing variants from schema iteration order.
use studio_db_api::{Codec, CodecContext, CodecError, DbEnum};
use studio_runtime_schema::generated;

pub(crate) const TRANSCRIPT_FAMILY: &str = "transcript";

/// The argument is an opaque AgentId in the existing domain contract. Preserve
/// empty/legacy text and original label case; SQL LIKE is ASCII-insensitive.
pub(crate) fn transcript_argument(collection: &str) -> Option<&str> {
    let (family, argument) = collection.split_once(':')?;
    family
        .eq_ignore_ascii_case(TRANSCRIPT_FAMILY)
        .then_some(argument)
}

macro_rules! collection_codes {
($($variant:ident = $code:literal => $tag:ident),+ $(,)?) => {
#[derive(Clone,Copy,Debug,PartialEq,Eq,DbEnum)]
#[repr(i64)]
pub(crate) enum CollectionKind { $($variant = $code,)+ Transcript = 38, Other = 39 }
impl CollectionKind {
pub fn of(label: &str) -> Self {
if transcript_argument(label).is_some() { return Self::Transcript; }
$(if label == generated::$tag::default().as_str() { return Self::$variant; })+
Self::Other
}
}
};
}

collection_codes! {
AccountCost = 1 => SyncAccountCostEntityPayloadCollection,
Accounts = 2 => SyncAccountsEntityPayloadCollection,
Agent = 3 => SyncAgentEntityPayloadCollection,
AgentDetail = 4 => SyncAgentDetailEntityPayloadCollection,
AgentSummary = 5 => SyncAgentSummaryEntityPayloadCollection,
AgentTail = 6 => SyncAgentTailEntityPayloadCollection,
Annotation = 7 => SyncAnnotationEntityPayloadCollection,
Chat = 8 => SyncChatEntityPayloadCollection,
Complaint = 9 => SyncComplaintEntityPayloadCollection,
Desktop = 10 => SyncDesktopEntityPayloadCollection,
Edge = 11 => SyncEdgeEntityPayloadCollection,
Event = 12 => SyncEventEntityPayloadCollection,
MessageReceipt = 13 => SyncMessageReceiptEntityPayloadCollection,
Model = 14 => SyncModelEntityPayloadCollection,
Monitor = 15 => SyncMonitorEntityPayloadCollection,
PeerTeam = 16 => SyncPeerTeamEntityPayloadCollection,
Plan = 17 => SyncPlanEntityPayloadCollection,
Project = 18 => SyncProjectEntityPayloadCollection,
QuestionHistory = 19 => SyncQuestionHistoryEntityPayloadCollection,
Queue = 20 => SyncQueueEntityPayloadCollection,
RateLimit = 21 => SyncRateLimitEntityPayloadCollection,
Request = 22 => SyncRequestEntityPayloadCollection,
RetainedWorktree = 23 => SyncRetainedWorktreeEntityPayloadCollection,
Room = 24 => SyncRoomEntityPayloadCollection,
Rule = 25 => SyncRuleEntityPayloadCollection,
SessionCost = 26 => SyncSessionCostEntityPayloadCollection,
StudioError = 27 => SyncStudioErrorEntityPayloadCollection,
Task = 28 => SyncTaskEntityPayloadCollection,
TaskDetail = 29 => SyncTaskDetailEntityPayloadCollection,
Terminal = 30 => SyncTerminalEntityPayloadCollection,
UiPreferences = 31 => SyncUiPreferencesEntityPayloadCollection,
Work = 32 => SyncWorkEntityPayloadCollection,
WorkDecision = 33 => SyncWorkDecisionEntityPayloadCollection,
WorkDetail = 34 => SyncWorkDetailEntityPayloadCollection,
WorkRelease = 35 => SyncWorkReleaseEntityPayloadCollection,
WorkResult = 36 => SyncWorkResultEntityPayloadCollection,
Workspace = 37 => SyncWorkspaceEntityPayloadCollection,
}

/// Exact comparison text plus the existing typed argument; neither is sorted by code.
pub(crate) struct CollectionName {
    raw: String,
    kind: CollectionKind,
    agent: Option<crate::AgentId>,
}
impl CollectionName {
    pub fn from_stored(raw: String) -> Self {
        let kind = CollectionKind::of(&raw);
        let agent = transcript_argument(&raw).map(crate::AgentId::from);
        Self { raw, kind, agent }
    }
    pub fn as_str(&self) -> &str {
        &self.raw
    }
    pub fn kind(&self) -> CollectionKind {
        self.kind
    }
    pub fn agent(&self) -> Option<&crate::AgentId> {
        self.agent.as_ref()
    }
    pub fn non_transcript(&self) -> bool {
        self.kind != CollectionKind::Transcript
    }
}

pub(crate) struct CollectionNameCodec;
impl Codec<CollectionName> for CollectionNameCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(_: &CodecContext, value: &'a CollectionName) -> Result<&'a str, CodecError> {
        Ok(value.as_str())
    }
    fn decode(_: &CodecContext, value: String) -> Result<CollectionName, CodecError> {
        Ok(CollectionName::from_stored(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::DbValue;
    #[test]
    fn f1_collection_codes_cover_the_existing_discriminator_without_copying_names() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime-http/openapi.json");
        let mut mapping: studio_runtime_schema::raw_json::RawExtensions =
            studio_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for name in [
            "components",
            "schemas",
            "SyncEntityPayload",
            "discriminator",
            "mapping",
        ] {
            mapping = studio_json::from_str(mapping.remove(name).unwrap().as_str()).unwrap();
        }
        assert_eq!(mapping.len() + 2, CollectionKind::CODE_NAMES.len());
        for label in mapping.keys() {
            assert_ne!(CollectionKind::of(label), CollectionKind::Other, "{label}");
            assert_ne!(
                CollectionKind::of(label),
                CollectionKind::Transcript,
                "{label}"
            );
        }
        assert_eq!(
            CollectionKind::CODE_NAMES,
            &[
                (1, "AccountCost"),
                (2, "Accounts"),
                (3, "Agent"),
                (4, "AgentDetail"),
                (5, "AgentSummary"),
                (6, "AgentTail"),
                (7, "Annotation"),
                (8, "Chat"),
                (9, "Complaint"),
                (10, "Desktop"),
                (11, "Edge"),
                (12, "Event"),
                (13, "MessageReceipt"),
                (14, "Model"),
                (15, "Monitor"),
                (16, "PeerTeam"),
                (17, "Plan"),
                (18, "Project"),
                (19, "QuestionHistory"),
                (20, "Queue"),
                (21, "RateLimit"),
                (22, "Request"),
                (23, "RetainedWorktree"),
                (24, "Room"),
                (25, "Rule"),
                (26, "SessionCost"),
                (27, "StudioError"),
                (28, "Task"),
                (29, "TaskDetail"),
                (30, "Terminal"),
                (31, "UiPreferences"),
                (32, "Work"),
                (33, "WorkDecision"),
                (34, "WorkDetail"),
                (35, "WorkRelease"),
                (36, "WorkResult"),
                (37, "Workspace"),
                (38, "Transcript"),
                (39, "Other")
            ]
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncAccountCostEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::AccountCost
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncAccountCostEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncAccountsEntityPayloadCollection::default().as_str()),
            CollectionKind::Accounts
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncAccountsEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncAgentEntityPayloadCollection::default().as_str()),
            CollectionKind::Agent
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncAgentEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncAgentDetailEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::AgentDetail
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncAgentDetailEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncAgentSummaryEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::AgentSummary
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncAgentSummaryEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncAgentTailEntityPayloadCollection::default().as_str()),
            CollectionKind::AgentTail
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncAgentTailEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncAnnotationEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::Annotation
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncAnnotationEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncChatEntityPayloadCollection::default().as_str()),
            CollectionKind::Chat
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncChatEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncComplaintEntityPayloadCollection::default().as_str()),
            CollectionKind::Complaint
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncComplaintEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncDesktopEntityPayloadCollection::default().as_str()),
            CollectionKind::Desktop
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncDesktopEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncEdgeEntityPayloadCollection::default().as_str()),
            CollectionKind::Edge
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncEdgeEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncEventEntityPayloadCollection::default().as_str()),
            CollectionKind::Event
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncEventEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncMessageReceiptEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::MessageReceipt
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncMessageReceiptEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncModelEntityPayloadCollection::default().as_str()),
            CollectionKind::Model
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncModelEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncMonitorEntityPayloadCollection::default().as_str()),
            CollectionKind::Monitor
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncMonitorEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncPeerTeamEntityPayloadCollection::default().as_str()),
            CollectionKind::PeerTeam
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncPeerTeamEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncPlanEntityPayloadCollection::default().as_str()),
            CollectionKind::Plan
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncPlanEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncProjectEntityPayloadCollection::default().as_str()),
            CollectionKind::Project
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncProjectEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncQuestionHistoryEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::QuestionHistory
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncQuestionHistoryEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncQueueEntityPayloadCollection::default().as_str()),
            CollectionKind::Queue
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncQueueEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncRateLimitEntityPayloadCollection::default().as_str()),
            CollectionKind::RateLimit
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncRateLimitEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncRequestEntityPayloadCollection::default().as_str()),
            CollectionKind::Request
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncRequestEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncRetainedWorktreeEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::RetainedWorktree
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncRetainedWorktreeEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncRoomEntityPayloadCollection::default().as_str()),
            CollectionKind::Room
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncRoomEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncRuleEntityPayloadCollection::default().as_str()),
            CollectionKind::Rule
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncRuleEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncSessionCostEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::SessionCost
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncSessionCostEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncStudioErrorEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::StudioError
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncStudioErrorEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncTaskEntityPayloadCollection::default().as_str()),
            CollectionKind::Task
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncTaskEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncTaskDetailEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::TaskDetail
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncTaskDetailEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncTerminalEntityPayloadCollection::default().as_str()),
            CollectionKind::Terminal
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncTerminalEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncUiPreferencesEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::UiPreferences
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncUiPreferencesEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncWorkEntityPayloadCollection::default().as_str()),
            CollectionKind::Work
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncWorkEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncWorkDecisionEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::WorkDecision
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncWorkDecisionEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncWorkDetailEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::WorkDetail
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncWorkDetailEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncWorkReleaseEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::WorkRelease
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncWorkReleaseEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(
                generated::SyncWorkResultEntityPayloadCollection::default().as_str()
            ),
            CollectionKind::WorkResult
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncWorkResultEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of(generated::SyncWorkspaceEntityPayloadCollection::default().as_str()),
            CollectionKind::Workspace
        );
        assert!(
            studio_runtime_schema::sync_entity_branch(
                generated::SyncWorkspaceEntityPayloadCollection::default().as_str()
            )
            .unwrap()
            .is_some()
        );
        assert_eq!(
            CollectionKind::of("future\0collection"),
            CollectionKind::Other
        );
        for label in [
            "transcript:a",
            "TRANSCRIPT:A",
            "Transcript:",
            "transcript:a:b",
        ] {
            let name = CollectionName::from_stored(label.into());
            assert_eq!(name.kind(), CollectionKind::Transcript);
            assert_eq!(name.as_str(), label);
            assert_eq!(
                name.agent().unwrap().as_ref(),
                label.split_once(':').unwrap().1
            );
            assert!(!name.non_transcript());
        }
        assert!(CollectionName::from_stored("transcript".into()).non_transcript());
    }
}
