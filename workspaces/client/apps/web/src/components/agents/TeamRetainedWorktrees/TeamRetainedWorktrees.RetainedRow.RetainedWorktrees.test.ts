// @vitest-environment happy-dom
import { act, createElement as h } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, test, vi } from "vitest";
import type { Agent } from "../../../types.ts";
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => ({ get: vi.fn(), save: vi.fn() }),
    useServerResourceReads: () => vi.fn(),
    useServerFileDialogs: () => ({
      canRevealPath: () => true,
      revealPath: vi.fn(),
    }),
  }),
);
import { RetainedWorktree } from "./TeamRetainedWorktrees.tsx";
function render(component: ReturnType<typeof h>) {
  return renderToStaticMarkup(h(MantineProvider, null, component));
}
const worker: Agent = {
  id: "worker",
  worktree: true,
};

test("retained worktrees show reason and path without a delete action", () => {
  const html = render(
    h(RetainedWorktree, {
      agent: {
        ...worker,
        workspaceCleanupIntent: {
          phase: "not_sent",
          reason: "Submodule vendor has changes",
          path: "/work/retained",
        },
      },
    }),
  );
  expect(html).toContain("Worktree retained");
  expect(html).toContain("Submodule vendor has changes");
  expect(html).toContain("/work/retained");
  expect(html).toContain("Open folder");
  expect(html).not.toContain("Delete");
  expect(render(h(RetainedWorktree, { agent: worker }))).not.toContain(
    "Worktree retained",
  );
});

vi.mock("../support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({ models: [] }),
}));
vi.mock("../../accounts/Accounts/Accounts.tsx", () => ({}));
import { fixture } from "../../sidebar/chat-row/support/testFixture.ts";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { required } from "../../conversation/ComposerAttachments/componentTestFixtures.ts";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import { DEFAULT_SUBSCRIPTION_GRACE_MS } from "../../../sync/subscriptions/contracts.ts";
import { TeamPresence } from "../TeamPresence/TeamPresence.tsx";
import TeamFixture from "../support/TeamFixture.tsx";
import TeamPanel from "../TeamPanelView/TeamPanelView.tsx";

test("mounted presence discovers retained-only cleanup and releases scoped interest on close", async () => {
  const f = fixture(),
    host = required(document.body.lastElementChild);
  const released = vi.fn();
  f.added.mockImplementation((ref) => vi.fn(() => released(ref)));
  const show = (hasRetained: boolean, open = false) => {
    const children = h(TeamPresence, {
      children: ["Team toggle", open && h(TeamPanel, { key: "panel" })],
    });
    const controls = {
      leadId: "lead",
      hasRetained,
      setTeamOpen: (open: boolean) => show(true, open),
    };
    const team = h(TeamFixture, controls, children);
    const server = h(ServerStoreProvider, { value: f.store }, team);
    f.root.render(h(MantineProvider, { env: "test" }, server));
  };
  const ref = { kind: "retained-worktrees", agentId: "lead" } as const;
  try {
    await act(async () => show(false));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
      f.publish([agentSummaryFixture("lead")]);
    });
    expect(host.textContent).not.toContain("Team toggle");
    await act(async () => show(true));
    expect(host.textContent).toContain("Team toggle");
    expect(f.added.mock.calls.map(([ref]) => ref)).not.toContainEqual(ref);
    await act(async () => show(true, true));
    await act(() => vi.advanceTimersByTimeAsync(100));
    await act(async () =>
      f.subscriptions.publish(ref, [
        {
          id: "deleted",
          rootId: "lead",
          name: "Deleted worker",
          cleanup: { phase: "unknown", reason: null, path: "/work/kept" },
        },
      ]),
    );
    expect(host.textContent).toMatch(
      /Deleted worker.*Cleanup outcome unconfirmed.*\/work\/kept/s,
    );
    expect(host.textContent).not.toMatch(/budget|Turns used|capabilities/i);
    await act(async () =>
      host.querySelector<HTMLButtonElement>("#team-close")?.click(),
    );
    expect(host.querySelector("#team")).toBeNull();
    expect(host.textContent).toContain("Team toggle");
    await act(() => vi.advanceTimersByTimeAsync(DEFAULT_SUBSCRIPTION_GRACE_MS));
    expect(released).toHaveBeenCalledWith(ref);
  } finally {
    await f.close();
  }
});
