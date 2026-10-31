import { afterEach, expect, it, vi } from "vitest";
import { createCommandClient } from "../commands";
import { createOutbox } from "./send";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() {
    return this.values.size;
  }
  clear() {
    this.values.clear();
  }
  getItem(key: string) {
    return this.values.get(key) ?? null;
  }
  key(index: number) {
    return [...this.values.keys()][index] ?? null;
  }
  removeItem(key: string) {
    this.values.delete(key);
  }
  setItem(key: string, value: string) {
    this.values.set(key, value);
  }
}

class MemoryBuffer {
  values = new Map<string, string | Blob>();
  async put(key: string, value: string | Blob) {
    this.values.set(key, value);
  }
  async putIfExists(key: string, value: string) {
    if (this.values.has(key)) this.values.set(key, value);
  }
  async deleteIfEqual(key: string, value: string) {
    if (this.values.get(key) === value) this.values.delete(key);
  }
  async delete(key: string) {
    this.values.delete(key);
  }
  async loadAll(): Promise<[string, string | Blob][]> {
    return [...this.values];
  }
}

afterEach(() => vi.unstubAllGlobals());

it("sends another message while an earlier request remains uncertain and retries the exact command", async () => {
  vi.stubGlobal("navigator", { onLine: true });
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const post = vi
    .fn()
    .mockRejectedValueOnce(new TypeError("offline"))
    .mockImplementation(async (request) => ({
      rows: [
        {
          collection: "messageReceipt",
          payload: { id: request.id, status: "delivered" },
        },
      ],
    }));
  const command = createCommandClient(storage, post as never);
  const outbox = createOutbox({
    api: {
      command,
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  await outbox.durableSend({ id: "old", room: "chat", text: "old" });
  await outbox.durableSend({ id: "new", room: "chat", text: "new" });
  expect(post).toHaveBeenCalledTimes(2);
  await outbox.changeOutbox("old", "resume");
  expect(post).toHaveBeenCalledTimes(3);
  expect(post.mock.calls[2]?.[0]).toEqual(post.mock.calls[0]?.[0]);
});

it("continues sending when storage fails", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  vi.spyOn(buffer, "put").mockRejectedValue(new Error("disk full"));
  const post = vi.fn(async (request) => ({
    status: "applied" as const,
    ref_kind: "message",
    ref_id: request.id,
    rev: 1,
    seq: 1,
    rows: [],
  }));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  expect(
    await outbox.durableSend({ id: "second", room: "chat", text: "hello" }),
  ).toMatchObject({ kind: "server" });
  expect(post).toHaveBeenCalledOnce();
});

it("does not resurrect a discarded send when its request fails later", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  let reject!: (error: Error) => void;
  const post = vi.fn(
    () =>
      new Promise<never>((_, fail) => {
        reject = fail;
      }),
  );
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post as never),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  const send = outbox.durableSend({
    id: "pending",
    room: "chat",
    text: "hello",
  });
  await vi.waitFor(() => expect(post).toHaveBeenCalled());
  await outbox.changeOutbox("pending", "cancel");
  reject(new Error("lost response"));
  await send;
  expect(buffer.values.size).toBe(0);
});

it("removes a pending request when the stream confirms the matching workspace and message", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const post = vi.fn().mockRejectedValue(new Error("lost response"));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post as never),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  await outbox.durableSend({ id: "pending", room: "chat", text: "hello" });
  await outbox.reconcileOutboxReceipts(
    "chat",
    [{ id: "pending", status: "delivered" }],
    "other",
  );
  expect(buffer.values.size).toBe(1);
  await outbox.reconcileOutboxReceipts(
    "chat",
    [{ id: "pending", status: "delivered" }],
    "workspace",
  );
  expect(storage.length).toBe(1);
});

it("removes a definitive rejection from the buffer", async () => {
  const { ApiError } = await import("../api");
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const post = vi
    .fn()
    .mockRejectedValueOnce(new ApiError("refused", 400))
    .mockImplementation(async (request) => ({
      rows: [
        {
          collection: "messageReceipt",
          payload: { id: request.id, status: "delivered" },
        },
      ],
    }));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post as never),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  expect(
    await outbox.durableSend({ id: "refused", room: "chat", text: "hello" }),
  ).toMatchObject({ status: "failed", error: "refused" });
  expect(post).toHaveBeenCalledTimes(1);
  expect(buffer.values.size).toBe(0);
  await expect(outbox.changeOutbox("refused", "resume")).rejects.toThrow(
    "no longer",
  );
  expect(storage.length).toBe(1);
});

it("does not send a stored message to a different workspace", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  let workspaceId = "first";
  const post = vi.fn().mockRejectedValue(new Error("offline"));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post as never),
      getStreamSession: () => ({ workspaceId }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  await outbox.durableSend({ id: "saved", room: "chat", text: "hello" });
  workspaceId = "second";
  await outbox.changeOutbox("saved", "resume");
  expect(post).toHaveBeenCalledTimes(1);
  await outbox.changeOutbox("saved", "cancel");
  expect(storage.length).toBe(1);
});

it("allows a damaged saved message to be discarded without sending it", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  storage.setItem(
    `studio-message-send:${JSON.stringify(["chat", "damaged"])}`,
    "not json",
  );
  const post = vi.fn();
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  await expect(outbox.changeOutbox("damaged", "resume")).rejects.toThrow(
    "cannot be retried",
  );
  await outbox.changeOutbox("damaged", "cancel");
  expect(storage.length).toBe(0);
  expect(post).not.toHaveBeenCalled();
});

it("accepts a command reply whose receipt projection precedes the flat receipt", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const post = vi.fn(async (request) => ({
    rows: [
      {
        collection: "messageReceipt",
        payload: {
          collection: "messageReceipt",
          id: request.id,
          value: { id: request.id, status: "pending" },
        },
      },
      {
        collection: "messageReceipt",
        payload: { id: request.id, status: "queued" },
      },
    ],
  }));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post as never),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  const result = await outbox.durableSend({
    id: "one",
    room: "chat",
    text: "hello",
  });
  expect(result).toMatchObject({ kind: "server", id: "one", status: "queued" });
  expect(storage.length).toBe(1);
});

it("accepts an applied message reference even when the response omits receipt rows", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const post = vi.fn(async (request) => ({
    status: "applied" as const,
    ref_kind: "message",
    ref_id: request.id,
    seq: 1,
    rev: 1,
    rows: [],
  }));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  const result = await outbox.durableSend({
    id: "one",
    room: "chat",
    text: "hello",
  });
  expect(result).toMatchObject({
    kind: "server",
    id: "one",
    status: "pending",
  });
  expect(storage.length).toBe(1);
});

it("coalesces a reconnect with a send whose persistence callback is still running", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  let release!: () => void;
  const persisted = new Promise<void>((resolve) => {
    release = resolve;
  });
  const post = vi.fn(async (request) => {
    await persisted;
    return {
      rows: [
        {
          collection: "messageReceipt",
          payload: { id: request.id, status: "delivered" },
        },
      ],
    };
  });
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post as never),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  const first = outbox.durableSend(
    { id: "one", room: "chat", text: "hello" },
    [],
    () => persisted,
  );
  const reconnect = outbox.changeOutbox("one", "resume");
  release();
  await Promise.all([first, reconnect]);
  expect(post).toHaveBeenCalledTimes(1);
});

it("keeps a remote outcome-unknown error uncertain instead of treating it as a refusal", async () => {
  const { ApiError } = await import("../api");
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const post = vi
    .fn()
    .mockRejectedValue(
      new ApiError("Outcome unknown", 409, { code: "request_pending" }),
    );
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  expect(
    await outbox.durableSend({ id: "pending", room: "chat", text: "hello" }),
  ).toMatchObject({ status: "uncertain" });
});

it("restores an uncertain command after reload and removes it on confirmation", async () => {
  const storage = new MemoryStorage();
  const records = new Map<string, string>();
  const buffer = {
    putIfExists: async (key: string, value: string) => {
      if (records.has(key)) records.set(key, value);
    },
    deleteIfEqual: async (key: string, value: string) => {
      if (records.get(key) === value) records.delete(key);
    },
    put: async (key: string, value: string) => {
      records.set(key, value);
    },
    delete: async (key: string) => {
      records.delete(key);
    },
    loadAll: async () => [...records.entries()],
  };
  const post = vi
    .fn()
    .mockRejectedValueOnce(new Error("offline"))
    .mockImplementation(async (request) => ({
      rows: [
        {
          collection: "messageReceipt",
          payload: { id: request.id, status: "delivered" },
        },
      ],
    }));
  const options = {
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  };
  await createOutbox(options).durableSend({
    id: "saved",
    room: "chat",
    text: "hello",
  });
  expect(records.size).toBe(1);
  await createOutbox(options).changeOutbox("saved", "resume");
  expect(records.size).toBe(0);
  expect(post.mock.calls[1]?.[0]).toEqual(post.mock.calls[0]?.[0]);
});

it("dispatches immediately when the buffer write never finishes", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  vi.spyOn(buffer, "put").mockImplementation(() => new Promise(() => {}));
  const post = vi.fn(async (request) => ({
    status: "applied" as const,
    ref_kind: "message",
    ref_id: request.id,
    rev: 1,
    seq: 1,
    rows: [],
  }));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  void outbox.durableSend({ id: "stalled", room: "chat", text: "hello" });
  await vi.waitFor(() => expect(post).toHaveBeenCalledOnce());
});

it("does not resurrect a command confirmed in another tab", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const post = vi
    .fn()
    .mockRejectedValueOnce(new Error("offline"))
    .mockImplementation(async (request) => ({
      rows: [
        {
          collection: "messageReceipt",
          payload: { id: request.id, status: "delivered" },
        },
      ],
    }));
  const options = {
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  };
  const first = createOutbox(options);
  await first.durableSend({ id: "shared", room: "chat", text: "hello" });
  await createOutbox(options).changeOutbox("shared", "resume");
  expect(buffer.values.size).toBe(0);
  await first.editOutboxDisplay("shared", "edited");
  expect(buffer.values.size).toBe(0);
});

it("retries initialization after a storage failure", async () => {
  const storage = new MemoryStorage();
  const buffer = new MemoryBuffer();
  const load = vi
    .spyOn(buffer, "loadAll")
    .mockRejectedValueOnce(new Error("unavailable"));
  const post = vi
    .fn()
    .mockRejectedValueOnce(new Error("offline"))
    .mockImplementation(async (request) => ({
      status: "applied" as const,
      ref_kind: "message",
      ref_id: request.id,
      rev: 1,
      seq: 1,
      rows: [],
    }));
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    storage,
    buffer,
    watchResourceConnection: () => () => {},
  });
  await outbox.durableSend({ id: "retry-init", room: "chat", text: "hello" });
  await outbox.changeOutbox("retry-init", "resume");
  expect(load.mock.calls.length).toBeGreaterThan(1);
  expect(post).toHaveBeenCalledTimes(2);
});
