// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, test, vi } from "vitest";
import Accounts from "./Accounts.tsx";
import { ProviderKind } from "../../../generated/api.ts";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";

test("changing the callback while an account modal is open does not report a close", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const first = vi.fn(),
    second = vi.fn();
  const state = {
    data: {
      accounts: [],
      archivedAccounts: [],
      logins: [],
      defaultAccountKey: "default",
      defaultAccountGroups: [],
      supportsDelete: true,
      supportsDisconnect: true,
    },
    scope: "fixture",
    error: "",
    setData() {},
    refresh: async () => null,
  };
  const command = {
    action: "account-sign-in" as const,
    forceAdd: true,
    provider: ProviderKind.codex,
    email: null,
    label: "New account",
    serverLabel: "Fixture",
  };
  const fixture = observableFixture();
  const render = (report: (open: boolean) => void) =>
    root.render(
      createElement(
        fixture.Provider,
        null,
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(Accounts, {
            managerOnly: true,
            state: state.data,
            scope: state.scope,
            onError() {},
            externalCommand: command,
            onModalOpenChange: report,
          }),
        ),
      ),
    );
  try {
    await act(async () => render(first));
    expect(first).toHaveBeenLastCalledWith(true);
    first.mockClear();
    await act(async () => render(second));
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenLastCalledWith(true);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
  expect(second).toHaveBeenLastCalledWith(false);
});
