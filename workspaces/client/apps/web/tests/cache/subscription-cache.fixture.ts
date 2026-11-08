import { observeGeneratedState } from "../../src/sync/generatedState.testHelper.ts";
import { serializeStateCollections } from "../../src/generated/observableResources.ts";
import { syncGet } from "../../src/api.ts";
import { createSyncClient } from "../../src/sync/client.ts";
import { createResourceEvents } from "../../src/sync/resourceEvents.ts";
import { createEntitySequenceScope } from "../../src/sync/entitySequence.ts";
import { createTranscriptCache } from "../../src/sync/transcriptCache.ts";
import { indexedDb } from "../../src/indexedDb.ts";
import { createTokenRateCache } from "../../src/usage/tokenRate.ts";
import { subscriptionKey } from "../../src/sync/subscriptions/contracts.ts";

const workspaceId = "b".repeat(32);
const ref = { kind: "state" } as const;
const storage = indexedDb("cache-browser-witness", "subscriptions", 1);
let transport: ReturnType<typeof createResourceEvents>;
const client = createSyncClient({
  api: {
    syncGet,
    syncPost: async () => {
      throw new Error("unexpected request");
    },
    // `?saved=1` starts like a returning app that knows its workspace.
    saved: <T>(key: string, fallback: T) =>
      key === "codex-sync-workspace" &&
      new URLSearchParams(location.search).has("saved")
        ? (workspaceId as T)
        : fallback,
    save: () => {},
    setWorkspace: () => {},
    isApiSchemaMismatch: () => false,
  },
  resources: {
    watchResourceChanges: (...args) => transport.watchResourceChanges(...args),
    watchResourceConnection: (...args) =>
      transport.watchResourceConnection(...args),
  },
  sequences: createEntitySequenceScope(),
  transcripts: createTranscriptCache(),
  cacheStorage: storage,
  databaseSuffix: "cache-browser-witness",
});
transport = createResourceEvents({
  api: {
    get: async () => {
      throw new Error("unexpected GET");
    },
    post: async () => {
      throw new Error("unexpected POST");
    },
    clearSchemaUpdateAttemptAfterMatch: () => {},
    receiveStreamSession: () => {},
    clearStreamSession: () => {},
    isApiSchemaMismatch: () => false,
    matchingApiSchemaResponseGeneration: () => 0,
    markApiSchemaMismatch: () => {},
    onApiSchemaMismatch: () => () => {},
  },
  sync: client,
  tokenRates: createTokenRateCache(),
  onCachePolicies: client.setCachePolicies,
  onCachePoliciesReset: client.clearCachePolicies,
  openEvents: (url) => new EventSource(url),
});
transport.watchResourceChanges(ref, (version) => {
  if (!version) return;
  document.querySelector("output")!.textContent = String(version.revision);
});
// A returning app reads the state projection, as the application does.
observeGeneratedState(
  client,
  (payload) => {
    document.querySelector("data")!.textContent = JSON.stringify(
      serializeStateCollections(payload),
    );
  },
  () => {},
);
const key = JSON.stringify([workspaceId, subscriptionKey(ref)]);
declare global {
  interface Window {
    cacheWitness: { read(): Promise<unknown>; corrupt(): Promise<void> };
  }
}
window.cacheWitness = {
  read: () => storage.get(key),
  corrupt: () => storage.put(key, { version: -1 }),
};
