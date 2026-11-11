import { createNet } from "./net/net.ts";
import { NetStatus } from "./net/status.ts";
import type { ServerSettings } from "./ServerSettingsContext.tsx";
import { ServerAreas } from "../app/ServerAreasProvider.tsx";
import { ScreenOwner } from "../app/startup/ScreenOwner.ts";
import { createModelReader } from "./modelReader.ts";
import { createSidebarServices } from "../components/sidebar/support/services.tsx";
import { serverSidebarBackend } from "./serverSidebarBackend.ts";
import { watchServerName } from "./serverNames.ts";
import { createUploads, UPLOAD_DATABASE_NAME } from "../sync/uploads.ts";
import { createServerAudio } from "../native/serverAudio.ts";
import { createRecordingStorage } from "../dictation/storage.ts";
import {
  createLocalDraftStorage,
  LOCAL_DRAFT_DATABASE_NAME,
} from "../sync/localDraft.ts";
import { storageNameForServer } from "./environment.ts";
import { serverStoragePrefix, createStorageEventKey } from "./storage.ts";
import { createChatCreationStore } from "../chatCreation.ts";
import { createFileActions } from "../fileActions.ts";
import { createFileDialogs } from "../native/fileDialogs.ts";
import { watchResourceReads } from "../sync/subscriptions/reads.ts";
import { createDrafts } from "../sync/drafts.ts";
import { createOutbox } from "../sync/send.ts";
import { createSyncClient } from "../sync/client.ts";
import { createServerSubscriptions } from "../sync/subscriptions/server.ts";
import { createServerStore } from "../store/server.ts";
import { createTranscriptCache } from "../sync/transcriptCache.ts";
import { createTokenRateCache } from "../usage/tokenRate.ts";
import { createEntitySequenceScope } from "../sync/entitySequence.ts";
import { createApiScope } from "./net/api.ts";
import { hasNativeClient } from "../native/client.ts";
import { nativeFetch } from "../native/http.ts";
import { preferenceStorageForServer } from "../sync/uiPreferenceStore.ts";
import { createMutationJournal } from "./mutationIdentity.ts";
import type { StudioServer } from "./registry.ts";
import { createServerTransport } from "./net/serverTransport.ts";
import { serverCredentialAdapter } from "./net/transport.ts";
import { scopedSessionOutbox } from "../browserPersistence.ts";
import { indexedDb } from "../indexedDb.ts";

/** Services belong to a server, never to the currently visible chat. */
export function createServerServices(server: StudioServer) {
  const transport = createServerTransport(server, (owner, request) => {
    if (owner.id === "local") {
      if (hasNativeClient()) return nativeFetch(request, owner.origin);
      if (!window.codexDesktop) return fetch(request);
    }
    return serverCredentialAdapter().fetch(owner, request);
  });
  const storage = preferenceStorageForServer(transport.owner.id);
  // The native core stores the projections of the local server; every other server keeps the page cache.
  const localNativeClient = () =>
    transport.owner.id === "local" && hasNativeClient();
  const sequences = createEntitySequenceScope();
  const cacheStorage = indexedDb(
    `studio-subscription-cache:${transport.owner.id}`,
    "subscriptions",
    1,
  );
  const netStatus = new NetStatus();
  const api = createApiScope({
    origin: transport.owner.origin,
    fetch: transport.fetch,
    requestStatus: netStatus,
    localStorage: storage,
    sessionStorage: scopedSessionOutbox(`:server:${transport.owner.id}:`),
    beginMutation: createMutationJournal(transport.owner.id, storage),
  });
  // The pending layer of this server ends when its own stream checkpoint reaches the response.
  const stopSequences = sequences.onEntitySequenceAdvance((sequence, reset) => {
    if (reset) api.pending.resetClientSeq();
    api.pending.observeClientSeq(sequence);
  });
  const audio = createServerAudio();
  const transcripts = createTranscriptCache();
  const tokenRates = createTokenRateCache();
  let runtime:
    | {
        sync: ReturnType<typeof createSyncClient>;
        resources: ReturnType<typeof createServerSubscriptions>;
        store: ReturnType<typeof createServerStore>;
      }
    | undefined;
  let stopName = () => {};
  let stopConnection = () => {};
  const synchronization = () => {
    if (runtime) return runtime;
    let sync!: ReturnType<typeof createSyncClient>;
    const resources = createServerSubscriptions({
      api,
      tokenRates,
      sync: () => sync,
      owner: transport.owner,
      nativeCore: localNativeClient,
    });
    sync = createSyncClient({
      api,
      resources,
      sequences,
      transcripts,
      onFreshReceipts: async (workspaceId, receipts) => {
        for (const receipt of receipts) {
          if (receipt.agent)
            await outbox.reconcileOutboxReceipts(
              receipt.agent,
              [receipt],
              workspaceId,
            );
        }
      },
      cacheStorage,
      databaseSuffix:
        transport.owner.id === "local"
          ? ""
          : "server" +
            Array.from(new TextEncoder().encode(transport.owner.id), (byte) =>
              byte.toString(16).padStart(2, "0"),
            ).join(""),
    });
    runtime = { sync, resources, store: createServerStore(resources) };
    stopConnection = resources.watchResourceConnection((health) =>
      netStatus.receive(health),
    );
    if (transport.owner.id !== "local")
      stopName = watchServerName(transport.owner, runtime.store);
    return runtime;
  };
  const outbox = createOutbox({
    api,
    serverId: transport.owner.id,
    watchResourceConnection: (...args) =>
      synchronization().resources.watchResourceConnection(...args),
  });
  let draftService: ReturnType<typeof createDrafts> | undefined;
  const localDrafts = createLocalDraftStorage(
    storage,
    storageNameForServer(transport.owner.id, LOCAL_DRAFT_DATABASE_NAME),
  );
  const uploads = createUploads({
    databaseName: storageNameForServer(
      transport.owner.id,
      UPLOAD_DATABASE_NAME,
    ),
    syncGet: api.syncGet,
    syncPost: api.syncPost,
    refreshSession: api.refreshSession,
    syncDatabase: () => synchronization().sync.syncDatabase(),
  });
  let initialization: Promise<void> | undefined;
  async function ready() {
    if (!initialization) {
      initialization = Promise.all([
        api.get("/api/sync/identity"),
        api.refreshSession(),
      ])
        .then(([identity]) => {
          api.setWorkspace(identity.workspaceId);
        })
        .catch((error) => {
          initialization = undefined;
          throw error;
        });
    }
    await initialization;
  }
  const net = createNet(api, () => synchronization().store, netStatus, {
    storage,
    updateQueueRequest: localDrafts.updateQueueRequest,
  });
  const stopSchema = api.onApiSchemaMismatch(() =>
    netStatus.receive("schema-mismatch"),
  );
  const screen = new ScreenOwner();
  let areas: ServerAreas | undefined;
  let sidebarServices: ReturnType<typeof createSidebarServices> | undefined;
  return {
    net,
    screen,
    shellSettings: undefined as ServerSettings | undefined,
    get areas(): ServerAreas {
      if (!areas || areas.disposed)
        areas = new ServerAreas({ services: this, store: this.store, screen });
      return areas;
    },
    get sidebarServices(): ReturnType<typeof createSidebarServices> {
      if (!sidebarServices) {
        const backend = serverSidebarBackend(this, () => true);
        sidebarServices = createSidebarServices(
          backend,
          [backend],
          () => backend.ownerId,
        );
      }
      return sidebarServices;
    },
    modelReader: createModelReader(transport.owner, (options) =>
      api.get("/api/models", options),
    ),
    owner: transport.owner,
    audio,
    chatCreations: createChatCreationStore(storage),
    recordings: createRecordingStorage(transport.owner.id),
    localDrafts,
    uploads,
    storageEventKey: createStorageEventKey(
      serverStoragePrefix(transport.owner.id),
    ),
    fileDialogs: createFileDialogs(transport.owner.id),
    fileActions: createFileActions(transport.owner.id),
    watchResourceReads: ((refs, read, failed, options) =>
      watchResourceReads(refs, read, failed, {
        ...options,
        events: synchronization().resources,
      })) satisfies typeof watchResourceReads,
    sequences,
    transcripts,
    outbox,
    get drafts() {
      return (draftService ??= createDrafts({
        serverId: transport.owner.id,
        api,
        resources: {
          watchResourceChanges: (...args) =>
            synchronization().resources.watchResourceChanges(...args),
          replaceStream: () => synchronization().resources.replaceStream(),
        },
      }));
    },
    tokenRates,
    get sync() {
      return synchronization().sync;
    },
    get resources() {
      return synchronization().resources;
    },
    get store() {
      return synchronization().store;
    },
    api,
    storage,
    ready,
    close(removeCache = false) {
      stopSchema();
      stopConnection();
      stopSequences();
      tokenRates.dispose();
      areas?.dispose();
      audio.close();
      stopName();
      runtime?.store.dispose();
      runtime?.resources.dispose();
      runtime?.sync.disposeCache();
      transport.close();
      if (removeCache) void cacheStorage.destroy();
    },
  };
}
export type ServerServices = ReturnType<typeof createServerServices>;
