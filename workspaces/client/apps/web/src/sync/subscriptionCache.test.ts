import { afterEach, expect, it, vi } from "vitest";
import {
  SUBSCRIPTION_CACHE_INDEX_KEY,
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

function recordPuts(storage: { put: { mock: { calls: unknown[][] } } }) {
  return storage.put.mock.calls.filter(
    ([key]) => key !== SUBSCRIPTION_CACHE_INDEX_KEY,
  );
}

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
  expect(recordPuts(storage)).toHaveLength(1);
  cache.save(ref, cursor, [row]);
  events.dispatchEvent(new Event("visibilitychange"));
  await vi.advanceTimersByTimeAsync(0);
  expect(recordPuts(storage)).toHaveLength(2);
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

const PAYLOAD = 1_000;
const agentRef = (agentId: string) =>
  ({ kind: "transcript", agentId }) as const;
// Stored size of one transcript record with PAYLOAD bytes (7-character agent ids, the longest used here).
const ONE_RECORD_BYTES = new TextEncoder().encode(
  JSON.stringify({
    version: SUBSCRIPTION_CACHE_VERSION,
    contractHash: API_SCHEMA_HASH,
    subscription: subscriptionKey(agentRef("agent00")),
    cursor,
    data: [{ kind: "transcript", ...paddedTranscript("agent00", PAYLOAD) }],
  }),
).byteLength;
// Budget for the eviction tests: two records fit, three do not.
const BUDGET = 2 * ONE_RECORD_BYTES + 10;
const keyOf = (agentId: string) =>
  JSON.stringify(["workspace", subscriptionKey(agentRef(agentId))]);
const recordKeys = (values: Map<string, unknown>) =>
  [...values.keys()].filter((key) => key !== SUBSCRIPTION_CACHE_INDEX_KEY);

function memoryStorage() {
  const values = new Map<string, unknown>();
  return {
    values,
    get: vi.fn(async (key: string) => values.get(key)),
    put: vi.fn(async (key: string, value: unknown) => {
      values.set(key, structuredClone(value));
    }),
    delete: vi.fn(async (key: string) => {
      values.delete(key);
    }),
  };
}

function paddedTranscript(agentId: string, size: number) {
  return {
    id: `transcript:${agentId}`,
    seq: 4,
    payload: JSON.stringify({
      format: TRANSCRIPT_PROJECTION_FORMAT,
      metadata: { pad: "x".repeat(size) },
      order: [],
    }),
  };
}

async function setupBudgetedCache(agentIds: string[]) {
  vi.useFakeTimers();
  const storage = memoryStorage();
  const collection = new LocalCollection("fixture");
  const cache = createSubscriptionCache(storage, collection, "workspace", {
    maxBytes: BUDGET,
  });
  for (const agentId of agentIds)
    await cache.setPolicy(agentRef(agentId), SubscriptionCachePolicy.cache);
  const saveAndFlush = async (agentId: string, size = PAYLOAD) => {
    cache.save(agentRef(agentId), cursor, [paddedTranscript(agentId, size)]);
    await vi.advanceTimersByTimeAsync(5_000);
  };
  return { storage, collection, cache, saveAndFlush };
}

function recordBytes(values: Map<string, unknown>) {
  return recordKeys(values).reduce(
    (sum, key) =>
      sum +
      new TextEncoder().encode(JSON.stringify(values.get(key))).byteLength,
    0,
  );
}

it("evicts the least recently written record when a write exceeds the budget", async () => {
  const { storage, cache, saveAndFlush } = await setupBudgetedCache([
    "a",
    "b",
    "c",
  ]);
  await saveAndFlush("a");
  await saveAndFlush("b");
  expect(recordKeys(storage.values)).toEqual([keyOf("a"), keyOf("b")]);
  await saveAndFlush("c");
  expect(recordKeys(storage.values)).toEqual([keyOf("b"), keyOf("c")]);
  expect(storage.delete).toHaveBeenCalledWith(keyOf("a"));
  cache.stop();
});

it("a read makes a record the most recently used", async () => {
  const { storage, cache, saveAndFlush } = await setupBudgetedCache([
    "a",
    "b",
    "c",
  ]);
  await saveAndFlush("a");
  await saveAndFlush("b");
  expect(await cache.load(agentRef("a"))).toEqual(cursor);
  await vi.advanceTimersByTimeAsync(0);
  await saveAndFlush("c");
  expect(recordKeys(storage.values)).toEqual([keyOf("a"), keyOf("c")]);
  expect(storage.delete).toHaveBeenCalledWith(keyOf("b"));
  cache.stop();
});

it("does not keep a record bigger than the whole budget", async () => {
  const { storage, cache, saveAndFlush } = await setupBudgetedCache(["a"]);
  await saveAndFlush("a", 3 * PAYLOAD);
  expect(recordKeys(storage.values)).toEqual([]);
  expect(storage.put.mock.calls.map(([key]) => key)).not.toContain(keyOf("a"));
  await saveAndFlush("a");
  expect(recordKeys(storage.values)).toEqual([keyOf("a")]);
  await saveAndFlush("a", 3 * PAYLOAD);
  expect(recordKeys(storage.values)).toEqual([]);
  cache.stop();
});

it("keeps the stored records under the budget after many writes", async () => {
  const agents = Array.from({ length: 20 }, (_, index) => `agent${index}`);
  const { storage, cache, saveAndFlush } = await setupBudgetedCache(agents);
  for (const agentId of agents) {
    await saveAndFlush(agentId);
    expect(recordBytes(storage.values)).toBeLessThanOrEqual(BUDGET);
  }
  expect(recordKeys(storage.values)).toEqual([
    keyOf("agent18"),
    keyOf("agent19"),
  ]);
  cache.stop();
});
