import { ProjectServerSelection } from "../../../app/projects/CreationOwner.ts";
// @vitest-environment happy-dom
import { NewChatModelStore } from "./NewChatModelStore.ts";
import { createServerServices } from "../../../servers/serverServices.ts";
import { localServer } from "../../../servers/registry.ts";
import { createElement } from "react";
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
import { observableFixture } from "../../shell/integration-support/observableFixture.tsx";
import NewChatModelSettings from "./NewChatModelSettings.tsx";

const chooser = vi.hoisted(() => vi.fn());
vi.mock("../../agents/ExecutionSettings/ExecutionSettings.tsx", () => ({
  ExecutionSettings: (props: unknown) => {
    chooser(props);
    return null;
  },
  settingsFor: () => ({
    model: null,
    effort: null,
    fast_mode: false,
    daybreak_enabled: false,
  }),
}));
vi.mock("../../agents/support/WorkerModelPicker.tsx", async (load) => ({
  ...(await load<
    typeof import("../../agents/support/WorkerModelPicker.tsx")
  >()),
  useWorkerModels: () => ({
    models: [],
    loading: false,
    error: "",
    retry: () => {},
  }),
}));
vi.mock("../models/support/modelChoices.ts", () => ({
  useModelChoices: () => ({}),
  readModelChoices: () => ({}),
  isModelEnabled: () => true,
}));

test("new chat renders the shared chooser with lead and exclusion handlers", () => {
  const fixture = observableFixture();
  try {
    const services = createServerServices(localServer());
    renderToStaticMarkup(
      createElement(
        fixture.Provider,
        null,
        createElement(
          MantineProvider,
          null,
          createElement(NewChatModelSettings, {
            model: new NewChatModelStore(
              services,
              "p",
              "/project",
              services.areas,
            ),
          }),
        ),
      ),
    );
    services.close();
    expect(chooser).toHaveBeenCalledWith(
      expect.objectContaining({
        workerExcludedAccountKeys: [],
        reviewerExcludedAccountKeys: [],
        onWorkerExcludedAccountKeysChange: expect.any(Function),
        onReviewerExcludedAccountKeysChange: expect.any(Function),
        onAccountChange: expect.any(Function),
      }),
    );
  } finally {
    fixture.store.dispose();
  }
});

test("foreign selection prepares its owners before rendering settings", () => {
  const connection = {
    id: "foreign",
    label: "Foreign",
    origin: "https://foreign.test",
  };
  const services = createServerServices(connection);
  const areas = vi.spyOn(services, "areas", "get");
  const owner = new ProjectServerSelection(
    {
      id: "p",
      name: "P",
      path: "/project",
      locations: [{ serverId: "foreign", projectId: "p", path: "/project" }],
    },
    () => [{ id: "foreign", label: "Foreign", connection }],
    "foreign",
    () => undefined,
    () => services,
  );
  try {
    const beforeRender = areas.mock.calls.length;
    const model = owner.model;
    if (!model) throw new Error("Missing foreign draft");
    renderToStaticMarkup(
      createElement(
        MantineProvider,
        null,
        createElement(NewChatModelSettings, { model }),
      ),
    );
    expect(areas).toHaveBeenCalledTimes(beforeRender);
    expect(beforeRender).toBe(1);
  } finally {
    owner.dispose();
  }
});
