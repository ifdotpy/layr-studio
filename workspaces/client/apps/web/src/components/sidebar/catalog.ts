import { AgentSource as GeneratedApiAgentSource } from "../../generated/api.ts";
import type { Agent } from "../../types";
import { retainArray } from "../../hooks/snapshotSelection";

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
export function createAppCatalogSelector() {
  let source: Agent[] | undefined;
  let byId = new Map<string, Agent>();
  let leads: Agent[] = [];
  let version = 0;
  const selected = new Map<
    string,
    { version: number; team: Agent[]; workers: Agent[] }
  >();
  const teamFor = (root: string) => {
    let cached = selected.get(root);
    if (cached?.version !== version) {
      const team = retainArray(
        cached?.team || [],
        (source || []).filter((row) => row.id === root || row.rootId === root),
      );
      const workers =
        team === cached?.team
          ? cached.workers
          : retainArray(
              cached?.workers || [],
              team.filter((row) => !row.isLead),
            );
      cached = { version, team, workers };
      selected.set(root, cached);
    }
    return cached!;
  };
  return (next: Agent[]) => {
    if (source !== next) {
      version++;
      reportSidebarModelWork("app-index", next.length);
      const nextById = new Map<string, Agent>();
      const nextLeads: Agent[] = [];
      for (const row of next) {
        nextById.set(row.id, row);
        if (
          row.source === GeneratedApiAgentSource.managed &&
          row.isLead &&
          !row.sharedRoomId
        )
          nextLeads.push(row);
      }
      byId = nextById;
      leads = retainArray(leads, nextLeads);
      source = next;
      for (const id of selected.keys()) if (!byId.has(id)) selected.delete(id);
    }
    return {
      byId,
      leads,
      team: (root: string) => teamFor(root).team,
      workers: (root: string) => teamFor(root).workers,
    };
  };
}
