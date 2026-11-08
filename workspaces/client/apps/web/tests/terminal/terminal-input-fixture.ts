import "../../src/rendererStyles.ts";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../src/store/runtime.ts";
import { cacheFixture } from "../cacheFixture.ts";
import { ServerViewStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import TerminalDock from "../../src/components/shell/TerminalDock/TerminalDock.tsx";
import { setToken } from "../../src/api.ts";
const Terminal = observer(function Terminal() {
  const agent = resourceData(useServerStore().state().current)?.agents.get(
    "agent",
  );
  return React.createElement(TerminalDock, {
    agent,
    notify: (value) => window.notices.push(value),
  });
});
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
        React.createElement(Terminal),
      ),
    ),
  );
  window.reactRoot = root;
}

window.terminalInputRecoveryMount = mount;
