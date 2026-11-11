// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { ExecutionSettings } from "./ExecutionSettings.tsx";
import { ProviderKind } from "../../../generated/api.ts";
import { createModelsMirror } from "../../../generated/observableResources.ts";
import { ServerAreasContextProvider } from "../../../app/ServerAreasProvider.tsx";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { agentSummaryFixture } from "../../../servers/agentSummaryFixture.ts";
import { resourceData } from "../../../store/runtime.ts";
import { workerCatalogKey } from "../support/WorkerModelPicker.tsx";

it("clicks Retry model list through the retained catalog flow", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const f = observableFixture([
    agentSummaryFixture("lead", { isLead: true, provider: ProviderKind.codex }),
  ]);
  const models = createModelsMirror();
  vi.spyOn(f.store, "models").mockReturnValue({
    current: { status: "live", data: models.current },
  });
  const areas = conversationActionsFixture(undefined, f.store);
  const read = vi
    .spyOn(areas.scope.services.modelReader, "read")
    .mockRejectedValueOnce(new Error("retry this catalog"))
    .mockResolvedValue({ data: [] });
  const catalog = areas.agentsUi.workerCatalogs.get(
    workerCatalogKey("default"),
  )!;
  await catalog.retry();
  const agent = resourceData(f.store.state().current)!.agents.get("lead")!;
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(
            ServerAreasContextProvider,
            { value: areas },
            createElement(
              ServerStoreProvider,
              { value: f.store },
              createElement(ExecutionSettings, {
                agentsUi: areas.agentsUi,
                input: { kind: "draft", draft: agent },
                catalog,
                presentation: "panel",
                settingsRow: true,
              }),
            ),
          ),
        ),
      ),
    );
    const button = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Retry model list",
    );
    expect(button).toBeDefined();
    await act(async () => button!.click());
    expect(read).toHaveBeenCalledTimes(2);
    expect(read).toHaveBeenLastCalledWith({
      query: { account_key: "default", workers: undefined, retry: "1" },
    });
  } finally {
    await act(async () => root.unmount());
    areas.dispose();
    areas.scope.services.close();
    f.store.dispose();
    vi.unstubAllGlobals();
  }
});
