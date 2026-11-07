import { snapshotFixture } from "../snapshotFixture";
import { sidebarRowFixture } from "./sidebarRowFixture";
import { expect, it } from "vitest";
import {
  navigationSnapshot,
  isServerCommand,
  readServerAccounts,
} from "./navigation";

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

it("accepts an exact public account key and rejects malformed keys", () => {
  const command = {
    action: "account-sign-in",
    provider: "claude",
    email: null,
    label: "Work",
    serverLabel: "Remote",
    accountKey: "profile-exact",
  };
  expect(isServerCommand(command)).toBe(true);
  expect(isServerCommand({ ...command, accountKey: 12 })).toBe(false);
  expect(isServerCommand({ ...command, accountKey: "" })).toBe(false);
  expect(isServerCommand({ ...command, forceAdd: true })).toBe(false);
});

it.each(["configure-profile", "rename", "default", "disconnect", "remove"])(
  "validates exact account keys for %s commands",
  (operation) => {
    const command = {
      action: "account-action",
      operation,
      provider: "claude",
      email: "same@example.test",
      label: "Team",
      accountKey: "team",
    };
    expect(isServerCommand(command)).toBe(true);
    expect(isServerCommand({ ...command, accountKey: 12 })).toBe(false);
    expect(isServerCommand({ ...command, accountKey: "" })).toBe(false);
  },
);

it("accepts account commands with only public account identity fields", () => {
  expect(
    isServerCommand({
      action: "account-sign-in",
      provider: "claude",
      email: "user@example.com",
      label: "Work",
      serverLabel: "Remote",
      forceAdd: true,
    }),
  ).toBe(true);
  expect(
    isServerCommand({
      action: "account-action",
      operation: "rename",
      provider: "codex",
      email: null,
      label: "Personal",
      nextLabel: "Home",
    }),
  ).toBe(true);
});

it("rejects malformed account providers and operations", () => {
  expect(
    isServerCommand({
      action: "account-sign-in",
      provider: "gemini",
      email: "user@example.com",
      label: "Work",
      serverLabel: "Remote",
    }),
  ).toBe(false);
  expect(
    isServerCommand({
      action: "account-action",
      operation: "login",
      provider: "claude",
      email: null,
      label: "Work",
    }),
  ).toBe(false);
});

it("uses the server-visible rows instead of stale move records", () => {
  const data = snapshotFixture([
    { id: "moved", movedTo: { server: "target", agentId: "moved", move: "m" } },
  ]);
  expect(navigationSnapshot(data, null, "", []).chats).toEqual([]);
  const row = sidebarRowFixture("moved", { projectPath: "/source/project" });
  data.entityWorkspaceId = "workspace-target";
  const target = navigationSnapshot(data, null, "", [row]);
  expect(target.workspaceId).toBe("workspace-target");
  expect(target.chats[0]?.path).toBe("/source/project");
  expect(target.chats.map((row) => row.id)).toEqual(["moved"]);
});

it("validates the original completed-result target for mark-unread commands", () => {
  const target = {
    threadId: "native",
    turnId: "turn",
    read: true,
    revision: 7,
  };
  const command = { action: "mark-unread", id: "same", target };
  expect(isServerCommand(command)).toBe(true);
  for (const bad of [
    null,
    {},
    { ...target, revision: -1 },
    { ...target, revision: 1.5 },
    { ...target, revision: Infinity },
    { ...target, read: "true" },
    { ...target, threadId: null },
    { ...target, turnId: 1 },
  ])
    expect(isServerCommand({ ...command, target: bad })).toBe(false);
  expect(
    isServerCommand({
      action: "mark-unread",
      id: "same",
      threadId: "native",
      turnId: "turn",
    }),
  ).toBe(false);
});
