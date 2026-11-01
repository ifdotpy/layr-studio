import { syncGet } from "../../src/api";
import { createSyncClient } from "../../src/sync/client";
import { createResourceEvents } from "../../src/sync/resourceEvents";
import { createEntitySequenceScope } from "../../src/sync/entitySequence";
import { createTranscriptCache } from "../../src/sync/transcriptCache";
import { indexedDb } from "../../src/indexedDb";
import { createTokenRateCache } from "../../src/usage/tokenRate";
import { subscriptionKey } from "../../src/sync/subscriptions/contracts";

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
    saved: (_key, fallback) => fallback,
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
  requestAnimationFrame(() =>
    requestAnimationFrame(() => void client.cacheSubscription(ref)),
  );
});
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
