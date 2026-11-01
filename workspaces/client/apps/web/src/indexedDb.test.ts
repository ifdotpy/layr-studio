import { expect, it, vi } from "vitest";
import { indexedDb } from "./indexedDb";

it("closes the connection before deleting its database", async () => {
  const close = vi.fn();
  const deleteDatabase = vi.fn(() => {
    expect(close).toHaveBeenCalledOnce();
    const request = {} as IDBOpenDBRequest;
    queueMicrotask(() => request.onsuccess?.(new Event("success")));
    return request;
  });
  vi.stubGlobal("indexedDB", {
    open: () => {
      const request = Object.assign({} as IDBOpenDBRequest, {
        result: {
          close,
          transaction: () => {
            const tx = Object.assign({} as IDBTransaction, {
              objectStore: () => ({ get: () => ({ result: undefined }) }),
            });
            queueMicrotask(() => tx.oncomplete?.(new Event("complete")));
            return tx;
          },
        },
      });
      queueMicrotask(() => request.onsuccess?.(new Event("success")));
      return request;
    },
    deleteDatabase,
  });
  const db = indexedDb("studio-subscription-cache:server", "subscriptions", 1);
  await db.get("key");
  await db.destroy();
  expect(deleteDatabase).toHaveBeenCalledWith(
    "studio-subscription-cache:server",
  );
  vi.unstubAllGlobals();
});
