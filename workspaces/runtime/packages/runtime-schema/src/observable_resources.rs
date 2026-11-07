//! Selects existing state entities for the generated observable collections.
pub use studio_observable_codegen::{
    ObservableArg, ObservableCollection, ObservableModel, ObservableResource,
};

pub fn models() -> Vec<ObservableModel> {
    vec![
        ObservableModel::named("AgentSummaryEntityDto")
            .identity("id")
            .track_nested(&["operationalStatus", "markRead", "startWait"]),
        ObservableModel::named("AgentDetailEntityDto")
            .identity("id")
            .opaque("error")
            .track_nested(&[
                "overview",
                "activity",
                "commandProgress",
                "error",
                "nativeThreadBlock",
                "nativeSafetyBuffering",
                "startAttempt",
                "worktreePreparation",
                "nativeRelease",
                "workspaceCleanupIntent",
                "worktree",
                "startWait",
            ]),
        ObservableModel::named("ProjectEntityDto")
            .identity("id")
            .opaque("folders"),
        ObservableModel::named("NativeThreadBlockDto").opaque("error"),
        ObservableModel::named("BackgroundTaskView").keyed_by(&["source", "value.id"]),
        ObservableModel::named("ProcessView").identity("id"),
        ObservableModel::named("Approval"),
        ObservableModel::named("Request")
            .opaque("schema")
            .opaque("input"),
        ObservableModel::named("Answer").opaque("content"),
        ObservableModel::named("Detail")
            .opaque("params")
            .opaque("preview"),
        ObservableModel::named("Question").identity("id"),
        ObservableModel::named("QuestionAnswer").identity("id"),
        ObservableModel::named("TaskDetailResponse"),
        ObservableModel::named("WorkSubscriptionData").declared_fields(),
        ObservableModel::named("WorkDetail")
            .declared_fields()
            .identity("id")
            .opaque("resultSchema")
            .opaque("gate")
            .track_nested(&["results", "decisions", "submissionRefusal", "policy"]),
        ObservableModel::named("WorkResultRecord")
            .declared_fields()
            .identity("id")
            .opaque("result")
            .track_nested(&["gate"]),
        ObservableModel::named("WorkDecisionRecord").declared_fields(),
        ObservableModel::named("TaskGateEvidence").track_nested(&[]),
        ObservableModel::named("TerminalRecord")
            .declared_fields()
            .identity("id"),
        ObservableModel::named("RetainedWorktreeEntityDto").identity("id"),
        ObservableModel::named("MonitorEntityDto").identity("id"),
        ObservableModel::named("RoomEntityDto")
            .declared_fields()
            .identity("id")
            .track_nested(&["radio", "lastMessage"]),
        ObservableModel::named("RoomDetailDto")
            .declared_fields()
            .identity("id")
            .track_nested(&["radio"]),
        ObservableModel::named("RoomRadio")
            .declared_fields()
            .track_nested(&["active", "controls"]),
        ObservableModel::named("RoomRadioActive").opaque("identity"),
        ObservableModel::named("RadioControls")
            .declared_fields()
            .omit("busyNotice"),
        ObservableModel::named("ComplaintEntityDto").identity("id"),
        ObservableModel::named("ComplaintDetailResponse").identity("id"),
        ObservableModel::named("ComplaintResponseDto").identity("id"),
        ObservableModel::named("MessageReceiptEntityDto").identity("id"),
        ObservableModel::named("QueueEntityDto"),
        ObservableModel::named("QueueItem").identity("id"),
        ObservableModel::named("ChatRead").declared_fields(),
        ObservableModel::named("ChatMessage")
            .identity("id")
            .opaque("deliveries"),
        ObservableModel::named("MessageRecord")
            .identity("id")
            .opaque("deliveries"),
        ObservableModel::named("AnalyticsTurnRecord")
            .declared_fields()
            .track_nested(&[]),
        ObservableModel::named("TranscriptItemResponse")
            .declared_fields()
            .identity("id")
            .track_nested(&["inputs"]),
        ObservableModel::named("TranscriptData"),
        ObservableModel::named("TranscriptMessageRecord")
            .declared_fields()
            .identity("id")
            .track_nested(&["inputs", "fullTextBlob"])
            .opaque("details")
            .opaque("nativeError")
            .opaque("previousError"),
        ObservableModel::named("TranscriptInput")
            .identity_or_index("id")
            .declared_fields()
            .track_nested(&[]),
        ObservableModel::named("PeerTeamEntityDto").identity("id"),
        ObservableModel::named("WorkspaceEntityDto").opaque("nativeNotices"),
        ObservableModel::named("SidebarOrderDto").opaque("groups"),
        ObservableModel::of::<crate::agent_tail::AgentTail>().identity("id"),
    ]
}
pub fn resources() -> Vec<ObservableResource> {
    vec![
        ObservableResource::collections(
            "state",
            "state",
            "SyncEntityPayload",
            vec![
                ObservableCollection::named("agentSummary", "agents", "AgentSummaryEntityDto"),
                ObservableCollection::named("project", "projects", "ProjectEntityDto"),
                ObservableCollection::named("peerTeam", "peerTeams", "PeerTeamEntityDto"),
                ObservableCollection::named("workspace", "workspaces", "WorkspaceEntityDto"),
                ObservableCollection::named("room", "rooms", "RoomEntityDto"),
                ObservableCollection::named("complaint", "complaints", "ComplaintEntityDto"),
                ObservableCollection::named(
                    "messageReceipt",
                    "messageReceipts",
                    "MessageReceiptEntityDto",
                ),
                ObservableCollection::named("queue", "queues", "QueueEntityDto"),
                ObservableCollection::of::<crate::agent_tail::AgentTail>("agentTail", "agentTail"),
            ],
        ),
        ObservableResource::collections(
            "state",
            "requests",
            "SyncEntityPayload",
            vec![ObservableCollection::named(
                "request", "requests", "Approval",
            )],
        ),
        ObservableResource::collections(
            "state",
            "questionHistory",
            "SyncEntityPayload",
            vec![ObservableCollection::named(
                "questionHistory",
                "questionHistory",
                "Approval",
            )],
        ),
        ObservableResource::named(
            "agentDetail",
            "agentDetail",
            "AgentDetailEntityDto",
            vec![ObservableArg {
                name: "agentId".into(),
                brand: "AgentId".into(),
            }],
        ),
        ObservableResource::named(
            "processes",
            "processes",
            "ProcessList",
            vec![ObservableArg {
                name: "agentId".into(),
                brand: "AgentId".into(),
            }],
        ),
        ObservableResource::named(
            "taskDetail",
            "taskDetail",
            "TaskDetailResponse",
            vec![ObservableArg {
                name: "taskId".into(),
                brand: "TaskId".into(),
            }],
        ),
        ObservableResource::mapped(
            "monitors",
            "monitors",
            vec![ObservableArg {
                name: "agentId".into(),
                brand: "AgentId".into(),
            }],
        ),
        ObservableResource::named(
            "work",
            "work",
            "WorkSubscriptionData",
            vec![ObservableArg {
                name: "agentId".into(),
                brand: "AgentId".into(),
            }],
        ),
        ObservableResource::mapped(
            "tasks",
            "tasks",
            vec![ObservableArg {
                name: "agentId".into(),
                brand: "AgentId".into(),
            }],
        ),
        ObservableResource::mapped("terminals", "terminals", vec![]),
        ObservableResource::named(
            "terminal",
            "terminal",
            "TerminalRecord",
            vec![ObservableArg {
                name: "terminalId".into(),
                brand: "TerminalId".into(),
            }],
        ),
        ObservableResource::named(
            "terminal-output",
            "terminalOutput",
            "TerminalOutput",
            vec![ObservableArg {
                name: "terminalId".into(),
                brand: "TerminalId".into(),
            }],
        ),
        ObservableResource::mapped(
            "retained-worktrees",
            "retainedWorktrees",
            vec![ObservableArg {
                name: "agentId".into(),
                brand: "AgentId".into(),
            }],
        ),
        ObservableResource::mapped(
            "room",
            "room",
            vec![
                ObservableArg {
                    name: "roomId".into(),
                    brand: "RoomId".into(),
                },
                ObservableArg {
                    name: "legacy".into(),
                    brand: "RoomLegacy".into(),
                },
            ],
        ),
        ObservableResource::named(
            "complaint",
            "complaint",
            "ComplaintDetailResponse",
            vec![ObservableArg {
                name: "complaintId".into(),
                brand: "ComplaintId".into(),
            }],
        ),
        ObservableResource::mapped(
            "transcript",
            "transcript",
            vec![
                ObservableArg {
                    name: "agentId".into(),
                    brand: "AgentId".into(),
                },
                ObservableArg {
                    name: "tail".into(),
                    brand: "TranscriptTail".into(),
                },
            ],
        ),
        ObservableResource::mirror("TextBlobResponse", "textBlob"),
        ObservableResource::mirror("ResourceLiveDeltaEvent", "liveDelta"),
        ObservableResource::mirror("TranscriptItemResponse", "transcriptItem"),
        ObservableResource::mirror("AnalyticsTurnRecord", "turnError"),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn task_terminal_and_retained_resources_are_generated_and_scoped() {
        let resources = super::resources();
        for kind in [
            "work",
            "tasks",
            "terminal",
            "terminal-output",
            "retained-worktrees",
        ] {
            let resource = resources
                .iter()
                .find(|resource| resource.kind == kind)
                .unwrap_or_else(|| panic!("missing generated resource: {kind}"));
            assert!(!resource.args.is_empty(), "{kind} must be scoped");
        }
    }

    #[test]
    fn pilot_collections_share_the_existing_state_resource() {
        let resources = super::resources();
        assert_eq!(resources.len(), 20);
        assert_eq!(resources[0].kind, "state");
        assert_eq!(
            resources[0]
                .collections
                .iter()
                .map(|item| item.collection.as_str())
                .collect::<Vec<_>>(),
            [
                "agentSummary",
                "project",
                "peerTeam",
                "workspace",
                "room",
                "complaint",
                "messageReceipt",
                "queue",
                "agentTail"
            ]
        );
    }
}
