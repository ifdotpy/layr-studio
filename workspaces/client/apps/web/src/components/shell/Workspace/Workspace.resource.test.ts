import type { GetResult } from "../../../api.ts";
import { watchResourceReads } from "../../../sync/subscriptions/reads.ts";
import type { ResourceVersion } from "../../../sync/subscriptions/contracts.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { autorun, flowResult } from "mobx";
import { expect, it, vi } from "vitest";
import { WorkspaceResources } from "../../../app/shell/WorkspaceResources.ts";
import { WorkspaceResource } from "../../../app/shell/WorkspaceResource.ts";

const api = vi.hoisted(() => ({ get: vi.fn(), post: vi.fn() }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({ useServerApi: () => api }),
);
let workspace: WorkspaceResources;
vi.mock("../../../app/ServerAreasProvider.tsx", () => ({
  useServerAreas: () => ({ workspace }),
}));
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}

it("coalesces refreshes without invalidating outstanding reads and keeps their ETag", async () => {
  const first = deferred<{ value: string } | undefined>();
  const read = vi
    .fn<WorkspaceResource<{ value: string }>["read"]>()
    .mockImplementationOnce((_signal, _etag, metadata) => {
      metadata.etag = "v1";
      return first.promise;
    })
    .mockImplementationOnce(async (_signal, etag, metadata) => {
      expect(etag).toBe("v1");
      metadata.notModified = true;
      metadata.etag = "v1";
      return undefined;
    });
  const state = new WorkspaceResource(read);
  const stop = state.start();
  const pending = flowResult(state.load());
  await flowResult(state.load());
  await flowResult(state.load());
  expect(read).toHaveBeenCalledTimes(1);
  first.resolve({ value: "first" });
  await pending;
  await Promise.resolve();
  expect(read).toHaveBeenCalledTimes(2);
  expect(state.data).toEqual({ value: "first" });
  expect(state.loading).toBe(false);
  stop();
});

it("keeps the last successful data on error and clears the error on a not-modified response", async () => {
  const read = vi
    .fn<WorkspaceResource<{ value: string }>["read"]>()
    .mockResolvedValueOnce({ value: "retained" })
    .mockRejectedValueOnce(new Error("Disconnected"))
    .mockImplementationOnce(async (_signal, _etag, metadata) => {
      metadata.notModified = true;
      return undefined;
    });
  const state = new WorkspaceResource(read),
    stop = state.start();
  await flowResult(state.load());
  await flowResult(state.load());
  expect(state.data).toEqual({ value: "retained" });
  expect(state.error).toBe("Disconnected");
  expect(state.loading).toBe(false);
  await flowResult(state.load());
  expect(state.error).toBe("");
  stop();
});

it("aborts cleanup and restarts a committed lifecycle without accepting an aborted response", async () => {
  const first = deferred<{ value: string } | undefined>();
  const read = vi
    .fn<WorkspaceResource<{ value: string }>["read"]>()
    .mockReturnValueOnce(first.promise)
    .mockResolvedValueOnce({ value: "new" });
  const state = new WorkspaceResource(read),
    stop = state.start();
  const pending = flowResult(state.load());
  stop();
  expect(read.mock.calls[0]?.[0].aborted).toBe(true);
  const stopAgain = state.start();
  await flowResult(state.load());
  first.resolve({ value: "aborted" });
  await pending;
  await Promise.resolve();
  expect(state.data).toEqual({ value: "new" });
  stopAgain();
});

it("the panel caller tracks query changes, command revisions and late cleanup responses", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const first = deferred<{ checkpoints: [] }>();
  api.get
    .mockReset()
    .mockReturnValueOnce(first.promise)
    .mockResolvedValue({ checkpoints: [{ id: "new" }] });
  workspace = new WorkspaceResources(api);
  const View = observer(({ agent }: { agent: string }) => {
    const state = workspace.reads["/api/checkpoints"]({ agent });
    return createElement(
      "p",
      null,
      state.data?.checkpoints[0]?.id ??
        (state.loading ? "Loading" : state.error),
    );
  });
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host);
  try {
    await act(async () => root.render(createElement(View, { agent: "old" })));
    expect(host.textContent).toBe("Loading");
    await act(async () => root.render(createElement(View, { agent: "new" })));
    expect(api.get.mock.calls[0]?.[1].signal.aborted).toBe(true);
    expect(host.textContent).toBe("new");
    await act(async () => first.resolve({ checkpoints: [] }));
    expect(host.textContent).toBe("new");
    await act(async () => workspace.reload());
    expect(api.get).toHaveBeenCalledTimes(3);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    workspace.dispose();
    vi.unstubAllGlobals();
  }
});

it("loads only while observed and aborts when its last observer leaves", async () => {
  const pending = deferred<{ value: string } | undefined>();
  const read = vi
    .fn<WorkspaceResource<{ value: string }>["read"]>()
    .mockReturnValue(pending.promise);
  const state = new WorkspaceResource(read);
  expect(read).not.toHaveBeenCalled();
  const stop = autorun(() => {
    void state.data;
  });
  await Promise.resolve();
  expect(read).toHaveBeenCalledTimes(1);
  stop();
  await Promise.resolve();
  expect(read.mock.calls[0]?.[0].aborted).toBe(true);
  pending.resolve({ value: "late" });
  await pending.promise;
  expect(state.data).toBeNull();
});

it("capabilities retry a failed native catalog revision after the same reconnect baseline", async () => {
  vi.useFakeTimers();
  let notify: (version?: ResourceVersion) => void = () => {
    throw new Error("Missing watcher");
  };
  const stopWatch = vi.fn();
  const watch = vi.fn<
    NonNullable<ConstructorParameters<typeof WorkspaceResources>[2]>
  >((refs, read, failed, options) =>
    watchResourceReads(refs, read, failed, {
      ...options,
      events: {
        watchResourceChanges: (_ref, receive) => {
          notify = receive;
          return stopWatch;
        },
        watchResourceConnection: () => () => {},
      },
    }),
  );
  const catalog = {
    skills: [],
    mcp: [{ name: "files", status: "failed" }],
    mcpSession: {
      account: "account",
      thread: "thread",
      epoch: 1,
      connection: "connection",
      generation: 1,
    },
    managed: [],
    observedNative: [],
    errors: [],
  } satisfies Pick<
    GetResult<"/api/capabilities">,
    "skills" | "mcp" | "mcpSession" | "managed" | "observedNative" | "errors"
  >;
  const held = deferred<typeof catalog>();
  api.get
    .mockReset()
    .mockRejectedValueOnce(new Error("Disconnected"))
    .mockResolvedValueOnce(catalog)
    .mockReturnValueOnce(held.promise)
    .mockResolvedValue(catalog);
  api.post
    .mockReset()
    .mockResolvedValue({ status: "completed", message: "Reconnected" });
  workspace = new WorkspaceResources(api, "server", watch);
  const { state, inventory } = workspace.capabilities("agent", "account");
  const stop = autorun(() => void state.data);
  const stopSecondObserver = autorun(() => void state.data);
  try {
    await vi.advanceTimersByTimeAsync(0);
    const baseline = { epoch: "server", revision: 5 };
    notify(baseline);
    await vi.advanceTimersByTimeAsync(0);
    expect(state.error).toBe("Disconnected");
    expect(api.get).toHaveBeenCalledTimes(1);
    notify(baseline); // Reconnected stream repeats the same native catalog baseline.
    await vi.advanceTimersByTimeAsync(0);
    expect(api.get).toHaveBeenCalledTimes(2);
    expect(state.error).toBe("");
    const invalidation = { ...baseline, revision: 6 };
    notify(invalidation);
    notify(invalidation);
    await vi.advanceTimersByTimeAsync(0);
    expect(api.get).toHaveBeenCalledTimes(3);
    await flowResult(inventory.reconnectServer("files"));
    expect(inventory.pageError).toBe("");
    expect(api.post).toHaveBeenCalledTimes(1);
    expect(Object.values(inventory.feedback)).toEqual([
      { busy: false, message: "Reconnected" },
    ]);
    expect(api.get).toHaveBeenCalledTimes(3); // No concurrent discovery while the older read is held.
    held.resolve(catalog);
    await vi.advanceTimersByTimeAsync(0);
    expect(api.get).toHaveBeenCalledTimes(4); // Exactly one sequential read after reconnect completed.
    expect(workspace.capabilities("agent", "other").state).not.toBe(state);
  } finally {
    held.resolve(catalog);
    stopSecondObserver();
    stop();
    await vi.advanceTimersByTimeAsync(0);
    workspace.dispose();
    vi.useRealTimers();
  }
  expect(stopWatch).toHaveBeenCalledTimes(1);
});
