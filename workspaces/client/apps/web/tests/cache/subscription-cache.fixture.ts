import { createSyncClient } from "../../src/sync/client";
import { createResourceEvents } from "../../src/sync/resourceEvents";
import { createEntitySequenceScope } from "../../src/sync/entitySequence";
import { createTranscriptCache } from "../../src/sync/transcriptCache";
import { indexedDb } from "../../src/indexedDb";
import { subscriptionKey } from "../../src/sync/subscriptions/contracts";

const workspaceId = "b".repeat(32);
const ref = { kind: "state" } as const;
const storage = indexedDb("cache-browser-witness", "subscriptions");
let transport: ReturnType<typeof createResourceEvents>;
const client = createSyncClient({
  api: {
    syncGet: async () => (await fetch("/api/sync/identity")).json(),
    syncPost: async () => {
      throw new Error("unexpected request");
    },
    saved: (_key, fallback) => fallback,
    save: () => {},
    setWorkspace: () => {},
    isApiSchemaMismatch: () => false,
  } as unknown as Parameters<typeof createSyncClient>[0]["api"],
  resources: {
    watchResourceChanges: (...args) => transport.watchResourceChanges(...args),
    watchResourceConnection: (...args) =>
      transport.watchResourceConnection(...args),
  },
  sequences: createEntitySequenceScope(),
  transcripts: createTranscriptCache(),
  cacheStorage: storage,
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
  } as unknown as Parameters<typeof createResourceEvents>[0]["api"],
  sync: client,
  tokenRates: {
    configureTokenRateStream: () => {},
    receiveResourceTokenRates: () => true,
    getWorkspaceTokenRates: () => ({ records: [] }),
  } as unknown as Parameters<typeof createResourceEvents>[0]["tokenRates"],
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
