import { needsAnswer } from "./approvals";
import type { Room, Snapshot } from "./types";

export const roomLeadIds = (room: Room): string[] => room.leadIds ?? [];

// A conversation belongs to one lead tree, even when two chats use the same folder.
// A missing selection must never expand into all sessions.
export function chatSnapshot(
  data: Snapshot | null,
  rootId?: string,
): Snapshot | null {
  if (!data) return null;
  const runtime = data.runtime;
  const members = data.threads.filter(
    (agent) => !!rootId && (agent.id === rootId || agent.rootId === rootId),
  );
  const ids = new Set(members.map((agent) => agent.id));
  const owns = (id?: string) => !!id && ids.has(id);
  return {
    ...data,
    threads: members,
    chats: [],
    runtime: {
      ...runtime,
      agents: runtime.agents.filter((agent) => owns(agent.id)),
      rooms: runtime.rooms.filter(
        (room) => !!rootId && roomLeadIds(room).includes(rootId),
      ),
      requests: runtime.requests.filter((request) => owns(request.agent)),
      complaints: runtime.complaints.filter(
        (complaint) => !!rootId && complaint.leadId === rootId,
      ),
      monitors: runtime.monitors.filter((monitor) =>
        owns(monitor.agent || undefined),
      ),
      tasks: runtime.tasks.filter((task) => owns(task.agent || undefined)),
      work: runtime.work.filter((task) => !!rootId && task.rootId === rootId),
      rules: runtime.rules.filter((rule) => owns(rule.agent || undefined)),
    },
  };
}

export function messageAttentionCount(data: Snapshot | null): number {
  if (!data) return 0;
  return (
    data.runtime.requests.filter(needsAnswer).length +
    data.runtime.complaints.filter((complaint) => complaint.needsUserResponse)
      .length
  );
}
