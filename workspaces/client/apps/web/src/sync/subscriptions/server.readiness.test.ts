import type { ConnectionHealth } from "../connectionState.ts";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ResourceVersion } from "../resourceEvents.ts";
import type { ResourceRef } from "./contracts.ts";

const mocks = vi.hoisted(() => ({
  changes: new Map<string, Set<(version?: ResourceVersion) => void>>(),
  connections: new Set<(status: ConnectionHealth, stream: "open") => void>(),
}));
vi.mock("../resourceEvents.ts", () => ({
  createResourceEvents: () => ({
    watchResourceChanges: (
      ref: ResourceRef,
      accept: (version?: ResourceVersion) => void,
    ) => {
      const key = JSON.stringify(ref);
      let listeners = mocks.changes.get(key);
      if (!listeners) mocks.changes.set(key, (listeners = new Set()));
      listeners.add(accept);
      return () => listeners.delete(accept);
    },
    watchResourceConnection: (
      accept: (status: ConnectionHealth, stream: "open") => void,
    ) => {
      mocks.connections.add(accept);
      accept("live", "open");
      return () => mocks.connections.delete(accept);
    },
    dispose: vi.fn(),
  }),
}));
vi.mock("../client.ts", () => ({ syncDatabase: vi.fn() }));
vi.mock("../resume.ts", () => ({ onResume: () => () => {} }));
import { createServerSubscriptions } from "./server.ts";

const disposals: (() => void)[] = [];
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  mocks.changes.clear();
  mocks.connections.clear();
  vi.useRealTimers();
});
function fixture() {
  const get = vi.fn();
  const subscriptions = createServerSubscriptions({
    owner: { id: "remote", origin: "https://server.example" },
    api: { get },
    tokenRates: {},
    sync: () => ({}),
  } as unknown as Parameters<typeof createServerSubscriptions>[0]);
  disposals.push(subscriptions.dispose);
  const deliver = (ref: ResourceRef, version: ResourceVersion) => {
    for (const accept of mocks.changes.get(JSON.stringify(ref)) ?? [])
      accept(version);
  };
  return { subscriptions, get, deliver };
}
const inlineViews = [
  {
    ref: { kind: "processes", agentId: "lead" },
    snapshot: {
      processes: { processes: [] as never[], activities: [] as never[] },
    },
    value: { processes: [], activities: [] },
  },
  {
    ref: { kind: "complaint", complaintId: "message" },
    snapshot: { complaint: { id: "message", text: "Reply" } },
    value: { id: "message", text: "Reply" },
  },
  {
    ref: { kind: "room", roomId: "team", legacy: true },
    snapshot: { legacyMessages: { messages: [] } },
    value: { messages: [] },
  },
] as const;

it.each(inlineViews)(
  "the actual server adapter confirms applied $ref.kind inline data without HTTP",
  async ({ ref, snapshot, value }) => {
    const f = fixture();
    f.subscriptions.subscribe(ref, () => {});
    f.deliver(ref, {
      epoch: "server",
      revision: 1,
      viewSnapshot: snapshot,
    } as ResourceVersion);
    await vi.advanceTimersByTimeAsync(0);
    expect(f.subscriptions.getSnapshot(ref)).toMatchObject({
      data: value,
      status: "live",
      error: null,
    });
    expect(f.get).not.toHaveBeenCalled();
  },
);

it("a view hint stays unconfirmed until a completed inline application", async () => {
  const f = fixture();
  const { ref, snapshot, value } = inlineViews[0];
  f.get.mockRejectedValue(new Error("view unavailable"));
  f.subscriptions.subscribe(ref, () => {});
  f.deliver(ref, { epoch: "server", revision: 1 });
  await vi.advanceTimersByTimeAsync(0);
  expect(f.subscriptions.getSnapshot(ref).status).not.toBe("live");
  f.deliver(ref, { epoch: "server", revision: 2, viewSnapshot: snapshot });
  await vi.advanceTimersByTimeAsync(0);
  expect(f.subscriptions.getSnapshot(ref)).toMatchObject({
    data: value,
    status: "live",
    error: null,
  });
  expect(f.get).not.toHaveBeenCalled();
});

it("disposal prevents a later inline delivery from publishing data or readiness", async () => {
  const f = fixture();
  const { ref, snapshot } = inlineViews[0];
  const stop = f.subscriptions.subscribe(ref, () => {}, { graceMs: 0 });
  stop();
  f.deliver(ref, { epoch: "server", revision: 1, viewSnapshot: snapshot });
  await vi.advanceTimersByTimeAsync(0);
  expect(f.subscriptions.getSnapshot(ref).data).toBeUndefined();
  expect(f.subscriptions.getSnapshot(ref).status).not.toBe("live");
  expect(f.get).not.toHaveBeenCalled();
});

it("an unchanged reconnect inline baseline re-establishes successful view readiness", async () => {
  const f = fixture();
  const { ref, snapshot } = inlineViews[0];
  f.subscriptions.subscribe(ref, () => {});
  f.deliver(ref, { epoch: "server", revision: 1, viewSnapshot: snapshot });
  await vi.advanceTimersByTimeAsync(0);
  expect(f.subscriptions.getSnapshot(ref).status).toBe("live");
  for (const accept of mocks.connections) accept("degraded", "open");
  for (const accept of mocks.connections) accept("live", "open");
  f.deliver(ref, { epoch: "server", revision: 1, viewSnapshot: snapshot });
  await vi.advanceTimersByTimeAsync(0);
  expect(f.subscriptions.getSnapshot(ref).status).toBe("live");
  expect(f.get).not.toHaveBeenCalled();
});

it("a revision-only hint retains the last view until its next completed payload", async () => {
  const f = fixture();
  const { ref, snapshot, value } = inlineViews[0];
  f.subscriptions.subscribe(ref, () => {});
  f.deliver(ref, { epoch: "server", revision: 1, viewSnapshot: snapshot });
  f.deliver(ref, { epoch: "server", revision: 2 });
  await vi.advanceTimersByTimeAsync(0);
  expect(f.get).not.toHaveBeenCalled();
  expect(f.subscriptions.getSnapshot(ref).data).toEqual(value);
  f.deliver(ref, { epoch: "server", revision: 3, viewSnapshot: snapshot });
  expect(f.subscriptions.getSnapshot(ref)).toMatchObject({
    data: value,
    status: "live",
  });
});

it.each([
  {
    ref: { kind: "room", roomId: "team" },
    snapshot: { room: { messages: [] } },
  },
  {
    ref: { kind: "terminal-output", terminalId: "terminal" },
    snapshot: {
      terminalOutput: {
        text: "uncommitted",
        offset: 11,
        status: "running",
        truncated: false,
      },
    },
  },
] as const)(
  "revoked durable $ref.kind view proof cannot be replaced by the retained payload",
  async ({ ref, snapshot }) => {
    const f = fixture();
    f.subscriptions.subscribe(ref, () => {});
    f.deliver(ref, {
      epoch: "server",
      revision: 1,
      viewSnapshot: snapshot,
    } as ResourceVersion);
    await vi.advanceTimersByTimeAsync(0);
    expect(f.subscriptions.getSnapshot(ref).status).not.toBe("live");
  },
);
