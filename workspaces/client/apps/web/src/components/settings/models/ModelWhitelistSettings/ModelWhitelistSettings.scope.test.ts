// @vitest-environment happy-dom
import { act, createElement, createContext, useContext } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { ModelWhitelistSettings } from "./ModelWhitelistSettings.tsx";

let storage: Storage;
const StorageContext = createContext<Storage | null>(null);
const write = vi.hoisted(() => vi.fn());
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStorage: () => useContext(StorageContext),
    useServerOwner: () => null,
  }),
);
vi.mock("../../../../servers/ServerSettingsContext.tsx", () => ({
  useServerSettings: () => null,
}));
vi.mock("../../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({
    models: [
      {
        model: "fixture",
        provider: "codex",
        supportedReasoningEfforts: [],
        serviceTiers: [],
      },
    ],
    loading: false,
    error: "",
  }),
}));
vi.mock("../../../../sync/uiPreferenceStatus.ts", () => ({
  readPreferenceWriteStatus: () => ({ state: "saved" }),
  retryPreferenceWrites: () => {},
}));
vi.mock("../support/modelChoices.ts", () => ({
  isModelEnabled: () => true,
  modelChoiceKey: () => "codex:fixture",
  readModelChoices: () => ({}),
  useModelChoices: () => ({}),
  writeModelChoices: write,
}));

it("writes model changes to the current server storage after a mounted rerender", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"),
    root = createRoot(host);
  const render = () =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            StorageContext.Provider,
            { value: storage },
            createElement(ModelWhitelistSettings),
          ),
        ),
      ),
    );
  try {
    storage = localStorage;
    await render();
    storage = sessionStorage;
    await render();
    const select = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Choose all models",
    );
    expect(select).toBeDefined();
    await act(async () => select?.click());
    expect(write).toHaveBeenCalledWith(
      { "codex:fixture": "whitelisted" },
      sessionStorage,
    );
    expect(write.mock.calls[0]?.[1]).toBe(sessionStorage);
  } finally {
    await act(async () => root.unmount());
    vi.unstubAllGlobals();
  }
});
