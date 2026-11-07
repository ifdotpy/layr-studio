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
                ObservableCollection::of::<crate::agent_tail::AgentTail>("agentTail", "agentTail"),
            ],
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
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn pilot_collections_share_the_existing_state_resource() {
        let resources = super::resources();
        assert_eq!(resources.len(), 2);
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
                "agentTail"
            ]
        );
    }
}
