import { beforeEach, expect, it, vi } from "vitest";

const rows = vi.hoisted(() => new Map<string, unknown>());
vi.mock("../indexedDb.ts", () => ({
  indexedDb: () => ({
    get: async (key: string) => rows.get(key),
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
  expect(await store.get("one")).toBe("hello");
  expect(await store.get("missing")).toBeUndefined();
  expect(await store.loadAll()).toEqual([["one", "hello"]]);
});

it("reads only the requested record and discards it if invalid", async () => {
  const other = unsentKey("other", "upload", "file");
  rows.set(other, { broken: true });
  rows.set(key, { broken: true });
  expect(await store.get("one")).toBeUndefined();
  expect(rows.has(key)).toBe(false);
  expect(rows.has(other)).toBe(true);
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
      body: { text: "hello", assets: ["asset"], asTextFile: true },
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

const textFeatures = ["form", "answer", "queue-edit", "reply"] as const;

it("writes each text input kind at once and removes it only on its confirmation", async () => {
  for (const feature of textFeatures) {
    const buffer = unsentStore("server", feature);
    const rowKey = unsentKey("server", feature, "item");
    await buffer.put("item", `${feature} text`);
    expect(rows.has(rowKey)).toBe(true);
    await buffer.deleteIfEqual("item", "older text");
    expect(await buffer.get("item")).toBe(`${feature} text`);
    await buffer.deleteIfEqual("item", `${feature} text`);
    expect(rows.has(rowKey)).toBe(false);
  }
});

it("restores each text input kind after a simulated reload", async () => {
  for (const feature of textFeatures)
    await unsentStore("server", feature).put("item", `${feature} text`);
  vi.resetModules();
  const reloaded = await import("./store.ts");
  for (const feature of textFeatures)
    expect(await reloaded.unsentStore("server", feature).get("item")).toBe(
      `${feature} text`,
    );
});

it("keeps a draft, upload, recording, and send until its confirmation and restores it after reload", async () => {
  const draft = { id: "draft", payload: "text", seq: 1, _deleted: false };
  const upload = {
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
  const recording = {
    id: "r",
    chatId: "chat",
    created: 1,
    sampleRate: 16000,
    samples: 1,
    state: "recording" as const,
  };
  const sent = {
    request: {
      id: "one",
      client: "client",
      number: 1,
      kind: "send" as const,
      target: "chat",
      body: { text: "hello" },
    },
    localState: { kind: "stored" as const },
    workspaceId: "workspace",
    created: 1,
    attachments: [],
  };
  const sendId = JSON.stringify(["chat", "one"]);
  const stores = await import("./store.ts");
  await stores.draftStore("server").put(draft.id, draft);
  await stores.uploadStore("server").put(upload.id, upload);
  await stores.recordingStore("server").put("record:r", {
    kind: "record",
    value: recording,
  });
  await stores.sendStore("server").put(sendId, sent);
  const before = rows.size;
  expect(before).toBe(4);

  vi.resetModules();
  const reloaded = await import("./store.ts");
  expect(
    (await reloaded.draftStore("server").loadAll()).map(([id]) => id),
  ).toEqual([draft.id]);
  expect(
    (await reloaded.uploadStore("server").loadAll()).map(([id]) => id),
  ).toEqual([upload.id]);
  expect(
    (await reloaded.recordingStore("server").loadAll()).map(([key]) => key),
  ).toEqual(["record:r"]);
  expect(
    (await reloaded.sendStore("server").loadAll()).map(([id]) => id),
  ).toEqual([sendId]);

  await reloaded.draftStore("server").delete(draft.id);
  await reloaded.uploadStore("server").delete(upload.id);
  await reloaded.recordingStore("server").delete("record:r");
  await reloaded.sendStore("server").delete(sendId);
  expect(rows.size).toBe(0);
});
