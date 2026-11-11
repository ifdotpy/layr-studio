import { createServerStore } from "../../src/store/server.ts";
import { defaultSubscriptions } from "../../src/sync/subscriptions/events.ts";
import "../../src/rendererStyles.ts";
import { FixtureAreas } from "../fixtures/owner-support.tsx";

import { cacheFixture } from "../cacheFixture.ts";
import { ServerStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import React from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import TerminalDock from "../../src/components/shell/TerminalDock/TerminalDock.tsx";
import { setToken } from "../../src/servers/net/api.ts";
let disposeStore = () => {};
window.addEventListener("pagehide", () => disposeStore(), { once: true });
export function mount(shell: string) {
  window.reactRoot?.unmount();
  disposeStore();
  localStorage.setItem("codex.terminal.open", cacheFixture(true));
  localStorage.setItem(
    "codex.terminal.selected",
    cacheFixture("shell:" + shell),
  );
  setToken("fixture-token");
  const store = createServerStore(defaultSubscriptions);
  disposeStore = store.dispose;
  const root = createRoot(document.getElementById("app")!);
  root.render(
    React.createElement(
      MantineProvider,
      null,
      React.createElement(
        ServerStoreProvider,
        { value: store },
        React.createElement(FixtureAreas, {
          selectedId: "agent",
          notify: (message: string) => window.notices.push(message),
          children: React.createElement(TerminalDock),
        }),
      ),
    ),
  );
  window.reactRoot = root;
}

window.terminalInputRecoveryMount = mount;
