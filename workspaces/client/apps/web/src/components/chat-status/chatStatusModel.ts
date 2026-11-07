import { AgentSource, AgentStatus } from "../../generated/api";
import { needsAnswer } from "../../approvals";
import type { Agent, Snapshot } from "../../types";
import type { components } from "../../generated/api";
import { isCommandTask } from "../backgroundTaskModel";
import {
  formatOperationalStatus,
  waitLabel,
} from "../../status/operationalStatus.ts";
import type { ChatIndicatorKind } from "../../status/operationalStatus.ts";
export type {
  ChatIndicatorKind,
  ChatIndicator,
} from "../../status/operationalStatus.ts";

/** Operator urgency, most urgent first. Owns ordering for the fleet board and the team panel. */
export const CHAT_INDICATOR_URGENCY: readonly ChatIndicatorKind[] = [
  "answer",
  "error",
  "working",
  "unread",
  "paused",
  "none",
  "loading",
];
export type ReadState = components["schemas"]["ReadStateDto"];

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

export const endedWaitLabel = formatOperationalStatus({
  reason: "turn_ended",
}).label;

function epochMatches(epoch: unknown, agentEpoch: number | null | undefined) {
  if (epoch == null) return true;
  return (
    typeof epoch === "number" &&
    Number.isFinite(epoch) &&
    (agentEpoch == null || epoch === agentEpoch)
  );
}

export interface ChatWaitState {
  live: boolean;
  label: string;
  agents: Agent[];
  commands: ChatActivity[];
  monitors: ChatActivity[];
  event?: string | undefined;
  inputs: number;
}

export function chatChildren(data: Snapshot): Map<string, Agent[]> {
  const children = new Map<string, Agent[]>();
  for (const child of data.threads) {
    if (child.source !== AgentSource.managed || !child.parentId) continue;
    const siblings = children.get(child.parentId);
    if (siblings) siblings.push(child);
    else children.set(child.parentId, [child]);
  }
  return children;
}

// A saved waiting status alone cannot promise another turn.
export function chatWaitState(
  data: Snapshot,
  agent: Agent,
  activities = chatActivities(data),
  children = chatChildren(data),
): ChatWaitState {
  const runtime = data.runtime;
  const hasWork = (child: Agent) => {
    const pending = [child];
    const seen = new Set([agent.id]);
    while (pending.length) {
      const current = pending.pop()!;
      if (seen.has(current.id)) continue;
      seen.add(current.id);
      if (
        current.archived ||
        ["paused", "failed", "interrupted"].includes(current.status || "")
      )
        continue;
      if (
        current.inFlight ||
        ["queued", "starting", "running", "approval"].includes(
          current.status || "",
        )
      )
        return true;
      const parked =
        current.status === AgentStatus.parked && !!current.parkedEvent;
      if (
        (current.autoWake === false && !parked) ||
        !["waiting", "parked", "waiting_on_monitor"].includes(
          current.status || "",
        )
      )
        continue;
      if (parked) return true;
      // Use the same owner and epoch checks as the visible activity records.
      if (
        activities
          .get(current.id)
          ?.some(
            (activity) =>
              activity.agentId === current.id &&
              (activity.kind === "monitor" ||
                (activity.kind === "task" && activity.commandTask)),
          )
      )
        return true;
      pending.push(...(children.get(current.id) || []));
    }
    return false;
  };
  const agents = (children.get(agent.id) || []).filter(hasWork);
  const owned = activities
    .get(agent.id)
    ?.filter((activity) => activity.agentId === agent.id);
  const commands =
    owned?.filter(
      (activity) =>
        activity.kind === "task" &&
        activity.commandTask &&
        (activity.background || !agent.inFlight),
    ) ?? [];
  const monitors =
    owned?.filter((activity) => activity.kind === "monitor") ?? [];
  const inputs =
    runtime?.requests?.filter(
      (request) =>
        request.agent === agent.id &&
        needsAnswer(request) &&
        epochMatches(request.epoch, agent.epoch),
    ).length ?? 0;
  const event = agent.parkedEvent || undefined;
  const live =
    agents.length > 0 ||
    commands.length > 0 ||
    monitors.length > 0 ||
    !!event ||
    inputs > 0;
  const label = live
    ? waitLabel({
        agents: agents.length,
        commands: commands.length,
        monitors: monitors.length,
        inputs,
        event: event ?? null,
      })
    : formatOperationalStatus({ reason: "turn_ended" }).label;
  return {
    live,
    label,
    agents,
    commands,
    monitors,
    event,
    inputs,
  };
}

export function backgroundActivities(activities: ChatActivity[]) {
  return activities.filter(
    (activity) =>
      activity.background &&
      (activity.kind === "monitor" || activity.commandTask),
  );
}

// The visible reasons and the spinner use the same activity records.
export function chatActivities(data: Snapshot): Map<string, ChatActivity[]> {
  const runtime = data.runtime;
  const agents = data.threads.filter(
    (agent) => agent.source === AgentSource.managed,
  );
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
  const entries = runtime
    ? [
        ...(runtime.tasks || []).map((record) => ({
          kind: "task" as const,
          record,
        })),
        ...(runtime.monitors || []).map((record) => ({
          kind: "monitor" as const,
          record,
        })),
      ]
    : [];
  for (const { kind, record } of entries) {
    const agent = byId.get(record.agent || "");
    if (
      !agent ||
      !["starting", "running", "approval"].includes(record.status || "")
    )
      continue;
    concrete.add(agent.id);
    const background =
      kind === "monitor" ||
      agent.inFlight === false ||
      !!(record.turnId && agent.turnId && agent.turnId !== record.turnId);
    const command =
      record.command ||
      (kind === "task" ? record.query : undefined) ||
      record.name ||
      (kind === "task" ? record.kind : undefined) ||
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
export function hasCompletedResult(agent: Agent) {
  return !!(
    agent.threadId &&
    agent.lastCompletedTurn &&
    agent.lastCompletedTurnStatus === "completed"
  );
}
export function unreadResult(
  agent: Agent,
  readState: ReadState | null = savedReadState(agent),
) {
  return (
    hasCompletedResult(agent) &&
    !(
      readState?.read === true &&
      readState.threadId === agent.threadId &&
      readState.turnId === agent.lastCompletedTurn
    )
  );
}

function savedReadState(agent: Agent): ReadState | null {
  return agent.readState ?? null;
}
