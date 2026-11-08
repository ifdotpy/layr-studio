// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { observable, observableRef, runInAction } from "mobx";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { settingsAreasFixture } from "./SettingsAreasFixture.ts";
import {
  loadStudioPreferences,
  useAppPreferences,
} from "./useAppPreferences.ts";
import { createUiPreferenceBridge } from "../../sync/uiPreferenceBridge.ts";
import { preferenceMemory } from "../../servers/storage.ts";
import {
  defaultStudioPreferences,
  studioPreferencesStorageKey,
} from "../../studioPreferences.ts";
import { readPreferenceWriteStatus } from "../../sync/uiPreferenceStatus.ts";
import type { syncPost as preferencePost } from "../../api.ts";
const stops: (() => void)[] = [];
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  preferenceMemory.clear();
});
afterEach(async () => {
  await act(async () => stops.splice(0).forEach((stop) => stop()));
  preferenceMemory.clear();
  vi.unstubAllGlobals();
});
async function fixture() {
  const screen = observable({ active: true, channel: null });
  const preferences = observable(
    { value: loadStudioPreferences(preferenceMemory).value },
    { value: observableRef },
  );
  const post = vi.fn<typeof preferencePost<"/api/sync/preferences">>(
    async (_path, body) => ({ fields: body.fields ?? {} }),
  );
  const bridge = createUiPreferenceBridge(preferenceMemory, post),
    stopBridge = bridge.start();
  bridge.receiveProjection({});
  let connection: (status: string) => void = () => {};
  const stopConnection = vi.fn();
  const areas = settingsAreasFixture({
    preferences,
    scope: {
      screen,
      services: {
        storage: preferenceMemory,
        resources: {
          watchResourceConnection(next: typeof connection) {
            connection = next;
            return stopConnection;
          },
        },
      },
    },
  });
  let current!: ReturnType<typeof useAppPreferences>;
  const Probe = observer(function Probe() {
    current = useAppPreferences(areas, bridge, () => {});
    return null;
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  await act(async () =>
    root.render(
      createElement(MantineProvider, { children: createElement(Probe) }),
    ),
  );
  const dispose = () => {
    root.unmount();
    host.remove();
    stopBridge();
  };
  stops.push(dispose);
  return {
    get current() {
      return current;
    },
    preferences,
    screen,
    post,
    bridge,
    connection: (status: string) => connection(status),
    stopConnection,
    dispose,
  };
}
test("invalid stored preferences recover to editable safe defaults", async () => {
  preferenceMemory.setItem(studioPreferencesStorageKey, "broken");
  const f = await fixture();
  expect(f.current.preferenceError).toContain("safe defaults");
  await act(async () => f.current.update({ theme: "dark" }));
  expect(f.current.preferenceError).toBe("");
  expect(f.preferences.value.theme).toBe("dark");
});
test("an edit normalizes invalid stored fields before merging", async () => {
  preferenceMemory.setItem(
    studioPreferencesStorageKey,
    JSON.stringify({ ...defaultStudioPreferences, mainFontSize: 999 }),
  );
  const f = await fixture();
  await act(async () =>
    f.current.update({ contentLayout: "custom", contentWidth: 80 }),
  );
  expect(f.preferences.value.mainFontSize).toBe(
    defaultStudioPreferences.mainFontSize,
  );
  expect(f.preferences.value.contentWidth).toBe(80);
});
test("a control edit retains a different field written before storage delivery", async () => {
  const f = await fixture();
  preferenceMemory.setItem(
    studioPreferencesStorageKey,
    JSON.stringify({ ...defaultStudioPreferences, mainFontSize: 18 }),
  );
  await act(async () => f.current.update({ theme: "dark" }));
  expect(f.preferences.value).toMatchObject({
    theme: "dark",
    mainFontSize: 18,
  });
});
test("lost writes retry exactly the same field versions", async () => {
  const f = await fixture();
  f.post.mockRejectedValueOnce(new TypeError("Lost response"));
  await act(async () =>
    f.current.update({ contentLayout: "custom", contentWidth: 80 }),
  );
  await vi.waitFor(() =>
    expect(readPreferenceWriteStatus(preferenceMemory, "user").state).toBe(
      "error",
    ),
  );
  await act(async () => f.bridge.retryPendingWrites());
  await vi.waitFor(() =>
    expect(readPreferenceWriteStatus(preferenceMemory, "user").state).toBe(
      "saved",
    ),
  );
  expect(f.post.mock.calls[1]).toEqual(f.post.mock.calls[0]);
});
test("only the active screen applies document preferences and disposal removes listeners", async () => {
  const f = await fixture();
  await act(async () =>
    runInAction(() => {
      f.screen.active = false;
    }),
  );
  const style = vi.spyOn(document.documentElement.style, "setProperty");
  style.mockClear();
  await act(async () =>
    f.current.update({ contentLayout: "custom", contentWidth: 80 }),
  );
  expect(style).not.toHaveBeenCalledWith("--studio-content-width-ratio", "0.8");
  await act(async () =>
    runInAction(() => {
      f.screen.active = true;
    }),
  );
  expect(style).toHaveBeenCalledWith("--studio-content-width-ratio", "0.8");
  await act(async () => f.dispose());
  stops.pop();
  expect(f.stopConnection).toHaveBeenCalledOnce();
  window.dispatchEvent(
    new StorageEvent("storage", {
      key: studioPreferencesStorageKey,
      newValue: JSON.stringify({ ...defaultStudioPreferences, theme: "light" }),
    }),
  );
  expect(f.preferences.value.contentWidth).toBe(80);
  style.mockRestore();
});
test("initial live and recovery retry pending writes; same health does not", async () => {
  const f = await fixture(),
    retry = vi.spyOn(f.bridge, "retryPendingWrites");
  f.connection("live");
  f.connection("live");
  expect(retry).toHaveBeenCalledOnce();
  f.connection("degraded");
  f.connection("live");
  expect(retry).toHaveBeenCalledTimes(2);
});
