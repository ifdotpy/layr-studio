import { stateFromFixture } from "./sidebarSourceFixture.ts";
import { componentSnapshot as snapshotFixture } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { agentSummaryFixture } from "./agentSummaryFixture.ts";
import { navigationFromSummaries } from "./sidebarSourceFixture.ts";
import { expect, it } from "vitest";
import { navigationSnapshot, readServerAccounts } from "./navigation.ts";

it("preserves exact account keys in the received public account snapshot", () => {
  const account = {
    provider: "claude",
    email: "work@example.test",
    plan: "Max",
    status: "ready",
    label: "Work",
    isDefault: true,
  };
  expect(readServerAccounts([{ ...account, accountKey: "exact" }])).toEqual([
    { ...account, accountKey: "exact" },
  ]);
  expect(readServerAccounts([account])).toEqual([account]);
  expect(readServerAccounts([{ ...account, accountKey: 12 }])).toBeNull();
  expect(readServerAccounts([{ ...account, accountKey: "" }])).toBeNull();
});

it("uses the server-visible rows instead of stale move records", () => {
  const data = Object.assign(
    snapshotFixture([
      {
        id: "moved",
        movedTo: { server: "target", agentId: "moved", move: "m" },
      },
    ]),
    { token: "", stateDir: "/fixture" },
  );
  expect(
    navigationSnapshot(stateFromFixture(data), null, "", []).chats,
  ).toEqual([]);
  const row = agentSummaryFixture("moved", { projectPath: "/source/project" });
  data.entityWorkspaceId = "workspace-target";
  const target = navigationFromSummaries(data, [row]);
  expect(target.workspaceId).toBe("workspace-target");
  expect(target.chats[0]?.path).toBe("/source/project");
  expect(target.chats.map((row) => row.id)).toEqual(["moved"]);
});
