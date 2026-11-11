import { createRetainedWorktreesMirror } from "../generated/observableResources.ts";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { action } from "mobx";
import { AgentStatus, AgentSource } from "../generated/api.ts";
import { AppTeam } from "../components/shell/AppTeam/AppTeam.tsx";
import { ServerAreas } from "./ServerAreasProvider.tsx";
import { createServerServices } from "../servers/serverServices.ts";
import { observableFixture } from "../components/shell/integration-support/observableFixture.tsx";
import { componentSnapshot as snapshotFixture } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);

const h = vi.hoisted(() => ({
  area: undefined as unknown,
  controls: undefined as
    | {
        completedOpen: boolean;
        setCompletedOpen(open: boolean): void;
        hasTeam(): boolean;
      }
    | undefined,
  retained: false,
}));
vi.mock("./ServerAreasProvider.tsx", async (original) => ({
  ...(await original<typeof import("./ServerAreasProvider.tsx")>()),
  useServerAreas: () => h.area,
}));
vi.mock("../components/agents/support/TeamPanelContext.tsx", () => ({
  TeamProvider: ({
    value,
    children,
  }: {
    value: NonNullable<typeof h.controls>;
    children: React.ReactNode;
  }) => {
    h.controls = value;
    h.retained = value.hasTeam();
    return createElement(
      "div",
      { "data-completed": value.completedOpen },
      children,
    );
  },
  TeamPresence: ({ children }: { children: React.ReactNode }) => children,
  useTeamArea: (
    _id: string | undefined,
    _query: string,
    _filter: string,
    _reads: unknown,
    retained = false,
  ) => {
    h.retained = retained;
    return { view: { members: [], workers: [] } };
  },
}));
vi.mock("../components/agents/TeamPresence/TeamPresence.tsx", () => ({
  TeamPresence: ({ children }: { children: React.ReactNode }) => children,
}));
let cleanup = () => {};
afterEach(() => {
  cleanup();
  h.controls = undefined;
});
async function mount(retained = false) {
  const services = createServerServices({
    id: `team-${crypto.randomUUID()}`,
    origin: "https://fixture.invalid",
    credentialId: "fixture",
    label: "Fixture",
  });
  const models = observableFixture(
    ["lead", "A", "B"].map((id) => ({
      id,
      rootId: "lead",
      isLead: id === "lead",
      source: AgentSource.managed,
      status: AgentStatus.completed,
      hasBackgroundWork: false,
    })),
  );
  const worktrees = createRetainedWorktreesMirror();
  if (retained)
    worktrees.apply([
      {
        id: "retained",
        rootId: "lead",
        cleanup: { path: "/retained", phase: "not_sent", reason: "retained" },
      },
    ]);
  const area = new ServerAreas({
    services,
    store: {
      ...models.store,
      retainedWorktrees: () => ({
        current: { status: "live", data: worktrees.current },
      }),
    },
    screen: { active: true, channel: null },
  });
  action(() => {
    setStartupFixture(
      area,
      Object.assign(
        snapshotFixture([], {
          retainedWorktrees: retained
            ? [
                {
                  id: "retained",
                  rootId: "lead",
                  cleanup: {
                    path: "/retained",
                    phase: "not_sent",
                    reason: "retained",
                  },
                },
              ]
            : [],
        }),
        { token: "", stateDir: "/fixture" },
      ),
    );
    area.navigation.selectedId = "A";
  })();
  h.area = area;
  const navigation = area.navigation;
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  cleanup = () => {
    act(() => root.unmount());
    host.remove();
    area.dispose();
    models.store.dispose();
    services.close();
  };
  await act(async () =>
    root.render(createElement(AppTeam, { children: null })),
  );
  return navigation;
}
it("clears a completed override on A to B to A", async () => {
  const navigation = await mount();
  expect(h.controls?.completedOpen).toBe(true);
  await act(async () => h.controls?.setCompletedOpen(false));
  await act(async () =>
    action(() => {
      navigation.selectedId = "B";
    })(),
  );
  await act(async () =>
    action(() => {
      navigation.selectedId = "A";
    })(),
  );
  expect(h.controls?.completedOpen).toBe(true);
});
it("passes the global retained-only team hint", async () => {
  await mount(true);
  expect(h.retained).toBe(true);
});

import { setStartupFixture } from "../components/shell/integration-support/observableFixture.tsx";

import { ownerFixture } from "./conversation/ownerFixture.ts";
it.each([true, false])(
  "posts and settles a %s read-state write through its owner",
  async (read) => {
    const f = ownerFixture();
    const focus = vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const visibility = vi
      .spyOn(document, "visibilityState", "get")
      .mockReturnValue("visible");
    const proof = { id: "lead", threadId: "thread", turnId: "turn" };
    const target = {
      threadId: "thread",
      turnId: "turn",
      revision: 2,
      read: !read,
    };
    f.post.mockResolvedValue({
      id: "lead",
      readState: { ...target, read, revision: 3 },
    });
    try {
      await f.read.write(proof, read, target);
      expect(f.post).toHaveBeenCalledWith(
        "/api/organization",
        {
          id: "lead",
          read_state: {
            thread_id: "thread",
            turn_id: "turn",
            read,
            expected_revision: 2,
          },
        },
        { workspaceId: "workspace", timeoutMs: 15000 },
      );
      expect(f.read.scope.states.get("lead")).toMatchObject({
        read,
        revision: 3,
      });
      expect(f.read.scope.requests.size).toBe(0);
    } finally {
      focus.mockRestore();
      visibility.mockRestore();
      f.dispose();
    }
  },
);
