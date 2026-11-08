import { ServerViewStoreProvider } from "../../src/servers/ServerServicesContext.tsx";
import { refreshState } from "../../src/sync/client";
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
import BackgroundTasks from "../../src/components/shell/BackgroundTasks.tsx";
// The isolated subtree uses the same singleton bootstrap as App.
await refreshState();
createRoot(document.getElementById("root")!).render(
  React.createElement(
    MantineProvider,
    { defaultColorScheme: "dark" },
    React.createElement(
      ServerViewStoreProvider,
      null,
      React.createElement(BackgroundTasks, {
        opened: true,
        close: () => {},
        leadId: "lead",
        initialFocus: { id: "monitor", leadId: "lead", requestId: "focus" },
        openAgent: () => {},
        refresh: async () => {},
        notify: (message) => (window.notices ??= []).push(message),
      }),
    ),
  ),
);
