import {
  AgentSource,
  AgentStatus,
  MarkUnreadEligibility,
  ProviderKind,
} from "../generated/api.ts";
import type { components } from "../generated/api.ts";

type AgentSummary = components["schemas"]["AgentSummaryEntityDto"];

/** Test-only summary facts. Scenarios state operational and read outcomes explicitly. */
export function agentSummaryFixture(
  id: string,
  overrides: Partial<AgentSummary> = {},
): AgentSummary {
  return {
    id,
    name: id,
    kind: "agent",
    source: AgentSource.managed,
    isLead: true,
    rootId: id,
    parentId: null,
    cwd: "/same/project",
    projectPath: "/same/project",
    projectId: null,
    projectServerId: null,
    projectFolder: null,
    projectFolderRevision: 0,
    created: 1,
    updated: 1,
    pinned: false,
    archived: false,
    provider: ProviderKind.codex,
    model: null,
    status: AgentStatus.completed,
    autoWake: true,
    inFlight: false,
    operationalStatus: { reason: "ready" },
    markRead: null,
    markUnread: MarkUnreadEligibility.no_completed_result,
    readStateSupported: true,
    threadId: null,
    lastCompletedTurn: null,
    lastCompletedTurnStatus: null,
    readState: null,
    stall: null,
    startWait: null,
    ...overrides,
  };
}
