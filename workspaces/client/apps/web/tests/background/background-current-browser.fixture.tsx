import "../../src/rendererStyles.ts";
import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import {
  createStateMirror,
  createRequestsMirror,
} from "../../src/generated/observableResources.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { agentSummaryFixture } from "../../src/servers/agentSummaryFixture.ts";
import type { components } from "../../src/generated/api.ts";

type Focus = React.ComponentProps<typeof BackgroundTasks>["initialFocus"];
declare global {
  interface Window {
    renderFixture: (
      data: Snapshot,
      listing: components["schemas"]["ProcessListing"],
      leadId?: string,
      focus?: Focus,
    ) => void;
    refreshCalls: number;
    holdRefresh: boolean;
    resolveRefresh: () => void;
    rejectRefresh: (reason: Error) => void;
    notifications: string[];
  }
}

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "../../src/style.css";
import { ServerServicesProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createServerServices } from "../../src/servers/serverServices.ts";
import BackgroundTasks from "../../src/components/shell/tasks/BackgroundTasks/BackgroundTasks.tsx";
const services = createServerServices({
  id: "local",
  label: "Fixture",
  origin: location.origin,
});
await services.ready();
const root = createRoot(document.getElementById("root")!);
const model = createStateMirror();
const requests = createRequestsMirror();
const store: typeof services.store = {
  ...services.store,
  state: () => ({ current: { status: "live", data: model.current } }),
  requests: () => ({ current: { status: "live", data: requests.current } }),
};
window.renderFixture = (data, listing, leadId = "lead", initialFocus) => {
  model.apply(
    data.threads.map((agent) => ({
      collection: "agentSummary" as const,
      id: agent.id,
      value: agentSummaryFixture(agent.id, {
        name: agent.name ?? agent.id,
        rootId: agent.rootId ?? agent.id,
        epoch: agent.epoch ?? 0,
        accountKey: agent.accountKey ?? "default",
        inFlight: agent.inFlight ?? false,
      }),
    })),
  );
  requests.apply(
    data.runtime.requests.map((value) => ({
      collection: "request" as const,
      id: value.id,
      value,
    })),
  );
  services.resources.publish({ kind: "processes", agentId: leadId }, listing);
  root.render(
    React.createElement(
      MantineProvider,
      { defaultColorScheme: "dark" },
      React.createElement(ServerServicesProvider, {
        services,
        children: React.createElement(ServerStoreProvider, {
          value: store,
          children: React.createElement(BackgroundTasks, {
            opened: true,
            close: () => {},
            leadId,
            initialFocus,
            openAgent: () => {},
            refresh: async () => {
              window.refreshCalls = (window.refreshCalls || 0) + 1;
              window.dispatchEvent(new Event("fixture-refresh"));
              if (window.holdRefresh)
                await new Promise<void>((resolve, reject) => {
                  window.resolveRefresh = resolve;
                  window.rejectRefresh = reject;
                });
            },
            notify: (error) => {
              (window.notifications ??= []).push(error);
              window.dispatchEvent(new Event("fixture-notification"));
            },
          }),
        }),
      }),
    ),
  );
};

document.getElementById("root")!.dataset.ready = "true";
