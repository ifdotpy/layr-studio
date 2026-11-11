// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { act, createElement as h } from "react";
import { createRoot } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, test, vi } from "vitest";
import { action, computed, observable } from "mobx";
import type { Agent } from "../../../types.ts";
import type { SidebarAgent } from "../../sidebar/support/viewModel.ts";
import SessionActivity, {
  type SessionActivityItem,
} from "../SessionActivity/SessionActivity.tsx";
import { RetainedWorktree } from "../TeamRetainedWorktrees/TeamRetainedWorktrees.tsx";
import { PhaseWaitDetails } from "../PhaseWaitDetails/PhaseWaitDetails.tsx";
import { uniqueModelLabels } from "./executionProjection.ts";
import type { ModelOption } from "../../settings/models/support/modelOption.ts";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";

vi.mock("mobx", async (load) => {
  const mobx = await load<typeof import("mobx")>();
  return { ...mobx, computed: vi.fn(mobx.computed) };
});
const services = vi.hoisted(() => ({
  reveal: vi.fn(),
  owner: { id: "server-a" },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerFileDialogs: () => ({
      canRevealPath: () => true,
      revealPath: services.reveal,
    }),
    useServerOwner: () => services.owner,
    useServerStore: () => ({
      state: () => ({
        current: { status: "live", data: { agents: new Map() } },
      }),
    }),
  }),
);
vi.mock("./state.ts", () => ({
  useActivityView: () => ({ commands: [], monitors: [] }),
}));
vi.mock("../../../hooks/useVisualActivity.ts", () => ({
  useVisualActivity: () => [null, false],
}));

afterEach(() => vi.unstubAllGlobals());

test("waiting details rerenders without allocating MobX computed nodes", () => {
  vi.mocked(computed).mockClear();
  const agent: SidebarAgent = agentSummaryFixture("lead");
  renderToStaticMarkup(h(PhaseWaitDetails, { agent }));
  renderToStaticMarkup(h(PhaseWaitDetails, { agent }));
  expect(computed).not.toHaveBeenCalled();
});

test.each(["path", "agent", "server"])(
  "a pending reveal error stays with its retained %s target",
  async (changed) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    const host = document.createElement("div"),
      root = createRoot(host);
    const pending = Promise.withResolvers<void>();
    services.owner = { id: "server-a" };
    services.reveal.mockReturnValue(pending.promise);
    const agent: Agent = {
      id: "a",
      workspaceCleanupIntent: { phase: "unknown", path: "/a", reason: null },
    };
    const show = (value: Agent) =>
      root.render(
        h(
          MantineProvider,
          { env: "test" },
          h(RetainedWorktree, { agent: value }),
        ),
      );
    try {
      await act(async () => show(agent));
      await act(async () => host.querySelector("button")!.click());
      if (changed === "server") services.owner = { id: "server-b" };
      await act(async () =>
        show({
          ...agent,
          id: changed === "agent" ? "b" : "a",
          workspaceCleanupIntent: {
            ...agent.workspaceCleanupIntent!,
            path: changed === "path" ? "/b" : "/a",
          },
        }),
      );
      await act(async () => pending.reject(new Error("A reveal failed")));
      expect(host.querySelector('[role="alert"]')).toBeNull();
    } finally {
      await act(async () => root.unmount());
    }
  },
);

test("activity expansion resets after shrinking to one or zero", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const activities: SessionActivityItem[] = ["one", "two"].map((id) => ({
    id,
    kind: "agent",
    agentId: id,
    agentName: id,
    label: id,
    status: "running",
  }));
  const show = (count: number) =>
    root.render(
      h(SessionActivity, {
        activities: activities.slice(0, count),
        onOpen: () => {},
      }),
    );
  try {
    for (const count of [1, 0]) {
      await act(async () => show(2));
      await act(async () =>
        host
          .querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
          .click(),
      );
      expect(host.querySelectorAll("li")).toHaveLength(2);
      await act(async () => show(count));
      await act(async () => show(2));
      expect(host.querySelectorAll("li")).toHaveLength(1);
    }
  } finally {
    await act(async () => root.unmount());
  }
});

test("observable activity membership resets expansion without replacing the props array", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const second: SessionActivityItem = {
    id: "two",
    kind: "agent",
    agentId: "two",
    agentName: "two",
    label: "two",
    status: "running",
  };
  const activities = observable.array<SessionActivityItem>([
    { ...second, id: "one" },
    second,
  ]);
  try {
    await act(async () =>
      root.render(h(SessionActivity, { activities, onOpen: () => {} })),
    );
    await act(async () =>
      host.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!.click(),
    );
    await act(async () => action(() => activities.pop())());
    await act(async () => action(() => activities.push(second))());
    expect(host.querySelectorAll("li")).toHaveLength(1);
  } finally {
    await act(async () => root.unmount());
  }
});

test("the prop-driven execution projection lives outside the mutation store and avoids nested label scans", () => {
  const source = readFileSync(
    "src/components/agents/support/ExecutionSettingsStore.ts",
    "utf8",
  );
  expect(source).not.toContain("export function projectExecution(");
  expect(source).not.toContain(
    "rows.filter((other) => other.label === row.label)",
  );
});

test("model label deduplication keeps current, recommended, then first without repeated scans", () => {
  let reads = 0;
  const rows: ModelOption[] = Array.from({ length: 200 }, (_, index) => ({
    value: String(index),
    provider: "codex",
    badges: index === 101 ? ["default"] : [],
    get label() {
      reads++;
      return String(index % 100);
    },
  }));
  const result = uniqueModelLabels(rows, "102");
  expect(result.map((row) => row.value)).toEqual([
    ...rows
      .slice(0, 100)
      .filter((row) => row.value !== "1" && row.value !== "2")
      .map((row) => row.value),
    "101",
    "102",
  ]);
  expect(reads).toBeLessThanOrEqual(rows.length * 3);
  expect(uniqueModelLabels(rows, "1").find((row) => row.value === "1")).toBe(
    rows[1],
  );
  expect(uniqueModelLabels([rows[1]!, { ...rows[1]! }], "1")).toEqual([
    rows[1],
  ]);
});
