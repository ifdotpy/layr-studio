import type { CommandResponse } from "../commands";
import { DeliveryMomentInput } from "../generated/api";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => void values.delete(key),
    setItem: (key, value) => void values.set(key, String(value)),
  };
}
import { afterEach, expect, it, vi } from "vitest";

afterEach(() => {
  vi.resetModules();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// The answer to a send lists the stored stream row of the receipt before the receipt of the
// answer. The stream row keeps the receipt under `value`. The outbox must settle, not stay queued.
it("settles a sent message when the answer lists the stream receipt row first", async () => {
  vi.stubGlobal("navigator", { onLine: true });
  vi.stubGlobal("location", { origin: "http://studio.test" });
  vi.stubGlobal("window", {
    addEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  });
  vi.stubGlobal("document", {
    documentElement: { dataset: {} },
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  });
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
  const { createCommandClient } = await import("../commands");
  const { createOutbox } = await import("./send");
  const response: CommandResponse = {
    status: "applied",
    ref_kind: "message",
    ref_id: "stable-id",
    rev: 1,
    seq: 9,
    rows: [
      {
        collection: "messageReceipt",
        id: "stable-id",
        seq: 9,
        payload: {
          collection: "messageReceipt",
          id: "stable-id",
          value: { id: "stable-id", agent: "chat", status: "delivered" },
        },
      },
      {
        collection: "messageReceipt",
        id: "stable-id",
        seq: 0,
        payload: { id: "stable-id", status: "delivered" },
      },
    ],
  };
  const command = createCommandClient(memoryStorage(), async () => response);
  const buffer = {
    loadAll: async () => [],
    put: vi.fn(async () => undefined),
    putIfExists: vi.fn(async () => undefined),
    delete: vi.fn(async () => undefined),
  };
  const outbox = createOutbox({
    api: {
      command,
      getStreamSession: () => ({ workspaceId: "workspace-a" }) as never,
    },
    buffer,
    watchResourceConnection: vi.fn(() => () => undefined) as never,
  });
  const result = await outbox.durableSend({
    id: "stable-id",
    room: "chat",
    text: "hello",
    delivery: DeliveryMomentInput["queue"],
  });
  expect(result).toMatchObject({ kind: "server", status: "delivered" });
  expect(buffer.delete).toHaveBeenCalledWith(
    expect.stringContaining("stable-id"),
  );
});
