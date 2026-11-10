import "../../src/rendererStyles.ts";
import {
  ModelCatalogFixture,
  type ModelCatalogFixtureSource,
} from "../../src/components/servers/ServerServicesProvider/modelCatalogFixture.tsx";
import { ServerViewStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observer } from "mobx-react-lite";
import React from "react";
import { createRoot } from "react-dom/client";
import type { Root } from "react-dom/client";
import { flushSync } from "react-dom";
import { useWorkerModels } from "../../src/components/agents/support/WorkerModelPicker.tsx";
import { get } from "../../src/servers/net/api.ts";

type Reader = ModelCatalogFixtureSource;
export type ModelSnapshot = Parameters<
  Parameters<Reader["subscribeState"]>[0]
>[0];
declare global {
  interface Window {
    unmountCatalog: () => void;
    modelSnapshot: ModelSnapshot;
    modelGets: number;
    acceptModelSnapshot:
      | ((snapshot: ModelSnapshot) => void)
      | undefined
      | undefined;
    catalogResult: ReturnType<typeof useWorkerModels> | undefined;
    catalogRoot: Root | undefined | undefined;
    mountCatalog: (account?: string, workers?: boolean) => void;
    publishModelSnapshot: (snapshot: ModelSnapshot) => void;
  }
}

const reader: Reader = {
  scope: "fixture",
  cacheOwner: {},
  read: (options) => get("/api/models", options),
  subscribeState: (accept) => {
    window.acceptModelSnapshot = accept;
    accept(window.modelSnapshot);
    return () => {
      window.acceptModelSnapshot = undefined;
    };
  },
};
const Fixture = observer(function Fixture({
  account,
  workers,
}: {
  account: string;
  workers: boolean;
}) {
  const result = useWorkerModels(account, true, workers);
  window.catalogResult = result;
  return <pre>{JSON.stringify(result)}</pre>;
});
window.mountCatalog = (account = "default", workers = false) => {
  const element = document.getElementById("app")!;
  const root = (window.catalogRoot ||= createRoot(element));
  flushSync(() =>
    root.render(
      <ServerViewStoreProvider>
        <ModelCatalogFixture value={reader}>
          <Fixture account={account} workers={workers} />
        </ModelCatalogFixture>
      </ServerViewStoreProvider>,
    ),
  );
};
window.publishModelSnapshot = (snapshot) => {
  window.modelSnapshot = snapshot;
  window.acceptModelSnapshot?.(snapshot);
};

window.unmountCatalog = () => {
  flushSync(() => window.catalogRoot?.unmount());
  window.catalogRoot = undefined;
};
