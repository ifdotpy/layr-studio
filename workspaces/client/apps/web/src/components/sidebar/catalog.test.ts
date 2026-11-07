import { AgentSource as GeneratedApiAgentSource } from "../../generated/api.ts";
import { required } from "../componentTestFixtures";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Agent } from "../../types";
import { createAppCatalogSelector, mostRecentActivity } from "./catalog";

const agent = (id: string, fields: Partial<Agent> = {}): Agent => ({
  id,
  name: id,
  source: GeneratedApiAgentSource.managed,
  isLead: true,
  cwd: "/workspace/line",
  projectPath: "/work",
  created: 1,
  ...fields,
});
afterEach(() => vi.unstubAllGlobals());

describe("App catalog", () => {
  it("selects the most recent activity with a stable ID tie-break", () => {
    expect(
      mostRecentActivity([
        agent("created", { created: 100 }),
        agent("updated", { created: 1, updated: 50 }),
      ])?.id,
    ).toBe("created");
    expect(
      mostRecentActivity([
        agent("z", { created: 10 }),
        agent("a", { created: 10 }),
      ])?.id,
    ).toBe("a");
    expect(
      mostRecentActivity([
        agent("z", { created: undefined, updated: undefined }),
        agent("a", { created: undefined, updated: undefined }),
      ])?.id,
    ).toBe("a");
    expect(mostRecentActivity([])).toBeUndefined();
  });

  it("retains another tree and updates membership, source order and root selection", () => {
    const select = createAppCatalogSelector();
    const rows = [
      agent("lead"),
      agent("worker", { isLead: false, rootId: "lead" }),
      agent("other"),
    ];
    const before = select(rows);
    const beforeTeam = before.team("lead");
    const beforeWorkers = before.workers("lead");
    expect(before.team("lead")).toEqual(rows.slice(0, 2));
    expect(select(rows).leads).toBe(before.leads);
    const after = select([
      required(rows[0]),
      required(rows[1]),
      { ...required(rows[2]), name: "Changed" },
    ]);
    expect(after.team("lead")).toBe(beforeTeam);
    expect(after.workers("lead")).toBe(beforeWorkers);
    const moved = select([
      required(rows[0]),
      { ...required(rows[1]), rootId: "other" },
      required(rows[2]),
    ]);
    expect(moved.workers("lead")).toEqual([]);
    expect(moved.team("other")?.map((row) => row.id)).toEqual([
      "worker",
      "other",
    ]);
    expect(select([]).byId.size).toBe(0);
  });
});
