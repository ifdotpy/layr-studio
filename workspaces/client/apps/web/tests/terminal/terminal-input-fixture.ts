import { cacheFixture } from "../cacheFixture.ts";
import { ServerViewStoreProvider } from "../../src/servers/ServerServicesContext.tsx";
import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import TerminalDock from "../../src/components/TerminalDock.tsx";
import { setToken } from "../../src/api.ts";
export function mount(shell: string) {
  window.reactRoot?.unmount();
  localStorage.setItem("codex.terminal.open", cacheFixture(true));
  localStorage.setItem(
    "codex.terminal.selected",
    cacheFixture("shell:" + shell),
  );
  setToken("fixture-token");
  const root = createRoot(document.getElementById("app")!);
  root.render(
    React.createElement(
      MantineProvider,
      null,
      React.createElement(
        ServerViewStoreProvider,
        null,
        React.createElement(TerminalDock, {
          agentId: "agent",
          notify: (value) => window.notices.push(value),
        }),
      ),
    ),
  );
  window.reactRoot = root;
}

window.terminalInputRecoveryMount = mount;
