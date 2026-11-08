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
vi.mock("./useUiPreferenceSync", () => ({ useUiPreferenceSync() {} }));
vi.mock("./useServerDiscovery", () => ({
  useServerDiscovery: () => ({
    snapshot: captured.snapshot,
    error: "",
    busy: false,
    pending: null,
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
