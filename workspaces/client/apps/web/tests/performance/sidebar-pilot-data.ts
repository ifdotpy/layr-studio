import type { Agent } from "../../src/types.ts";
import { snapshotFixture } from "../../src/snapshotFixture.ts";

/** Fixture sizes are shared by preparation, the browser and validation. */
export const pilotSize = { chats: 1000, visible: 30, updates: 120 };
export function pilotAgents(now = Date.now() / 1000): Agent[] {
  return Array.from({ length: pilotSize.chats }, (_, i) => ({
    id: `pilot-${i}`,
    name: `Chat ${i}`,
    kind: "agent",
    source: "managed",
    isLead: true,
    rootId: `pilot-${i}`,
    cwd: "/fixture",
    status: "completed",
    created: i < pilotSize.visible ? now : 1,
    updated: i < pilotSize.visible ? now : 1,
    threadId: `thread-${i}`,
    epoch: 0,
  }));
}
export function pilotSnapshot(agents: Agent[]) {
  return snapshotFixture(agents, {
    projects: [
      { id: "fixture", path: "/fixture", name: "Fixture", folders: [] },
    ],
    sidebarOrder: { revision: 0, groups: {} },
  });
}
export function pilotUpdate(agents: Agent[], index: number): Agent[] {
  const target = index % pilotSize.visible;
  return agents.map((agent, i) =>
    i === target
      ? {
          ...agent,
          name: `Updated ${index}`,
          status:
            Math.floor(index / pilotSize.visible) % 2 ? "completed" : "running",
        }
      : agent,
  );
}
