// @vitest-environment happy-dom
import { StrictMode, act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { configure, spy, observable, action } from "mobx";
import { expect, it, vi } from "vitest";
import type { CombinedSidebarServices } from "../../../servers/combinedSidebarServices.ts";
import type { DiscoverySnapshot } from "../../../servers/discoveryModel.ts";

const captured = vi.hoisted(() => ({
  source: undefined as CombinedSidebarServices | undefined,
  firstBackends: [] as string[],
  realApp: false,
}));
const discoverySnapshot = observable.box<DiscoverySnapshot | null>(null, {
  deep: false,
});
const setDiscoverySnapshot = action((value: DiscoverySnapshot | null) =>
  discoverySnapshot.set(value),
);
vi.mock("../../shell/App/App.tsx", async (original) => {
  const actual = await original<typeof import("../../shell/App/App.tsx")>();
  return {
    ...actual,
    default: () => (captured.realApp ? createElement(actual.default) : null),
  };
});
vi.mock(
  "../../shell/ServerAreasProvider/ServerAreasProvider.tsx",
  async (original) => {
    const actual =
      await original<
        typeof import("../../shell/ServerAreasProvider/ServerAreasProvider.tsx")
      >();
    return {
      ...actual,
      ServerAreasProvider: ({
        areas,
        children,
      }: {
        areas: import("../../../app/contracts.ts").ServerAreas;
        children: ReactNode;
      }) =>
        captured.realApp
          ? createElement(actual.ServerAreasProvider, { areas, children })
          : children,
    };
  },
);
vi.mock("../ServerAccessSettings/ServerAccessSettings.tsx", () => ({
  default: () => null,
}));
vi.mock("../../../servers/useUiPreferenceSync.ts", () => ({
  startUiPreferenceSync: () => () => {},
}));
vi.mock("../../../servers/useServerDiscovery.ts", () => ({
  ServerDiscovery: class {
    get snapshot() {
      return discoverySnapshot.get();
    }
    start() {
      return () => {};
    }
    error = "";
    busy = false;
    pending = null;
    actions = { find() {}, setAutoPair() {}, setAutoUpdate() {} };
  },
}));
vi.mock("../../../servers/registry.ts", async (original) => ({
  ...(await original<typeof import("../../../servers/registry.ts")>()),
  readServers: () => [
    {
      id: "remote",
      origin: "https://remote.test",
      label: "Remote",
      credentialId: "first",
      alias: "REM",
    },
  ],
}));
vi.mock("../../../servers/CombinedServerSidebar.tsx", async (original) => {
  const actual =
    await original<
      typeof import("../../../servers/CombinedServerSidebar.tsx")
    >();
  return {
    ...actual,
    CombinedSidebarOwner: class {
      constructor(
        ...args: ConstructorParameters<typeof actual.CombinedSidebarOwner>
      ) {
        captured.source = args[1];
        captured.firstBackends.push([...args[1].backends.keys()].join(","));
        const owner = new actual.CombinedSidebarOwner(...args);
        return owner;
      }
    },
  };
});
import MultiServerApp, {
  ShellState,
  createShellState,
} from "./MultiServerApp.tsx";

it("commits complete sidebar connections before paint without MobX render or action-boundary warnings in StrictMode", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  configure({ enforceActions: "observed" });
  const warnings = vi.spyOn(console, "warn").mockImplementation(() => {});
  const renderWrites: string[] = [];
  const stop = spy((event) => {
    if (
      event.type === "update" &&
      event.observableKind === "object" &&
      ["backends", "aliases", "channel"].includes(String(event.name)) &&
      /mountMemo|updateMemo/.test(new Error().stack || "")
    )
      renderWrites.push(String(event.name));
  });
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const shell = new ShellState();
  const stopShell = shell.start();
  const render = () =>
    root.render(
      createElement(
        StrictMode,
        null,
        createElement(
          MantineProvider,
          null,
          createElement(MultiServerApp, { shell }),
        ),
      ),
    );
  try {
    await act(async () => render());
    expect(captured.firstBackends.length).toBeGreaterThan(0);
    expect(
      captured.firstBackends.every((value) => value === "local,remote"),
    ).toBe(true);
    expect(captured.source && [...captured.source.backends.keys()]).toEqual([
      "local",
      "remote",
    ]);
    expect(captured.source?.aliases.remote).toBe("REM");
    await act(async () =>
      setDiscoverySnapshot({
        localServerId: "local",
        aliases: { remote: "NEW" },
        autoPair: false,
        autoUpdate: false,
        servers: [],
      }),
    );
    await act(async () => render());
    expect(captured.source?.aliases.remote).toBe("NEW");
    expect(renderWrites).toEqual([]);
    expect(
      warnings.mock.calls.filter((args) =>
        String(args[0]).toLowerCase().includes("[mobx]"),
      ),
    ).toEqual([]);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    stopShell();
    stop();
    warnings.mockRestore();
    vi.unstubAllGlobals();
  }
});

it("opens settings with streamed running counts and accounts without a summary request", async () => {
  const { observableFixture } =
    await import("../../shell/integration-support/observableFixture.tsx");
  const serverServices = await import("../../../servers/serverServices.ts");
  const { createServerServices } = serverServices;
  const { AgentSource, AccountStatus, ProviderKind } =
    await import("../../../generated/api.ts");
  const summary = await import("../../../servers/net/summary.ts");
  const read = vi
    .spyOn(summary, "fetchServerSummary")
    .mockRejectedValue(new Error("Unexpected summary read"));
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  setDiscoverySnapshot(null);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  const f = observableFixture([
    { id: "one", source: AgentSource.managed, inFlight: true },
    { id: "two", source: AgentSource.managed, inFlight: true },
  ]);
  const account = {
    id: "work",
    provider: ProviderKind.codex,
    label: "Work",
    status: AccountStatus.ready,
    home: "",
    source: "fixture",
  };
  f.apply([
    ...(
      await import("../../../generated/observableResources.ts")
    ).serializeStateCollections(f.mirror.current),
    {
      collection: "accounts",
      id: "current",
      value: {
        accounts: [account],
        archivedAccounts: [],
        defaultAccountKey: "work",
        defaultAccountGroups: [],
        supportsDelete: true,
        supportsDisconnect: true,
      },
    },
  ]);
  const service = createServerServices({
    id: "local",
    label: "Local",
    origin: "http://localhost",
  });
  Object.defineProperty(service, "store", { value: f.store });
  const create = vi
    .spyOn(serverServices, "createServerServices")
    .mockImplementation((owner) =>
      owner.id === "local" ? service : createServerServices(owner),
    );
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const shell = new ShellState();
  const stopShell = shell.start();
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(MultiServerApp, { shell }),
        ),
      ),
    );
    await act(async () =>
      service.screen.channel?.publish({
        kind: "studio-server-navigation",
        navigation: {
          projects: [],
          chats: [],
          alerts: [],
          ready: true,
          busy: true,
          agentsRunning: 99,
          opened: null,
          error: "",
        },
      }),
    );
    const open = host.querySelector<HTMLButtonElement>(
      'button[aria-label="Studio settings"]',
    );
    if (!open) throw new Error("Missing settings action");
    await act(async () => open.click());
    const cards = [...document.querySelectorAll(".server-settings-facts")];
    const local = cards.find((card) => card.textContent?.includes("localhost"));
    expect(local?.textContent).toContain("Codex 1");
    expect(local?.textContent).toContain("Agents running2");
    expect(read).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    host.remove();
    stopShell();
    service.close();
    f.store.dispose();
    read.mockRestore();
    create.mockRestore();
    vi.unstubAllGlobals();
  }
});

it("retains unchanged service and channel owners, replaces changed credentials, and closes every owner", async () => {
  const { ShellState } = await import("./MultiServerApp.tsx");
  const recovery = await import("../../../servers/schemaRecovery.ts");
  const shell = new ShellState();
  let stop: (() => void) | undefined;
  try {
    shell.connectServices(undefined, false);
    const local = shell.serviceInstances.get("local")!;
    const remote = shell.serviceInstances.get("remote")!;
    const channel = shell.screens.get("remote")!;
    const closeRemote = remote.close;
    const close = vi
      .spyOn(remote, "close")
      .mockImplementation(() => closeRemote());
    const localClose = vi.spyOn(local, "close");
    expect(channel.ready).toBe(true);
    shell.connectServices(undefined, false);
    expect(shell.serviceInstances.get("remote")).toBe(remote);
    expect(shell.screens.get("remote")).toBe(channel);
    expect(close).not.toHaveBeenCalled();
    shell.set({
      paired: shell.paired.map((server) => ({
        ...server,
        credentialId: "replacement",
      })),
    });
    shell.connectServices(undefined, false);
    expect(close).toHaveBeenCalledExactlyOnceWith(true);
    expect(channel.ready).toBe(false);
    expect(shell.serviceInstances.get("remote") === remote).toBe(false);
    expect(shell.serviceInstances.get("local")).toBe(local);
    const replacement = shell.serviceInstances.get("remote")!;
    const replacementClose = vi.spyOn(replacement, "close");
    const revive = vi.spyOn(recovery, "recoverMatchingServers");
    stop = shell.start();
    shell.set({ settingsOpen: true });
    expect(revive).toHaveBeenCalledTimes(1);
    shell.set({ settingsTab: "servers" });
    expect(revive).toHaveBeenCalledTimes(2);
    window.dispatchEvent(new Event("focus"));
    expect(revive).toHaveBeenCalledTimes(3);
    stop();
    window.dispatchEvent(new Event("focus"));
    shell.set({ settingsOpen: false });
    shell.set({ settingsOpen: true });
    expect(revive).toHaveBeenCalledTimes(3);
    expect(localClose).toHaveBeenCalledExactlyOnceWith();
    expect(replacementClose).toHaveBeenCalledExactlyOnceWith();
    expect(shell.serviceInstances.size).toBe(0);
    expect(shell.screens.size).toBe(0);
  } finally {
    stop?.();
    shell.close();
    vi.restoreAllMocks();
  }
});

it.each(["credentials", "revival"])(
  "binds the real App to its replacement owner after %s changes",
  async (change) => {
    const module = await import("../../../servers/serverServices.ts");
    const create = module.createServerServices;
    const owners: ReturnType<typeof create>[] = [];
    const createSpy = vi
      .spyOn(module, "createServerServices")
      .mockImplementation((owner) => {
        const service = create(owner),
          close = service.close;
        vi.spyOn(service, "close").mockImplementation(() => close());
        owners.push(service);
        return service;
      });
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    window.matchMedia = vi.fn(() => ({
      matches: false,
      media: "",
      onchange: null,
      addListener() {},
      removeListener() {},
      addEventListener() {},
      removeEventListener() {},
      dispatchEvent: () => true,
    }));
    captured.realApp = true;
    setDiscoverySnapshot(null);
    const host = document.body.appendChild(document.createElement("div")),
      root = createRoot(host);
    const shell = new ShellState();
    const stopShell = shell.start();
    try {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            { env: "test" },
            createElement(MultiServerApp, { shell }),
          ),
        ),
      );
      const local = owners.find((service) => service.owner.id === "local")!;
      expect(local.shellSettings).toBe(shell);
      const previous = shell.serviceInstances.get("remote")!,
        channel = previous.screen.channel!;
      expect(channel.ready).toBe(true);
      await act(async () => {
        if (change === "credentials")
          shell.set({
            paired: shell.paired.map((server) => ({
              ...server,
              credentialId: "replacement",
            })),
          });
        else shell.set({ revived: { remote: 1 } });
      });
      const replacement = shell.serviceInstances.get("remote")!;
      expect(replacement).not.toBe(previous);
      expect(channel.ready).toBe(false);
      expect(replacement.screen.channel).not.toBe(channel);
      expect(replacement.screen.channel?.ready).toBe(true);
    } finally {
      await act(async () => root.unmount());
      host.remove();
      stopShell();
      captured.realApp = false;
      for (const owner of owners) owner.close();
      createSpy.mockRestore();
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
    }
  },
);

it("keeps shell connections alive while the document is cached and closes them when it leaves", () => {
  const shell = createShellState();
  const connected = shell.serviceInstances.size;
  expect(connected).toBeGreaterThan(0);
  const areas = [...shell.serviceInstances.values()].map(
    (service) => service.areas,
  );
  expect(areas.every((owner) => owner.stops.length > 0)).toBe(true);
  const cached = new Event("pagehide");
  Object.defineProperty(cached, "persisted", { value: true });
  window.dispatchEvent(cached);
  expect(shell.serviceInstances.size).toBe(connected);
  const leaving = new Event("pagehide");
  Object.defineProperty(leaving, "persisted", { value: false });
  window.dispatchEvent(leaving);
  expect(shell.serviceInstances.size).toBe(0);
  expect(
    areas.every((owner) => owner.disposed && owner.stops.length === 0),
  ).toBe(true);
});
