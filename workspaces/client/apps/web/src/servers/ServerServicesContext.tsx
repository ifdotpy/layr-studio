import * as defaultUploads from "../sync/uploads";
import { defaultServerAudio } from "../native/serverAudio";
import * as defaultRecordings from "../dictation/storage";
import * as defaultLocalDrafts from "../sync/localDraft";
import { serverStorageEventKey } from "./storage";
import * as defaultChatCreations from "../chatCreation";
import { serverLocalStorage } from "./storage";
import * as defaultFileActions from "../fileActions";
import * as defaultFileDialogs from "../native/fileDialogs";
import { watchResourceReads } from "../sync/subscriptions/reads";
import * as defaultActivity from "./activity";
import { useSyncedDrafts } from "../sync/drafts";
import * as defaultOutbox from "../sync/send";
import {
  createContext,
  useContext,
  useState,
  useEffect,
  Fragment,
  type ReactNode,
} from "react";
import { localSidebarServices } from "../components/sidebar/services";
import * as defaultApi from "../api";
import * as defaultSync from "../sync/client";
import { defaultSubscriptions as defaultResources } from "../sync/subscriptions/events";
import * as defaultTranscripts from "../sync/transcriptCache";
import * as defaultTokenRates from "../usage/tokenRate";
import type { ServerServices } from "./serverServices";
import { createServerStore } from "../store/index.ts";

const Context = createContext<ServerServices | null>(null);
const StoreContext = createContext<ReturnType<typeof createServerStore> | null>(
  null,
);
export const ServerStoreProvider = StoreContext.Provider;

/** An isolated server-view window keeps its existing transport and credential owner. */
export function ServerViewStoreProvider({ children }: { children: ReactNode }) {
  const [store] = useState(() => createServerStore(defaultResources));
  useEffect(() => () => store.dispose(), [store]);
  return <ServerStoreProvider value={store}>{children}</ServerStoreProvider>;
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
export const useServerTranscripts = () =>
  useServerServices()?.transcripts ?? defaultTranscripts;
export const useServerTokenRates = () =>
  useServerServices()?.tokenRates ?? defaultTokenRates;

export const useServerOutbox = () =>
  useServerServices()?.outbox ?? defaultOutbox;

export function useServerDrafts() {
  const services = useServerServices();
  const useDrafts = services?.useDrafts ?? useSyncedDrafts;
  return useDrafts();
}

export const useServerActivityScope = () =>
  useServerServices()?.activity ?? defaultActivity;

export const useServerResourceReads = () =>
  useServerServices()?.watchResourceReads ?? watchResourceReads;

export const useServerFileDialogs = () =>
  useServerServices()?.fileDialogs ?? defaultFileDialogs;

export const useServerFileActions = () =>
  useServerServices()?.fileActions ?? defaultFileActions;

export const useServerStorage = () =>
  useServerServices()?.storage ?? serverLocalStorage;

export const useServerChatCreations = () =>
  useServerServices()?.chatCreations ?? defaultChatCreations;

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
