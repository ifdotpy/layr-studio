import { beforeEach, expect, it, vi } from "vitest";

const rows = vi.hoisted(() => new Map<string, unknown>());
vi.mock("../indexedDb.ts", () => ({
  indexedDb: () => ({
    put: async (key: string, value: unknown) => {
      rows.set(key, value);
    },
    delete: async (key: string) => {
      rows.delete(key);
    },
    update: async (key: string, change: (value: unknown) => unknown) => {
      const next = change(rows.get(key));
      if (next === undefined) rows.delete(key);
      else rows.set(key, next);
    },
    entries: async () => [...rows.entries()],
    putMany: async (values: [string, unknown][]) => {
      for (const [key, value] of values) rows.set(key, value);
    },
    scan: async <T>(
      inspect: (values: [IDBValidKey, unknown][]) => {
        value: T;
        remove: IDBValidKey[];
      },
    ) => {
      const result = inspect([...rows.entries()]);
      for (const key of result.remove)
        if (typeof key === "string") rows.delete(key);
      return result.value;
    },
  }),
}));
import { unsentKey, unsentStore } from "./store.ts";

const store = unsentStore("server", "form");
const key = unsentKey("server", "form", "one");
beforeEach(() => {
  rows.clear();
  vi.spyOn(console, "error").mockImplementation(() => {});
});

it("deletes a corrupt record", async () => {
  rows.set(key, { broken: true });
  expect(await store.loadAll()).toEqual([]);
  expect(rows.has(key)).toBe(false);
});

it("deletes a contradictory record", async () => {
  rows.set(key, { version: 1, kind: "text", value: "hello", blob: new Blob() });
  expect(await store.loadAll()).toEqual([]);
  expect(rows.has(key)).toBe(false);
});

it("deletes an unknown record version", async () => {
  rows.set(key, { version: 999, kind: "text", value: "hello" });
  expect(await store.loadAll()).toEqual([]);
  expect(rows.has(key)).toBe(false);
});

it("round trips a valid record", async () => {
  await store.put("one", "hello");
  expect(await store.loadAll()).toEqual([["one", "hello"]]);
});

it("deletes an invalid message payload instead of returning a partial record", async () => {
  const messageKey = unsentKey(
    "server",
    "message",
    JSON.stringify(["chat", "one"]),
  );
  rows.set(messageKey, {
    version: 1,
    kind: "text",
    value: '{"body":{"id":"one"}}',
  });
  const { sendStore } = await import("./store.ts");
  expect(await sendStore("server").loadAll()).toEqual([]);
  expect(rows.has(messageKey)).toBe(false);
});

it("round trips a typed draft record", async () => {
  const { draftStore } = await import("./store.ts");
  const draft = { id: "draft", payload: "text", seq: 1, _deleted: false };
  const store = draftStore("server");
  await store.put(draft.id, draft);
  expect(await store.loadAll()).toEqual([[draft.id, draft]]);
});

it("round trips a typed upload and its bytes", async () => {
  const { uploadStore } = await import("./store.ts");
  const value = {
    id: "file",
    workspace: "workspace",
    stateDir: "state",
    agent: "chat",
    name: "file.txt",
    size: 3,
    mime: "text/plain",
    created: 1,
    bytes: new Blob(["abc"]),
  };
  const store = uploadStore("server");
  await store.put(value.id, value);
  const restored = (await store.loadAll())[0]?.[1];
  expect(restored?.id).toBe(value.id);
  expect(await restored?.bytes.text()).toBe("abc");
});

it("round trips a recording and its chunk", async () => {
  const { recordingStore } = await import("./store.ts");
  const store = recordingStore("server");
  const value = {
    id: "r",
    chatId: "chat",
    created: 1,
    sampleRate: 16000,
    samples: 1,
    state: "recording" as const,
  };
  await store.put("record:r", { kind: "record", value });
  await store.put('chunk:["r",0]', {
    kind: "chunk",
    id: "r",
    index: 0,
    value: new Blob([new Int16Array([1])]),
  });
  expect((await store.loadAll()).map(([key]) => key).sort()).toEqual([
    'chunk:["r",0]',
    "record:r",
  ]);
});

it("round trips the exact generated command request", async () => {
  const { sendStore } = await import("./store.ts");
  const store = sendStore("server");
  const value = {
    request: {
      id: "one",
      client: "client",
      number: 1,
      kind: "send" as const,
      target: "chat",
      body: { text: "hello", assets: ["asset"] },
    },
    workspaceId: "workspace",
    created: 1,
    attachments: [],
    localState: { kind: "stored" as const },
  };
  const id = JSON.stringify(["chat", "one"]);
  await store.put(id, value);
  const restored = (await store.loadAll())[0]?.[1];
  expect(JSON.stringify(restored?.request)).toBe(JSON.stringify(value.request));
});

it("discards a recording with a missing chunk", async () => {
  const { recordingStore } = await import("./store.ts");
  const store = recordingStore("server");
  await store.put("record:r", {
    kind: "record",
    value: {
      id: "r",
      chatId: "chat",
      created: 1,
      sampleRate: 16000,
      samples: 2,
      state: "ready",
    },
  });
  await store.put('chunk:["r",1]', {
    kind: "chunk",
    id: "r",
    index: 1,
    value: new Blob([new Int16Array([1, 2])]),
  });
  expect(await store.loadAll()).toEqual([]);
  expect(rows.size).toBe(0);
});

it("deletes and logs a record with an invalid key", async () => {
  rows.set("not-json", { version: 1, kind: "text", value: "orphan" });
  expect(await store.loadAll()).toEqual([]);
  expect(rows.has("not-json")).toBe(false);
  expect(console.error).toHaveBeenCalled();
});
