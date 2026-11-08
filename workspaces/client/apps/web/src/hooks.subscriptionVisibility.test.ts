// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createSubscriptions } from "./sync/subscriptions/module";
import { createServerStore } from "./store/index";
import { ServerStoreProvider } from "./servers/ServerServicesContext";
import { SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS } from "./generated/sharedValues";
import { useTranscriptHistory } from "./conversation/historyFixture.ts";
import { DEFAULT_SUBSCRIPTION_GRACE_MS } from "./sync/subscriptions/contracts";
import type { ResourceVersion } from "./sync/resourceEvents";
import type {
  ConnectionHealth,
  ConnectionActivity,
} from "./sync/connectionState";
import type { ResourceConnectionDetail } from "./sync/resourceConnection";
import type { ResourceLiveDeltaEvent } from "./generated/observableResources";

const h = vi.hoisted(() => ({
  get: vi.fn(),
  connection: undefined as
    | undefined
    | ((
        health: ConnectionHealth,
        activity?: ConnectionActivity,
        detail?: ResourceConnectionDetail,
      ) => void),
  delta: undefined as undefined | ((event: ResourceLiveDeltaEvent) => void),
}));
vi.mock("./servers/ServerServicesContext", async (original) => ({
  ...(await original<typeof import("./servers/ServerServicesContext")>()),
  useServerApi: () => api,
  useServerResources: () => resources,
}));
const api = { syncGet: h.get };
const resources = {
  watchLiveDeltas: (accept: NonNullable<typeof h.delta>) => {
    h.delta = accept;
    return () => {
      h.delta = undefined;
    };
  },
  watchResourceConnection: (accept: NonNullable<typeof h.connection>) => {
    h.connection = accept;
    return () => {
      h.connection = undefined;
    };
  },
};
beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
  document.body.replaceChildren();
});

it("the real history hook opens only visible interest and releases it with its store alive", async () => {
  const removed = vi.fn();
  let accept: (version?: ResourceVersion) => void = () => {};
  const added = vi.fn((_ref, callback) => {
    accept = callback;
    return removed;
  });
  const subscriptions = createSubscriptions({
    watchResourceChanges: added,
    watchResourceConnection: (callback) => {
      callback("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  let session: ReturnType<typeof useTranscriptHistory> | undefined;
  const View = observer(function View({ visible }: { visible: boolean }) {
    session = useTranscriptHistory("chat", true, "scope", "workspace", visible);
    return createElement(
      "output",
      null,
      session.connection?.message || "",
      session.items.length,
    );
  });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const render = async (visible: boolean) =>
    act(async () => {
      root.render(
        createElement(
          ServerStoreProvider,
          { value: store },
          createElement(View, { visible }),
        ),
      );
    });
  try {
    await render(false);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(added).not.toHaveBeenCalled();
    await render(true);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(added).toHaveBeenCalledOnce();
    await act(async () => {
      accept({ epoch: "fixture", revision: 1 });
      subscriptions.publish(
        {
          kind: "transcript",
          agentId: "chat",
          tail: SYNC_TRANSCRIPT_INITIAL_TAIL_ITEMS,
        },
        {
          items: [{ id: "one", role: "assistant", text: "hello" }],
          truncated: false,
        },
      );
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(session?.loaded).toBe(true);
    expect(session?.items[0]?.text).toBe("hello");
    await act(async () => {
      h.connection?.("degraded", "connecting");
    });
    expect(host.textContent).toContain("Reconnecting");
    await act(async () => {
      h.connection?.("live", "open", {});
    });
    expect(session?.connection?.message).toBe("");
    expect(h.get).not.toHaveBeenCalled();
    await render(false);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(DEFAULT_SUBSCRIPTION_GRACE_MS + 1);
    });
    expect(removed).toHaveBeenCalled();
    expect(h.delta).toBeUndefined();
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
  }
});
