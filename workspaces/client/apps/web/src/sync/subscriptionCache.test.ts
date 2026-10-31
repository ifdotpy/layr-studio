import { afterEach, expect, it, vi } from "vitest";
import {
  cacheableSubscription,
  createSubscriptionCache,
  subscriptionRows,
} from "./subscriptionCache";
import { LocalCollection } from "./localCollection";
import { cursorDocumentId } from "./subscriptions/cursors";
import { API_SCHEMA_HASH } from "../generated/apiSchema";
import { AgentStatus } from "../generated/api";

const ref = { kind: "transcript", agentId: "agent" } as const;
const cursor = { workspaceId: "workspace", seq: 4 };
it("requests snapshots for team-scoped subscriptions", () => {
  expect(cacheableSubscription({ kind: "tasks", agentId: "lead" })).toBe(false);
  expect(cacheableSubscription({ kind: "monitors", agentId: "lead" })).toBe(
    false,
  );
  expect(cacheableSubscription({ kind: "work", agentId: "lead" })).toBe(false);
});
const row = { id: "transcript:agent", seq: 4, payload: "{}" };
const cursorRow = {
  id: "transcript:cursor:agent",
  seq: 4,
  payload: JSON.stringify(cursor),
};

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

it("hydrates the cached data before returning its resume cursor", async () => {
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: 1,
      contractHash: API_SCHEMA_HASH,
      kind: ref.kind,
      cursor,
      data: [row, cursorRow],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  expect(await cache.load(ref)).toEqual(cursor);
  expect(await collection.query({})).toEqual([
    { ...row, _deleted: false },
    { ...cursorRow, _deleted: false },
  ]);
  cache.stop();
});

it("drops a malformed entity before exposing a resume cursor", async () => {
  const selected = { kind: "agentDetail", agentId: "agent" } as const;
  const cursorId = await cursorDocumentId(selected);
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: 1,
      contractHash: API_SCHEMA_HASH,
      kind: selected.kind,
      cursor,
      data: [
        {
          id: cursorId,
          seq: 4,
          payload: JSON.stringify(cursor),
          _deleted: false,
        },
        {
          id: "entity:agentDetail:agent",
          seq: 4,
          payload: "",
          _deleted: false,
          entity: { collection: "agentDetail", id: "agent", value: null },
        },
      ],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  expect(await cache.load(selected)).toBeUndefined();
  expect(storage.delete).toHaveBeenCalledOnce();
  expect(collection.all()).toEqual([]);
  cache.stop();
});

it("does not save auxiliary team summaries under a tasks cursor", async () => {
  const collection = new LocalCollection("fixture");
  await collection.hydrate([
    {
      id: "entity:agentSummary:child",
      seq: 2,
      payload: "",
      _deleted: false,
      entity: {
        collection: "agentSummary",
        id: "child",
        value: {
          id: "child",
          rootId: "lead",
          epoch: 1,
          name: "Child",
          status: AgentStatus.idle,
        },
      },
    },
  ]);
  const rows = await subscriptionRows(
    { kind: "tasks", agentId: "lead" },
    collection,
  );
  expect(rows).toEqual([]);
});

it("drops a foreign entity instead of resurrecting it under another cursor", async () => {
  const selected = { kind: "tasks", agentId: "lead" } as const;
  const cursorId = await cursorDocumentId(selected);
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: 1,
      contractHash: API_SCHEMA_HASH,
      kind: selected.kind,
      cursor,
      data: [
        {
          id: cursorId,
          seq: 4,
          payload: JSON.stringify(cursor),
          _deleted: false,
        },
        {
          id: "entity:agentSummary:child",
          seq: 2,
          payload: "",
          _deleted: false,
          entity: {
            collection: "agentSummary",
            id: "child",
            value: {
              id: "child",
              rootId: "lead",
              epoch: 1,
              name: "Child",
              status: AgentStatus.idle,
            },
          },
        },
      ],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  expect(await cache.load(selected)).toBeUndefined();
  expect(collection.all()).toEqual([]);
  expect(storage.delete).toHaveBeenCalledOnce();
  cache.stop();
});

it.each([
  { version: 2, kind: ref.kind, cursor, data: [row, cursorRow] },
  { version: 1, kind: "state", cursor, data: [row, cursorRow] },
  { version: 1, kind: ref.kind, cursor, data: [{ id: "bad" }] },
  { version: 1, kind: ref.kind, cursor, data: [row] },
  {
    version: 1,
    contractHash: "wrong",
    kind: ref.kind,
    cursor,
    data: [row, cursorRow],
  },
])(
  "drops corrupt or incompatible records without returning a cursor",
  async (record) => {
    const storage = {
      get: vi
        .fn()
        .mockResolvedValue({ contractHash: API_SCHEMA_HASH, ...record }),
      put: vi.fn(),
      delete: vi.fn(),
    };
    const cache = createSubscriptionCache(
      storage,
      new LocalCollection("fixture"),
      "workspace",
    );
    expect(await cache.load(ref)).toBeUndefined();
    expect(storage.delete).toHaveBeenCalledOnce();
    cache.stop();
  },
);

it("coalesces writes until five seconds and flushes on hide", async () => {
  vi.useFakeTimers();
  const events = new EventTarget();
  vi.stubGlobal("document", {
    visibilityState: "hidden",
    addEventListener: events.addEventListener.bind(events),
    removeEventListener: events.removeEventListener.bind(events),
  });
  const storage = {
    get: vi.fn(),
    put: vi.fn().mockResolvedValue(undefined),
    delete: vi.fn(),
  };
  const cache = createSubscriptionCache(
    storage,
    new LocalCollection("fixture"),
    "workspace",
  );
  cache.save(ref, cursor, [row]);
  cache.save(ref, cursor, [row]);
  expect(storage.put).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(5_000);
  expect(storage.put).toHaveBeenCalledOnce();
  cache.save(ref, cursor, [row]);
  events.dispatchEvent(new Event("visibilitychange"));
  expect(storage.put).toHaveBeenCalledTimes(2);
  cache.stop();
});
