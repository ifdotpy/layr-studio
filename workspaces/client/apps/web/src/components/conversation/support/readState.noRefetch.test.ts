import { setStartupFixture } from "../../shell/integration-support/observableFixture.tsx";
// @vitest-environment happy-dom
import { conversationActionsFixture } from "../../shell/integration-support/observableFixture.tsx";
import { defaultSyncClient } from "../../../sync/client.ts";
import type { ServerServices } from "../../../servers/serverServices.ts";
import { MarkUnreadEligibility } from "../../../generated/api.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import { afterEach, expect, it, vi } from "vitest";
import type { Agent } from "../../../types.ts";
import { componentSnapshot as snapshotFixture } from "../ComposerAttachments/componentTestFixtures.ts";

const h = vi.hoisted(() => ({
  store: null as
    | ReturnType<
        typeof import("../../shell/integration-support/observableFixture.tsx").observableFixture
      >["store"]
    | null,
  post: vi.fn(),
  recovery: vi.fn(),
}));
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useState: (value: unknown) => [value, vi.fn()],
  useRef: (value: unknown) => ({ current: value }),
  useCallback: (callback: unknown) => callback,
  useMemo: (factory: () => unknown) => factory(),
  useEffect: (effect: () => unknown) => {
    effect();
  },
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStore: () => h.store,
    useServerApi: () => ({ post: h.post }),
    useServerSync: () => ({ refreshState: h.recovery }),
  }),
);

afterEach(() => {
  vi.unstubAllGlobals();
  h.post.mockReset();
  h.recovery.mockReset();
});
it.each([false, true])(
  "read receipt lostResponse=%s uses canonical success or one uncertainty read",
  async (lostResponse) => {
    vi.stubGlobal("document", {
      visibilityState: "visible",
      hasFocus: () => true,
    });
    const agent: Agent = {
      id: "lead",
      threadId: "thread",
      lastCompletedTurn: "turn",
      lastCompletedTurnStatus: "completed",
      readStateSupported: true,
    };
    h.store = observableFixture([
      {
        ...agent,
        markRead: {
          threadId: "thread",
          turnId: "turn",
          read: false,
          revision: 0,
        },
        markUnread: MarkUnreadEligibility["allowed"],
      },
    ]).store;
    const snapshot = {
      ...Object.assign(snapshotFixture([agent]), {
        token: "",
        stateDir: "/fixture",
      }),
      stateDir: "fixture",
      threads: [agent],
    };
    const refresh = vi.fn(async () => {});
    h.recovery.mockResolvedValue(snapshot);
    if (lostResponse)
      h.post.mockRejectedValue(new Error("Response unavailable"));
    else
      h.post.mockResolvedValue({
        id: "lead",
        readState: {
          threadId: "thread",
          turnId: "turn",
          revision: 1,
          read: true,
        },
      });
    const area = conversationActionsFixture(() => {}, h.store);
    area.scope.services.api.post = h.post;
    Object.defineProperty(area.scope.services, "sync", {
      value: {
        ...defaultSyncClient,
        refreshState: h.recovery,
      } satisfies ServerServices["sync"],
    });
    setStartupFixture(area, snapshot);
    area.navigation.selectedId = "lead";
    area.notices.notify = vi.fn();
    const state = area.read;
    state.observeRead({ id: "lead", threadId: "thread", turnId: "turn" });
    expect(h.post).not.toHaveBeenCalled();
    state.select("lead");
    state.observeRead({ id: "lead", threadId: "thread", turnId: "turn" });
    expect(h.post).toHaveBeenCalledTimes(1);
    await Promise.resolve();
    await Promise.resolve();
    expect(refresh).not.toHaveBeenCalled();
    expect(h.recovery).toHaveBeenCalledTimes(lostResponse ? 1 : 0);
  },
);
