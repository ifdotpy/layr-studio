import { createNet } from "../../../servers/net/net.ts";
import * as netApi from "../../../servers/net/api.ts";
import * as netLocalDrafts from "../../../sync/localDraft.ts";
import { serverLocalStorage } from "../../../servers/storage.ts";
import { useServerStorage } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { outboxRequests } from "../../../browserPersistence.ts";
import { resourceData } from "../../../store/runtime.ts";
import type { ComponentSnapshot as Snapshot } from "../ComposerAttachments/componentTestFixtures.ts";
// @vitest-environment happy-dom
import { observable, runInAction } from "mobx";
import { createModelsMirror } from "../../../generated/observableResources.ts";
vi.mock("mobx-react-lite", async (original) => ({
  ...(await original<typeof import("mobx-react-lite")>()),
  observer: <T>(component: T) => component,
}));
// These probes own resource-read effects; context detail is outside their fixture.
vi.mock("../../agents/support/state.ts", async (original) => ({
  ...(await original<typeof import("../../agents/support/state.ts")>()),
  useAgentDetail: () => undefined,
}));
import {
  MessageSendStatus,
  ProviderKind,
  AgentSource,
} from "../../../generated/api.ts";
import {
  afterAll,
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import type { Agent } from "../../../types.ts";

// Execute the real effects and read callbacks without rendering unrelated UI.
const {
  projectionListeners,
  contexts,
  effects,
  cleanups,
  states,
  refs,
  memos,
  cursor,
  watchers,
  watchOptions,
  resourceVersions,
  connectionWatchers,
  connectionStream,
  get,
  post,
  syncPost,
  reconcile,
} = vi.hoisted(() => ({
  projectionListeners: new Set<(snapshot: unknown) => void>(),
  contexts: new Map<unknown, unknown>(),
  effects: [] as Array<() => void | (() => void)>,
  cleanups: [] as Array<() => void>,
  states: [] as Array<{ value: unknown }>,
  refs: [] as Array<{ current: unknown }>,
  memos: [] as Array<{
    value: unknown;
    dependencies: readonly unknown[] | undefined;
  }>,
  cursor: { state: 0, ref: 0, memo: 0, dirty: false, usage: false },
  watchers: new Map<string, (version?: unknown) => void>(),
  watchOptions: new Map<string, { waitForBaseline?: boolean }>(),
  resourceVersions: new Map<string, unknown>(),
  connectionWatchers: new Set<
    (status: string, stream: "stopped" | "connecting" | "open") => void
  >(),
  connectionStream: {
    value: "connecting" as "stopped" | "connecting" | "open",
  },
  get: vi.fn(),
  post: vi.fn(),
  syncPost: vi.fn(),
  reconcile: vi.fn(async (..._args: unknown[]) => {}),
}));

vi.mock("react", async (importOriginal) => {
  const original = await importOriginal<typeof import("react")>();
  const scheduleEffect = (
    effect: () => void | (() => void),
    _dependencies?: readonly unknown[],
  ) => effects.push(effect);
  return {
    ...original,
    useCallback: (callback: unknown) => callback,
    useLayoutEffect: scheduleEffect,
    // No provider is mounted, so a context read returns its default value.
    useContext: (context: { _currentValue: unknown }) =>
      contexts.has(context) ? contexts.get(context) : context._currentValue,
    useMemo: (compute: () => unknown, dependencies?: readonly unknown[]) => {
      const index = cursor.memo++;
      const previous = memos[index];
      if (
        !previous ||
        !dependencies ||
        !previous.dependencies ||
        previous.dependencies.length !== dependencies.length ||
        dependencies.some(
          (value, i) => !Object.is(value, previous.dependencies![i]),
        )
      )
        memos[index] = { value: compute(), dependencies };
      return memos[index]!.value;
    },
    useEffect: scheduleEffect,
    useRef: (value: unknown) =>
      refs[cursor.ref++] ?? (refs[cursor.ref - 1] = { current: value }),
    useState: (initial: unknown) => {
      const index = cursor.state++;
      const entry =
        states[index] ??
        (states[index] = {
          value: typeof initial === "function" ? initial() : initial,
        });
      return [
        entry.value,
        (value: unknown) => {
          const next = typeof value === "function" ? value(entry.value) : value;
          if (!Object.is(next, entry.value)) cursor.dirty = true;
          entry.value = next;
        },
      ];
    },
    useSyncExternalStore: (
      subscribe: (listener: () => void) => () => void,
      getSnapshot: () => unknown,
    ) => {
      const index = cursor.state++;
      const entry = states[index] ?? (states[index] = { value: getSnapshot() });
      entry.value = getSnapshot();
      scheduleEffect(
        () =>
          subscribe(() => {
            entry.value = getSnapshot();
          }),
        [subscribe],
      );
      return entry.value;
    },
  };
});
vi.mock("../../../servers/net/api.ts", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../../servers/net/api.ts")>()),
  get,
  post,
  syncPost,
}));
vi.mock("../../../sync/localDraft.ts", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../../sync/localDraft.ts")>()),
  updateQueueRequest: async (
    suffix: string,
    fallback: unknown,
    update: (value: unknown) => unknown,
  ) => {
    const key = outboxRequests().key("queueChange", suffix);
    const raw = localStorage.getItem(key);
    const result = update(raw === null ? fallback : JSON.parse(raw));
    if (result === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(result));
    return result;
  },
}));
vi.mock("../../../sync/send.ts", async (original) => ({
  ...(await original<typeof import("../../../sync/send.ts")>()),
  reconcileOutboxReceipts: reconcile,
}));
// Mantine hooks use the real React dispatcher, which this harness replaces.
vi.mock("@mantine/hooks", async (original) => ({
  ...(await original<typeof import("@mantine/hooks")>()),
  useMediaQuery: () => false,
}));
vi.mock("../../../sync/resume.ts", () => ({ onResume: () => () => {} }));
vi.mock("../../../usePendingCommands.ts", () => ({
  usePendingCommands: () => [],
}));
vi.mock("../../../sync/subscriptions/events.ts", async (importOriginal) => ({
  ...(await importOriginal<
    typeof import("../../../sync/subscriptions/events.ts")
  >()),
  watchResourceChanges: (
    resource: unknown,
    callback: (version?: unknown) => void,
    options?: { waitForBaseline?: boolean },
  ) => {
    const key = JSON.stringify(resource);
    watchers.set(key, callback);
    watchOptions.set(key, options || {});
    const known = resourceVersions.get(key);
    if (known) callback(known);
    return () => {
      watchers.delete(key);
      watchOptions.delete(key);
    };
  },
  watchResourceConnection: (
    listener: (
      status: string,
      stream: "stopped" | "connecting" | "open",
    ) => void,
  ) => {
    connectionWatchers.add(listener);
    listener("connecting", connectionStream.value);
    return () => connectionWatchers.delete(listener);
  },
}));

vi.mock("../../../sync/client.ts", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../../sync/client.ts")>()),
  subscribeStateProjection: (accept: (snapshot: unknown) => void) => {
    projectionListeners.add(accept);
    return () => projectionListeners.delete(accept);
  },
}));

import {
  createStateMirror,
  type MessageReceiptEntityDto,
} from "../../../generated/observableResources.ts";
const nativeState = createStateMirror();
const modelState = createModelsMirror();
let modelReader = { scope: "fixture", read: get };
const costsState = observable.box<
  import("../../../store/runtime.ts").ResourceStatus<
    import("../../../generated/observableResources.ts").CostsData
  >
>({ status: "loading" }, { deep: false });
const sessionState = observable.box<
  import("../../../store/runtime.ts").ResourceStatus<
    import("../../../generated/observableResources.ts").SessionCostData
  >
>({ status: "loading" }, { deep: false });
const reconnect = vi.fn();
const nativeOwner = observableFixture();
afterAll(() => nativeOwner.store.dispose());
const nativeStore: typeof nativeOwner.store = {
  ...nativeOwner.store,
  state: () => ({ current: { status: "live", data: nativeState.current } }),
  agentDetail: () => ({ current: { status: "loading" } }),
  models: () => ({ current: { status: "live", data: modelState.current } }),
  costs: () => ({
    get current() {
      return costsState.get();
    },
  }),
  sessionCost: () => ({
    get current() {
      return sessionState.get();
    },
  }),
};
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerStore: () => nativeStore,
    useServerNet: () =>
      createNet(netApi, () => nativeStore, undefined, {
        storage: serverLocalStorage,
        updateQueueRequest: netLocalDrafts.updateQueueRequest,
      }),
    useServerModelReader: () => modelReader,
    useServerResources: () => ({ replaceStream: reconnect }),
  }),
);
function receiveReceipts(receipts: MessageReceiptEntityDto[]) {
  nativeState.apply(
    receipts.map((receipt) => ({
      collection: "messageReceipt",
      id: receipt.id,
      value: receipt,
    })),
  );
}
function publishQueue(revision: string, text: string | null) {
  nativeState.apply([
    {
      collection: "queue",
      id: "lead",
      value: {
        items:
          text === null
            ? []
            : [
                {
                  id: "message-one",
                  text,
                  created: 1,
                  kind: "user",
                  status: MessageSendStatus["queued"],
                },
              ],
        revision,
        capabilities: { reorder: true, receipts: true },
      },
    },
  ]);
}

import { conversationActionsFixture } from "../../shell/integration-support/observableFixture.tsx";
import { useMessageQueue } from "./useMessageQueue.ts";
import { ApiError } from "../../../api.ts";
import {
  useWorkerModels,
  readWorkerModels,
} from "../../agents/support/WorkerModelPicker.tsx";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import ProductUsage from "../Usage/Usage.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import { serializeStateCollections } from "../../../generated/observableResources.ts";
function Usage({
  agent,
  opened = false,
  visible = true,
}: {
  agent: Agent;
  stateDir: string;
  limits: null;
  reload: () => unknown;
  opened?: boolean;
  visible?: boolean;
}) {
  runInAction(() => {
    nativeState.apply([
      ...serializeStateCollections(nativeState.current).filter(
        (row) => row.collection !== "agentSummary",
      ),
      {
        collection: "agentSummary",
        id: agent.id,
        value: {
          ...agentSummaryFixture(agent.id),
          ...agent,
          isLead: !!agent.isLead,
          inFlight: !!agent.inFlight,
          projectServerId: agent.projectServerId ?? null,
        },
      },
    ]);
  });
  cursor.usage = true;
  return ProductUsage({ agent, opened, visible, onChange: () => {} });
}

function selectedReceipts(workspaceId = "") {
  vi.stubGlobal("location", { origin: "https://fixture.test" });
  const models = observableFixture([
    { id: "lead", source: AgentSource.managed },
  ]);
  const area = conversationActionsFixture(() => {}, nativeStore);
  Object.assign(area.scope.screen, { active: false });
  area.startup.workspaceId = workspaceId;
  area.navigation.selectedId = "lead";
  const state = nativeStore.state;
  Object.assign(area.scope.store, {
    state: () => ({
      current: {
        status: "live",
        data: {
          ...nativeState.current,
          agents: models.mirror.current.agents,
        },
      },
    }),
  });
  cleanups.push(() => Object.assign(area.scope.store, { state }));
  area.delivery.entries = [
    {
      id: "message-one",
      body: { id: "message-one", room: "lead", text: "pending" },
      created: 1,
      status: { source: "device", state: { kind: "stored" } },
    },
  ];
  const selected = area.selected;
  const stop = selected.start();
  cleanups.push(() => {
    stop();
    models.store.dispose();
    area.scope.services.close();
  });
  return selected.receipts;
}

function mount<T>(action: () => T): T {
  let result: T;
  do {
    cursor.state = cursor.ref = cursor.memo = 0;
    cursor.dirty = false;
    cursor.usage = false;
    result = action();
    for (const effect of effects.splice(0)) {
      const cleanup = effect();
      if (cleanup) cleanups.push(cleanup);
    }
  } while (cursor.usage && cursor.dirty);
  return result;
}

describe("resource read callers", () => {
  beforeEach(() => {
    projectionListeners.clear();
    nativeState.apply([]);
    modelState.apply([]);
    runInAction(() => {
      costsState.set({ status: "loading" });
      sessionState.set({ status: "loading" });
    });
    modelReader = { scope: "fixture", read: get };
    projectionListeners.add((snapshot) => {
      const runtime = (snapshot as Snapshot).runtime;
      if (runtime.accountsSummary)
        nativeState.apply([
          {
            collection: "accounts",
            id: "current",
            value: runtime.accountsSummary,
          },
        ]);
      runInAction(() => {
        costsState.set({
          status: "live",
          data: Object.values(runtime.accountCostsByAccount ?? {}),
        });
        const cost = runtime.sessionCostsByAgent?.lead;
        if (cost) sessionState.set({ status: "live", data: cost });
      });
    });
    reconnect.mockClear();
    vi.useFakeTimers();
    get.mockReset();
    post.mockReset();
    syncPost.mockReset();
    reconcile.mockClear();
    contexts.clear();
    effects.length = 0;
    states.length = 0;
    refs.length = 0;
    memos.length = 0;
    cursor.state = 0;
    cursor.ref = 0;
    cursor.memo = 0;
    watchers.clear();
    watchOptions.clear();
    resourceVersions.clear();
    connectionWatchers.clear();
    connectionStream.value = "connecting";
    const storage = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
    });
    vi.stubGlobal("document", {
      hidden: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
    vi.stubGlobal("navigator", { onLine: true });
    vi.stubGlobal("window", {
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      setInterval,
      clearInterval,
    });
  });

  afterEach(() => {
    for (const cleanup of cleanups.splice(0).reverse()) cleanup();
    expect(post).not.toHaveBeenCalled();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("never reconciles an outbox from a cached projection", async () => {
    mount(() => selectedReceipts("workspace"));
    receiveReceipts([
      {
        id: "message-one",
        agent: "lead",
        status: MessageSendStatus["delivered"],
      },
    ]);
    await vi.advanceTimersByTimeAsync(0);
    expect(reconcile).not.toHaveBeenCalled();
  });

  it("waits for the workspace identity before it reads message receipts", async () => {
    mount(() => selectedReceipts());
    expect(watchers.size).toBe(0);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(get).not.toHaveBeenCalled();
    expect(reconcile).not.toHaveBeenCalled();
  });

  it("applies safe account summaries without baseline or update GETs", async () => {
    const accountView = {
      get data() {
        return resourceData(nativeStore.state().current)?.accountsSummary.get(
          "current",
        );
      },
    };
    const initial = {
      accounts: [{ id: "connected", email: "person@example.test" }],
      archivedAccounts: [],
      defaultAccountKey: "connected",
      defaultAccountGroups: [],
      supportsDelete: true,
      supportsDisconnect: true,
    };
    for (const accept of projectionListeners)
      accept({ runtime: { accountsSummary: initial } });
    expect(accountView.data).toEqual(initial);
    const changed = {
      ...initial,
      accounts: [{ id: "connected", disconnected: true }],
    };
    for (const accept of projectionListeners)
      accept({ runtime: { accountsSummary: changed } });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(accountView.data).toEqual(changed);
    expect(get).not.toHaveBeenCalled();
    expect(watchers.size).toBe(0);
  });

  it("applies queue projections without GETs at baseline, update, and idle", async () => {
    const props = {
      id: "lead",
      enabled: true,
      scope: "scope",
      workspaceId: "workspace",
      refresh: vi.fn(async () => {}),
    };
    mount(() => useMessageQueue(props));
    publishQueue("v1", "old");
    expect(
      watchers.has(JSON.stringify({ kind: "queue", agentId: "lead" })),
    ).toBe(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(get).not.toHaveBeenCalled();
    expect(mount(() => useMessageQueue(props)).items).toMatchObject([
      { text: "old" },
    ]);
    publishQueue("v2", "new");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(mount(() => useMessageQueue(props)).items).toMatchObject([
      { text: "new" },
    ]);
    expect(get).not.toHaveBeenCalled();
    expect(syncPost).not.toHaveBeenCalled();
  });

  it("an acknowledged queue mutation converges from the entity stream without a GET", async () => {
    const props = {
      id: "lead",
      enabled: true,
      scope: "scope",
      workspaceId: "workspace",
      refresh: vi.fn(async () => {}),
    };
    publishQueue("v1", "first payload");
    syncPost.mockResolvedValue({});
    const queue = mount(() => useMessageQueue(props));
    const item = queue.items[0]!;
    await queue.sendNow(item);
    expect(syncPost).toHaveBeenCalledOnce();
    expect(reconnect).not.toHaveBeenCalled();
    expect(get).not.toHaveBeenCalled();
    publishQueue("v2", null);
    expect(mount(() => useMessageQueue(props)).items).toEqual([]);
    expect(get).not.toHaveBeenCalled();
  });

  it.each(["read", "parse"])(
    "blocks queue writes after a %s failure without losing the conversation",
    async (failure) => {
      const storage = useServerStorage();
      const key = outboxRequests(storage).key("queueChange", "scope:lead");
      storage.setItem(key, "unreadable-current-schema");
      const write = vi.spyOn(storage, "setItem").mockClear();
      const read = vi.spyOn(storage, "getItem");
      if (failure === "read")
        read.mockImplementation(() => {
          throw new Error("storage unavailable");
        });
      publishQueue("v1", "kept message");
      const queue = mount(() =>
        useMessageQueue({
          id: "lead",
          enabled: true,
          scope: "scope",
          workspaceId: "workspace",
        }),
      );
      expect(queue.blocked).toBe(true);
      expect(queue.error).not.toBe("");
      expect(queue.items).toMatchObject([{ text: "kept message" }]);
      await expect(queue.sendNow(queue.items[0]!)).rejects.toThrow();
      await expect(queue.reorder(["message-one"])).rejects.toThrow();
      await expect(queue.cancel(queue.items[0]!)).rejects.toThrow();
      await expect(queue.retry()).rejects.toThrow();
      expect(syncPost).not.toHaveBeenCalled();
      expect(write).not.toHaveBeenCalled();
      read.mockRestore();
      write.mockRestore();
      expect(storage.getItem(key)).toBe("unreadable-current-schema");
    },
  );

  it("retains a request on signature conflict and clears an ordinary refusal", async () => {
    const key = outboxRequests().key("queueChange", "scope:lead");
    const request = {
      action: "send_now",
      agent: "lead",
      id: "a",
      expectedText: "text",
      expected_revision: "r",
      request_id: "request",
    };
    localStorage.setItem(key, JSON.stringify(request));
    get.mockResolvedValue({
      items: [],
      revision: "r",
      capabilities: { reorder: true, receipts: true },
    });
    const queue = mount(() =>
      useMessageQueue({
        id: "lead",
        enabled: true,
        scope: "scope",
        workspaceId: "workspace",
      }),
    );
    syncPost.mockRejectedValueOnce(new ApiError("signature conflict", 409));
    await expect(queue.retry()).rejects.toThrow("signature conflict");
    expect(localStorage.getItem(key)).toBe(JSON.stringify(request));
    syncPost.mockRejectedValueOnce(new ApiError("ordinary conflict", 400));
    await expect(queue.retry()).rejects.toThrow("ordinary conflict");
    expect(localStorage.getItem(key)).toBeNull();
  });

  it("uses streamed models for display and a fresh owner read for selection", async () => {
    const publish = (model: string) =>
      modelState.apply([
        {
          collection: "model",
          id: "default:" + model,
          value: {
            accountKey: "default",
            model,
            provider: ProviderKind.codex,
            catalogPosition: 0,
          },
        },
      ]);
    publish("streamed");
    expect(mount(() => useWorkerModels("default", true)).models).toMatchObject([
      { model: "streamed" },
    ]);
    publish("updated");
    expect(mount(() => useWorkerModels("default", true)).models).toMatchObject([
      { model: "updated" },
    ]);
    expect(get).not.toHaveBeenCalled();
    get.mockResolvedValueOnce({ data: [{ model: "fresh" }] });
    expect(await readWorkerModels(modelReader, "default", true)).toMatchObject([
      { model: "fresh" },
    ]);
    expect(get).toHaveBeenCalledExactlyOnceWith({
      query: { account_key: "default", workers: "1", fresh: "1" },
    });
  });

  it("releases cost interest when the mounted footer is hidden", () => {
    mount(() =>
      Usage({
        agent: { id: "hidden" } as Agent,
        stateDir: "private-fixture",
        limits: null,
        reload: vi.fn(),
        visible: false,
        opened: true,
      }),
    );
    expect(
      [...watchers.keys()].some((key) => key.includes("session-cost")),
    ).toBe(false);
    expect(watchers.has(JSON.stringify({ kind: "costs" }))).toBe(false);
  });

  it("requests account cost interest once when the limits window opens without polling", async () => {
    post.mockResolvedValue({});
    const render = (opened: boolean) =>
      mount(() =>
        Usage({
          agent: { id: "lead", rootId: "lead", accountKey: "default" } as Agent,
          stateDir: "private-fixture",
          limits: null,
          reload: vi.fn(),
          opened,
        }),
      );
    render(false);
    for (const accept of projectionListeners) accept({ runtime: {} });
    expect(post).not.toHaveBeenCalled();
    render(true);
    render(true);
    await vi.advanceTimersByTimeAsync(600_000);
    expect(post).toHaveBeenCalledExactlyOnceWith("/api/costs/interest", {
      account_key: "default",
    });
    expect(get).not.toHaveBeenCalled();
    render(false);
    render(true);
    expect(post).toHaveBeenCalledTimes(2);
    post.mockClear();
  });

  it("shows a failed refresh as stale while retaining the streamed saved value and its age", async () => {
    vi.setSystemTime(new Date(200_000));
    post.mockRejectedValue(new Error("Interest unavailable"));
    const render = () =>
      mount(() =>
        Usage({
          agent: { id: "lead", rootId: "lead", accountKey: "default" } as Agent,
          stateDir: "private-fixture",
          limits: null,
          reload: vi.fn(),
          opened: true,
        }),
      );
    render();
    for (const accept of projectionListeners) accept({ runtime: {} });
    await Promise.resolve();
    const saved = {
      accountKey: "default",
      at: 100,
      error: "Scan failed",
      stale: true,
      refreshing: false,
      data: { last30DaysUSD: 3 },
    };
    for (const accept of projectionListeners)
      accept({ runtime: { accountCostsByAccount: { default: saved } } });
    const view = JSON.stringify(render());
    expect(view).toContain("Refresh failed · stale estimate");
    expect(view).toContain("Updated 100 seconds ago");
    expect(view).toContain("$3.00");
    expect(post).toHaveBeenCalledTimes(1);
    expect(get).not.toHaveBeenCalled();
    post.mockClear();
  });

  it("settles a lost interest response with a newer authoritative cost result", async () => {
    let reject!: (error: Error) => void;
    post.mockReturnValue(
      new Promise((_resolve, fail) => {
        reject = fail;
      }),
    );
    const render = (opened: boolean) =>
      mount(() =>
        Usage({
          agent: { id: "lead", rootId: "lead", accountKey: "default" } as Agent,
          stateDir: "private-fixture",
          limits: null,
          reload: vi.fn(),
          opened,
        }),
      );
    render(false);
    const saved = {
      accountKey: "default",
      at: 100,
      checkedAt: 100,
      error: null,
      stale: true,
      refreshing: false,
      data: { last30DaysUSD: 3 },
    };
    for (const accept of projectionListeners)
      accept({ runtime: { accountCostsByAccount: { default: saved } } });
    render(true);
    reject(new Error("Response lost"));
    await Promise.resolve();
    expect(JSON.stringify(render(true))).toContain(
      "Refresh failed · stale estimate",
    );
    for (const accept of projectionListeners)
      accept({ runtime: { accountCostsByAccount: { default: { ...saved } } } });
    expect(JSON.stringify(render(true))).toContain(
      "Refresh failed · stale estimate",
    );
    const fresh = { ...saved, at: 200, checkedAt: 200, stale: false };
    for (const accept of projectionListeners)
      accept({ runtime: { accountCostsByAccount: { default: fresh } } });
    const view = JSON.stringify(render(true));
    expect(view).not.toContain("Refresh failed · stale estimate");
    expect(view).toContain("$3.00");
    post.mockClear();
  });

  it("ignores an obsolete opening rejection and a rejection after fresh SSE settlement", async () => {
    const rejections: Array<(error: Error) => void> = [];
    post.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejections.push(reject);
        }),
    );
    const render = (opened: boolean) =>
      mount(() =>
        Usage({
          agent: { id: "lead", rootId: "lead", accountKey: "default" } as Agent,
          stateDir: "private-fixture",
          limits: null,
          reload: vi.fn(),
          opened,
        }),
      );
    render(true);
    for (const accept of projectionListeners) accept({ runtime: {} });
    render(false);
    render(true);
    rejections[0]!(new Error("Old opening failed"));
    await Promise.resolve();
    expect(JSON.stringify(render(true))).not.toContain(
      "Refresh failed · stale estimate",
    );
    const fresh = {
      accountKey: "default",
      at: 200,
      checkedAt: 200,
      error: null,
      stale: false,
      refreshing: false,
      data: { last30DaysUSD: 4 },
    };
    for (const accept of projectionListeners)
      accept({ runtime: { accountCostsByAccount: { default: fresh } } });
    rejections[1]!(new Error("New response lost after SSE"));
    await Promise.resolve();
    const view = JSON.stringify(render(true));
    expect(view).not.toContain("Refresh failed · stale estimate");
    expect(view).toContain("$4.00");
    post.mockClear();
  });

  it("does not treat a switched account's initial cached value as refresh confirmation", async () => {
    const rejections: Array<(error: Error) => void> = [];
    post.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejections.push(reject);
        }),
    );
    const render = (accountKey: string) =>
      mount(() =>
        Usage({
          agent: { id: "lead", rootId: "lead", accountKey } as Agent,
          stateDir: "private-fixture",
          limits: null,
          reload: vi.fn(),
          opened: true,
        }),
      );
    render("default");
    for (const accept of projectionListeners)
      accept({
        runtime: {
          accountCostsByAccount: {
            second: {
              accountKey: "second",
              at: 100,
              stale: false,
              refreshing: false,
              error: null,
              data: { last30DaysUSD: 5 },
            },
            default: {
              accountKey: "default",
              at: 100,
              stale: false,
              refreshing: false,
              error: null,
            },
          },
        },
      });
    render("second");
    expect(post).toHaveBeenCalledTimes(2);
    for (const accept of projectionListeners)
      accept({
        runtime: {
          accountCostsByAccount: {
            second: {
              accountKey: "second",
              at: 100,
              stale: false,
              refreshing: false,
              error: null,
              data: { last30DaysUSD: 5 },
            },
          },
        },
      });
    expect(post).toHaveBeenCalledTimes(2);
    rejections[1]!(new Error("Second account interest failed"));
    await Promise.resolve();
    const view = JSON.stringify(render("second"));
    expect(view).toContain("Refresh failed · stale estimate");
    expect(view).toContain("$5.00");
    post.mockClear();
  });

  it("keeps distinct refresh attempts when selection returns from B to A", async () => {
    const rejections: Array<(error: Error) => void> = [];
    post.mockImplementation(
      () => new Promise((_resolve, reject) => rejections.push(reject)),
    );
    const render = (accountKey: string) =>
      mount(() =>
        Usage({
          agent: { id: "lead", accountKey },
          stateDir: "fixture",
          limits: null,
          reload() {},
          opened: true,
        }),
      );
    render("a");
    const saved = {
      accountKey: "a",
      at: 100,
      stale: false,
      refreshing: false,
      error: null,
      data: { last30DaysUSD: 3 },
    };
    for (const accept of projectionListeners)
      accept({ runtime: { accountCostsByAccount: { a: saved } } });
    render("b");
    render("a");
    expect(post).toHaveBeenCalledTimes(3);
    rejections[1]!(new Error("Old B failed"));
    await Promise.resolve();
    expect(JSON.stringify(render("a"))).not.toContain(
      "Refresh failed · stale estimate",
    );
    rejections[2]!(new Error("Newest A response lost"));
    await Promise.resolve();
    expect(JSON.stringify(render("a"))).toContain(
      "Refresh failed · stale estimate",
    );
    for (const accept of projectionListeners)
      accept({
        runtime: { accountCostsByAccount: { a: { ...saved, at: 200 } } },
      });
    expect(JSON.stringify(render("a"))).not.toContain(
      "Refresh failed · stale estimate",
    );
    post.mockClear();
  });

  it("applies both cost display projections without GETs", async () => {
    const session = {
      rootId: "lead",
      pricingState: "ready",
      totalUSD: 0.5,
      unknownModels: [],
      breakdown: {},
      estimated: true,
    };
    const account = { accountKey: "default", totalUSD: 1 };
    mount(() =>
      Usage({
        agent: { id: "lead", rootId: "lead", accountKey: "default" } as Agent,
        stateDir: "private-fixture",
        limits: null,
        reload: vi.fn(),
      }),
    );

    expect(watchers.has(JSON.stringify({ kind: "costs" }))).toBe(false);
    for (const accept of projectionListeners)
      accept({
        runtime: {
          sessionCostsByAgent: { lead: session },
          accountCostsByAccount: { default: account },
        },
      });
    expect(sessionState.get()).toMatchObject({
      status: "live",
      data: { totalUSD: 0.5 },
    });
    expect(costsState.get()).toMatchObject({ status: "live", data: [account] });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(get).not.toHaveBeenCalled();
    expect(syncPost).not.toHaveBeenCalled();
  });
});
