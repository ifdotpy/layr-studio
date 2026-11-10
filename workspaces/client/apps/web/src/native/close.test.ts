import { required } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { beforeEach, afterEach, expect, it, vi } from "vitest";
let initializeNativeClose: typeof import("./close.ts").initializeNativeClose;
const storage = vi.hoisted(() => ({ put: vi.fn(async () => {}) }));
vi.mock("../indexedDb.ts", () => ({ indexedDb: () => storage }));
let createDirectDrafts: typeof import("../sync/directDrafts.ts").createDirectDrafts;
let draftStore: typeof import("../unsent/store.ts").draftStore;
let unsentStore: typeof import("../unsent/store.ts").unsentStore;

beforeEach(async () => {
  vi.resetModules();
  storage.put.mockReset().mockResolvedValue(undefined);
  ({ initializeNativeClose } = await import("./close.ts"));
  ({ draftStore, unsentStore } = await import("../unsent/store.ts"));
  ({ createDirectDrafts } = await import("../sync/directDrafts.ts"));
});

afterEach(() => vi.unstubAllGlobals());

async function setup() {
  const listeners = new Set<(event: MessageEvent) => void>();
  let close!: (event: {
    payload: { id: number; action?: "reload" };
  }) => Promise<void>;
  let desktopError!: (event: { payload: { message: string } }) => void;
  const invoke = vi.fn(async () => {});
  const root = { inert: false };
  const append = vi.fn();
  vi.stubGlobal("location", { protocol: "tauri:" });
  vi.stubGlobal("window", {
    addEventListener: (_: string, listener: (event: MessageEvent) => void) =>
      listeners.add(listener),
    removeEventListener: (_: string, listener: (event: MessageEvent) => void) =>
      listeners.delete(listener),
  });
  vi.stubGlobal("document", {
    documentElement: root,
    getElementById: () => null,
    createElement: () => ({ style: {}, append: vi.fn(), remove: vi.fn() }),
    body: { append },
  });
  vi.stubGlobal("__TAURI__", {
    core: { invoke },
    event: {
      listen: async (name: string, callback: typeof close) => {
        if (name === "studio-close-request") close = callback;
        else if (name === "studio-desktop-error")
          desktopError = callback as unknown as typeof desktopError;
        return () => {};
      },
    },
  });
  await initializeNativeClose();
  return {
    invoke,
    desktopError: (message: string) => desktopError({ payload: { message } }),
    root,
    append,
    close: () => close({ payload: { id: 1 } }),
    reload: () => close({ payload: { id: 2, action: "reload" } }),
    message: (event: unknown) => {
      for (const listener of listeners) listener(event as MessageEvent);
    },
  };
}

function pendingWrite(feature: "form" | "draft" = "form") {
  let resolve!: () => void;
  let reject!: (reason: Error) => void;
  const transaction = new Promise<void>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  storage.put.mockReturnValueOnce(transaction);
  const saving =
    feature === "form"
      ? unsentStore("local", "form").put("draft", "last typed text")
      : draftStore("local").put("draft", {
          id: "draft",
          payload: "last typed text",
          seq: 0,
          _deleted: false,
        });
  return { saving, resolve, reject };
}

it.each(["form", "draft"] as const)(
  "waits for the unsent %s transaction before acknowledging close",
  async (feature) => {
    const harness = await setup();
    const write = pendingWrite(feature);
    const closing = harness.close();
    await Promise.resolve();
    await Promise.resolve();
    expect(harness.root.inert).toBe(true);
    expect(harness.invoke).not.toHaveBeenCalledWith("close_response", {
      id: 1,
      ready: true,
    });
    write.resolve();
    await write.saving;
    await closing;
    expect(harness.invoke).toHaveBeenLastCalledWith("close_response", {
      id: 1,
      ready: true,
    });
  },
);

it("keeps the window editable and reports a failed text save", async () => {
  const harness = await setup();
  const write = pendingWrite();
  const failed = expect(write.saving).rejects.toThrow("disk full");
  const closing = harness.close();
  write.reject(new Error("disk full"));
  await failed;
  await closing;
  expect(harness.root.inert).toBe(false);
  expect(harness.invoke).toHaveBeenLastCalledWith("close_response", {
    id: 1,
    ready: false,
  });
  expect(required(harness.append.mock.calls[0])[0].textContent).toContain(
    "disk full",
  );
  expect(required(harness.append.mock.calls[0])[0].textContent).toContain(
    "draft is not saved",
  );
  // The visible first refusal acknowledges the failure for this session.
  // Even the second close must still wait for any newer pending write.
  const newer = pendingWrite();
  const secondClose = harness.close();
  await new Promise<void>((resolve) => setImmediate(resolve));
  expect(harness.invoke).toHaveBeenLastCalledWith("close_response", {
    id: 1,
    ready: false,
  });
  newer.resolve();
  await newer.saving;
  await secondClose;
  expect(harness.invoke).toHaveBeenLastCalledWith("close_response", {
    id: 1,
    ready: true,
  });
});

it("refuses a permanent save failure once, then permits another close", async () => {
  const harness = await setup();
  const write = pendingWrite();
  const failed = expect(write.saving).rejects.toThrow("permanent failure");
  write.reject(new Error("permanent failure"));
  await failed;
  await harness.close();
  expect(harness.invoke).toHaveBeenLastCalledWith("close_response", {
    id: 1,
    ready: false,
  });
  expect(required(harness.append.mock.calls[0])[0].textContent).toContain(
    "draft is not saved",
  );
  await harness.close();
  expect(harness.invoke).toHaveBeenLastCalledWith("close_response", {
    id: 1,
    ready: true,
  });
});

it("reload preserves the text save barrier and reports a rejected save", async () => {
  const harness = await setup();
  const write = pendingWrite();
  const failed = expect(write.saving).rejects.toThrow("draft not saved");
  const reload = harness.reload();
  expect(harness.root.inert).toBe(true);
  expect(harness.invoke).toHaveBeenCalledTimes(1);
  write.reject(new Error("draft not saved"));
  await failed;
  await reload;
  expect(harness.invoke).toHaveBeenLastCalledWith("close_response", {
    id: 2,
    ready: false,
  });
  expect(harness.root.inert).toBe(false);
  expect(required(harness.append.mock.calls[0])[0].textContent).toContain(
    "Reload was cancelled",
  );
});

it("shows desktop configuration failures without freezing the editor", async () => {
  const harness = await setup();
  harness.desktopError("The active server prevents this change.");
  expect(harness.root.inert).toBe(false);
  const alert = required(harness.append.mock.calls[0])[0];
  expect(alert.role).toBe("alert");
  expect(alert.textContent).toBe("The active server prevents this change.");
  alert.append.mock.calls[0][0].onclick();
  expect(alert.remove).toHaveBeenCalledOnce();
});

it("waits for text saves in a native server frame before replying to its parent", async () => {
  const write = pendingWrite();
  const postMessage = vi.fn();
  const parent = { postMessage };
  let receive!: (event: {
    source: unknown;
    origin: string;
    data: { kind: string; id: string };
  }) => Promise<void>;
  const root = { inert: false };
  vi.stubGlobal("location", { protocol: "studio-view:" });
  vi.stubGlobal("window", {
    parent,
    addEventListener: (_: string, listener: typeof receive) => {
      receive = listener;
    },
  });
  vi.stubGlobal("document", { documentElement: root });
  await initializeNativeClose();
  const closing = receive({
    source: parent,
    origin: "http://localhost",
    data: { kind: "studio-native-close-query", id: "frame-close" },
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(root.inert).toBe(true);
  expect(postMessage).not.toHaveBeenCalled();
  write.resolve();
  await write.saving;
  await closing;
  expect(postMessage).toHaveBeenCalledWith(
    {
      kind: "studio-native-close-reply",
      id: "frame-close",
      ready: true,
    },
    "http://localhost",
  );
  await receive({
    source: parent,
    origin: "http://localhost",
    data: { kind: "studio-native-close-resume", id: "frame-close" },
  });
  expect(root.inert).toBe(false);
});

function transaction() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function composer() {
  return createDirectDrafts({
    post: async () => [],
    changed: () => {},
    buffer: draftStore("local"),
  });
}
function edit(drafts: ReturnType<typeof composer>, id: string, text: string) {
  drafts.edit({ id, payload: text, seq: 0, _deleted: false });
}
const turn = () => new Promise<void>((resolve) => setImmediate(resolve));

it("waits for successive real composer edits queued before a close", async () => {
  const harness = await setup();
  const a = transaction();
  const b = transaction();
  storage.put.mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise);
  const drafts = composer();
  edit(drafts, "queued", "A");
  edit(drafts, "queued", "B");
  const closing = harness.close();
  try {
    await turn();
    expect(harness.invoke).not.toHaveBeenCalledWith("close_response", {
      id: 1,
      ready: true,
    });
    a.resolve();
    await turn();
    expect(harness.invoke).not.toHaveBeenCalledWith("close_response", {
      id: 1,
      ready: true,
    });
  } finally {
    a.resolve();
    b.resolve();
    await closing;
    await turn();
  }
});

it("drains real composer queues across several keys", async () => {
  const harness = await setup();
  const a = transaction();
  const b = transaction();
  const c = transaction();
  storage.put
    .mockReturnValueOnce(a.promise)
    .mockReturnValueOnce(c.promise)
    .mockReturnValueOnce(b.promise);
  const drafts = composer();
  edit(drafts, "one", "A");
  await turn();
  edit(drafts, "one", "B");
  edit(drafts, "two", "C");
  await turn();
  const closing = harness.close();
  try {
    a.resolve();
    c.resolve();
    await turn();
    expect(harness.invoke).not.toHaveBeenCalledWith("close_response", {
      id: 1,
      ready: true,
    });
  } finally {
    a.resolve();
    b.resolve();
    c.resolve();
    await closing;
    await turn();
  }
});

it("drains a real composer write started while close is flushing", async () => {
  const harness = await setup();
  const a = transaction();
  const b = transaction();
  storage.put.mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise);
  const drafts = composer();
  edit(drafts, "first", "A");
  await turn();
  const closing = harness.close();
  edit(drafts, "later", "B");
  try {
    a.resolve();
    await turn();
    expect(harness.invoke).not.toHaveBeenCalledWith("close_response", {
      id: 1,
      ready: true,
    });
  } finally {
    a.resolve();
    b.resolve();
    await closing;
    await turn();
  }
});
