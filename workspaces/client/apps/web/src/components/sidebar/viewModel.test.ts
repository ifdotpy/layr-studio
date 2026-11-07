import { autorun, observable, runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import { createStateMirror } from "../../generated/observableResources.ts";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { createSidebarViewModel } from "./viewModel.ts";

it("keeps grouping and order independent of tails while search observes them only when open", () => {
  const mirror = createStateMirror();
  const agent = agentSummaryFixture("a");
  const other = agentSummaryFixture("b");
  const publish = (tail: string) =>
    mirror.apply([
      { collection: "agentSummary", id: agent.id, value: agent },
      { collection: "agentSummary", id: other.id, value: other },
      { collection: "agentTail", id: agent.id, value: { id: agent.id, tail } },
    ]);
  publish("first");
  const controls = observable({ query: "" });
  const rank = vi.fn(() => 0);
  const model = createSidebarViewModel(
    () => mirror.current,
    () => ({
      query: controls.query,
      archived: false,
      opened: null,
      rank,
    }),
  );
  const groups = vi.fn(),
    search = vi.fn();
  const stopGroups = autorun(() => groups(model.groups));
  const stopSearch = autorun(() =>
    search(model.searchResults.map((row) => row.id)),
  );
  const original = model.groups;
  const comparisonsBeforeTail = rank.mock.calls.length;
  expect(comparisonsBeforeTail).toBeGreaterThan(0);
  publish("needle");
  expect(rank).toHaveBeenCalledTimes(comparisonsBeforeTail);
  expect(model.groups).toBe(original);
  expect(groups).toHaveBeenCalledTimes(1);
  expect(search).toHaveBeenCalledTimes(1);
  runInAction(() => {
    controls.query = "needle";
  });
  expect(search).toHaveBeenLastCalledWith(["a"]);
  const groupsAfterOpeningSearch = groups.mock.calls.length;
  const comparisonsAfterOpeningSearch = rank.mock.calls.length;
  publish("other");
  expect(search).toHaveBeenLastCalledWith([]);
  expect(groups).toHaveBeenCalledTimes(groupsAfterOpeningSearch);
  expect(rank).toHaveBeenCalledTimes(comparisonsAfterOpeningSearch);
  stopSearch();
  stopGroups();
});

it("updates observed membership and preserves surviving native model references", () => {
  const mirror = createStateMirror();
  const a = agentSummaryFixture("a"),
    b = agentSummaryFixture("b");
  const apply = (agents: (typeof a)[]) =>
    mirror.apply(
      agents.map((value) => ({
        collection: "agentSummary" as const,
        id: value.id,
        value,
      })),
    );
  apply([a]);
  const model = createSidebarViewModel(
    () => mirror.current,
    () => ({
      query: "",
      archived: false,
      opened: null,
      rank: () => 0,
    }),
  );
  const counts: number[] = [];
  const stop = autorun(() => {
    counts.push(model.groups.get(a.projectPath || "")?.length || 0);
  });
  const native = model.rows[0]?.agent;
  apply([a, b]);
  apply([a, b]);
  apply([a]);
  expect(counts).toEqual([1, 2, 1]);
  expect(model.rows[0]?.agent).toBe(native);
  stop();
});

it("owns compact recency, team membership, and summary visibility in the shared client facade", () => {
  const mirror = createStateMirror();
  const now = 200000;
  const summaries = [
    agentSummaryFixture("old", { updated: 1 }),
    agentSummaryFixture("recent", { updated: now - 1 }),
    agentSummaryFixture("pinned", { updated: 1, pinned: true }),
    agentSummaryFixture("running", { updated: 1, status: "running" }),
    agentSummaryFixture("unread", {
      updated: 1,
      operationalStatus: { reason: "unread_result" },
    }),
    agentSummaryFixture("team", { updated: 1 }),
    agentSummaryFixture("worker", { isLead: false }),
    agentSummaryFixture("moved", {
      movedTo: { server: "remote", move: "move-1" },
    }),
  ];
  mirror.apply([
    ...summaries.map((value) => ({
      collection: "agentSummary" as const,
      id: value.id,
      value,
    })),
    {
      collection: "peerTeam",
      id: "t",
      value: {
        id: "t",
        name: "Team",
        projectPath: "/same/project",
        members: ["team"],
      },
    },
  ]);
  const model = createSidebarViewModel(
    () => mirror.current,
    () => ({ query: "", archived: false, opened: null, rank: () => 0 }),
  );
  expect(model.rows.map((row) => row.id)).not.toContain("worker");
  expect(model.rows.map((row) => row.id)).not.toContain("moved");
  expect(
    model.rows.filter((row) => row.compactVisible(now)).map((row) => row.id),
  ).toEqual(["recent", "pinned", "running", "unread", "team"]);
});

it("retains legacy saved chat order when no effective items rank exists", () => {
  const mirror = createStateMirror();
  const agents = [
    agentSummaryFixture("older", { updated: 1 }),
    agentSummaryFixture("newer", { updated: 999 }),
  ];
  mirror.apply(
    agents.map((value) => ({
      collection: "agentSummary" as const,
      id: value.id,
      value,
    })),
  );
  const legacy = JSON.stringify(["chats", "/same/project", false, false]);
  const saved = ["older", "newer"];
  const model = createSidebarViewModel(
    () => mirror.current,
    () => ({
      query: "",
      archived: false,
      opened: null,
      rank: (group, id) =>
        group === legacy ? saved.indexOf(id) : Number.MAX_SAFE_INTEGER,
    }),
  );
  expect(model.orderedRows.map((row) => row.id)).toEqual(saved);
});
