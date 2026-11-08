import "../../src/rendererStyles.ts";
import { createDataObserver } from "../../src/sync/subscriptions/data.ts";
import { createSubscriptions } from "../../src/sync/subscriptions/module.ts";
import { createServerStore } from "../../src/store/server.ts";
import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import ClaudeSignIn from "../../src/components/accounts/ClaudeSignIn/ClaudeSignIn.tsx";
import type { ServerServices } from "../../src/servers/serverServices.ts";
import { ServerServicesProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type {
  ResourceProgress,
  ResourceRef,
  ResourceVersion,
} from "../../src/sync/subscriptions/contracts.ts";
import { fixtureElement } from "./browser-helpers.ts";

const reads = { facts: 0, auth: 0, jobs: 0 };
window.reads = reads;
const selectors: ResourceRef[] = (window.selectors = []);
const listeners = new Map<string, Set<(version?: ResourceVersion) => void>>();
const resources = {
  watchResourceChanges(
    resource: ResourceRef,
    accept: (version?: ResourceVersion) => void,
  ) {
    const key = JSON.stringify(resource);
    const callbacks = listeners.get(key) ?? new Set();
    listeners.set(key, callbacks);
    callbacks.add(accept);
    selectors.push(resource);
    return () => {
      callbacks.delete(accept);
    };
  },
  watchResourceConnection(
    accept: Parameters<
      ServerServices["resources"]["watchResourceConnection"]
    >[0],
  ) {
    accept("live", "open", {});
    return () => {};
  },
};
const api = {
  get: async () => {
    reads.auth++;
    throw new Error("unexpected auth read");
  },
  saved: (key: string, fallback: unknown) =>
    key === "claude-sign-in:fixture:claude-one" ? "login-one" : fallback,
  save: () => {},
  stableRequestId: () => "",
  clearStableRequestId: () => {},
};
const subscriptions = createSubscriptions(
  resources,
  createDataObserver(
    () => ({ subscribeTranscriptProjection: () => () => {} }),
    resources,
  ),
);
const store = createServerStore(subscriptions);
// This one-off fixture omits unused service adapters and API methods.
const services: Partial<Record<keyof ServerServices, unknown>> = {
  owner: { id: "local", origin: location.origin },
  api,
  resources: subscriptions,
  store,
};
const account = {
  id: "claude-one",
  label: "Claude",
  provider: "claude",
  status: "signedOut",
} as React.ComponentProps<typeof ClaudeSignIn>["account"];
// The spec supplies the correlated resource/value pair at this fixture boundary.
const progress = (
  resource: ResourceProgress["resource"],
  value: ResourceProgress["value"],
) => {
  const version: ResourceVersion = {
    epoch: "fixture",
    revision: 1,
    progress: { resource, value } as ResourceProgress,
  };
  for (const accept of listeners.get(JSON.stringify(resource)) ?? [])
    accept(version);
};
window.progress = progress;
createRoot(fixtureElement("root")).render(
  <MantineProvider>
    <ServerServicesProvider services={services as ServerServices}>
      <ClaudeSignIn
        account={account}
        scope="fixture"
        onClose={() => {}}
        onReady={() => {
          window.ready = true;
        }}
      />
    </ServerServicesProvider>
  </MantineProvider>,
);
declare global {
  interface Window {
    reads: typeof reads;
    selectors: typeof selectors;
    progress: typeof progress;
    ready?: boolean;
  }
}
