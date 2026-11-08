import type { Agent } from "../../src/types.ts";
import { componentSnapshot as snapshotFixture } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";

/** Fixture sizes are shared by preparation, the browser and validation. */
export const pilotSize = { chats: 1000, visible: 30, updates: 120 };
export type PilotAgent = Agent & {
  operationalStatus: NonNullable<Agent["operationalStatus"]>;
  expectedIndicator: "none" | "working";
};
export function pilotAgents(now = Date.now() / 1000): PilotAgent[] {
  return Array.from({ length: pilotSize.chats }, (_, i) => ({
    id: `pilot-${i}`,
    name: `Chat ${i}`,
    kind: "agent",
    source: "managed",
    isLead: true,
    rootId: `pilot-${i}`,
    cwd: "/fixture",
    projectPath: "/fixture",
    status: "completed",
    operationalStatus: { reason: "read_result" },
    expectedIndicator: "none",
    created: i < pilotSize.visible ? now : 1,
    updated: i < pilotSize.visible ? now : 1,
    threadId: `thread-${i}`,
    epoch: 0,
  }));
}
export function pilotSnapshot(agents: Agent[]) {
  return Object.assign(
    snapshotFixture(agents, {
      projects: [
        { id: "fixture", path: "/fixture", name: "Fixture", folders: [] },
      ],
      sidebarOrder: { revision: 0, groups: {} },
    }),
    { token: "", stateDir: "/fixture" },
  );
}
export function pilotUpdate(agents: PilotAgent[], index: number): PilotAgent[] {
  const target = index % pilotSize.visible;
  return agents.map<PilotAgent>((agent, i) =>
    i === target
      ? {
          ...agent,
          name: `Updated ${index}`,
          ...(Math.floor(index / pilotSize.visible) % 2
            ? {
                status: "completed",
                operationalStatus: { reason: "read_result" },
                expectedIndicator: "none",
              }
            : {
                status: "running",
                operationalStatus: { reason: "working" },
                expectedIndicator: "working",
              }),
        }
      : agent,
  );
}
