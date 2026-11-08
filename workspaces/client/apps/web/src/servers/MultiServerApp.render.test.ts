// @vitest-environment happy-dom
import { StrictMode, act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { configure, spy } from "mobx";
import { expect, it, vi } from "vitest";
import type { CombinedSidebarServices } from "./combinedSidebarServices.ts";
import type { DiscoverySnapshot } from "./discoveryModel.ts";

const captured = vi.hoisted(() => ({
  source: undefined as CombinedSidebarServices | undefined,
  snapshot: null as DiscoverySnapshot | null,
  firstBackends: [] as string[],
}));
vi.mock("../App", async (original) => ({
  ...(await original<typeof import("../App")>()),
  default: () => null,
}));
vi.mock("../app/ServerAreasProvider.tsx", async (original) => ({
  ...(await original<typeof import("../app/ServerAreasProvider.tsx")>()),
  ServerAreasProvider: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("./ServerAccessSettings", () => ({ default: () => null }));
vi.mock("./useUiPreferenceSync", () => ({ useUiPreferenceSync() {} }));
vi.mock("./useServerDiscovery", () => ({
  // Like the real hook, every render derives a fresh snapshot object.
  useServerDiscovery: () => ({
    snapshot: captured.snapshot && {
      ...captured.snapshot,
      aliases: { ...captured.snapshot.aliases },
      servers: [...captured.snapshot.servers],
    },
    error: "",
    busy: false,
    pending: null,
    actions: { find() {}, setAutoPair() {}, setAutoUpdate() {} },
  }),
}));
vi.mock("./registry", async (original) => ({
  ...(await original<typeof import("./registry")>()),
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
vi.mock("./CombinedServerSidebar", async (original) => {
  const actual = await original<typeof import("./CombinedServerSidebar")>();
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
import MultiServerApp from "./MultiServerApp.tsx";

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
  const render = () =>
    root.render(
      createElement(
        StrictMode,
        null,
        createElement(MantineProvider, null, createElement(MultiServerApp)),
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
    captured.snapshot = {
      localServerId: "local",
      aliases: { remote: "NEW" },
      autoPair: false,
      autoUpdate: false,
      servers: [],
    };
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
    stop();
    warnings.mockRestore();
    vi.unstubAllGlobals();
  }
});

it("opens settings with streamed running counts and accounts without a summary request", async () => {
  const { observableFixture } =
    await import("../components/observableFixture.tsx");
  const serverServices = await import("./serverServices.ts");
  const { createServerServices } = serverServices;
  const { AgentSource, AccountStatus, ProviderKind } =
    await import("../generated/api.ts");
  const summary = await import("./summary.ts");
  const read = vi
    .spyOn(summary, "fetchServerSummary")
    .mockRejectedValue(new Error("Unexpected summary read"));
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  captured.snapshot = null;
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
      await import("../generated/observableResources.ts")
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
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(MultiServerApp),
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
    service.close();
    f.store.dispose();
    read.mockRestore();
    create.mockRestore();
    vi.unstubAllGlobals();
  }
});
