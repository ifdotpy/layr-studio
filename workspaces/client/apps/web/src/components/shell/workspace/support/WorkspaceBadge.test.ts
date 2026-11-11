import {
  WorkspaceMode as GeneratedApiWorkspaceMode,
  AgentRole as GeneratedApiAgentRole,
} from "../../../../generated/api.ts";
import { expect, test } from "vitest";
import type { Agent } from "../../../../types.ts";
import { workspaceBadgeInfo } from "./WorkspaceBadge.tsx";

const agent = (values: Partial<Agent>): Agent => ({
  id: "worker",
  isLead: false,
  cwd: "/projects/example",
  ...values,
});

test.each([
  ["layr-native", "Layr", "Native Layr workspace"],
  ["worktree", "WT", "Git worktree"],
  ["shared", "SHARED", "Shared folder"],
] as const)("shows %s workspace as %s", (workspaceMode, label, name) => {
  const current = agent({ workspaceMode });
  const info = workspaceBadgeInfo(current);
  expect(info).toEqual({ label, title: `${name} · /projects/example` });
});

test("hides the badge for leads without a workspace mode", () => {
  expect(workspaceBadgeInfo(agent({ isLead: true }))).toBeNull();
});

test.each([
  ["layr-native", "Layr"],
  ["worktree", "WT"],
] as const)("shows the chat workspace %s as %s", (workspaceMode, label) => {
  const current = agent({ isLead: true, workspaceMode });
  expect(workspaceBadgeInfo(current)?.label).toBe(label);
});

test("shows the legacy layr worker badge", () => {
  const current = agent({
    legacyVmWorkspace: true,
  });
  expect(workspaceBadgeInfo(current)).toEqual({
    label: "Unavailable",
    title: "Legacy workspace unavailable · /projects/example",
  });
});

test("missing modes retain lead badge visibility", () => {
  expect(workspaceBadgeInfo()).toBeNull();
  for (const workspaceMode of [undefined, null]) {
    expect(workspaceBadgeInfo(agent({ workspaceMode }))).toBeNull();
    expect(
      workspaceBadgeInfo(
        agent({ isLead: true, workspaceMode, worktree: true }),
      ),
    ).toBeNull();
  }
  expect(
    workspaceBadgeInfo(
      agent({ isLead: true, workspaceMode: GeneratedApiWorkspaceMode.shared }),
    ),
  ).toBeNull();
});

test.each([
  [{ worktree: true, role: GeneratedApiAgentRole.reviewer }, "WT"],
  [{ role: GeneratedApiAgentRole.reviewer }, "SHARED"],
  [{ worktree: false }, "SHARED"],
  [{ worktree: "/legacy/worktree" }, undefined],
] satisfies [Partial<Agent>, string | undefined][])(
  "preserves legacy workspace inference for %j",
  (values, label) =>
    expect(workspaceBadgeInfo(agent(values))?.label).toBe(label),
);
