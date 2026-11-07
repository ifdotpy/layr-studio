import { autorun, observable, runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import { createStateMirror } from "../../generated/observableResources.ts";
import { agentSummaryFixture } from "../../servers/agentSummaryFixture.ts";
import { createSidebarViewModel } from "./viewModel.ts";

it("keeps grouping and order independent of tails while search observes them only when open", () => {
  const mirror = createStateMirror();
  const agent = agentSummaryFixture("a");
  const publish = (tail: string) =>
    mirror.apply([
      { collection: "agentSummary", id: agent.id, value: agent },
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
  publish("needle");
  expect(model.groups).toBe(original);
  expect(groups).toHaveBeenCalledTimes(1);
  expect(search).toHaveBeenCalledTimes(1);
  runInAction(() => {
    controls.query = "needle";
  });
  expect(search).toHaveBeenLastCalledWith(["a"]);
  const groupsAfterOpeningSearch = groups.mock.calls.length;
  publish("other");
  expect(search).toHaveBeenLastCalledWith([]);
  expect(groups).toHaveBeenCalledTimes(groupsAfterOpeningSearch);
  expect(rank).not.toHaveBeenCalled(); // One row needs no comparison.
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
