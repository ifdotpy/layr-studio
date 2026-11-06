import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import ClaudeSignIn from "../../src/components/ClaudeSignIn.tsx";
import type { ServerServices } from "../../src/servers/serverServices.ts";
import { ServerServicesProvider } from "../../src/servers/ServerServicesContext.tsx";
import type {
  ResourceProgress,
  ResourceRef,
  ResourceVersion,
} from "../../src/sync/subscriptions/contracts.ts";
import { fixtureElement } from "./browser-helpers.ts";

const reads = { facts: 0, auth: 0, jobs: 0 };
window.reads = reads;
const selectors: ResourceRef[] = (window.selectors = []);
const listeners = new Map<string, (version?: ResourceVersion) => void>();
const resources = {
  watchResourceChanges(
    resource: ResourceRef,
    accept: (version?: ResourceVersion) => void,
  ) {
    const key = JSON.stringify(resource);
    listeners.set(key, accept);
    selectors.push(resource);
    return () => {
      listeners.delete(key);
    };
  },
  watchResourceConnection(
    accept: (
      state: Parameters<
        Parameters<ServerServices["resources"]["watchResourceConnection"]>[0]
      >[0],
    ) => void,
  ) {
    accept("live");
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
// This one-off fixture omits unused service adapters and API methods.
const services: Partial<Record<keyof ServerServices, unknown>> = {
  owner: { id: "local", origin: location.origin },
  api,
  resources,
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
) =>
  listeners.get(JSON.stringify(resource))?.({
    epoch: "fixture",
    revision: 1,
    progress: { resource, value } as ResourceProgress,
  });
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
