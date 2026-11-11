const fixtureServerStore = createServerStore(defaultSubscriptions);
window.addEventListener("pagehide", () => fixtureServerStore.dispose(), {
  once: true,
});
import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import "../../src/rendererStyles.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { refreshState } from "../../src/sync/client.ts";
declare global {
  interface Window {
    notices: string[];
  }
}

import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "../../src/style.css";
import { BackgroundTasksFixture as BackgroundTasks } from "../fixtures/owner-support.tsx";
// The isolated subtree uses the same singleton bootstrap as App.
await refreshState();
createRoot(document.getElementById("root")!).render(
  React.createElement(
    MantineProvider,
    { defaultColorScheme: "dark" },
    React.createElement(
      ServerStoreProvider,
      { value: fixtureServerStore },
      React.createElement(BackgroundTasks, {
        opened: true,
        close: () => {},
        leadId: "lead",
        initialFocus: { id: "monitor", leadId: "lead", requestId: "focus" },
        openAgent: () => {},
        notify: (message) => (window.notices ??= []).push(message),
      }),
    ),
  ),
);
