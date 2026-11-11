// @vitest-environment happy-dom
import { FrontendLocality, SubscriptionReplyKind } from "../generated/api.ts";
import { afterEach, expect, it, vi } from "vitest";
import { cacheScopePrefix } from "./cacheScope.ts";
import { createSyncClient } from "./client.ts";
import { createApiScope } from "../servers/net/api.ts";
import { createMutationJournal } from "../servers/mutationIdentity.ts";
import { createEntitySequenceScope } from "./entitySequence.ts";
import { createTranscriptCache } from "./transcriptCache.ts";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../generated/apiSchema.ts";
import { SUBSCRIPTION_CACHE_VERSION } from "./subscriptionCache.ts";
import { subscriptionKey } from "./subscriptions/contracts.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
const workspaceA = "a".repeat(32),
  workspaceB = "b".repeat(32);
const cleanup: (() => void)[] = [];
afterEach(() => {
  cleanup.splice(0).forEach((stop) => stop());
  vi.restoreAllMocks();
});
function fixture(holdCache = false, holdIdentity = false, cold = false) {
  const cached = {
    version: SUBSCRIPTION_CACHE_VERSION,
    contractHash: API_SCHEMA_HASH,
    subscription: subscriptionKey({ kind: "state" }),
    cursor: { workspaceId: workspaceA, seq: 1 },
    data: [
      {
        kind: "entity",
        id: "entity:agentSummary:saved",
        seq: 1,
        entity: {
          collection: "agentSummary",
          id: "saved",
          value: agentSummaryFixture("saved"),
        },
      },
    ],
  };
  const hint = {
    serverId: "local",
    credentialId: "",
    tokenFingerprint: "old-token",
    workspaceId: workspaceA,
  };
  const rows = new Map<string, unknown>([
    [
      cacheScopePrefix(hint) +
        JSON.stringify([workspaceA, subscriptionKey({ kind: "state" })]),
      cached,
    ],
    [JSON.stringify([workspaceA, subscriptionKey({ kind: "state" })]), cached],
  ]);
  const delayed = Promise.withResolvers<void>();
  const delayedIdentity = Promise.withResolvers<Response>();
  let status = 200,
    workspace = workspaceA;
  let sessionToken = "token";
  let invalidate = () => {};
  let authStopped = false;
  const retryNow = vi.fn(() => {
    authStopped = false;
  });
  const transcripts = createTranscriptCache();
  const storage = {
    onInvalidation: (accept: () => void) => {
      invalidate = accept;
      return () => {};
    },
    get: vi.fn(async (key: string) => {
      const value = rows.get(key);
      if (holdCache && key.includes(workspaceA)) await delayed.promise;
      return value;
    }),
    put: vi.fn(async (key: string, value: unknown) => {
      rows.set(key, value);
    }),
    delete: async (key: string) => {
      rows.delete(key);
    },
    clear: vi.fn(async () => {
      rows.clear();
    }),
  };
  const values = new Map<string, string>();
  const preferences: Storage = {
    get length() {
      return values.size;
    },
    key: (index) => [...values.keys()][index] ?? null,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => {
      values.set(key, value);
    },
    removeItem: (key) => {
      values.delete(key);
    },
    clear: () => values.clear(),
  };
  preferences.setItem("codex-sync-workspace", JSON.stringify(workspaceA));
  preferences.setItem("codex-sync-cache-scope", JSON.stringify(hint));
  if (cold) preferences.clear();
  const api = createApiScope({
    origin: "https://session.example",
    localStorage: preferences,
    sessionStorage: preferences,
    beginMutation: createMutationJournal("session", preferences),
    fetch: async (request) =>
      holdIdentity && new URL(request.url).pathname === "/api/sync/identity"
        ? delayedIdentity.promise
        : new Response(
            JSON.stringify(
              status === 200
                ? new URL(request.url).pathname === "/api/session"
                  ? { token: sessionToken }
                  : { workspaceId: workspace }
                : { error: "revoked" },
            ),
            {
              status,
              headers: {
                [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
                "Content-Type": "application/json",
              },
            },
          ),
  });
  const client = createSyncClient({
    api,
    cacheStorage: storage,
    sequences: createEntitySequenceScope(),
    transcripts,
    databaseSuffix: "session-test",
    resources: {
      retryNow,
      rejectAuthorization: () => {
        authStopped = true;
      },
      watchResourceChanges: () => () => {},
      watchResourceConnection: (listener) => {
        listener(
          authStopped ? "degraded" : "live",
          authStopped ? "stopped" : "open",
          authStopped ? { failure: "auth-rejected" } : {},
        );
        return () => {};
      },
    },
  });
  cleanup.push(client.disposeCache);
  return {
    api,
    client,
    rows,
    storage,
    preferences,
    delayed,
    delayedIdentity,
    transcripts,
    retryNow,
    sessionToken: (value: string) => {
      sessionToken = value;
    },
    invalidate: () => invalidate(),
    revoke: () => {
      status = 401;
    },
    replace: () => {
      workspace = workspaceB;
    },
  };
}
it("removes warmed rows and persisted snapshots when the session is rejected", async () => {
  const f = fixture();
  const { db } = await f.client.syncDatabase();
  await vi.waitFor(() => expect(f.client.projectionMetadata.ready).toBe(true));
  f.revoke();
  await expect(f.api.refreshSession()).rejects.toThrow();
  expect(db.projections.all()).toEqual([]);
  expect(f.client.projectionMetadata.ready).toBe(false);
  expect([...f.rows.keys()].some((key) => key.includes(workspaceA))).toBe(
    false,
  );
});
it("discards hydration which finishes after authorization rejection", async () => {
  const f = fixture(true);
  const { db } = await f.client.syncDatabase();
  f.revoke();
  await expect(f.api.refreshSession()).rejects.toThrow();
  f.delayed.resolve();
  await Promise.resolve();
  await Promise.resolve();
  expect(db.projections.all()).toEqual([]);
  expect(f.client.projectionMetadata.ready).toBe(false);
});
it("clears persisted and hydrated rows when the confirmed token differs", async () => {
  const f = fixture();
  const { db } = await f.client.syncDatabase();
  await vi.waitFor(() => expect(f.client.projectionMetadata.ready).toBe(true));
  await f.api.refreshSession();
  expect(db.projections.all()).toEqual([]);
  expect(f.client.projectionMetadata.ready).toBe(false);
  expect(f.rows.size).toBe(0);
});
it("replaces the saved workspace when the authoritative identity changes", async () => {
  const f = fixture();
  f.replace();
  const storage = await f.client.syncDatabase();
  await vi.waitFor(() => expect(storage.workspaceId).toBe(workspaceB));
  expect(f.client.projectionMetadata.workspaceId).toBe(workspaceB);
  expect(storage.db.projections.all()).toEqual([]);
  expect(
    JSON.parse(f.preferences.getItem("codex-sync-workspace") || "null"),
  ).toBe(workspaceB);
});
it("keeps the newer session identity when an older identity request finishes late", async () => {
  const f = fixture(false, true);
  const storage = await f.client.syncDatabase();
  f.api.receiveStreamSession({
    sessionId: "newer",
    workspaceId: workspaceB,
    routes: [],
    locality: FrontendLocality.local,
    cachePolicies: [],
  });
  f.delayedIdentity.resolve(
    new Response(JSON.stringify({ workspaceId: workspaceA }), {
      headers: {
        [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
        "Content-Type": "application/json",
      },
    }),
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(storage.workspaceId).toBe(workspaceB);
});

it("does not restart hydration when a cursor resumes during rejected-scope deletion", async () => {
  const f = fixture();
  const entered = Promise.withResolvers<void>(),
    release = Promise.withResolvers<void>();
  f.storage.put.mockImplementation(async (key, value) => {
    f.rows.set(key, value);
    if (key.endsWith("subscription-cache:index")) {
      entered.resolve();
      await release.promise;
    }
  });
  const { db } = await f.client.syncDatabase();
  await entered.promise;
  const reading = f.client.readSubscriptionCursor({ kind: "state" });
  const rejecting = f.api.rejectSession();
  try {
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(db.projections.all()).toEqual([]);
  } finally {
    release.resolve();
    await Promise.all([reading, rejecting]);
  }
  expect(db.projections.all()).toEqual([]);
});
it("adopts overlapping notifications of a new token without skipping invalidation", async () => {
  const f = fixture();
  const { db } = await f.client.syncDatabase();
  await vi.waitFor(() => expect(f.client.projectionMetadata.ready).toBe(true));
  const release = Promise.withResolvers<void>(),
    entered = Promise.withResolvers<void>();
  f.storage.clear.mockImplementationOnce(async () => {
    entered.resolve();
    await release.promise;
    f.rows.clear();
  });
  const first = f.api.refreshSession();
  await entered.promise;
  const second = f.api.refreshSession();
  try {
    expect(db.projections.all()).toEqual([]);
  } finally {
    release.resolve();
    await Promise.all([first, second]);
  }
  expect(db.projections.all()).toEqual([]);
  expect(f.client.projectionMetadata.ready).toBe(false);
});
it("discards a workspace-A writer paused before workspace B is confirmed", async () => {
  const f = fixture(false, true);
  const { db } = await f.client.syncDatabase();
  const entered = Promise.withResolvers<void>(),
    release = Promise.withResolvers<void>();
  const query = db.projections.query.bind(db.projections);
  vi.spyOn(db.projections, "query").mockImplementation(async (...args) => {
    entered.resolve();
    await release.promise;
    return query(...args);
  });
  const ref = { kind: "state" } as const;
  const writing = f.client.persistResourceEntityChanges(
    {
      protocol: 4,
      workspaceId: workspaceA,
      epoch: "server",
      revision: 2,
      reason: "initial",
      resources: [ref],
      resourceVersions: [
        {
          resource: ref,
          revision: 2,
          cursor: { workspaceId: workspaceA, seq: 2 },
          replyKind: SubscriptionReplyKind.snapshot,
          entityChanges: {
            after: 0,
            through: 2,
            documents: [
              {
                op: "upsert",
                seq: 2,
                collection: "agentSummary",
                id: "old-A",
                value: agentSummaryFixture("old-A"),
              },
            ],
          },
        },
      ],
    },
    () => true,
  );
  await entered.promise;
  f.api.receiveStreamSession({
    sessionId: "replacement",
    workspaceId: workspaceB,
    routes: [],
    locality: FrontendLocality.local,
    cachePolicies: [],
  });
  release.resolve();
  await writing;
  expect(db.projections.all()).toEqual([]);
  expect(f.client.projectionMetadata.ready).toBe(false);
});

it("stops projection notifications when their scope is revoked by a listener", async () => {
  const f = fixture();
  const { db } = await f.client.syncDatabase();
  await vi.waitFor(() => expect(f.client.projectionMetadata.ready).toBe(true));
  const epoch = f.client.scopeEpoch;
  const listener = db.projections.$.subscribe((event) => {
    if (
      event.documentData.id === "entity:agentSummary:trigger" &&
      event.context === "projection"
    )
      void f.api.rejectSession();
  });
  await db.projections.applyBatch(
    [
      { id: "entity:agentSummary:trigger", seq: 2, payload: "" },
      { id: "state:entities:ready", seq: 2, payload: "ready" },
    ],
    { isCurrent: () => f.client.scopeEpoch === epoch },
  );
  listener.unsubscribe();
  expect(db.projections.all()).toEqual([]);
  expect(f.client.projectionMetadata.ready).toBe(false);
});

it("discards token hashing that finishes after credential invalidation", async () => {
  const f = fixture();
  await f.client.syncDatabase();
  const digest = Promise.withResolvers<ArrayBuffer>();
  vi.spyOn(crypto.subtle, "digest").mockReturnValueOnce(digest.promise);
  f.api.setToken("token");
  expect(crypto.subtle.digest).toHaveBeenCalled();
  f.invalidate();
  expect(f.client.currentScope).toBeNull();
  digest.resolve(new Uint8Array([1]).buffer);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(f.client.currentScope).toBeNull();
  expect(f.retryNow).not.toHaveBeenCalled();
});

it("drops an A transcript observer instead of caching or delivering B under A", async () => {
  const f = fixture();
  const { db } = await f.client.syncDatabase();
  const delivered: unknown[] = [];
  const stopA = f.client.subscribeTranscriptProjection(
    "transcript:chat",
    (value) => delivered.push(value),
    () => {},
    true,
  );
  cleanup.push(stopA);
  await new Promise((resolve) => setTimeout(resolve, 0));
  f.api.receiveStreamSession({
    sessionId: "B",
    workspaceId: workspaceB,
    routes: [],
    locality: FrontendLocality.local,
    cachePolicies: [],
  });
  const stopB = f.client.subscribeTranscriptProjection(
    "transcript:chat",
    () => {},
    () => {},
    true,
  );
  cleanup.push(stopB);
  await new Promise((resolve) => setTimeout(resolve, 0));
  await db.projections.applyBatch([
    {
      id: "transcript:chat",
      seq: 2,
      payload: JSON.stringify({
        format: 3,
        order: [],
        metadata: { label: "B private" },
      }),
    },
  ]);
  await vi.waitFor(() =>
    expect(f.transcripts.peekTranscript(workspaceB, "chat")?.payload).toEqual({
      label: "B private",
      items: [],
    }),
  );
  expect(f.transcripts.peekTranscript(workspaceA, "chat")).toBeUndefined();
  expect(delivered).not.toContainEqual({ label: "B private", items: [] });
});

it("keeps a cold identity rejected until a new token restarts transport", async () => {
  const f = fixture(false, true, true);
  const opening = f.client.syncDatabase();
  await f.api.rejectSession();
  f.delayedIdentity.resolve(
    new Response(JSON.stringify({ workspaceId: workspaceA }), {
      headers: {
        [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
        "Content-Type": "application/json",
      },
    }),
  );
  await opening;
  const rejectedScope = f.client.currentScope;
  await f.api.refreshSession();
  expect(rejectedScope).toBeNull();
  expect(f.retryNow).toHaveBeenCalledOnce();
});

it("reattaches a mounted transcript after a token epoch in the same workspace", async () => {
  const f = fixture();
  const { db } = await f.client.syncDatabase();
  const delivered: unknown[] = [];
  const stop = f.client.subscribeTranscriptProjection(
    "transcript:chat",
    (value) => delivered.push(value),
    () => {},
    true,
  );
  cleanup.push(stop);
  await new Promise((resolve) => setTimeout(resolve, 0));
  await f.api.refreshSession();
  await db.projections.applyBatch([
    {
      id: "transcript:chat",
      seq: 2,
      payload: JSON.stringify({
        format: 3,
        order: [],
        metadata: { label: "fresh after refresh" },
      }),
    },
  ]);
  await vi.waitFor(() =>
    expect(delivered).toContainEqual({
      label: "fresh after refresh",
      items: [],
    }),
  );
});

it("retries the rejected transport after overlapping T1 and T2 recovery", async () => {
  const f = fixture();
  await f.client.syncDatabase();
  const fingerprint = async (token: string) =>
    Array.from(
      new Uint8Array(
        await crypto.subtle.digest("SHA-256", new TextEncoder().encode(token)),
      ),
      (byte) => byte.toString(16).padStart(2, "0"),
    ).join("");
  const first = await fingerprint("T1"),
    second = await fingerprint("T2");
  const entered = Promise.withResolvers<void>(),
    release = Promise.withResolvers<void>();
  f.storage.clear.mockImplementationOnce(async () => {
    entered.resolve();
    await release.promise;
    f.rows.clear();
  });
  const rejection = f.api.rejectSession();
  await entered.promise;
  try {
    f.sessionToken("T1");
    const firstRecovery = f.api.refreshSession();
    await vi.waitFor(() =>
      expect(f.client.currentScope?.tokenFingerprint).toBe(first),
    );
    f.sessionToken("T2");
    const secondRecovery = f.api.refreshSession();
    await vi.waitFor(() =>
      expect(f.client.currentScope?.tokenFingerprint).toBe(second),
    );
    release.resolve();
    await Promise.all([firstRecovery, secondRecovery]);
  } finally {
    release.resolve();
    await rejection;
  }
  await vi.waitFor(() => expect(f.retryNow).toHaveBeenCalledOnce());
});

it("revokes a pending hash when the client is disposed and starts no cache", async () => {
  const f = fixture();
  await f.client.syncDatabase();
  const digest = Promise.withResolvers<ArrayBuffer>();
  vi.spyOn(crypto.subtle, "digest").mockReturnValueOnce(digest.promise);
  f.api.setToken("token");
  f.client.disposeCache();
  const epoch = f.client.scopeEpoch;
  const reads = f.storage.get.mock.calls.length;
  digest.resolve(new Uint8Array([1]).buffer);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(f.client.scopeEpoch).toBe(epoch);
  expect(f.storage.get).toHaveBeenCalledTimes(reads);
  expect(f.retryNow).not.toHaveBeenCalled();
});
