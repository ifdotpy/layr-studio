import { CacheScopeOwner } from "./cacheScope.ts";
import { afterEach, expect, it, vi } from "vitest";
const stores = vi.hoisted(() => new Map<string, Map<string, unknown>>());
vi.mock("../indexedDb.ts", () => ({
  indexedDb: (name: string) => {
    let values = stores.get(name);
    if (!values) stores.set(name, (values = new Map()));
    const rows = values;
    return {
      get: async (key: string) => rows.get(key),
      put: async (key: string, value: unknown) => {
        rows.set(key, value);
      },
      scan: async <T>(
        inspect: (values: [string, unknown][]) => {
          value: T;
          remove: string[];
        },
      ) => {
        const result = inspect([...rows]);
        result.remove.forEach((key) => rows.delete(key));
        return result.value;
      },
      delete: async (key: string) => {
        rows.delete(key);
      },
      destroy: async () => {
        rows.clear();
      },
    };
  },
}));
import {
  subscriptionCacheStorage,
  invalidateSubscriptionCache,
} from "./subscriptionCacheStorage.ts";
afterEach(() => stores.clear());
it("clears snapshots and notifies hydrated owners when a credential is replaced", async () => {
  const first = subscriptionCacheStorage("server", "credential-A");
  const owner = new CacheScopeOwner(
    {
      serverId: "server",
      credentialId: "credential-A",
      tokenFingerprint: "hash",
      workspaceId: "workspace",
    },
    first,
    () => {},
  );
  await first.get("snapshot");
  await first.put("snapshot", "private conversation");
  const invalidated = vi.fn();
  const stop = first.onInvalidation(() => {
    invalidated();
    void owner.replace(null);
  });
  const second = subscriptionCacheStorage("server", "credential-B");
  await owner.replace(null);
  expect(await second.get("snapshot")).toBeUndefined();
  expect(invalidated).toHaveBeenCalledOnce();
  stop();
});
it("isolates scope snapshots and refuses an old epoch's late writer", async () => {
  const storage = subscriptionCacheStorage("server", "credential");
  const initial = {
    serverId: "server",
    credentialId: "credential",
    tokenFingerprint: "hash-A",
    workspaceId: "workspace",
  };
  const owner = new CacheScopeOwner(initial, storage, () => {});
  const previous = owner.scopedStorage();
  if (!previous) throw new Error("Missing scoped storage");
  await previous.put("snapshot", "private");
  await owner.replace({ ...initial, tokenFingerprint: "hash-B" });
  await previous.put("snapshot", "late private write");
  expect(await previous.get("snapshot")).toBeUndefined();
  expect(await owner.scopedStorage()?.get("snapshot")).toBeUndefined();
});
it("forgets the persisted snapshots and notifies open views on credential forgetting", async () => {
  const storage = subscriptionCacheStorage("server", "credential");
  await storage.get("snapshot");
  await storage.put("snapshot", "conversation");
  const invalidated = vi.fn();
  const stop = storage.onInvalidation(invalidated);
  await invalidateSubscriptionCache("server");
  expect(await storage.get("snapshot")).toBeUndefined();
  expect(invalidated).toHaveBeenCalledOnce();
  stop();
});
