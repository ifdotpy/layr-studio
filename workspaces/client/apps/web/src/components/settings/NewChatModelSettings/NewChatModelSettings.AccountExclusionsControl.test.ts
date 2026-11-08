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
  isModelEnabled: () => true,
}));

test("new chat renders the shared chooser with lead and exclusion handlers", () => {
  const fixture = observableFixture();
  try {
    renderToStaticMarkup(
      createElement(
        fixture.Provider,
        null,
        createElement(
          MantineProvider,
          null,
          createElement(NewChatModelSettings, {
            projectId: "p",
            path: "/project",
            onChange: () => {},
          }),
        ),
      ),
    );
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
