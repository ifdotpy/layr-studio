import { BackgroundTaskKind } from "../../generated/api.ts";
import { AgentSource, AgentStatus } from "../../generated/api.ts";
import type { Agent, DeepReadonly } from "../../types.ts";
import type { components } from "../../generated/api.ts";
import { isCommandTask } from "../backgroundTaskModel.ts";

export interface ChatActivity {
  id: string;
  kind: "task" | "monitor" | "agent";
  agentId: string;
  agentName: string;
  label: string;
  command?: string | undefined;
  created?: number | undefined;
  status: string;
  // Monitors and commands that outlive their turn. Only these appear above the chat.
  background?: boolean | undefined;
  commandTask?: boolean | undefined;
}

export type ActivityEntry = {
  kind: "task" | "monitor";
  record: DeepReadonly<
    | components["schemas"]["TaskRecordView"]
    | components["schemas"]["TaskProjection"]
    | components["schemas"]["ProcessView"]
  >;
};
export function activityEntries(
  activities: DeepReadonly<components["schemas"]["BackgroundTaskView"][]>,
): ActivityEntry[] {
  return activities.map((activity) => ({
    kind:
      activity.value.display.kind === BackgroundTaskKind["monitor"]
        ? "monitor"
        : "task",
    record: activity.value,
  }));
}

export function backgroundActivities(activities: ChatActivity[]) {
  return activities.filter(
    (activity) =>
      activity.background &&
      (activity.kind === "monitor" || activity.commandTask),
  );
}

export function chatActivities(
  source: readonly Agent[],
  entries: readonly ActivityEntry[],
): Map<string, ChatActivity[]> {
  const agents = source.filter((agent) => agent.source === AgentSource.managed);
  const byId = new Map(agents.map((agent) => [agent.id, agent]));
  const result = new Map<string, ChatActivity[]>();
  const concrete = new Set<string>();
  const add = (agent: Agent, activity: ChatActivity) => {
    const ids = new Set([agent.id]);
    if (agent.rootId && byId.has(agent.rootId)) ids.add(agent.rootId);
    for (const id of ids) {
      result.set(id, [...(result.get(id) || []), activity]);
    }
  };
  for (const { kind, record } of entries) {
    const agent = byId.get(record.agent || "");
    if (
      !agent ||
      !["starting", "running", "approval"].includes(record.status || "")
    )
      continue;
    concrete.add(agent.id);
    const turnId =
      "turnId" in record && typeof record.turnId === "string"
        ? record.turnId
        : undefined;
    const background =
      kind === "monitor" ||
      agent.inFlight === false ||
      !!(turnId && agent.turnId && agent.turnId !== turnId);
    const command =
      record.command ||
      (kind === "task" && "query" in record ? record.query : undefined) ||
      record.name ||
      (kind === "task"
        ? "kind" in record
          ? record.kind
          : record.display.kind === BackgroundTaskKind["command"]
            ? "command"
            : "tool"
        : undefined) ||
      undefined;
    const commandTask = kind === "task" && isCommandTask(record);
    add(agent, {
      id: record.id,
      kind,
      agentId: agent.id,
      agentName: agent.name || "Agent",
      label:
        kind === "monitor" || (background && commandTask)
          ? "Background command"
          : commandTask
            ? "Command"
            : "Tool call",
      command,
      created: record.created ?? undefined,
      status: record.status || "",
      background,
      commandTask,
    });
  }
  for (const agent of agents) {
    if (concrete.has(agent.id)) continue;
    if (
      agent.inFlight ||
      (agent.autoWake !== false &&
        ["queued", "starting", "running"].includes(agent.status || ""))
    )
      add(agent, {
        id: agent.id,
        kind: "agent",
        agentId: agent.id,
        agentName: agent.name || "Agent",
        label:
          agent.status === AgentStatus.queued
            ? "Waiting to start"
            : agent.status === AgentStatus.starting
              ? "Starting"
              : "Working",
        status: agent.status || "",
      });
  }
  for (const entries of result.values())
    entries.sort(
      (a, b) =>
        (a.created ?? Infinity) - (b.created ?? Infinity) ||
        a.id.localeCompare(b.id),
    );
  return result;
}
