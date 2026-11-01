import type { components } from "../../src/generated/api.ts";
import type { Snapshot } from "../../src/types.ts";
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
import { ServerServicesProvider } from "../../src/servers/ServerServicesContext.tsx";
import { createServerServices } from "../../src/servers/serverServices.ts";
import BackgroundTasks from "../../src/components/shell/BackgroundTasks.tsx";
const services = createServerServices({
  id: "local",
  label: "Fixture",
  origin: location.origin,
});
await services.ready();
const root = createRoot(document.getElementById("root")!);
window.renderFixture = (data, listing, leadId = "lead", initialFocus) => {
  services.resources.publish({ kind: "processes", agentId: leadId }, listing);
  root.render(
    React.createElement(
      MantineProvider,
      { defaultColorScheme: "dark" },
      React.createElement(ServerServicesProvider, {
        services,
        children: React.createElement(BackgroundTasks, {
          opened: true,
          close: () => {},
          data,
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
    ),
  );
};

document.getElementById("root")!.dataset.ready = "true";
