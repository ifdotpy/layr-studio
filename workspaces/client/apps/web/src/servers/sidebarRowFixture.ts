import type { SidebarChatRow } from "../generated/observableResources.ts";

export function sidebarRowFixture(
  id: string,
  patch: Partial<SidebarChatRow> = {},
): SidebarChatRow {
  return {
    id,
    name: id,
    projectPath: "/same/project",
    projectId: null,
    projectServerId: null,
    projectFolder: null,
    projectFolderRevision: 0,
    teamId: null,
    groupKey: JSON.stringify(["items", "/same/project", ""]),
    orderRank: null,
    sortKey: id,
    activityAt: 1,
    pinned: false,
    archived: false,
    provider: "codex",
    model: null,
    indicator: { kind: "none", label: "Ready" },
    markRead: null,
    markUnread: "no_completed_result",
    canChangeProject: true,
    canArchive: true,
    compactPinned: false,
    searchText: id.toLowerCase(),
    copyProgress: null,
    waitingForLead: false,
    inFlight: false,
    ...patch,
  };
}
