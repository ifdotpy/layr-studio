import { createElement } from "react";
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
import NewChatModelSettings from "../NewChatModelSettings";

const chooser = vi.hoisted(() => vi.fn());
vi.mock("./ExecutionSettings", () => ({
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
vi.mock("./WorkerModelPicker", async (load) => ({
  ...(await load<typeof import("./WorkerModelPicker")>()),
  useWorkerModels: () => ({
    models: [],
    loading: false,
    error: "",
    retry: () => {},
  }),
}));
vi.mock("../models/modelChoices", () => ({
  useModelChoices: () => ({}),
  isModelEnabled: () => true,
}));

test("new chat renders the shared chooser with lead and exclusion handlers", () => {
  renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(NewChatModelSettings, {
        projectId: "p",
        path: "/project",
        onChange: () => {},
      }),
    ),
  );
  expect(chooser).toHaveBeenCalledWith(
    expect.objectContaining({
      excludedAccountKeys: [],
      onExcludedAccountKeysChange: expect.any(Function),
      onAccountChange: expect.any(Function),
    }),
  );
});
