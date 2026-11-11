import { ProviderKind } from "../../../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { preferenceMemory } from "../../../../servers/storage.ts";
import { createUiPreferenceBridge } from "../../../../sync/uiPreferenceBridge.ts";
import {
  preferenceStorageForServer,
  readPreferenceFields,
  receivePreferenceFields,
} from "../../../../sync/uiPreferenceStore.ts";
import type { syncPost } from "../../../../api.ts";
import { ModelField } from "../ModelField/ModelField.tsx";
import { ModelWhitelistSettings } from "../ModelWhitelistSettings/ModelWhitelistSettings.tsx";
import { toModelOptions } from "./modelOption.ts";
import { writeModelChoices } from "./modelChoices.ts";

const fixture = vi.hoisted(() => ({
  storage: undefined as Storage | undefined,
}));
const models = [
  { model: "astra", provider: ProviderKind["codex"], displayName: "Astra" },
  { model: "luna", provider: ProviderKind["codex"], displayName: "Luna" },
] satisfies Parameters<typeof toModelOptions>[0];
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStorage: () => {
      if (!fixture.storage) throw new Error("Missing fixture storage");
      return fixture.storage;
    },
    useServerOwner: () => undefined,
  }),
);
vi.mock("../../../agents/support/WorkerModelPicker.tsx", () => ({
  useWorkerModels: () => ({
    models,
    loading: false,
    error: "",
    retry: vi.fn(),
  }),
}));
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  preferenceMemory.clear();
  fixture.storage = preferenceStorageForServer("local");
});
afterEach(() => {
  vi.unstubAllGlobals();
});

it("updates a mounted picker after whitelist writes and received preferences with unchanged props", async () => {
  const host = document.createElement("div"),
    root = createRoot(host);
  const props = {
    label: "Models",
    options: toModelOptions(models),
    value: "astra",
    onChange: vi.fn(),
    retainCurrent: false,
  };
  try {
    await act(async () =>
      root.render(
        createElement(MantineProvider, null, createElement(ModelField, props)),
      ),
    );
    expect(
      host.querySelector('[role="option"][data-value="luna"]'),
    ).not.toBeNull();
    await act(async () => {
      writeModelChoices({ "codex:luna": "blacklisted" }, fixture.storage);
    });
    expect(host.querySelector('[role="option"][data-value="luna"]')).toBeNull();
    const key = '["studio-model-choices-v1"]';
    const previous = readPreferenceFields("server", fixture.storage)[key];
    if (!previous) throw new Error("Missing preference write");
    await act(async () => {
      receivePreferenceFields(
        "server",
        {
          [key]: { ...previous, timestamp: previous.timestamp + 1, value: {} },
        },
        fixture.storage,
      );
    });
    expect(
      host.querySelector('[role="option"][data-value="luna"]'),
    ).not.toBeNull();
  } finally {
    await act(async () => root.unmount());
  }
});

it("shows a delayed preference save failure and its Retry button", async () => {
  if (!fixture.storage) throw new Error("Missing fixture storage");
  let reject!: (failure: Error) => void;
  const post = vi
    .fn<typeof syncPost<"/api/sync/preferences">>()
    .mockImplementation(
      () =>
        new Promise((_resolve, fail) => {
          reject = fail;
        }),
    );
  const bridge = createUiPreferenceBridge(fixture.storage, post);
  const stop = bridge.start();
  bridge.receiveProjection({});
  const host = document.createElement("div"),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ModelWhitelistSettings),
        ),
      ),
    );
    const clear = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Clear selection",
    );
    if (!clear) throw new Error("Missing clear selection action");
    await act(async () => clear.click());
    expect(post).toHaveBeenCalledTimes(1);
    expect(
      [...host.querySelectorAll("button")].some(
        (button) => button.textContent === "Retry saving",
      ),
    ).toBe(false);
    await act(async () => {
      reject(new Error("Delayed preference write failed"));
    });
    expect(host.querySelector('[role="alert"]')?.textContent).toContain(
      "Delayed preference write failed",
    );
    expect(
      [...host.querySelectorAll("button")].some(
        (button) => button.textContent === "Retry saving",
      ),
    ).toBe(true);
  } finally {
    await act(async () => root.unmount());
    stop();
  }
});
