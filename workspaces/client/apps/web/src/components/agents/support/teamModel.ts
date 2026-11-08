import { AgentStatus } from "../../../generated/api.ts";
import type { Agent } from "../../../types.ts";
import {
  CHAT_INDICATOR_URGENCY,
  type ChatIndicator,
  type ChatIndicatorKind,
} from "../../../status/operationalStatus.ts";

export const teamFilterValue = (state: string) =>
  state === "working" ? "active" : state === "error" ? "attention" : state;
export function filterTeamWorkers<T extends Agent>(
  workers: T[],
  indicators: Map<string, ChatIndicator>,
  filter: string,
) {
  if (filter === "all") return workers;
  const { groups, completed } = teamDispatchGroups(workers, indicators);
  return filter === "completed"
    ? completed
    : groups.find((group) => teamFilterValue(group.kind) === filter)?.workers ||
        [];
}

/** Indentation follows the server-owned parent relation, including filtered parents. */
export function workerDepth(
  agent: Agent,
  agents: Map<string, Agent>,
  implicitRootId?: string,
) {
  const seen = new Set([agent.id]);
  let parentId = agent.parentId;
  let depth = 0;
  while (parentId && parentId !== implicitRootId && !seen.has(parentId)) {
    seen.add(parentId);
    depth++;
    parentId = agents.get(parentId)?.parentId;
  }
  return depth;
}

const teamIndicatorLabels: Record<ChatIndicatorKind, string> = {
  loading: "",
  answer: "Needs you",
  error: "Failed",
  working: "Working",
  unread: "New results",
  paused: "Stopped",
  none: "Waiting",
};
/** Display groups consume the same state owner as each rail. Completed results
 * remain in the native Finished disclosure unless they need operator action. */
export function teamDispatchGroups<T extends Agent>(
  workers: T[],
  indicators: Map<string, ChatIndicator>,
) {
  const kindOf = (agent: Agent) => indicators.get(agent.id)?.kind || "none";
  const finished = (agent: Agent) =>
    agent.status === AgentStatus.completed &&
    !agent.stall &&
    !["answer", "error", "working"].includes(kindOf(agent));
  return {
    groups: CHAT_INDICATOR_URGENCY.map((kind) => ({
      kind,
      name: teamIndicatorLabels[kind],
      workers: workers.filter(
        (agent) => !finished(agent) && kindOf(agent) === kind,
      ),
    })),
    completed: workers.filter(finished),
  };
}

/** Keep parents before descendants within each urgency lane. */
export function orderWorkerHierarchy<T extends Agent>(workers: T[]) {
  const ids = new Set(workers.map((agent) => agent.id));
  const children = new Map<string, T[]>();
  for (const agent of workers) {
    if (!agent.parentId || !ids.has(agent.parentId)) continue;
    const siblings = children.get(agent.parentId) || [];
    siblings.push(agent);
    children.set(agent.parentId, siblings);
  }
  const ordered: T[] = [];
  const seen = new Set<string>();
  const visit = (roots: T[]) => {
    const pending = roots.slice().reverse();
    while (pending.length) {
      const agent = pending.pop()!;
      if (seen.has(agent.id)) continue;
      seen.add(agent.id);
      ordered.push(agent);
      pending.push(...(children.get(agent.id) || []).slice().reverse());
    }
  };
  visit(workers.filter((agent) => !agent.parentId || !ids.has(agent.parentId)));
  visit(workers); // Broken parent cycles remain visible and cannot loop the renderer.
  return ordered;
}
