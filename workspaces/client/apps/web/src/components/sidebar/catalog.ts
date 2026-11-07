import { AgentSource as GeneratedApiAgentSource } from "../../generated/api.ts";
import type { Agent } from "../../types";

export type SidebarModelWork =
  | "catalog-row"
  | "order-compare"
  | "search-row"
  | "app-index";
declare global {
  interface Window {
    /** Optional counter for isolated browser performance checks. */
    __studioSidebarModelProbe?: (work: SidebarModelWork, count: number) => void;
  }
}
export function reportSidebarModelWork(work: SidebarModelWork, count = 1) {
  if (typeof window !== "undefined")
    window.__studioSidebarModelProbe?.(work, count);
}
/** A moved chat shows only at its target: the source keeps a move record without a
 * mailbox, and the target shows nothing until its import finishes. */
export function isVisibleSidebarAgent(
  agent: Pick<
    Agent,
    | "source"
    | "isLead"
    | "deletedAt"
    | "sharedRoomId"
    | "movedTo"
    | "moveImportPending"
  >,
) {
  return (
    agent.source === GeneratedApiAgentSource.managed &&
    agent.isLead &&
    !agent.deletedAt &&
    !agent.sharedRoomId &&
    !agent.movedTo &&
    !agent.moveImportPending
  );
}

/** Select the most recent activity, with ascending ID for timestamp ties. */
export function mostRecentActivity<
  T extends Pick<Agent, "id" | "updated" | "created">,
>(rows: T[]) {
  let selected: T | undefined;
  for (const row of rows) {
    if (!selected) {
      selected = row;
      continue;
    }
    const updated = row.updated ?? row.created ?? 0;
    const selectedUpdated = selected.updated ?? selected.created ?? 0;
    if (
      updated > selectedUpdated ||
      (updated === selectedUpdated && row.id < selected.id)
    )
      selected = row;
  }
  return selected;
}

export function countProjectChats(
  agents: readonly Pick<Agent, "projectPath" | "archived">[],
  archived: boolean,
) {
  const counts = new Map<string, number>();
  for (const agent of agents)
    if (!!agent.archived === archived) {
      const path = agent.projectPath || "";
      counts.set(path, (counts.get(path) || 0) + 1);
    }
  return counts;
}
export function itemGroup(path: string, parent: string | null = null) {
  return JSON.stringify(["items", path, parent]);
}
