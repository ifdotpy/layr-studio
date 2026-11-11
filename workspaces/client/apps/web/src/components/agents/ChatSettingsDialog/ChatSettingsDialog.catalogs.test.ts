// @vitest-environment happy-dom
import { act, createElement, Fragment } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { ChatSettingsDialog } from "./ChatSettingsDialog.tsx";
import { ServerAreasContextProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
const h = vi.hoisted(() => ({
  catalogs: [] as ReturnType<
    typeof import("../support/WorkerModelPicker.tsx").createWorkerCatalog
  >[],
}));
vi.mock("../ExecutionSettings/ExecutionSettings.tsx", async (original) => ({
  ...(await original<
    typeof import("../ExecutionSettings/ExecutionSettings.tsx")
  >()),
  DelegationSettings: ({
    catalog,
  }: {
    catalog: (typeof h.catalogs)[number];
  }) => {
    h.catalogs.push(catalog);
    return null;
  },
  ExecutionPermissions: () => null,
}));
vi.mock("../SubagentConcurrencyControl/SubagentConcurrencyControl.tsx", () => ({
  default: () => null,
}));
vi.mock("@mantine/core", async (original) => {
  const actual = await original<typeof import("@mantine/core")>();
  return {
    ...actual,
    Modal: Object.assign(
      ({ children }: { children: React.ReactNode }) =>
        createElement(Fragment, null, children),
      actual.Modal,
    ),
  };
});
it("retains pending and failed model retries across ChatSettingsDialog rerenders", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = observableFixture([agentSummaryFixture("lead", { isLead: true })]);
  const areas = conversationActionsFixture(undefined, f.store);
  areas.navigation.selectExternal("lead", { requestId: "open" });
  const pending =
    Promise.withResolvers<
      Awaited<ReturnType<typeof areas.scope.services.modelReader.read>>
    >();
  const read = vi
    .spyOn(areas.scope.services.modelReader, "read")
    .mockReturnValue(pending.promise);
  const root = createRoot(document.createElement("div"));
  const render = () =>
    createElement(
      MantineProvider,
      { env: "test" },
      createElement(
        ServerAreasContextProvider,
        { value: areas },
        createElement(ChatSettingsDialog, {
          areas,
          accounts: {
            accounts: [],
            archivedAccounts: [],
            defaultAccountKey: "default",
            defaultAccountGroups: [],
            supportsDelete: true,
            supportsDisconnect: true,
          },
          settings: {
            chatOpen: true,
            accountModalOpen: false,
            closeChatSettings: () => {},
          },
          onSignIn: () => {},
        }),
      ),
    );
  try {
    await act(async () => root.render(render()));
    const catalog = h.catalogs.at(-1)!;
    const retry = catalog.retry();
    await act(async () => root.render(render()));
    expect(h.catalogs.at(-1)).toBe(catalog);
    expect(read).toHaveBeenCalledTimes(1);
    pending.reject(new Error("retry failure"));
    await act(async () => retry);
    await act(async () => root.render(render()));
    expect(h.catalogs.at(-1)).toBe(catalog);
    expect(catalog.error).toBe("retry failure");
  } finally {
    await act(async () => root.unmount());
    areas.dispose();
    areas.scope.services.close();
    f.store.dispose();
    vi.unstubAllGlobals();
  }
});
