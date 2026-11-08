import { ProviderKind } from "../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement, useState } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import {
  ServerSettingsContext,
  useServerSettings,
} from "../../servers/ServerSettingsContext.tsx";
import type { StudioSettingsTab } from "../../components/StudioSettingsTabs.tsx";
import { createScreenChannel } from "../../servers/screenChannel.ts";
import { observer } from "mobx-react-lite";
import { makeAutoObservable } from "mobx";
import { SettingsUi } from "./useAppSettings.ts";
function useAppSettings(screen: ConstructorParameters<typeof SettingsUi>[0]) {
  const source = useServerSettings();
  const [settings] = useState(() => new SettingsUi(screen, source));
  return settings;
}
const stops: (() => void)[] = [];
beforeEach(() => vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true));
afterEach(async () => {
  await act(async () => stops.splice(0).forEach((stop) => stop()));
  vi.unstubAllGlobals();
});
async function fixture(external = false) {
  const publish = vi.fn();
  let current!: ReturnType<typeof useAppSettings>,
    setTab!: (tab: StudioSettingsTab) => void;
  const screen = {
    active: true,
    channel: external ? null : createScreenChannel("server-a", publish),
  };
  const Probe = observer(function Probe() {
    current = useAppSettings(screen);
    return null;
  });
  const source = makeAutoObservable(
    {
      settingsOpen: false,
      settingsTab: "accounts" as StudioSettingsTab,
      setSettingsOpen(open: boolean) {
        this.settingsOpen = open;
      },
      setSettingsTab(tab: StudioSettingsTab) {
        this.settingsTab = tab;
      },
    },
    {},
    { autoBind: true },
  );
  setTab = source.setSettingsTab;
  function Host() {
    return createElement(
      ServerSettingsContext.Provider,
      { value: external ? source : null },
      createElement(Probe),
    );
  }
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  await act(async () => root.render(createElement(Host)));
  stops.push(() => {
    root.unmount();
    host.remove();
  });
  return {
    get current() {
      return current;
    },
    publish,
    setTab,
  };
}
test("external settings producer stays live without copying props", async () => {
  const f = await fixture(true);
  await act(async () => f.current.openSettings("appearance"));
  expect(f.current.studioOpen).toBe(true);
  expect(f.current.tab).toBe("appearance");
  await act(async () => f.setTab("hotkeys"));
  expect(f.current.tab).toBe("hotkeys");
  await act(async () => f.current.setStudioOpen(false));
  expect(f.current.studioOpen).toBe(false);
});
test("shell delegation and account dialog close are real transitions", async () => {
  const f = await fixture();
  await act(async () => f.current.openSettings("accounts"));
  expect(f.current.studioOpen).toBe(false);
  expect(f.publish).toHaveBeenLastCalledWith({
    kind: "studio-server-open-settings",
    serverId: "server-a",
    tab: "accounts",
  });
  await act(async () => {
    f.current.run({
      action: "account-sign-in",
      accountKey: "same",
      provider: ProviderKind.codex,
      email: null,
      label: "Same",
      serverLabel: "Fixture",
    });
    f.current.setChatOpen(true);
  });
  expect(f.current.studioOpen).toBe(true);
  await act(async () => {
    f.current.reportAccountDialog(true);
    f.current.reportAccountDialog(true);
    f.current.closeChatSettings();
  });
  expect(f.current.chatOpen).toBe(true);
  await act(async () => {
    f.current.reportAccountDialog(false);
    f.current.reportAccountDialog(false);
  });
  expect(f.current.studioOpen).toBe(false);
  expect(
    f.publish.mock.calls
      .filter(([message]) => message.kind === "studio-server-account-dialog")
      .map(([message]) => message.opened),
  ).toEqual([true, false]);
});
test("switching an open local section to accounts delegates to the shell", async () => {
  const f = await fixture();
  await act(async () => f.current.openSettings("appearance"));
  await act(async () => f.current.activateTab("accounts"));
  expect(f.current.studioOpen).toBe(false);
  expect(f.publish).toHaveBeenCalledOnce();
});

test("framed settings keep local tabs separate from the authoritative shell and rejoin when direct", () => {
  const source = new SettingsUi({ active: true, channel: null }, null);
  const screen: {
    active: boolean;
    channel: ReturnType<typeof createScreenChannel> | null;
  } = makeAutoObservable({
    active: true,
    channel: createScreenChannel("local", () => {}),
  });
  const local = new SettingsUi(screen, source);
  source.openSettings("servers");
  expect(local.studioOpen).toBe(false);
  local.openSettings("appearance");
  expect(local.studioOpen).toBe(true);
  expect(local.tab).toBe("appearance");
  expect(source.tab).toBe("servers");
  screen.channel = null;
  expect(local.tab).toBe("servers");
  local.setStudioOpen(false);
  expect(source.studioOpen).toBe(false);
});
