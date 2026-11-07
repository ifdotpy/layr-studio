import { AgentSource as GeneratedApiAgentSource } from "../../generated/api.ts";
import { required } from "../componentTestFixtures";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Agent } from "../../types";
import { mostRecentActivity } from "./catalog";
import { createAgentCatalog, createTeamView } from "../agents/state.ts";
import { observableFixture } from "../observableFixture.tsx";
import { autorun } from "mobx";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";

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
    const rows = [
      agent("lead"),
      agent("worker", { isLead: false, rootId: "lead" }),
      agent("other"),
    ];
    const fixture = observableFixture(rows);
    const catalog = createAgentCatalog(fixture.store);
    const lead = createTeamView(fixture.store, "lead");
    const otherTeam = createTeamView(fixture.store, "other");
    const stop = autorun(() => {
      void catalog.leads;
      void lead.members;
      void lead.workers;
      void otherTeam.members;
    });
    const select = (rows: Agent[]) => {
      fixture.apply(
        rows.map((row) => ({
          collection: "agentSummary" as const,
          id: row.id,
          value: {
            ...agentSummaryFixture(row.id),
            ...row,
            inFlight: row.inFlight === true,
            projectServerId: row.projectServerId ?? null,
          },
        })),
      );
      return {
        leads: catalog.leads,
        byId: fixture.mirror.current.agents,
        team: (id: string) =>
          id === "lead" ? lead.members : otherTeam.members,
        workers: () => lead.workers,
      };
    };
    const before = select(rows);
    const beforeTeam = before.team("lead");
    const beforeWorkers = before.workers();
    expect(before.team("lead").map((row) => row.id)).toEqual(
      rows.slice(0, 2).map((row) => row.id),
    );
    expect(select(rows).leads).toBe(before.leads);
    const after = select([
      required(rows[0]),
      required(rows[1]),
      { ...required(rows[2]), name: "Changed" },
    ]);
    expect(after.team("lead")).toBe(beforeTeam);
    expect(after.workers()).toBe(beforeWorkers);
    const moved = select([
      required(rows[0]),
      { ...required(rows[1]), rootId: "other" },
      required(rows[2]),
    ]);
    expect(moved.workers()).toEqual([]);
    expect(moved.team("other")?.map((row) => row.id)).toEqual([
      "worker",
      "other",
    ]);
    expect(select([]).byId.size).toBe(0);
    stop();
    fixture.store.dispose();
  });
});
