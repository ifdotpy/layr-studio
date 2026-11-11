// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { action, makeAutoObservable } from "mobx";
import { workerCatalogKey } from "../../../agents/support/WorkerModelPicker.tsx";
import ConversationSettings from "./ConversationSettings.tsx";
import { ServerAreasContextProvider } from "../../../../app/ServerAreasProvider.tsx";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  conversationActionsFixture,
  observableFixture,
} from "../../../shell/integration-support/observableFixture.tsx";
import { resourceData } from "../../../../store/runtime.ts";
import { agentSummaryFixture } from "../../../../servers/agentSummaryFixture.ts";
vi.mock("mobx", async (original) => {
  const actual = await original<typeof import("mobx")>();
  return { ...actual, makeAutoObservable: vi.fn(actual.makeAutoObservable) };
});
vi.mock(
  "../../../agents/UnifiedAgentSettings/UnifiedAgentSettings.tsx",
  () => ({ UnifiedAgentSettings: () => null }),
);

it.each(["mount", "rerender", "account change"])(
  "allocates no catalog stores during %s",
  async (stage) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    const f = observableFixture([
      agentSummaryFixture("lead", { isLead: true }),
      agentSummaryFixture("other", { accountKey: "next" }),
    ]);
    let accountKey = "default";
    const areas = conversationActionsFixture(() => {}, f.store);
    const agent = resourceData(f.store.state().current)!.agents.get("lead")!;
    const host = document.createElement("div");
    const root = createRoot(host);
    const render = () =>
      createElement(
        ServerAreasContextProvider,
        { value: areas },
        createElement(
          ServerStoreProvider,
          { value: f.store },
          createElement(ConversationSettings, {
            agent: { ...agent, accountKey },
          }),
        ),
      );
    try {
      if (stage !== "mount") await act(async () => root.render(render()));
      vi.mocked(makeAutoObservable).mockClear();
      if (stage === "account change") accountKey = "next";
      await act(async () => root.render(render()));
      expect(makeAutoObservable).not.toHaveBeenCalled();
    } finally {
      await act(async () => root.unmount());
      areas.dispose();
      areas.scope.services.close();
      f.store.dispose();
      vi.unstubAllGlobals();
    }
  },
);

it("disposes the catalog of an account removed from the active server", () => {
  const f = observableFixture([
    agentSummaryFixture("lead", { isLead: true, accountKey: "next" }),
  ]);
  const areas = conversationActionsFixture(undefined, f.store);
  try {
    const catalog = areas.agentsUi.workerCatalogs.get(
      workerCatalogKey("next"),
    )!;
    action(() => f.apply([]))();
    expect(catalog).toHaveProperty("active", false);
  } finally {
    areas.dispose();
    areas.scope.services.close();
    f.store.dispose();
  }
});

it("retains a removed catalog only until its in-flight retry settles", async () => {
  const f = observableFixture([
    agentSummaryFixture("lead", { isLead: true, accountKey: "next" }),
  ]);
  const areas = conversationActionsFixture(undefined, f.store);
  const pending =
    Promise.withResolvers<
      Awaited<ReturnType<typeof areas.scope.services.modelReader.read>>
    >();
  const read = vi
    .spyOn(areas.scope.services.modelReader, "read")
    .mockReturnValue(pending.promise);
  try {
    const key = workerCatalogKey("next");
    const catalog = areas.agentsUi.workerCatalogs.get(key)!;
    const retry = catalog.retry();
    action(() => f.apply([]))();
    expect(catalog.active).toBe(true);
    expect(areas.agentsUi.workerCatalogs.get(key)).toBe(catalog);
    pending.resolve({ data: [] });
    await retry;
    expect(catalog.active).toBe(false);
    expect(areas.agentsUi.workerCatalogs.has(key)).toBe(false);
    await catalog.retry();
    expect(read).toHaveBeenCalledTimes(1);
  } finally {
    areas.dispose();
    areas.scope.services.close();
    f.store.dispose();
  }
});
