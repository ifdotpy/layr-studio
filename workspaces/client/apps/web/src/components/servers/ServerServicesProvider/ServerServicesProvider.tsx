import { createRequestRegistry } from "../../../renameRequest.ts";
import { defaultNetStatus } from "../../../servers/net/status.ts";
import { createNet } from "../../../servers/net/net.ts";
import type { ServerSettings } from "../../../servers/ServerSettingsContext.tsx";
import type { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
import type { AppScope } from "../../../app/contracts.ts";
import { ScreenOwner } from "../../../app/startup/ScreenOwner.ts";
import { createModelReader } from "../../../servers/modelReader.ts";
import { viewServer } from "../../../servers/registry.ts";
import { serverViewId } from "../../../servers/environment.ts";
import * as defaultSequences from "../../../sync/entitySequence.ts";
import * as defaultUploads from "../../../sync/uploads.ts";
import {
  createServerAudio,
  defaultServerAudio,
} from "../../../native/serverAudio.ts";
import * as defaultRecordings from "../../../dictation/storage.ts";
import * as defaultLocalDrafts from "../../../sync/localDraft.ts";
import { serverStorageEventKey } from "../../../servers/storage.ts";
import * as defaultChatCreations from "../../../chatCreation.ts";
import { serverLocalStorage } from "../../../servers/storage.ts";
import * as defaultFileActions from "../../../fileActions.ts";
import * as defaultFileDialogs from "../../../native/fileDialogs.ts";
import { watchResourceReads } from "../../../sync/subscriptions/reads.ts";
import { createDrafts } from "../../../sync/drafts.ts";
import * as defaultOutbox from "../../../sync/send.ts";
import { createContext, useContext, Fragment, type ReactNode } from "react";
import { localSidebarServices } from "../../sidebar/support/services.tsx";
import * as defaultApi from "../../../servers/net/api.ts";
import * as defaultSync from "../../../sync/client.ts";
import { defaultSubscriptions as defaultResources } from "../../../sync/subscriptions/events.ts";
import * as defaultTranscripts from "../../../sync/transcriptCache.ts";
import * as defaultTokenRates from "../../../usage/tokenRate.ts";
import type { ServerServices } from "../../../servers/serverServices.ts";
import { createServerStore } from "../../../store/server.ts";

const Context = createContext<ServerServices | null>(null);
const StoreContext = createContext<ReturnType<typeof createServerStore> | null>(
  null,
);
export const ServerStoreProvider = StoreContext.Provider;

export function createServerViewServices(
  createAreas: (scope: AppScope) => ServerAreas,
): ServerServices {
  const owner = viewServer(serverViewId || "local");
  const store = createServerStore(defaultResources);
  const net = createNet(defaultApi, () => store, defaultNetStatus, {
    storage: serverLocalStorage,
    updateQueueRequest: defaultLocalDrafts.updateQueueRequest,
  });
  const stopConnection = defaultResources.watchResourceConnection((health) =>
    net.status.receive(health),
  );
  const stopSchema = defaultApi.onApiSchemaMismatch(() =>
    net.status.receive("schema-mismatch"),
  );
  const screen = new ScreenOwner();
  const audio = createServerAudio();
  const drafts = createDrafts({
    api: defaultApi,
    resources: defaultResources,
    serverId: owner.id,
  });
  let areas: ServerAreas | undefined;
  const requests = createRequestRegistry(
    serverLocalStorage,
    defaultApi,
    () => store,
    (message) => areas?.notices.notify(message),
  );
  const services: ServerServices = {
    requests,
    owner,
    net,
    store,
    screen,
    shellSettings: undefined as ServerSettings | undefined,
    drafts,
    get areas() {
      return (areas ??= createAreas({ services, store, screen }));
    },
    api: defaultApi,
    sync: defaultSync.defaultSyncClient,
    resources: defaultResources,
    sidebarServices: localSidebarServices,
    modelReader: createModelReader(owner, (options) =>
      defaultApi.get("/api/models", options),
    ),
    audio,
    chatCreations: defaultChatCreations,
    recordings: defaultRecordings,
    localDrafts: defaultLocalDrafts,
    uploads: defaultUploads,
    storageEventKey: serverStorageEventKey,
    fileDialogs: defaultFileDialogs,
    fileActions: defaultFileActions,
    watchResourceReads,
    sequences: defaultSequences,
    transcripts: defaultTranscripts,
    outbox: defaultOutbox,
    tokenRates: defaultTokenRates,
    storage: serverLocalStorage,
    async ready() {
      await defaultApi.refreshSession();
    },
    close() {
      requests.dispose();
      stopConnection();
      stopSchema();
      areas?.dispose();
      store.dispose();
      audio.close();
    },
  };
  return services;
}

/** A chat/project subtree receives the services of its own resource owner. */
export function ServerServicesProvider({
  services,
  children,
}: {
  services: ServerServices;
  children: ReactNode;
}) {
  return (
    <Context.Provider value={services}>
      <Fragment
        key={JSON.stringify([
          services.owner.id,
          services.owner.origin,
          services.owner.credentialId,
        ])}
      >
        <ServerStoreProvider value={services.store}>
          {children}
        </ServerStoreProvider>
      </Fragment>
    </Context.Provider>
  );
}
export const useServerServices = () => useContext(Context);
export const useServerApi = () => useServerServices()?.api ?? defaultApi;
export function useServerSidebarServices() {
  const services = useServerServices();
  return services?.sidebarServices ?? localSidebarServices;
}
export const useServerModelReader = () => useServerServices()?.modelReader;
export const useServerSync = () => useServerServices()?.sync ?? defaultSync;
export const useServerResources = () =>
  useServerServices()?.resources ?? defaultResources;
export function useServerStore() {
  const store = useContext(StoreContext);
  if (!store)
    throw new Error(
      "The server store must be provided at the application root",
    );
  return store;
}
export const useServerTokenRates = () =>
  useServerServices()?.tokenRates ?? defaultTokenRates;

export const useServerOutbox = () =>
  useServerServices()?.outbox ?? defaultOutbox;

export const useServerFileDialogs = () =>
  useServerServices()?.fileDialogs ?? defaultFileDialogs;

export const useServerFileActions = () =>
  useServerServices()?.fileActions ?? defaultFileActions;

export const useServerStorage = () =>
  useServerServices()?.storage ?? serverLocalStorage;

export const useServerLocalDrafts = () =>
  useServerServices()?.localDrafts ?? defaultLocalDrafts;

export const useServerStorageEventKey = () =>
  useServerServices()?.storageEventKey ?? serverStorageEventKey;

export const useServerRecordings = () =>
  useServerServices()?.recordings ?? defaultRecordings;

export const useServerAudio = () =>
  useServerServices()?.audio ?? defaultServerAudio;

export const useServerUploads = () =>
  useServerServices()?.uploads ?? defaultUploads;

const defaultNets = new WeakMap<
  ReturnType<typeof createServerStore>,
  ReturnType<typeof createNet>
>();
export function useServerNet() {
  const services = useServerServices();
  const store = useServerStore();
  if (services) return services.net;
  let net = defaultNets.get(store);
  if (!net) {
    net = createNet(defaultApi, () => store, defaultNetStatus, {
      storage: serverLocalStorage,
      updateQueueRequest: defaultLocalDrafts.updateQueueRequest,
    });
    defaultNets.set(store, net);
  }
  return net;
}

export function useServerOwner() {
  return useServerServices()?.owner;
}
export function useServerPreferences() {
  const { saved, save } = useServerApi();
  return { saved, save };
}
export function useServerPending() {
  return useServerApi().pending;
}
