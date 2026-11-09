import { SYNC_STREAM_EVENT_MAX_BYTES } from "../generated/sharedValues.ts";
import type { StateCollections } from "../generated/observableResources.ts";
import { observeGeneratedState } from "./generatedState.testHelper.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import type {
  ConnectionActivity,
  ConnectionHealth,
} from "./connectionState.ts";
import { backgroundDisplay } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import {
  MessageSendStatus,
  SubscriptionCachePolicy,
  SubscriptionReplyKind,
  AgentStatus,
  TaskKind,
  TaskStatus,
  WorkStatus,
} from "../generated/api.ts";
import assert from "node:assert/strict";
import { afterEach, expect, it, vi } from "vitest";

import { SYNC_PROTOCOL_VERSION, type components } from "../generated/api.ts";
import type {
  ResourceChangeEvent,
  ResourceVersion,
} from "./subscriptions/events.ts";
import { ResourceEntityDelivery } from "./resourceEntityDelivery.ts";
import { createSubscriptionStore } from "./subscriptions/store.ts";
import { API_SCHEMA_HASH } from "../generated/apiSchema.ts";

const mocks = vi.hoisted(() => ({
  syncGet: vi.fn(),
  changes: new Set<(version?: ResourceVersion) => void>(),
  connections: new Set<
    (state: ConnectionHealth, stream: ConnectionActivity) => void
  >(),
  watch: vi.fn(),
  watchConnection: vi.fn(),
  stagingWrite: vi.fn(),
  sources: [] as {
    url: string;
    closed: boolean;
    emit(name: string, value: unknown): void;
  }[],
}));
vi.mock("../servers/eventSource.ts", () => ({
  openServerEvents: vi.fn(),
  SignedEventSource: class {
    listeners = new Map<string, (event: Event) => void>();
    onopen = null;
    onerror = null;
    closed = false;
    declare readonly url: string;
    constructor(url: string) {
      this.url = url;
      mocks.sources.push(this);
    }
    addEventListener(name: string, accept: (event: Event) => void) {
      this.listeners.set(name, accept);
    }
    close() {
      this.closed = true;
    }
    emit(name: string, value: unknown) {
      if (!this.closed)
        this.listeners.get(name)?.({
          data: JSON.stringify(value),
        } as MessageEvent);
    }
  },
}));
vi.mock("../api.ts", () => ({
  ApiError: class extends Error {},
  syncGet: mocks.syncGet,
  get: mocks.syncGet,
  post: vi.fn(),
  receiveStreamSession: vi.fn(),
  clearStreamSession: vi.fn(),
  clearSchemaUpdateAttemptAfterMatch: vi.fn(),
  matchingApiSchemaResponseGeneration: () => 0,
  markApiSchemaMismatch: vi.fn(),
  onApiSchemaMismatch: () => () => {},
  syncPost: vi.fn(),
  saved: (_key: string, fallback: unknown) => fallback,
  save: vi.fn(),
  setWorkspace: vi.fn(),
  isApiSchemaMismatch: () => false,
  isApiSchemaMismatchHandshake: () => false,
  onMatchingApiSchemaResponse: () => () => {},
}));
vi.mock("./subscriptions/events.ts", () => ({
  replaceStream: vi.fn(),
  watchResourceChanges: mocks.watch,
  watchResourceConnection: mocks.watchConnection,
}));
vi.mock("../usage/tokenRate.ts", () => ({
  configureTokenRateStream: vi.fn(),
  receiveResourceTokenRates: vi.fn(),
  getWorkspaceTokenRates: vi.fn(() => ({ records: [] })),
}));
vi.mock("./resume.ts", () => ({ onResume: () => () => {} }));
vi.mock("./entityTransfer.ts", async (original) => {
  const actual = await original<typeof import("./entityTransfer.ts")>();
  return {
    ...actual,
    EntityTransferAssembler: class extends actual.EntityTransferAssembler {
      constructor() {
        const rows = new Map<string, string>();
        super({
          async put(key, value) {
            rows.set(key, value);
            mocks.stagingWrite();
          },
          async putMany(values) {
            for (const [key, value] of values) rows.set(key, value);
          },
          async get(key) {
            return rows.get(key);
          },
          async remove(prefix) {
            for (const key of rows.keys())
              if (key.startsWith(prefix)) rows.delete(key);
          },
          async close() {
            rows.clear();
          },
        });
      }
    },
  };
});
vi.mock("./entityCacheStorage.ts", async (original) => ({
  ...(await original<typeof import("./entityCacheStorage.ts")>()),
  holdEntityProjectionDatabaseLock: async () => true,
  deleteOtherEntityProjectionDatabases: async () => {},
}));

const stops: (() => void)[] = [];
afterEach(async () => {
  for (const stop of stops.splice(0)) stop();
  mocks.syncGet.mockReset();
  mocks.watch.mockReset();
  mocks.watchConnection.mockReset();
  mocks.stagingWrite.mockReset();
  mocks.sources = [];
  mocks.changes.clear();
  mocks.connections.clear();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.resetModules();
});

async function setup() {
  const workspaceId = crypto.randomUUID().replaceAll("-", "");
  vi.stubGlobal("window", {
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    matchMedia: () => ({ matches: false }),
  });
  vi.stubGlobal("document", {
    hidden: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  });
  vi.stubGlobal("navigator", {
    onLine: true,
    locks: {
      request: (_key: string, write: () => Promise<unknown>) => write(),
    },
  });
  mocks.syncGet.mockImplementation(async (path: string) =>
    path === "/api/sync/identity"
      ? { workspaceId }
      : {
          workspaceId,
          documents: [],
          checkpoint: { seq: 0 },
          initialHigh: 0,
          maxSeq: 0,
          complete: true,
        },
  );
  mocks.watch.mockImplementation((_ref, callback) => {
    mocks.changes.add(callback);
    return () => mocks.changes.delete(callback);
  });
  mocks.watchConnection.mockImplementation((listener) => {
    mocks.connections.add(listener);
    return () => mocks.connections.delete(listener);
  });
  const client = await import("./client.ts");
  const { db } = await client.syncDatabase();
  const snapshots: StateCollections[] = [];
  const failed = vi.fn();
  const subscribe = () => {
    const { stop } = observeGeneratedState(
      client,
      (value) => snapshots.push(value),
      failed,
    );
    stops.push(stop);
    return stop;
  };
  const pulls = () =>
    mocks.syncGet.mock.calls.filter(([path]) => path === "/api/sync/pull");
  const notify = (version?: ResourceVersion) => {
    for (const callback of mocks.changes) callback(version);
  };
  const dispatch = vi.fn((event: ResourceChangeEvent) => {
    const entry = event.resourceVersions[0];
    assert.ok(entry != null);
    notify({
      epoch: event.epoch,
      revision: entry.revision,
      reason: event.reason,
      cursor: entry.cursor ?? undefined,
      replyKind: entry.replyKind ?? undefined,
      entitySequence: entry.revision,
    });
  });
  const delivery = new ResourceEntityDelivery(
    client.persistResourceEntityChanges,
    dispatch,
    () => {
      connection("degraded", "connecting");
      return true;
    },
  );
  const frame = (
    seq: number,
    epoch = "first",
    name = "SSE baseline",
  ): ResourceChangeEvent => ({
    protocol: SYNC_PROTOCOL_VERSION,
    workspaceId,
    epoch,
    revision: seq,
    reason: "initial",
    resources: [{ kind: "state" }],
    resourceVersions: [
      {
        revision: seq,
        resource: { kind: "state" },
        cursor: { workspaceId, seq },
        replyKind: SubscriptionReplyKind.snapshot,
        entityChanges: {
          after: 0,
          through: seq,
          documents: [
            {
              op: "upsert",
              collection: "agentSummary",
              id: "lead",
              seq,
              value: agentSummaryFixture("lead", {
                id: "lead",
                rootId: "lead",
                epoch: 1,
                name,
                status: AgentStatus.idle,
              }),
            },
          ],
        },
      },
    ],
  });
  const connection = (
    state: ConnectionHealth,
    stream: ConnectionActivity = "open",
  ) => {
    for (const callback of mocks.connections) callback(state, stream);
  };
  return {
    client,
    db,
    workspaceId,
    snapshots,
    failed,
    subscribe,
    pulls,
    notify,
    dispatch,
    delivery,
    frame,
    connection,
  };
}

it("opens workspace and writes projections without IndexedDB", async () => {
  const f = await setup();
  const { createEntitySequenceScope } = await import("./entitySequence.ts");
  const { createTranscriptCache } = await import("./transcriptCache.ts");
  const open = vi.fn();
  vi.stubGlobal("indexedDB", { open });
  const client = f.client.createSyncClient({
    api: {
      syncGet: mocks.syncGet,
      syncPost: vi.fn(),
      saved: (_key, fallback) => fallback,
      save: vi.fn(),
      setWorkspace: vi.fn(),
      isApiSchemaMismatch: () => false,
    },
    resources: {
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    },
    sequences: createEntitySequenceScope(),
    transcripts: createTranscriptCache(),
    databaseSuffix: "memory-test",
  });
  const { db } = await client.syncDatabase();
  await client.persistProjection(db.projections, {
    id: "entity:agentSummary:one",
    payload: "",
    seq: 1,
    _deleted: false,
  });
  expect(open).not.toHaveBeenCalled();
});

it("rejects cursorless entity delivery without changing applied data", async () => {
  const f = await setup();
  await f.client.persistResourceEntityChanges(f.frame(4), () => true);
  const frame = f.frame(5, "first", "Cursorless update");
  delete frame.resourceVersions[0]!.cursor;
  await expect(
    f.client.persistResourceEntityChanges(frame, () => true),
  ).rejects.toThrow("Entity subscription has no cursor");
  expect(await f.client.readSubscriptionCursor({ kind: "state" })).toEqual({
    workspaceId: f.workspaceId,
    seq: 4,
  });
  expect(
    f.db.projections.all().find((row) => row.id === "entity:agentSummary:lead")
      ?.seq,
  ).toBe(4);
});

it("a cold transcript waits for SSE through disconnect without HTTP reads", async () => {
  const f = await setup();
  const stop = f.client.subscribeTranscriptProjection(
    "transcript:lead",
    vi.fn(),
    vi.fn(),
  );
  stops.push(stop);
  await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
  f.connection("degraded", "connecting");
  await new Promise<void>((resolve) => setTimeout(resolve, 20));
  expect(f.pulls()).toEqual([]);
});

function transcriptFrame(
  workspaceId: string,
  seq: number,
  ids: string[],
  historyVersion = "same",
  truncated = false,
): ResourceChangeEvent {
  const resource = { kind: "transcript", agentId: "lead" } as const;
  return {
    protocol: SYNC_PROTOCOL_VERSION,
    workspaceId,
    epoch: "first",
    revision: seq,
    reason: "reconnect",
    resources: [resource],
    resourceVersions: [
      {
        resource,
        revision: seq,
        cursor: { workspaceId, seq },
        replyKind: SubscriptionReplyKind.snapshot,
        transcriptSnapshot: {
          items: ids.map((id) => ({ id, role: "assistant", text: id })),
          historyVersion,
          truncated,
          nextCursor: truncated ? ids[0] : null,
        },
      },
    ],
  };
}

it.each([
  {
    historyVersion: "same",
    tail: ["tail", "newest"],
    expected: ["tail", "newest"],
  },
  {
    historyVersion: "replacement",
    tail: ["tail", "newest"],
    expected: ["tail", "newest"],
  },
  {
    historyVersion: "same",
    tail: ["disjoint", "newest"],
    expected: ["disjoint", "newest"],
  },
])(
  "reconnect snapshot replaces the window $historyVersion $tail",
  async ({ historyVersion, tail, expected }) => {
    const f = await setup();
    await f.client.persistResourceEntityChanges(
      transcriptFrame(f.workspaceId, 4, ["older", "tail"]),
      () => true,
    );
    await f.client.persistResourceEntityChanges(
      transcriptFrame(f.workspaceId, 5, tail, historyVersion, true),
      () => true,
    );
    const [row] = await f.db.projections.getRows(["transcript:lead"], false);
    expect(JSON.parse(row!.payload).order).toEqual(expected);
  },
);

it.each([4, 5])(
  "a reset snapshot at seq %s drops an omitted older running tool even with the same history version",
  async (seq) => {
    const f = await setup();
    const { peekTranscript } = await import("./transcriptCache.ts");
    const before = transcriptFrame(f.workspaceId, 4, ["older-tool", "tail"]);
    before.resourceVersions[0]!.transcriptSnapshot!.items[0]!.toolStatus =
      "running";
    await f.client.persistResourceEntityChanges(before, () => true);
    // The older tool settles to lost while offline; the reset includes only its newer tail.
    await f.client.persistResourceEntityChanges(
      transcriptFrame(f.workspaceId, seq, ["tail", "newest"], "same", true),
      () => true,
    );
    expect(
      peekTranscript(f.workspaceId, "lead")?.payload?.items.map(
        (item) => item.id,
      ),
    ).toEqual(["tail", "newest"]);
    const [oldTool] = await f.db.projections.getRows(
      ["transcript:item:lead:older-tool"],
      true,
    );
    expect(oldTool?._deleted).toBe(true);
  },
);

it("an oversized unsubscribed recent transcript reaches the cache like its inline prefetch", async () => {
  const f = await setup();
  vi.stubGlobal("navigator", { onLine: true });
  const { peekTranscript } = await import("./transcriptCache.ts");
  const { createResourceEvents } = await import("./resourceEvents.ts");
  class Source {
    listeners = new Map<string, (event: Event) => void>();
    onerror = null;
    onopen = null;
    addEventListener(name: string, accept: (event: Event) => void) {
      this.listeners.set(name, accept);
    }
    close() {}
    emit(name: string, value: unknown) {
      this.listeners.get(name)?.({
        data: JSON.stringify(value),
      } as MessageEvent);
    }
  }
  const source = new Source();
  const transport = createResourceEvents({
    api: {
      get: mocks.syncGet,
      post: vi.fn().mockResolvedValue({ gen: 1 }),
      clearSchemaUpdateAttemptAfterMatch: vi.fn(),
      receiveStreamSession: vi.fn(),
      clearStreamSession: vi.fn(),
      isApiSchemaMismatch: () => false,
      matchingApiSchemaResponseGeneration: () => 0,
      markApiSchemaMismatch: vi.fn(),
      onApiSchemaMismatch: () => () => {},
    },
    sync: {
      syncDatabase: f.client.syncDatabase,
      persistResourceEntityChanges: f.client.persistResourceEntityChanges,
      persistStagedEntityTransfer: f.client.persistStagedEntityTransfer,
      readSubscriptionCursor: f.client.readSubscriptionCursor,
    },
    tokenRates: {
      configureTokenRateStream: vi.fn(),
      receiveResourceTokenRates: vi.fn(() => true),
      getWorkspaceTokenRates: vi.fn(() => ({ records: [] })),
    },
    openEvents: () => source,
  });
  stops.push(
    transport.dispose,
    transport.watchResourceChanges({ kind: "state" }, () => {}),
  );
  await vi.waitFor(() => expect(source.listeners.has("resources")).toBe(true));
  source.emit("api-schema", { hash: API_SCHEMA_HASH });
  const make = (agentId: string, seq: number) => {
    const event = transcriptFrame(f.workspaceId, seq, [agentId]);
    const resource = { kind: "transcript", agentId } as const;
    event.resources = [resource];
    event.resourceVersions[0]!.resource = resource;
    return event;
  };
  const small = make("recent-small", 4);
  source.emit("resources", small);
  await vi.waitFor(() =>
    expect(peekTranscript(f.workspaceId, "recent-small")?.seq).toBe(4),
  );
  const large = make("recent-large", 5);
  const entry = large.resourceVersions[0]!;
  entry.transcriptSnapshot!.items[0]!.text = "界".repeat(
    Math.ceil(SYNC_STREAM_EVENT_MAX_BYTES / 3) + 1,
  );
  const encoded = JSON.stringify(entry),
    fragmentChars = 30_000;
  const partCount = Math.ceil(encoded.length / fragmentChars);
  for (let partIndex = 0; partIndex < partCount; partIndex++)
    source.emit("entity-transfer", {
      protocol: SYNC_PROTOCOL_VERSION,
      workspaceId: f.workspaceId,
      epoch: "first",
      revision: 5,
      reason: "initial",
      resource: large.resources[0],
      cursor: entry.cursor,
      replyKind: SubscriptionReplyKind.snapshot,
      payloadKind: "transcript",
      transferId: "recent-prefetch",
      pageIndex: 0,
      partIndex,
      partCount,
      complete: partIndex + 1 === partCount,
      data: encoded.slice(
        partIndex * fragmentChars,
        (partIndex + 1) * fragmentChars,
      ),
    });
  await vi.waitFor(() =>
    expect(peekTranscript(f.workspaceId, "recent-large")?.seq).toBe(5),
  );
  expect(
    peekTranscript(f.workspaceId, "recent-large")?.payload?.items[0]?.text,
  ).toBe(entry.transcriptSnapshot!.items[0]!.text);
});

it("cancellation at transcript metadata publication keeps its cursor atomic for the next change", async () => {
  const f = await setup();
  const resource = { kind: "transcript", agentId: "lead" } as const;
  await f.client.persistResourceEntityChanges(
    transcriptFrame(f.workspaceId, 4, ["old"]),
    () => true,
  );
  let current = true;
  const subscription = f.db.projections.$.subscribe(({ documentData }) => {
    if (documentData.id === "transcript:lead" && documentData.seq === 5)
      current = false;
  });
  await f.client.persistResourceEntityChanges(
    transcriptFrame(f.workspaceId, 5, ["old", "new"]),
    () => current,
  );
  subscription.unsubscribe();
  const cursor = await f.client.readSubscriptionCursor(resource);
  const [meta] = await f.db.projections.getRows(["transcript:lead"], false);
  expect(cursor?.seq).toBe(meta!.seq);
  const next = transcriptFrame(f.workspaceId, 6, []);
  next.resourceVersions[0]!.transcriptSnapshot = undefined;
  next.resourceVersions[0]!.replyKind = SubscriptionReplyKind.delta;
  next.resourceVersions[0]!.transcriptChanges = {
    after: 5,
    through: 6,
    delta: {
      delta: true,
      items: [{ id: "next", role: "assistant", text: "next" }],
      removed: [],
      truncated: false,
    },
  };
  await f.client.persistResourceEntityChanges(next, () => true);
  expect((await f.client.readSubscriptionCursor(resource))?.seq).toBe(6);
});

it("rejects an existing transcript cursor behind its metadata before reconnect", async () => {
  const f = await setup();
  await f.client.persistResourceEntityChanges(
    transcriptFrame(f.workspaceId, 4, ["old"]),
    () => true,
  );
  const [meta] = await f.db.projections.getRows(["transcript:lead"], false);
  await f.db.projections.applyBatch([{ ...meta!, seq: 5 }]);
  expect(
    await f.client.readSubscriptionCursor({
      kind: "transcript",
      agentId: "lead",
    }),
  ).toBeUndefined();
  await f.client.persistResourceEntityChanges(
    transcriptFrame(f.workspaceId, 6, ["latest"]),
    () => true,
  );
  expect(
    (
      await f.client.readSubscriptionCursor({
        kind: "transcript",
        agentId: "lead",
      })
    )?.seq,
  ).toBe(6);
});

it("stages a fragmented transcript entry before publishing data and its cursor", async () => {
  const f = await setup();
  const { EntityTransferAssembler } = await import("./entityTransfer.ts");
  const assembler = new EntityTransferAssembler();
  const resource = { kind: "transcript", agentId: "lead" } as const;
  const cursor = { workspaceId: f.workspaceId, seq: 7 };
  const text = "界".repeat(100_000);
  const entry: components["schemas"]["ResourceRevisionEntry"] = {
    resource,
    revision: 7,
    cursor,
    replyKind: SubscriptionReplyKind.snapshot,
    transcriptSnapshot: {
      items: [{ id: "large", role: "assistant", text }],
      truncated: false,
      nextCursor: null,
    },
  };
  const encoded = JSON.stringify(entry);
  const partCount = Math.ceil(encoded.length / 30_000);
  let interval;
  for (let partIndex = 0; partIndex < partCount; partIndex++) {
    interval = await assembler.receive({
      protocol: SYNC_PROTOCOL_VERSION,
      payloadKind: "transcript",
      workspaceId: f.workspaceId,
      epoch: "first",
      revision: 10,
      reason: "reconnect",
      resource,
      cursor,
      replyKind: SubscriptionReplyKind.snapshot,
      transferId: "large-transcript",
      pageIndex: 0,
      partIndex,
      partCount,
      complete: partIndex + 1 === partCount,
      data: encoded.slice(partIndex * 30_000, (partIndex + 1) * 30_000),
    });
    expect(await f.client.readSubscriptionCursor(resource)).toBeUndefined();
  }
  assert.ok(interval);
  await f.client.persistStagedEntityTransfer(interval, () => false);
  expect(await f.client.readSubscriptionCursor(resource)).toBeUndefined();
  await f.client.persistStagedEntityTransfer(interval, () => true);
  expect(await f.client.readSubscriptionCursor(resource)).toEqual(cursor);
  expect(f.pulls()).toEqual([]);
  const [row] = await f.db.projections.getRows(
    ["transcript:item:lead:large"],
    false,
  );
  expect(JSON.parse(row!.payload).text).toBe(text);
  await interval.dispose();
  await assembler.invalidate();
});

it("a transferred terminal entry retains generation and offset independently of its revision", async () => {
  const f = await setup();
  const { EntityTransferAssembler } = await import("./entityTransfer.ts");
  const assembler = new EntityTransferAssembler();
  const resource = { kind: "terminal-output", terminalId: "shell" } as const;
  const cursor = { workspaceId: f.workspaceId, seq: 10, terminalGeneration: 2 };
  const entry: components["schemas"]["ResourceRevisionEntry"] = {
    resource,
    revision: 3,
    cursor,
    replyKind: SubscriptionReplyKind.snapshot,
    viewSnapshot: {
      terminalOutput: {
        text: "terminal",
        offset: 10,
        status: "running",
        truncated: false,
      },
    },
  };
  const frame: components["schemas"]["ResourceEntityTransferEvent"] = {
    protocol: SYNC_PROTOCOL_VERSION,
    payloadKind: "view",
    workspaceId: f.workspaceId,
    epoch: "first",
    revision: 20,
    reason: "reconnect",
    resource,
    cursor,
    replyKind: SubscriptionReplyKind.snapshot,
    transferId: "terminal-entry",
    pageIndex: 0,
    partIndex: 0,
    partCount: 1,
    complete: true,
    data: JSON.stringify(entry),
  };
  const interval = await assembler.receive(frame);
  assert.ok(interval);
  await f.client.persistStagedEntityTransfer(interval, () => true);
  expect(await f.client.readSubscriptionCursor(resource)).toEqual(cursor);
  await interval.dispose();
  await expect(
    assembler.receive({
      ...frame,
      transferId: "foreign-generation",
      cursor: { ...cursor, terminalGeneration: 3 },
    }),
  ).rejects.toThrow("identity changed");
  await assembler.invalidate();
});

it("reloads a scoped projection before exposing its resume cursor", async () => {
  const f = await setup();
  const { createEntitySequenceScope } = await import("./entitySequence.ts");
  const { createTranscriptCache } = await import("./transcriptCache.ts");
  const values = new Map<string, unknown>();
  const storage = {
    get: async (key: string) => values.get(key),
    put: async (key: string, value: unknown) => {
      values.set(key, value);
    },
    delete: async (key: string) => {
      values.delete(key);
    },
  };
  const makeClient = () =>
    f.client.createSyncClient({
      api: {
        syncGet: mocks.syncGet,
        syncPost: vi.fn(),
        saved: (_key, fallback) => fallback,
        save: vi.fn(),
        setWorkspace: vi.fn(),
        isApiSchemaMismatch: () => false,
      },
      resources: {
        watchResourceChanges: () => () => {},
        watchResourceConnection: () => () => {},
      },
      sequences: createEntitySequenceScope(),
      transcripts: createTranscriptCache(),
      databaseSuffix: "cache-test",
      cacheStorage: storage,
    });
  const first = makeClient();
  const ref = { kind: "agentDetail", agentId: "lead" } as const;
  const frame = f.frame(3);
  frame.resources = [ref];
  assert.ok(frame.resourceVersions[0]);
  frame.resourceVersions[0].resource = ref;
  frame.resourceVersions[0].entityChanges!.documents = [
    {
      op: "upsert",
      collection: "agentDetail",
      id: "lead",
      seq: 3,
      value: { id: "lead", canSend: true },
    },
  ];
  await first.persistResourceEntityChanges(frame, () => true);
  const state = { kind: "state" } as const;
  const baseline = f.frame(4);
  assert.ok(baseline.resourceVersions[0]?.entityChanges);
  baseline.resourceVersions[0].entityChanges.documents = [
    {
      op: "upsert",
      collection: "messageReceipt",
      id: "message",
      seq: 4,
      value: {
        id: "message",
        agent: "lead",
        status: MessageSendStatus.queued,
        materialized: false,
      },
    },
  ];
  await first.persistResourceEntityChanges(baseline, () => true);
  first.setCachePolicies([
    { resource: ref, policy: SubscriptionCachePolicy.cache },
    { resource: state, policy: SubscriptionCachePolicy.cache },
  ]);
  vi.useFakeTimers();
  await first.cacheSubscription(ref);
  await first.cacheSubscription(state);
  await vi.advanceTimersByTimeAsync(5_000);
  first.disposeCache();
  vi.useRealTimers();
  const reloaded = makeClient();
  expect(await reloaded.readSubscriptionCursor(ref)).toEqual({
    workspaceId: f.workspaceId,
    seq: 3,
  });
  const { db } = await reloaded.syncDatabase();
  expect(
    (await db.projections.findOne("entity:agentDetail:lead").exec())?.entity
      ?.value,
  ).toMatchObject({ canSend: true });
  expect(await reloaded.readSubscriptionCursor(state)).toEqual({
    workspaceId: f.workspaceId,
    seq: 4,
  });
  const receipt = () =>
    db.projections.findOne("entity:messageReceipt:message").exec();
  expect((await receipt())?.entity?.value).toMatchObject({
    status: MessageSendStatus.queued,
  });
  const delta = f.frame(5);
  assert.ok(delta.resourceVersions[0]?.entityChanges);
  delta.resourceVersions[0].replyKind = SubscriptionReplyKind.delta;
  delta.resourceVersions[0].entityChanges.after = 4;
  delta.resourceVersions[0].entityChanges.documents = [
    {
      op: "upsert",
      collection: "messageReceipt",
      id: "message",
      seq: 5,
      value: {
        id: "message",
        agent: "lead",
        status: MessageSendStatus.delivered,
        materialized: true,
      },
    },
  ];
  await reloaded.persistResourceEntityChanges(delta, () => true);
  expect((await receipt())?.entity?.value).toMatchObject({
    status: MessageSendStatus.delivered,
  });
  reloaded.disposeCache();
});

it("waits for the applied cold SSE baseline and publishes it without a speculative pull", async () => {
  const f = await setup();
  f.subscribe();
  await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
  expect(mocks.watch).toHaveBeenCalledWith(
    { kind: "state" },
    expect.any(Function),
    { waitForBaseline: true },
  );
  f.delivery.receive(f.frame(5));
  await vi.waitFor(() =>
    expect(f.snapshots.at(-1)?.agents.values().next().value?.name).toBe(
      "SSE baseline",
    ),
  );
  expect(f.client.projectionMetadata.complete).toBe(true);
  expect(f.client.projectionMetadata.workspaceId).toBe(f.workspaceId);
  expect(f.dispatch).toHaveBeenCalledOnce();
  expect(f.pulls()).toEqual([]);
  expect(
    mocks.syncGet.mock.calls.filter(([path]) => path === "/api/sync/identity"),
  ).toHaveLength(1);
});

it("accepts applied reconnect and process-restart snapshots without recovery reads", async () => {
  const f = await setup();
  f.subscribe();
  await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
  f.delivery.receive(f.frame(8));
  await vi.waitFor(() =>
    expect(f.snapshots.at(-1)?.agents.values().next().value?.name).toBe(
      "SSE baseline",
    ),
  );
  f.connection("degraded");
  f.delivery.receive({ ...f.frame(8), reason: "reconnect" });
  await vi.waitFor(() => expect(f.dispatch).toHaveBeenCalledTimes(2));
  f.delivery.receive({
    ...f.frame(9, "replacement", "Restarted"),
    reason: "reconnect",
  });
  await vi.waitFor(() =>
    expect(f.snapshots.at(-1)?.agents.values().next().value?.name).toBe(
      "Restarted",
    ),
  );
  expect(f.pulls()).toEqual([]);
  expect(await f.client.readSubscriptionCursor({ kind: "state" })).toEqual({
    workspaceId: f.workspaceId,
    seq: 9,
  });
});

it("keeps a partial scoped cache independent from canonical state readiness", async () => {
  const f = await setup();
  const frame = f.frame(3);
  frame.resources = [{ kind: "agentDetail", agentId: "lead" }];
  assert.ok(frame.resourceVersions[0] != null);
  assert.ok(frame.resources[0]);
  frame.resourceVersions[0].resource = frame.resources[0];
  frame.resourceVersions[0].entityChanges!.documents = [
    {
      op: "upsert",
      collection: "agentDetail",
      id: "lead",
      seq: 3,
      value: { id: "lead", canSend: true },
    },
  ];
  await f.client.persistResourceEntityChanges(frame, () => true);
  expect(await f.client.readSubscriptionCursor(frame.resources[0])).toEqual({
    workspaceId: f.workspaceId,
    seq: 3,
  });
  expect(
    await f.client.readSubscriptionCursor({ kind: "state" }),
  ).toBeUndefined();
  f.subscribe();
  await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
  f.delivery.receive(f.frame(5));
  await vi.waitFor(() =>
    expect(f.client.projectionMetadata.complete).toBe(true),
  );
  expect(f.pulls()).toEqual([]);
});

it.each(["revision-only", "overflow"] as const)(
  "%s replaces the stream and recovers through SSE without HTTP reads",
  async (reason) => {
    const f = await setup();
    const { replaceStream } = await import("./subscriptions/events.ts");
    f.subscribe();
    await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
    const frame = f.frame(5);
    frame.reason = reason === "overflow" ? "overflow" : "initial";
    frame.resourceVersions = [{ revision: 5 }];
    f.delivery.receive(frame);
    await vi.waitFor(() => expect(replaceStream).toHaveBeenCalledOnce());
    expect(f.pulls()).toEqual([]);
    f.delivery.receive(f.frame(5));
    await vi.waitFor(() =>
      expect(f.client.projectionMetadata.complete).toBe(true),
    );
    expect(f.pulls()).toEqual([]);
  },
);

it.each(["degraded", "offline", "stopped"] as const)(
  "cold %s transport waits for its SSE baseline without HTTP recovery",
  async (state) => {
    const f = await setup();
    f.subscribe();
    await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
    f.connection(
      state === "stopped" ? "connecting" : state,
      state === "stopped" ? "stopped" : "open",
    );
    await new Promise<void>((resolve) => setTimeout(resolve, 20));
    expect(f.pulls()).toEqual([]);
    expect(f.client.projectionMetadata.complete).toBe(false);
    f.delivery.receive(f.frame(5));
    await vi.waitFor(() =>
      expect(f.client.projectionMetadata.complete).toBe(true),
    );
    expect(f.pulls()).toEqual([]);
  },
);

it("heals a truncated ready cache on reload and recovers the missing chat", async () => {
  const f = await setup();
  const { createEntitySequenceScope } = await import("./entitySequence.ts");
  const { createTranscriptCache } = await import("./transcriptCache.ts");
  const { subscriptionKey } = await import("./subscriptions/contracts.ts");
  const state = { kind: "state" } as const;
  const storage = {
    get: vi.fn().mockResolvedValue({
      version: 2,
      contractHash:
        "c3ad6f495d4abc98b28f8d992e16505f11369c5f6f8976540792f8d886de314c",
      subscription: subscriptionKey(state),
      cursor: { workspaceId: f.workspaceId, seq: 100 },
      data: [],
    }),
    put: vi.fn(),
    delete: vi.fn(),
  };
  const client = f.client.createSyncClient({
    api: {
      syncGet: mocks.syncGet,
      syncPost: vi.fn(),
      saved: (_key, fallback) => fallback,
      save: vi.fn(),
      setWorkspace: vi.fn(),
      isApiSchemaMismatch: () => false,
    },
    resources: {
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    },
    sequences: createEntitySequenceScope(),
    transcripts: createTranscriptCache(),
    cacheStorage: storage,
    databaseSuffix: "truncated-cache-test",
  });
  expect(await client.readSubscriptionCursor(state)).toBeUndefined();
  const baseline = f.frame(100);
  baseline.resourceVersions[0]!.entityChanges!.documents = [
    {
      op: "upsert",
      collection: "agentSummary",
      id: "missing",
      seq: 4,
      value: agentSummaryFixture("missing", {
        name: "Missing chat",
        status: AgentStatus.completed,
      }),
    },
  ];
  await client.persistResourceEntityChanges(baseline, () => true);
  const { db } = await client.syncDatabase();
  const row = await db.projections
    .findOne("entity:agentSummary:missing")
    .exec();
  expect(row?.toJSON().entity?.id).toBe("missing");
  expect(f.pulls()).toEqual([]);
  client.disposeCache();
});

it("does not use HTTP recovery to bypass a schema-mismatched stream", async () => {
  const f = await setup();
  f.subscribe();
  await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
  f.connection("schema-mismatch", "stopped");
  expect(f.pulls()).toEqual([]);
  expect(f.snapshots).toEqual([]);
});

it.each([false, true])(
  "applies an entity delta with an already persisted prefix (staged=%s)",
  async (staged) => {
    const f = await setup();
    await f.client.persistResourceEntityChanges(f.frame(9), () => true);
    const event = f.frame(11);
    assert.ok(event.resourceVersions[0]?.entityChanges);
    event.resourceVersions[0].replyKind = "delta";
    event.resourceVersions[0].entityChanges.after = 8;
    if (staged) {
      const { EntityTransferAssembler } = await import("./entityTransfer.ts");
      const assembler = new EntityTransferAssembler();
      const interval = await assembler.receive({
        protocol: event.protocol,
        workspaceId: f.workspaceId,
        epoch: event.epoch,
        revision: event.revision,
        reason: event.reason,
        resource: { kind: "state" },
        cursor: { workspaceId: f.workspaceId, seq: 11 },
        replyKind: "delta",
        transferId: "covered-prefix",
        pageIndex: 0,
        payloadKind: "entity",
        partIndex: 0,
        partCount: 1,
        complete: true,
        data: JSON.stringify(event.resourceVersions[0].entityChanges),
      });
      try {
        await f.client.persistStagedEntityTransfer(interval!, () => true);
      } finally {
        await interval?.dispose();
        await assembler.invalidate();
      }
    } else await f.client.persistResourceEntityChanges(event, () => true);
    expect(await f.client.readSubscriptionCursor({ kind: "state" })).toEqual({
      workspaceId: f.workspaceId,
      seq: 11,
    });
  },
);

it.each([false, true])(
  "snapshot staged=%s preserves a newer independently subscribed row and evicts only covered versions",
  async (staged) => {
    const f = await setup();
    const tasks = { kind: "tasks", agentId: "lead" } as const;
    const newer = { kind: "task", taskId: "newer" } as const;
    const snapshot = (
      resource: ResourceChangeEvent["resources"][number],
      through: number,
      documents: components["schemas"]["EntityChangeBatch"]["documents"],
    ): ResourceChangeEvent => ({
      ...f.frame(through),
      resources: [resource],
      resourceVersions: [
        {
          resource,
          revision: through,
          cursor: { workspaceId: f.workspaceId, seq: through },
          replyKind: SubscriptionReplyKind.snapshot,
          entityChanges: { after: 0, through, documents },
        },
      ],
    });
    const row = (
      id: string,
      seq: number,
    ): components["schemas"]["EntityChangeBatch"]["documents"][number] => ({
      collection: "task",
      op: "upsert",
      id,
      seq,
      value: {
        id,
        agent: "lead",
        kind: TaskKind.command,
        status: TaskStatus.running,
        created: 1,
        display: backgroundDisplay(),
      },
    });
    await f.client.persistResourceEntityChanges(
      snapshot(tasks, 9, [row("older", 9)]),
      () => true,
    );
    await f.client.persistResourceEntityChanges(
      snapshot({ kind: "task", taskId: "equal" }, 10, [row("equal", 10)]),
      () => true,
    );
    // This replacement was captured before the separate task scope committed 11.
    // Its own applied cursor is still 9, so the same-ref stale guard cannot hide the race.
    const captured = snapshot(tasks, 10, []);
    const { EntityTransferAssembler } = await import("./entityTransfer.ts");
    const assembler = new EntityTransferAssembler();
    let interval: Awaited<ReturnType<typeof assembler.receive>>;
    try {
      assert.ok(captured.resourceVersions[0] != null);
      if (staged)
        interval = await assembler.receive({
          protocol: captured.protocol,
          workspaceId: f.workspaceId,
          epoch: captured.epoch,
          revision: captured.revision,
          reason: captured.reason,
          resource: tasks,
          cursor: { workspaceId: f.workspaceId, seq: 10 },
          replyKind: "snapshot",
          transferId: crypto.randomUUID(),
          pageIndex: 0,
          payloadKind: "entity",
          partIndex: 0,
          partCount: 1,
          complete: true,
          data: JSON.stringify(captured.resourceVersions[0].entityChanges),
        });
      await f.client.persistResourceEntityChanges(
        snapshot(newer, 11, [row("newer", 11)]),
        () => true,
      );
      if (staged) {
        expect(interval).toBeDefined();
        await f.client.persistStagedEntityTransfer(interval!, () => true);
      } else await f.client.persistResourceEntityChanges(captured, () => true);
      expect(
        (await f.db.projections.findOne("entity:task:newer").exec())?.toJSON(),
      ).toMatchObject({ seq: 11, entity: { id: "newer" } });
      expect(await f.client.readSubscriptionCursor(newer)).toEqual({
        workspaceId: f.workspaceId,
        seq: 11,
      });
      expect(await f.client.readSubscriptionCursor(tasks)).toEqual({
        workspaceId: f.workspaceId,
        seq: 10,
      });
      for (const [id, seq] of [
        ["older", 9],
        ["equal", 10],
      ] as const) {
        const [removed] = await f.db.projections.getRows(
          [`entity:task:${id}`],
          true,
        );
        expect(removed).toMatchObject({ seq, _deleted: true, evicted: true });
      }
      expect(f.pulls()).toEqual([]);
    } finally {
      await interval?.dispose();
      await assembler.invalidate();
    }
  },
);

it.each([false, true])(
  "the actual Work adapter recovers a failed inline baseline and queued delta through SSE (durable base=%s)",
  async (durableBase) => {
    const f = await setup();
    vi.stubGlobal("location", { origin: "http://studio.test" });
    vi.stubGlobal("BroadcastChannel", undefined);
    const { createServerSubscriptions } =
      await import("./subscriptions/server.ts");
    const api = await import("../api.ts");
    const subscriptions = createServerSubscriptions({
      owner: { id: "local", origin: "http://studio.test" },
      api: { ...api, fetchApiRequest: vi.fn() },
      tokenRates: {
        configureTokenRateStream: vi.fn(),
        receiveResourceTokenRates: vi.fn(() => true),
        getWorkspaceTokenRates: vi.fn(() => ({ records: [] })),
      },
      sync: () => f.client,
    } as unknown as Parameters<typeof createServerSubscriptions>[0]);
    const ref = { kind: "work", agentId: "lead" } as const;
    const stop = subscriptions.subscribe(ref, () => {}, { graceMs: 0 });
    stops.push(stop, subscriptions.dispose);
    await vi.waitFor(() => expect(mocks.sources).toHaveLength(1));
    const source = mocks.sources[0];
    assert.ok(source != null);
    source.emit("api-schema", { hash: API_SCHEMA_HASH });
    source.emit("session", {
      sessionId: "quiet-work-session",
      cachePolicies: [],
    });
    const frame = (seq: number, snapshot: boolean): ResourceChangeEvent => ({
      protocol: SYNC_PROTOCOL_VERSION,
      workspaceId: f.workspaceId,
      epoch: "server",
      revision: seq,
      reason: snapshot ? "initial" : "change",
      resources: [ref],
      resourceVersions: [
        {
          resource: ref,
          revision: seq,
          cursor: { workspaceId: f.workspaceId, seq },
          replyKind: snapshot ? "snapshot" : "delta",
          entityChanges: {
            after: snapshot ? 0 : 5,
            through: seq,
            documents: [
              {
                op: "upsert",
                collection: "workDetail",
                id: "assignment",
                seq,
                value: {
                  id: "assignment",
                  rootId: "lead",
                  title: "Assignment",
                  created: 1,
                  updated: 1,
                  version: 1,
                  status: snapshot && seq === 5 ? "ready" : "review",
                  policy: {
                    blockedBy: [],
                    displayStatus: snapshot && seq === 5 ? "ready" : "review",
                    acceptAvailable: false,
                    gateOverrideRequired: false,
                  },
                },
              },
            ],
          },
        },
      ],
    });
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    if (durableBase)
      await f.client.persistResourceEntityChanges(frame(4, true), () => true);
    vi.spyOn(f.db.projections, "applyBatch").mockRejectedValueOnce(
      new Error("inline storage unavailable"),
    );
    source.emit("resources", frame(5, true));
    source.emit("resources", frame(6, false));
    await vi.waitFor(() => expect(mocks.sources).toHaveLength(2), {
      timeout: 2500,
    });
    expect(source.closed).toBe(true);
    expect(await f.client.readSubscriptionCursor(ref)).toEqual(
      durableBase ? { workspaceId: f.workspaceId, seq: 4 } : undefined,
    );
    const replacement = mocks.sources[1];
    assert.ok(replacement != null);
    const query = new URL(replacement.url).searchParams;
    if (durableBase)
      expect(JSON.parse(query.get("since")!)).toContainEqual({
        resource: ref,
        since: { workspaceId: f.workspaceId, seq: 4 },
      });
    else
      expect(JSON.parse(query.get("since")!)).not.toContainEqual(
        expect.objectContaining({ resource: ref }),
      );
    replacement.emit("api-schema", { hash: API_SCHEMA_HASH });
    replacement.emit("resources", frame(6, true));
    await vi.waitFor(() =>
      expect(subscriptions.getSnapshot(ref)).toMatchObject({
        status: "live",
        data: { items: [expect.objectContaining({ status: "review" })] },
      }),
    );
    expect(subscriptions.getSnapshot(ref).status).toBe("live");
    expect(await f.client.readSubscriptionCursor(ref)).toEqual({
      workspaceId: f.workspaceId,
      seq: 6,
    });
    expect(f.pulls()).toEqual([]);
    expect(
      mocks.syncGet.mock.calls.every(([path]) => path === "/api/sync/identity"),
    ).toBe(true);
    error.mockRestore();
  },
);

it.each([
  { follower: false, staged: false, cancelled: false },
  { follower: false, staged: true, cancelled: false },
  { follower: false, staged: true, cancelled: true },
])(
  "transport follower=$follower staged=$staged cancelled=$cancelled publishes readiness only after the actual projection receipt",
  async ({ follower, staged, cancelled }) => {
    const f = await setup();
    vi.stubGlobal("location", { origin: "http://studio.test" });
    let peer!: (value: unknown) => void;
    vi.stubGlobal(
      "BroadcastChannel",
      follower
        ? class {
            onmessage: ((event: MessageEvent) => void) | null = null;
            constructor() {
              peer = (value) =>
                this.onmessage?.({ data: value } as MessageEvent);
            }
            postMessage() {}
            close() {}
          }
        : undefined,
    );
    if (follower)
      vi.stubGlobal("navigator", {
        onLine: true,
        locks: {
          request: (
            _key: string,
            options: unknown,
            callback?: (lock: null) => unknown,
          ) =>
            callback
              ? new Promise<void>((_resolve, reject) => {
                  (options as { signal: AbortSignal }).signal.addEventListener(
                    "abort",
                    () => reject(new DOMException("Aborted", "AbortError")),
                    { once: true },
                  );
                })
              : (options as () => unknown)(),
        },
      });
    class Source {
      listeners = new Map<string, (event: Event) => void>();
      onerror = null;
      onopen = null;
      addEventListener(name: string, accept: (event: Event) => void) {
        this.listeners.set(name, accept);
      }
      close() {}
      emit(name: string, value: unknown) {
        this.listeners.get(name)?.({
          data: JSON.stringify(value),
        } as MessageEvent);
      }
    }
    const source = new Source();
    let finish!: () => void;
    const gate = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const persist = vi.fn(
      async (
        ...args: Parameters<typeof f.client.persistResourceEntityChanges>
      ) => {
        await gate;
        await f.client.persistResourceEntityChanges(...args);
      },
    );
    const persistStaged = vi.fn(
      async (
        ...args: Parameters<typeof f.client.persistStagedEntityTransfer>
      ) => {
        await gate;
        await f.client.persistStagedEntityTransfer(...args);
      },
    );
    const { createResourceEvents } = await import("./resourceEvents.ts");
    const transport = createResourceEvents({
      api: {
        get: mocks.syncGet,
        post: vi.fn().mockResolvedValue({ gen: 1 }),
        clearSchemaUpdateAttemptAfterMatch: vi.fn(),
        receiveStreamSession: vi.fn(),
        clearStreamSession: vi.fn(),
        isApiSchemaMismatch: () => false,
        matchingApiSchemaResponseGeneration: () => 0,
        markApiSchemaMismatch: vi.fn(),
        onApiSchemaMismatch: () => () => {},
      },
      sync: {
        syncDatabase: f.client.syncDatabase,
        persistResourceEntityChanges: persist,
        readSubscriptionCursor: f.client.readSubscriptionCursor,
        persistStagedEntityTransfer: persistStaged,
      },
      tokenRates: {
        configureTokenRateStream: vi.fn(),
        receiveResourceTokenRates: vi.fn(() => true),
        getWorkspaceTokenRates: vi.fn(() => ({ records: [] })),
      },
      openEvents: () => source,
    });
    mocks.watch.mockImplementation(transport.watchResourceChanges);
    mocks.watchConnection.mockImplementation(transport.watchResourceConnection);
    const stopProjection = f.subscribe();
    const store = createSubscriptionStore(transport);
    store.watchResourceChanges({ kind: "state" }, () => {});
    stops.push(finish, store.dispose, transport.dispose);
    if (follower) {
      await vi.waitFor(() => expect(peer).toBeTypeOf("function"));
      const validatedAt = Date.now();
      peer({
        kind: "resource-event",
        apiSchemaHash: API_SCHEMA_HASH,
        workspaceId: f.workspaceId,
        tabId: "owner",
        health: {
          status: "live",
          generation: 1,
          sequence: 1,
          sentAt: validatedAt,
          validatedAt,
        },
        event: f.frame(5),
      });
    } else {
      await vi.waitFor(() =>
        expect(source.listeners.has("resources")).toBe(true),
      );
      source.emit("api-schema", { hash: API_SCHEMA_HASH });
      if (staged) {
        const frame = f.frame(5);
        assert.ok(frame.resourceVersions[0] != null);
        const encoded = JSON.stringify(frame.resourceVersions[0].entityChanges);
        const split = Math.floor(encoded.length / 2);
        const metadata = {
          protocol: frame.protocol,
          workspaceId: f.workspaceId,
          epoch: frame.epoch,
          revision: frame.revision,
          reason: frame.reason,
          resource: frame.resources[0],
          cursor: { workspaceId: f.workspaceId, seq: 5 },
          replyKind: "snapshot",
          transferId: "cold-resource",
          pageIndex: 0,
          partCount: 2,
        };
        source.emit("entity-transfer", {
          ...metadata,
          payloadKind: "entity",
          partIndex: 0,
          complete: false,
          data: encoded.slice(0, split),
        });
        await vi.waitFor(() =>
          expect(mocks.stagingWrite).toHaveBeenCalledOnce(),
        );
        expect(persistStaged).not.toHaveBeenCalled();
        expect(
          await f.client.readSubscriptionCursor({ kind: "state" }),
        ).toBeUndefined();
        expect(f.snapshots).toEqual([]);
        expect(f.pulls()).toEqual([]);
        source.emit("entity-transfer", {
          ...metadata,
          payloadKind: "entity",
          partIndex: 1,
          complete: true,
          data: encoded.slice(split),
        });
      } else source.emit("resources", f.frame(5));
    }
    await vi.waitFor(() =>
      expect(staged ? persistStaged : persist).toHaveBeenCalledOnce(),
    );
    expect(store.getSnapshot({ kind: "state" }).status).not.toBe("live");
    expect(f.snapshots).toEqual([]);
    expect(f.pulls()).toEqual([]);
    if (staged) expect(persist).not.toHaveBeenCalled();
    if (cancelled) {
      // The screen releases its watcher before the server module closes.
      // A still-active cold watcher intentionally recovers a stopped stream.
      stopProjection();
      transport.dispose();
      finish();
      assert.ok(persistStaged.mock.results[0] != null);
      await persistStaged.mock.results[0].value;
      expect(
        await f.client.readSubscriptionCursor({ kind: "state" }),
      ).toBeUndefined();
      expect(store.getSnapshot({ kind: "state" }).status).not.toBe("live");
      expect(f.snapshots).toEqual([]);
      expect(f.pulls()).toEqual([]);
      return;
    }
    finish();
    await vi.waitFor(() =>
      expect(store.getSnapshot({ kind: "state" }).status).toBe("live"),
    );
    await vi.waitFor(() =>
      expect(f.snapshots.at(-1)?.agents.values().next().value?.name).toBe(
        "SSE baseline",
      ),
    );
    expect(await f.client.readSubscriptionCursor({ kind: "state" })).toEqual({
      workspaceId: f.workspaceId,
      seq: 5,
    });
    expect(f.pulls()).toEqual([]);
  },
);

it.each(["burst", "inline removal", "staged removal"])(
  "the actual Work adapter preserves a still-active interval during %s",
  async (cause) => {
    const f = await setup();
    vi.stubGlobal("location", { origin: "http://studio.test" });
    vi.stubGlobal("BroadcastChannel", undefined);
    const { createServerSubscriptions } =
      await import("./subscriptions/server.ts");
    const api = await import("../api.ts");
    const subscriptions = createServerSubscriptions({
      owner: { id: "local", origin: "http://studio.test" },
      api: { ...api, fetchApiRequest: vi.fn() },
      tokenRates: {
        configureTokenRateStream: vi.fn(),
        receiveResourceTokenRates: vi.fn(() => true),
        getWorkspaceTokenRates: vi.fn(() => ({ records: [] })),
      },
      sync: () => f.client,
    } as unknown as Parameters<typeof createServerSubscriptions>[0]);
    const ref = { kind: "work", agentId: "lead" } as const;
    const stop = subscriptions.subscribe(ref, () => {}, { graceMs: 0 });
    const others = Array.from(
      { length: 10 },
      (_, index) =>
        ({ kind: "agentDetail", agentId: `other-${index}` }) as const,
    );
    const stopOthers = others.map((other) =>
      subscriptions.watchResourceChanges(other, () => {}, { graceMs: 0 }),
    );
    stops.push(stop, ...stopOthers, subscriptions.dispose);
    await vi.waitFor(() => expect(mocks.sources).toHaveLength(1));
    const source = mocks.sources[0];
    assert.ok(source != null);
    source.emit("api-schema", { hash: API_SCHEMA_HASH });
    let finish!: () => void;
    source.emit("session", {
      sessionId: "quiet-work-discard-session",
      cachePolicies: [],
    });
    const gate = new Promise<void>((resolve) => {
      finish = resolve;
    });
    stops.push(finish);
    // Keep the held storage write independent of wall-clock scheduling.
    // The test controls cancellation and reconnect, not the persistence timeout.
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const started = Promise.withResolvers<void>();
    const staged = cause === "staged removal";
    const originalInline = f.client.persistResourceEntityChanges;
    const originalStaged = f.client.persistStagedEntityTransfer;
    const held = staged
      ? vi
          .spyOn(f.client, "persistStagedEntityTransfer")
          .mockImplementationOnce(async (...args) => {
            started.resolve();
            await gate;
            return originalStaged(...args);
          })
      : vi
          .spyOn(f.client, "persistResourceEntityChanges")
          .mockImplementationOnce(async (...args) => {
            started.resolve();
            await gate;
            return originalInline(...args);
          });
    const frame: ResourceChangeEvent = {
      protocol: SYNC_PROTOCOL_VERSION,
      workspaceId: f.workspaceId,
      epoch: "server",
      revision: 5,
      reason: "initial",
      resources: [ref],
      resourceVersions: [
        {
          resource: ref,
          revision: 5,
          cursor: { workspaceId: f.workspaceId, seq: 5 },
          replyKind: SubscriptionReplyKind.snapshot,
          entityChanges: {
            after: 0,
            through: 5,
            documents: [
              {
                op: "upsert",
                collection: "workDetail",
                id: "assignment",
                seq: 5,
                value: {
                  id: "assignment",
                  rootId: "lead",
                  title: "Quiet assignment",
                  created: 1,
                  updated: 1,
                  version: 1,
                  status: WorkStatus.ready,
                  policy: {
                    blockedBy: [],
                    displayStatus: WorkStatus.ready,
                    acceptAvailable: false,
                    gateOverrideRequired: false,
                  },
                },
              },
            ],
          },
        },
      ],
    };
    try {
      assert.ok(source != null);
      assert.ok(frame.resourceVersions[0] != null);
      if (staged)
        source.emit("entity-transfer", {
          protocol: frame.protocol,
          workspaceId: f.workspaceId,
          epoch: frame.epoch,
          revision: 5,
          reason: "initial",
          resource: ref,
          cursor: { workspaceId: f.workspaceId, seq: 5 },
          replyKind: "snapshot",
          transferId: "quiet-work",
          pageIndex: 0,
          payloadKind: "entity",
          partIndex: 0,
          partCount: 1,
          complete: true,
          data: JSON.stringify(frame.resourceVersions[0].entityChanges),
        });
      else source.emit("resources", frame);
      await started.promise;
      expect(held).toHaveBeenCalledOnce();
      if (cause === "burst") {
        for (const [index, other] of others.entries())
          source.emit("resources", {
            protocol: frame.protocol,
            workspaceId: f.workspaceId,
            epoch: frame.epoch,
            revision: 6 + index,
            reason: "initial",
            resources: [other],
            resourceVersions: [
              {
                resource: other,
                revision: 6 + index,
                cursor: { workspaceId: f.workspaceId, seq: 6 + index },
                replyKind: "snapshot",
                entityChanges: { after: 0, through: 6 + index, documents: [] },
              },
            ],
          });
      } else {
        assert.ok(stopOthers[0]);
        stopOthers[0]();
      }
      expect(source.closed).toBe(false);
      finish();
      assert.ok(held.mock.results[0]);
      await held.mock.results[0].value;
      await vi.advanceTimersByTimeAsync(0);
      expect(mocks.sources).toHaveLength(1);
      expect(await f.client.readSubscriptionCursor(ref)).toEqual({
        workspaceId: f.workspaceId,
        seq: 5,
      });
      await vi.waitFor(() =>
        expect(subscriptions.getSnapshot(ref)).toMatchObject({
          status: "live",
          data: {
            items: [expect.objectContaining({ title: "Quiet assignment" })],
          },
        }),
      );
      expect(f.pulls()).toEqual([]);
      expect(
        mocks.syncGet.mock.calls.every(
          ([path]) => path === "/api/sync/identity",
        ),
      ).toBe(true);
    } finally {
      subscriptions.dispose();
      finish();
      await Promise.allSettled(held.mock.results.map((result) => result.value));
      await vi.advanceTimersByTimeAsync(0);
      vi.clearAllTimers();
      vi.useRealTimers();
    }
  },
);

it("discards a slow old stream policy after a reset and applies NoCache", async () => {
  const f = await setup();
  const { createEntitySequenceScope } = await import("./entitySequence.ts");
  const { createTranscriptCache } = await import("./transcriptCache.ts");
  const identity = Promise.withResolvers<{ workspaceId: string }>();
  const storage = { get: vi.fn(), put: vi.fn(), delete: vi.fn() };
  const client = f.client.createSyncClient({
    api: {
      syncGet: vi.fn().mockImplementation(() => identity.promise),
      syncPost: vi.fn(),
      saved: (_key, fallback) => fallback,
      save: vi.fn(),
      setWorkspace: vi.fn(),
      isApiSchemaMismatch: () => false,
    },
    resources: {
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    },
    sequences: createEntitySequenceScope(),
    transcripts: createTranscriptCache(),
    cacheStorage: storage,
    databaseSuffix: "slow-policy-test",
  });
  const ref = { kind: "state" } as const;
  client.setCachePolicies([
    { resource: ref, policy: SubscriptionCachePolicy.cache },
  ]);
  client.clearCachePolicies();
  client.setCachePolicies([
    { resource: ref, policy: SubscriptionCachePolicy.noCache },
  ]);
  identity.resolve({ workspaceId: f.workspaceId });
  await vi.waitFor(() => expect(storage.delete).toHaveBeenCalledOnce());
  await client.cacheSubscription(ref);
  expect(storage.put).not.toHaveBeenCalled();
  client.disposeCache();
});

it("sends only applied live receipt rows to the outbox", async () => {
  const f = await setup();
  const { createEntitySequenceScope } = await import("./entitySequence.ts");
  const { createTranscriptCache } = await import("./transcriptCache.ts");
  const onFreshReceipts = vi.fn();
  const options = {
    api: {
      syncGet: mocks.syncGet,
      syncPost: vi.fn(),
      saved: <T>(_key: string, fallback: T) => fallback,
      save: vi.fn(),
      setWorkspace: vi.fn(),
      isApiSchemaMismatch: () => false,
    },
    resources: {
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    },
    sequences: createEntitySequenceScope(),
    transcripts: createTranscriptCache(),
    databaseSuffix: "fresh-receipts-test",
    onFreshReceipts,
  };
  const client = f.client.createSyncClient(options);
  const frame = f.frame(1);
  frame.resourceVersions[0]!.entityChanges!.documents = [
    {
      op: "upsert",
      collection: "messageReceipt",
      id: "one",
      seq: 1,
      value: { id: "one", agent: "lead", status: MessageSendStatus.uncertain },
    },
  ];
  await client.persistResourceEntityChanges(frame, () => true);
  expect(onFreshReceipts).toHaveBeenCalledOnce();
  expect(onFreshReceipts).toHaveBeenCalledWith(f.workspaceId, [
    { id: "one", agent: "lead", status: MessageSendStatus.uncertain },
  ]);
});

it("publishes project sequences and complete-baseline identity from durable rows", async () => {
  const f = await setup();
  const { db, workspaceId } = await f.client.syncDatabase();
  const metadata = f.client.defaultSyncClient.projectionMetadata;
  expect(metadata.workspaceId).toBe(workspaceId);
  expect(metadata.complete).toBe(false);
  await db.projections.hydrate([
    {
      id: "entity:project:p",
      seq: 7,
      payload: "",
      entity: {
        collection: "project",
        id: "p",
        value: { id: "p", path: "/p" },
      },
    },
    { id: "state:entities:ready", seq: 1, payload: "ready" },
    { id: "state:entities:complete", seq: 1, payload: "complete" },
  ]);
  expect(metadata.projectSequences.p).toBe(7);
  expect(metadata.ready).toBe(true);
  expect(metadata.complete).toBe(true);
  await db.projections.hydrate([
    { id: "state:entities:ready", seq: 2, payload: "", _deleted: true },
  ]);
  expect(metadata.ready).toBe(false);
  expect(metadata.complete).toBe(false);
  await db.projections.hydrate([
    { id: "entity:project:p", seq: 8, payload: "", _deleted: true },
  ]);
  expect(metadata.projectSequences.p).toBeUndefined();
});

it("releases the generated observer's single durable state lease after cold-baseline recovery", async () => {
  const f = await setup();
  const stop = f.subscribe();
  await vi.waitFor(() => expect(mocks.watch).toHaveBeenCalledOnce());
  expect(f.pulls()).toEqual([]);
  f.connection("degraded", "connecting");
  f.delivery.receive(f.frame(5));
  await vi.waitFor(() =>
    expect(f.client.projectionMetadata.complete).toBe(true),
  );
  expect(f.pulls()).toEqual([]);
  expect(mocks.watch).toHaveBeenCalledOnce();
  stop();
  await vi.waitFor(() => expect(mocks.changes.size).toBe(0));
  f.notify({ epoch: "late", revision: 100, reason: "overflow" });
  f.connection("degraded", "connecting");
  await Promise.resolve();
  expect(f.pulls()).toEqual([]);
});

it.each([
  { kind: "multi-server" },
  { kind: "native-layr-settings" },
  { kind: "auth-progress", requestId: "login" },
  { kind: "rename-progress", agentId: "agent", requestId: "rename" },
  { kind: "voice", agentId: "agent" },
] as const)(
  "transient $kind cursor preparation never opens the sync database",
  async (ref) => {
    mocks.syncGet.mockRejectedValue(
      new Error("A transient resource must not open the database"),
    );
    const client = await import("./client.ts");
    await expect(client.readSubscriptionCursor(ref)).resolves.toBeUndefined();
    expect(mocks.syncGet).not.toHaveBeenCalled();
  },
);

it.each(["live", "degraded"] as const)(
  "explicit retry replaces only an unhealthy stream (%s)",
  async (health) => {
    const f = await setup();
    const stop = vi.fn();
    mocks.watchConnection.mockImplementation((accept) => {
      accept(health, health === "live" ? "open" : "connecting", {});
      return stop;
    });
    const { replaceStream } = await import("./subscriptions/events.ts");
    vi.mocked(replaceStream).mockClear();
    await f.client.refreshState();
    expect(stop).toHaveBeenCalledOnce();
    expect(replaceStream).toHaveBeenCalledTimes(health === "live" ? 0 : 1);
  },
);
