import { autorun } from "mobx";
import { expect, it } from "vitest";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import {
  createMergedSidebarSelector,
  createSidebarSourceStore,
} from "./mergedSidebar.ts";
import { sidebarSnapshot, type SidebarEntity } from "./sidebarSnapshot.ts";
import { snapshotFixture } from "../snapshotFixture.ts";

const facts = (name = "Original", tail = "old tail"): SidebarEntity[] => [
  {
    collection: "agentSummary",
    id: "same",
    value: agentSummaryFixture("same", { name }),
  },
  { collection: "agentTail", id: "same", value: { id: "same", tail } },
];
it("keeps equal native summary ids in separate generated source mirrors", () => {
  const store = createSidebarSourceStore();
  store.update("local", sidebarSnapshot(snapshotFixture([]), facts()));
  store.update("remote", sidebarSnapshot(snapshotFixture([]), facts()));
  const sources = store.sources(
    ["local", "remote"],
    new Set(["local", "remote"]),
  );
  const model = createMergedSidebarSelector()(sources);
  expect(model.rows.map((row) => row.id)).toEqual([
    model.chatKey("local", "same"),
    model.chatKey("remote", "same"),
  ]);
  expect(model.rows[0]?.agent).toBe(sources[0]?.collections.agents.get("same"));
  expect(model.rows[1]?.agent).toBe(sources[1]?.collections.agents.get("same"));
  expect(model.rows[0]?.agent.id).toBe("same");
  expect(model.data.threads).toEqual([]);
});
it("status and tail updates leave combined routing and group membership untouched", () => {
  const store = createSidebarSourceStore();
  const snapshot = (entities: SidebarEntity[]) =>
    sidebarSnapshot(snapshotFixture([]), entities);
  store.update("local", snapshot(facts()));
  const sources = () => store.sources(["local"], new Set(["local"]));
  const select = createMergedSidebarSelector();
  const model = select(sources());
  let groups = 0;
  const stop = autorun(() => {
    groups++;
    model.rows.map((row) => row.projectPath);
  });
  try {
    const row = model.rows[0];
    const before = groups;
    store.update("local", snapshot(facts("Original", "changed tail")));
    const afterTail = select(sources());
    expect(afterTail.rows[0]).toBe(row);
    expect(groups).toBe(before);
    const changed = facts("Original", "changed tail");
    changed[0] = {
      collection: "agentSummary",
      id: "same",
      value: agentSummaryFixture("same", {
        operationalStatus: { reason: "working" },
        inFlight: true,
      }),
    };
    store.update("local", snapshot(changed));
    expect(select(sources()).rows[0]).toBe(row);
    expect(groups).toBe(before);
  } finally {
    stop();
  }
});
