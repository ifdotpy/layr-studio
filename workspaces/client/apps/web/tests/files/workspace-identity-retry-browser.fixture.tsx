declare global {
  interface Window {
    workspace: string | undefined;
    workspaceIdentityRetryBrowserMount: typeof mount;
  }
}
window.workspaceIdentityRetryBrowserMount = mount;

import React from "react";
import { createRoot } from "react-dom/client";
import { useStartupFixture } from "../sync/startup-fixture.ts";
export function mount() {
  createRoot(document.body).render(
    React.createElement(function Harness() {
      window.workspace = useStartupFixture().startup.workspaceId;
      return null;
    }),
  );
}
