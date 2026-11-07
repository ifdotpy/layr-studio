import type { DeepReadonly } from "../deepReadonly.ts";
import { autorun } from "mobx";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { components } from "../generated/api.ts";
import {
  SYNC_PROTOCOL_VERSION,
  SubscriptionReplyKind,
} from "../generated/api.ts";
import { SYNC_STREAM_EVENT_MAX_BYTES } from "../generated/sharedValues.ts";
import { createSyncClient } from "../sync/client.ts";
import { createEntitySequenceScope } from "../sync/entitySequence.ts";
import { createTranscriptCache } from "../sync/transcriptCache.ts";
import { createDataObserver } from "../sync/subscriptions/data.ts";
import { createSubscriptions } from "../sync/subscriptions/module.ts";
import { subscriptionKey } from "../sync/subscriptions/contracts.ts";
import type {
  ResourceData,
  ResourceRef,
  ResourceVersion,
} from "../sync/subscriptions/contracts.ts";
import type { SubscriptionTransport } from "../sync/subscriptions/store.ts";
import { createObservableResourceRuntime } from "./runtime.ts";

// Only isolate the default server singletons. The clients below use the real
// projection, durable cursor, data adapter, subscription owner and slot runtime.
vi.mock("../api.ts", () => ({
  syncGet: vi.fn(),
  syncPost: vi.fn(),
  saved: (_key: string, fallback: unknown) => fallback,
  save: vi.fn(),
  setWorkspace: vi.fn(),
  isApiSchemaMismatch: () => false,
}));
vi.mock("../sync/subscriptions/events.ts", () => ({
  watchResourceChanges: () => () => {},
  watchResourceConnection: () => () => {},
}));

const cleanup: (() => void)[] = [];
beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("document", {
    hidden: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  });
  vi.stubGlobal("window", {
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    matchMedia: () => ({ matches: false }),
  });
  vi.stubGlobal("navigator", { onLine: true });
});
afterEach(() => {
  for (const stop of cleanup.splice(0).reverse()) stop();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function transportFixture() {
  const callbacks = new Map<string, Set<(version?: ResourceVersion) => void>>();
  const events: SubscriptionTransport = {
    watchResourceChanges(ref, accept) {
      const key = subscriptionKey(ref);
      let listeners = callbacks.get(key);
      if (!listeners) callbacks.set(key, (listeners = new Set()));
      listeners.add(accept);
      return () => {
        listeners.delete(accept);
      };
    },
    watchResourceConnection(accept) {
      accept("live", "open", {});
      return () => {};
    },
  };
  return {
    events,
    notify(ref: ResourceRef, version: ResourceVersion) {
      for (const accept of callbacks.get(subscriptionKey(ref)) ?? [])
        accept(version);
    },
  };
}

async function transcriptFixture(tail = 40) {
  const transport = transportFixture();
  const workspaceId = "a".repeat(32);
  const ref = { kind: "transcript", agentId: "lead", tail } as const;
  const read = vi.fn();
  read.mockResolvedValue({ workspaceId });
  const client = createSyncClient({
    api: {
      syncGet: read,
      syncPost: vi.fn(),
      saved: (_key, fallback) => fallback,
      save: vi.fn(),
      setWorkspace: vi.fn(),
      isApiSchemaMismatch: () => false,
    },
    resources: transport.events,
    sequences: createEntitySequenceScope(),
    transcripts: createTranscriptCache(),
    databaseSuffix: crypto.randomUUID(),
  });
  const subscriptions = createSubscriptions(
    transport.events,
    createDataObserver(() => client, transport.events),
  );
  const runtime = createObservableResourceRuntime(subscriptions);
  // Model projection is the generator's job. This adapter exposes exactly what
  // the handwritten transcript path publishes, including omitted local metadata.
  const get = runtime.keyed<typeof ref, DeepReadonly<ResourceData<typeof ref>>>(
    {
      create: (dto) => dto,
      apply: (_data, dto) => dto,
    },
  );
  const slot = get(ref);
  cleanup.push(() => {
    runtime.dispose();
    subscriptions.dispose();
    client.disposeCache();
  });
  cleanup.push(autorun(() => slot.current));
  await vi.advanceTimersByTimeAsync(100);
  expect(read).toHaveBeenCalledTimes(1); // Workspace identity only.
  read.mockClear();
  const deliver = async (
    entry: components["schemas"]["ResourceRevisionEntry"],
    epoch = "first",
  ) => {
    const event = {
      protocol: SYNC_PROTOCOL_VERSION,
      workspaceId,
      epoch,
      revision: entry.revision,
      reason: "change",
      resources: [ref],
      resourceVersions: [{ ...entry, resource: ref }],
    } satisfies components["schemas"]["ResourceChangeEvent"];
    await client.persistResourceEntityChanges(event, () => true);
    transport.notify(ref, {
      epoch,
      revision: entry.revision,
      cursor: entry.cursor ?? undefined,
      replyKind: entry.replyKind ?? undefined,
      transcriptSequence: entry.revision,
    });
    await vi.advanceTimersByTimeAsync(50);
  };
  const snapshot = (
    seq: number,
    nextCursor: string | null = "older",
  ): components["schemas"]["ResourceRevisionEntry"] => ({
    revision: seq,
    cursor: { workspaceId, seq },
    replyKind: SubscriptionReplyKind.snapshot,
    transcriptSnapshot: {
      items: [{ id: "answer", role: "assistant", text: "First" }],
      truncated: nextCursor !== null,
      nextCursor,
      nextAfterCursor: null,
      historyVersion: "history",
    },
  });
  return {
    ...transport,
    client,
    subscriptions,
    slot,
    ref,
    read,
    workspaceId,
    deliver,
    snapshot,
  };
}

describe("keyed windows reuse handwritten recovery", () => {
  it("retains transcript window metadata and cursor through normal SSE and equal interval replay, with zero reads", async () => {
    const f = await transcriptFixture();
    await f.deliver(f.snapshot(1));
    expect(f.slot.current).toMatchObject({
      status: "live",
      data: {
        items: [{ id: "answer", text: "First" }],
        truncated: true,
        nextCursor: "older",
        historyVersion: "history",
      },
    });
    const delta = {
      revision: 2,
      cursor: { workspaceId: f.workspaceId, seq: 2 },
      replyKind: SubscriptionReplyKind.delta,
      transcriptChanges: {
        after: 1,
        through: 2,
        delta: {
          delta: true as const,
          items: [{ id: "answer", role: "assistant", text: "Next" }],
          removed: [],
          truncated: true,
          nextCursor: "older",
        },
      },
    };
    await f.deliver(delta);
    const current = f.slot.current;
    await f.deliver(delta);
    expect(f.slot.current).toBe(current);
    expect(f.slot.current).toMatchObject({
      data: { items: [{ text: "Next" }], nextCursor: "older" },
    });
    expect(await f.client.readSubscriptionCursor(f.ref)).toEqual({
      workspaceId: f.workspaceId,
      seq: 2,
    });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(f.read).not.toHaveBeenCalled();
  });

  it("drops an old epoch's cursor before recovery and accepts partial local metadata", async () => {
    const f = await transcriptFixture();
    await f.deliver(f.snapshot(5));
    f.read.mockResolvedValue({
      workspaceId: f.workspaceId,
      documents: [
        {
          id: "transcript:lead",
          payload: JSON.stringify({ items: [] }),
          seq: 1,
          _deleted: false,
        },
      ],
      checkpoint: { seq: 1 },
      maxSeq: 1,
    });
    f.notify(f.ref, {
      epoch: "restarted",
      revision: 1,
      transcriptReadRequired: true,
    });
    await vi.advanceTimersByTimeAsync(50);
    expect(f.read).toHaveBeenCalledTimes(1);
    expect(f.read.mock.calls[0]?.[1]?.query).toMatchObject({
      scope: "transcript:lead",
      after: 0,
    });
    const current = f.slot.current;
    expect(current).toMatchObject({ data: { items: [] } });
    if (current.status === "live" || current.status === "stale") {
      expect(current.data.truncated).toBeUndefined();
      expect(current.data.nextCursor).toBeUndefined();
    }
  });

  it("recovers one oversized item from the saved cursor and resumes stream delivery without polling", async () => {
    const f = await transcriptFixture();
    await f.deliver(f.snapshot(1));
    const text = "x".repeat(SYNC_STREAM_EVENT_MAX_BYTES + 1);
    f.read.mockResolvedValue({
      workspaceId: f.workspaceId,
      documents: [
        {
          id: "transcript:lead",
          payload: JSON.stringify({
            items: [{ id: "answer", role: "assistant", text }],
            truncated: false,
            nextCursor: null,
          }),
          seq: 2,
          _deleted: false,
        },
      ],
      checkpoint: { seq: 2 },
      maxSeq: 2,
    });
    f.notify(f.ref, {
      epoch: "first",
      revision: 2,
      transcriptReadRequired: true,
    });
    await vi.advanceTimersByTimeAsync(50);
    expect(f.read).toHaveBeenCalledTimes(1);
    expect(f.read.mock.calls[0]?.[1]?.query).toMatchObject({
      scope: "transcript:lead",
      after: 1,
    });
    expect(f.slot.current).toMatchObject({
      data: { items: [{ text }], truncated: false, nextCursor: null },
    });
    await f.deliver({
      revision: 3,
      cursor: { workspaceId: f.workspaceId, seq: 3 },
      replyKind: SubscriptionReplyKind.delta,
      transcriptChanges: {
        after: 2,
        through: 3,
        delta: {
          delta: true,
          items: [{ id: "answer", role: "assistant", text: "Streaming again" }],
          removed: [],
          truncated: false,
        },
      },
    });
    expect(f.slot.current).toMatchObject({
      data: { items: [{ text: "Streaming again" }] },
    });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(f.read).toHaveBeenCalledTimes(1);
  });

  it("keeps room cursor and window data on inline updates and recovers only a missing view", async () => {
    const f = transportFixture();
    const ref = { kind: "room", roomId: "team" } as const;
    const first = {
      messages: [],
      room: { id: "team" },
      nextBefore: 20,
      nextAfter: null,
    } satisfies ResourceData<typeof ref>;
    const recovered = {
      messages: [],
      room: { id: "team" },
      nextBefore: null,
      nextAfter: null,
    } satisfies ResourceData<typeof ref>;
    const read = vi.fn(async () => recovered);
    const subscriptions = createSubscriptions(
      f.events,
      createDataObserver(
        () => ({
          subscribeTranscriptProjection: () => () => {},
        }),
        f.events,
        undefined,
        { room: read },
      ),
    );
    const runtime = createObservableResourceRuntime(subscriptions);
    const get = runtime.keyed<
      typeof ref,
      DeepReadonly<ResourceData<typeof ref>>
    >({
      create: (dto) => dto,
      apply: (_data, dto) => dto,
    });
    const slot = get(ref);
    cleanup.push(() => {
      runtime.dispose();
      subscriptions.dispose();
    });
    cleanup.push(autorun(() => slot.current));
    await vi.advanceTimersByTimeAsync(100);
    f.notify(ref, {
      epoch: "room",
      revision: 1,
      cursor: { workspaceId: "room-workspace", seq: 1 },
      replyKind: SubscriptionReplyKind.snapshot,
      viewSnapshot: { room: first },
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(slot.current).toMatchObject({ status: "live", data: first });
    expect(read).not.toHaveBeenCalled();
    f.notify(ref, { epoch: "room", revision: 2 });
    await vi.advanceTimersByTimeAsync(0);
    expect(slot.current).toMatchObject({ data: recovered });
    f.notify(ref, {
      epoch: "room",
      revision: 3,
      cursor: { workspaceId: "room-workspace", seq: 1 },
      replyKind: SubscriptionReplyKind.snapshot,
      viewSnapshot: { room: first },
    });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(slot.current).toMatchObject({ data: first });
    expect(read).toHaveBeenCalledTimes(1);
  });
});
