import { afterEach, expect, it, vi } from "vitest";
import {
  SUBSCRIPTION_CACHE_VERSION,
  createSubscriptionCache,
  subscriptionRows,
} from "./subscriptionCache";
import { LocalCollection } from "./localCollection";
import { subscriptionKey } from "./subscriptions/contracts";
import { TRANSCRIPT_PROJECTION_FORMAT } from "./transcriptMetadata";
import { API_SCHEMA_HASH } from "../generated/apiSchema";
import { AgentStatus, SubscriptionCachePolicy } from "../generated/api";

const ref = { kind: "transcript", agentId: "agent" } as const;
const cursor = { workspaceId: "workspace", seq: 4 };
const row = {
  id: "transcript:agent",
  seq: 4,
  payload: JSON.stringify({
    format: TRANSCRIPT_PROJECTION_FORMAT,
    metadata: {},
    order: [],
  }),
};
const cachedRow = { ...row, kind: "transcript" };
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
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(ref),
      cursor,
      data: [cachedRow],
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
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(selected),
      cursor,
      data: [
        {
          kind: "entity",
          id: "entity:agentDetail:agent",
          seq: 4,
          entity: { collection: "agentDetail", id: "agent", value: {} },
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

it("hydrates an agent detail with its matching identity", async () => {
  const selected = { kind: "agentDetail", agentId: "agent" } as const;
  const entity = {
    collection: "agentDetail",
    id: "agent",
    value: { id: "agent" },
  };
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(selected),
      cursor,
      data: [
        { kind: "entity", id: "entity:agentDetail:agent", seq: 4, entity },
      ],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  expect(await cache.load(selected)).toEqual(cursor);
  expect(collection.all()).toContainEqual({
    id: "entity:agentDetail:agent",
    seq: 4,
    payload: "",
    _deleted: false,
    entity,
  });
  expect(storage.delete).not.toHaveBeenCalled();
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
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(selected),
      cursor,
      data: [
        {
          kind: "entity",
          id: "entity:agentSummary:child",
          seq: 2,
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
  {
    version: SUBSCRIPTION_CACHE_VERSION,
    contractHash: API_SCHEMA_HASH,
    subscription: "other subscription",
    cursor,
    data: [cachedRow],
  },
  {
    version: SUBSCRIPTION_CACHE_VERSION - 1,
    subscription: subscriptionKey(ref),
    cursor,
    data: [cachedRow],
  },
  {
    version: SUBSCRIPTION_CACHE_VERSION,
    cursor,
    data: [cachedRow],
  },
  {
    version: SUBSCRIPTION_CACHE_VERSION,
    subscription: subscriptionKey(ref),
    cursor,
    data: [{ id: "bad" }],
  },
  {
    version: SUBSCRIPTION_CACHE_VERSION,
    subscription: subscriptionKey(ref),
    data: [cachedRow],
  },
  {
    version: SUBSCRIPTION_CACHE_VERSION,
    contractHash: "wrong",
    subscription: subscriptionKey(ref),
    cursor,
    data: [cachedRow],
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
  await cache.setPolicy(ref, SubscriptionCachePolicy.cache);
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

it("deletes a record and refuses writes when the server marks it NoCache", async () => {
  const storage = { get: vi.fn(), put: vi.fn(), delete: vi.fn() };
  const cache = createSubscriptionCache(
    storage,
    new LocalCollection("fixture"),
    "workspace",
  );
  await cache.setPolicy(ref, SubscriptionCachePolicy.noCache);
  cache.save(ref, cursor, [row, cursorRow]);
  vi.useFakeTimers();
  await vi.advanceTimersByTimeAsync(5_000);
  expect(storage.delete).toHaveBeenCalledOnce();
  expect(storage.put).not.toHaveBeenCalled();
  cache.stop();
});

it("does not hydrate a delayed cache load after the server says NoCache", async () => {
  const pending = Promise.withResolvers<unknown>();
  const storage = {
    get: vi.fn(() => pending.promise),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  const loading = cache.load(ref);
  await cache.setPolicy(ref, SubscriptionCachePolicy.noCache);
  pending.resolve({
    version: SUBSCRIPTION_CACHE_VERSION,
    contractHash: API_SCHEMA_HASH,
    subscription: subscriptionKey(ref),
    cursor,
    data: [cachedRow],
  });
  expect(await loading).toBeUndefined();
  expect(collection.all()).toEqual([]);
  cache.stop();
});

it("rejects rows newer than the record cursor", async () => {
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(ref),
      cursor,
      data: [{ ...cachedRow, seq: cursor.seq + 1 }],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  expect(await cache.load(ref)).toBeUndefined();
  expect(storage.delete).toHaveBeenCalledOnce();
  expect(collection.all()).toEqual([]);
  cache.stop();
});

it("stores current rows and omits deleted and evicted rows", async () => {
  vi.useFakeTimers();
  const selected = { kind: "state" } as const;
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
  await cache.setPolicy(selected, SubscriptionCachePolicy.cache);
  const entity = {
    collection: "agentSummary",
    id: "lead",
    value: {
      id: "lead",
      rootId: "lead",
      epoch: 1,
      name: "Lead",
      status: AgentStatus.idle,
    },
  } as const;
  cache.save(selected, cursor, [
    { id: "entity:agentSummary:lead", seq: 4, payload: "", entity },
    { id: "entity:agentSummary:deleted", seq: 3, payload: "", _deleted: true },
    {
      id: "entity:agentSummary:evicted",
      seq: 2,
      payload: "",
      _deleted: true,
      evicted: true,
    },
  ]);
  await vi.advanceTimersByTimeAsync(5_000);
  expect(storage.put.mock.calls[0]?.[1].data).toEqual([
    { kind: "entity", id: "entity:agentSummary:lead", seq: 4, entity },
  ]);
  cache.stop();
});

it("rejects a transcript record missing an ordered item", async () => {
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(ref),
      cursor,
      data: [
        {
          ...cachedRow,
          payload: JSON.stringify({
            format: TRANSCRIPT_PROJECTION_FORMAT,
            metadata: {},
            order: ["missing"],
          }),
        },
      ],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  expect(await cache.load(ref)).toBeUndefined();
  expect(storage.delete).toHaveBeenCalledOnce();
  expect(collection.all()).toEqual([]);
  cache.stop();
});

it("rejects malformed ordered transcript item JSON", async () => {
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(ref),
      cursor,
      data: [
        {
          ...cachedRow,
          payload: JSON.stringify({
            format: TRANSCRIPT_PROJECTION_FORMAT,
            metadata: {},
            order: ["one"],
          }),
        },
        {
          kind: "transcript",
          id: "transcript:item:agent:one",
          seq: 4,
          payload: "{",
        },
      ],
    }),
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
});

it("restores a complete ordered transcript and its cursor", async () => {
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(ref),
      cursor,
      data: [
        {
          ...cachedRow,
          payload: JSON.stringify({
            format: TRANSCRIPT_PROJECTION_FORMAT,
            metadata: {},
            order: ["one"],
          }),
        },
        {
          kind: "transcript",
          id: "transcript:item:agent:one",
          seq: 4,
          payload: JSON.stringify({ id: "one" }),
        },
      ],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace");
  expect(await cache.load(ref)).toEqual(cursor);
  expect(
    (await collection.findOne("transcript:item:agent:one").exec())?.payload,
  ).toBe('{"id":"one"}');
  cache.stop();
});

it.each([null, [], { id: "other" }])(
  "rejects corrupt ordered transcript items",
  async (item) => {
    const storage = {
      get: vi.fn().mockResolvedValue({
        version: SUBSCRIPTION_CACHE_VERSION,
        contractHash: API_SCHEMA_HASH,
        subscription: subscriptionKey(ref),
        cursor,
        data: [
          {
            ...cachedRow,
            payload: JSON.stringify({
              format: TRANSCRIPT_PROJECTION_FORMAT,
              metadata: {},
              order: ["one"],
            }),
          },
          {
            kind: "transcript",
            id: "transcript:item:agent:one",
            seq: 4,
            payload: JSON.stringify(item),
          },
        ],
      }),
      put: vi.fn(),
      delete: vi.fn(),
    };
    const collection = new LocalCollection("fixture");
    const cache = createSubscriptionCache(storage, collection, "workspace");
    expect(await cache.load(ref)).toBeUndefined();
    expect(storage.delete).toHaveBeenCalledOnce();
    expect(collection.all()).toEqual([]);
    cache.stop();
  },
);

it("rejects an entity with an invalid value", async () => {
  const selected = { kind: "agentDetail", agentId: "agent" } as const;
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: SUBSCRIPTION_CACHE_VERSION,
      contractHash: API_SCHEMA_HASH,
      subscription: subscriptionKey(selected),
      cursor,
      data: [
        {
          kind: "entity",
          id: "entity:agentDetail:agent",
          seq: 4,
          entity: {
            collection: "agentDetail",
            id: "agent",
            value: { id: "other" },
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
  expect(storage.delete).toHaveBeenCalledOnce();
  expect(collection.all()).toEqual([]);
  cache.stop();
});
