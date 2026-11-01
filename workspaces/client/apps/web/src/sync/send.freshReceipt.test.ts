import { expect, it, vi } from "vitest";
import type { StoredSend } from "../unsent/store";
import { MessageSendStatus } from "../generated/api";
import { ApiError } from "../api";

const observed = vi.hoisted(() => ({ entries: [] as unknown[] }));

vi.mock("react", () => ({
  useEffect: (effect: () => void) => effect(),
  useState: (initial: unknown) => [
    initial,
    (value: unknown) => {
      if (Array.isArray(value)) observed.entries = value;
    },
  ],
}));

import { createOutbox } from "./send";
import { createCommandClient } from "../commands";

it("fresh uncertain receipt blocks reconnect retry until explicit retry", async () => {
  const values = new Map<string, StoredSend>();
  const buffer = {
    put: async (key: string, value: StoredSend) => {
      values.set(key, value);
    },
    putIfExists: async (key: string, value: StoredSend) => {
      if (values.has(key)) values.set(key, value);
    },
    deleteIfEqual: async () => {},
    delete: async (key: string) => {
      values.delete(key);
    },
    loadAll: async (): Promise<[string, StoredSend][]> => [...values],
  };
  const storage: Storage = {
    length: 0,
    clear() {},
    getItem: () => null,
    key: () => null,
    removeItem() {},
    setItem() {},
  };
  const post = vi
    .fn()
    .mockRejectedValueOnce(new TypeError("offline"))
    .mockRejectedValueOnce(
      new ApiError("expired", 410, { code: "receipt_expired" }),
    );
  let onConnection: ((status: "live") => void) | undefined;
  const outbox = createOutbox({
    api: {
      command: createCommandClient(storage, post),
      getStreamSession: () => ({ workspaceId: "workspace" }),
    } as never,
    buffer,
    watchResourceConnection: (callback) => {
      onConnection = callback as typeof onConnection;
      return () => {};
    },
  });
  await outbox.durableSend({ id: "one", room: "chat", text: "hello" });
  await outbox.reconcileOutboxReceipts(
    "chat",
    [{ id: "one", status: MessageSendStatus.uncertain }],
    "workspace",
  );
  expect([...values.values()][0]?.localState.kind).toBe("transport_failed");
  expect([...values.values()][0]).not.toHaveProperty("receipt");
  outbox.useOutbox();
  onConnection?.("live");
  await new Promise((resolve) => setTimeout(resolve, 10));
  expect(post).toHaveBeenCalledTimes(1);
  await outbox.changeOutbox("one", "resume");
  expect(post).toHaveBeenCalledTimes(2);
  expect(post.mock.calls[1]?.[0]).toEqual(post.mock.calls[0]?.[0]);
  expect(observed.entries).toMatchObject([
    {
      id: "one",
      status: { source: "device", state: { kind: "receipt_expired" } },
    },
  ]);
  expect(observed.entries[0]).not.toHaveProperty("receipt");
  await outbox.changeOutbox("one", "resume");
  expect(post).toHaveBeenCalledTimes(2);
});
