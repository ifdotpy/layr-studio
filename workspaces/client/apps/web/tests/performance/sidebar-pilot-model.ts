import type { Agent } from "../../src/types.ts";
import { createSidebarMirror } from "../../src/generated/observableResources.ts";
import { sidebarRowFixture } from "../../src/servers/sidebarRowFixture.ts";
import { pilotSnapshot } from "./sidebar-pilot-data.ts";

export function pilotRow(agent: Agent) {
  return sidebarRowFixture(agent.id, {
    name: agent.name || "",
    projectPath: agent.cwd || "/fixture",
    groupKey: JSON.stringify(["items", agent.cwd || "/fixture", null]),
    sortKey: agent.id,
    activityAt: agent.updated || agent.created || 1,
    searchText: `${agent.name} ${agent.cwd}`.toLowerCase(),
    indicator:
      agent.status === "running"
        ? { kind: "working", label: "Working" }
        : { kind: "none", label: "Read" },
  });
}
export function createPilotModel(agents: Agent[], _notify: () => void) {
  const data = pilotSnapshot(agents);
  const mirror = createSidebarMirror();
  const apply = (next: Agent[]) => mirror.apply({ rows: next.map(pilotRow) });
  apply(agents);
  return { view: () => ({ data, rows: mirror.current.rows }), apply };
}
