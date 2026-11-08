import { stateFromFixture } from "./sidebarSourceFixture.ts";
import { createSidebarSourceStore } from "./sidebarSourceFixture.ts";
import { expect, it } from "vitest";
import { createMergedSidebarSelector } from "./mergedSidebar.ts";
import { sidebarSnapshot } from "./sidebarSnapshot.ts";
import { componentSnapshot } from "../components/componentTestFixtures.ts";
const snapshotFixture = (...args: Parameters<typeof componentSnapshot>) =>
  Object.assign(componentSnapshot(...args), {
    token: "",
    stateDir: "/fixture",
  });
import { agentEntities } from "./sidebarSourceFixture.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";

it("namespaces each server's confirmed creation before replication and lets streamed models win", () => {
  const store = createSidebarSourceStore();
  const a = agentSummaryFixture("same", { name: "Created A" });
  const b = agentSummaryFixture("same", { name: "Created B" });
  const creation = new Map([
    ["A", { unreplicatedCreatedAgents: [a], lookup: () => a }],
    ["B", { unreplicatedCreatedAgents: [b], lookup: () => b }],
  ]);
  for (const id of creation.keys())
    store.update(id, sidebarSnapshot(stateFromFixture(snapshotFixture()), []));
  const source = () =>
    store
      .sources(["A", "B"], new Set(["A", "B"]))
      .map((value) => ({ ...value, creation: creation.get(value.id) }));
  const select = createMergedSidebarSelector();
  const before = select(source());
  expect(before.rows.map((row) => row.agent.name).sort()).toEqual([
    "Created A",
    "Created B",
  ]);
  expect(new Set(before.rows.map((row) => row.id)).size).toBe(2);
  expect(
    before.rows.map((row) => before.chatReferences.get(row.id)?.owner).sort(),
  ).toEqual(["A", "B"]);
  store.update(
    "A",
    sidebarSnapshot(
      stateFromFixture(snapshotFixture()),
      agentEntities([agentSummaryFixture("same", { name: "Replicated A" })]),
    ),
  );
  const after = select(source());
  expect(after.rows.map((row) => row.agent.name).sort()).toEqual([
    "Created B",
    "Replicated A",
  ]);
});
