import { afterEach, expect, it, vi } from "vitest";
import { createSubscriptionCache } from "./subscriptionCache";
import { LocalCollection } from "./localCollection";

const ref = { kind: "transcript", agentId: "agent" } as const;
const cursor = { workspaceId: "workspace", seq: 4 };
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

it.each([
  { version: 2, kind: ref.kind, cursor, data: [row, cursorRow] },
  { version: 1, kind: "state", cursor, data: [row, cursorRow] },
  { version: 1, kind: ref.kind, cursor, data: [{ id: "bad" }] },
  { version: 1, kind: ref.kind, cursor, data: [row] },
])(
  "drops corrupt or incompatible records without returning a cursor",
  async (record) => {
    const storage = {
      get: vi.fn().mockResolvedValue(record),
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
