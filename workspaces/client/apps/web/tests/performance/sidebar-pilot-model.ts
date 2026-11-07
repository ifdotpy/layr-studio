import type { Agent } from "../../src/types.ts";
import type { components } from "../../src/generated/api.ts";
import { createStateMirror } from "../../src/generated/observableResources.ts";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import { pilotSnapshot } from "./sidebar-pilot-data.ts";

/** Explicit server facts; presentation comes from the actual shared view-model. */
export function pilotEntities(
  agents: Agent[],
): components["schemas"]["SyncEntityPayload"][] {
  return agents.map((agent) => ({
    collection: "agentSummary",
    id: agent.id,
    value: agentSummaryFixture(agent.id, {
      name: agent.name || "",
      cwd: agent.cwd || "/fixture",
      projectPath: agent.cwd || "/fixture",
      status: agent.status,
      created: agent.created,
      updated: agent.updated,
      threadId: agent.threadId,
      epoch: agent.epoch,
      operationalStatus:
        agent.status === "running"
          ? { reason: "working" }
          : { reason: "read_result" },
    }),
  }));
}
export function createPilotModel(agents: Agent[], _notify: () => void) {
  const data = pilotSnapshot(agents);
  const mirror = createStateMirror();
  const apply = (next: Agent[]) => mirror.apply(pilotEntities(next));
  apply(agents);
  // Sidebar builds its shared computed view-model over the actual state maps.
  return { view: () => ({ data, collections: mirror.current }), apply };
}
