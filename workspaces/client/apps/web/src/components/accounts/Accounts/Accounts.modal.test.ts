// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { SidebarListOwner } from "../../../app/shell/SidebarOwner.ts";
import * as frameBridge from "../../../servers/frameBridge.ts";
import { expect, test, vi } from "vitest";
import Accounts from "./Accounts.tsx";
import { ProviderKind } from "../../../generated/api.ts";
import { accountsFixture } from "./Accounts.fixture.ts";
import { ServerAreasContextProvider as Provider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ServerServicesProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

test("keeps an account modal open across renders and closes its report with the owner", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  vi.spyOn(SidebarListOwner.prototype, "start").mockReturnValue(() => {});
  vi.spyOn(frameBridge, "startAppFrame").mockReturnValue(() => {});
  const command = {
    action: "account-sign-in" as const,
    forceAdd: true as const,
    provider: ProviderKind.codex,
    email: null,
    label: "New account",
    serverLabel: "Fixture",
  };
  const fixture = accountsFixture();
  const stop = fixture.areas.ui.start();
  fixture.areas.settings.setCommand(command);
  const render = () =>
    root.render(
      createElement(
        ServerServicesProvider,
        { services: fixture.areas.scope.services, children: null },
        createElement(
          Provider,
          { value: fixture.areas },
          createElement(
            MantineProvider,
            { env: "test" },
            createElement(Accounts),
          ),
        ),
      ),
    );
  try {
    await act(async () => render());
    expect(fixture.areas.settings.accountModalOpen).toBe(true);
    await act(async () => render());
    expect(fixture.areas.settings.accountModalOpen).toBe(true);
  } finally {
    await act(async () => root.unmount());
    stop();
    expect(fixture.areas.settings.accountModalOpen).toBe(false);
    fixture.close();
    vi.restoreAllMocks();
    host.remove();
    vi.unstubAllGlobals();
  }
});
