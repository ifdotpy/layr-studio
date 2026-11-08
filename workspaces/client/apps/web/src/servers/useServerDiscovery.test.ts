import { afterEach, expect, it, vi } from "vitest";
import type { ServerAccessState } from "./accessContract.ts";
const mocks = vi.hoisted(() => ({
  effects: [] as (() => void | (() => void))[],
  snapshot: undefined as ServerAccessState | undefined,
  access: vi.fn(),
  adopt: vi.fn(async (_snapshot: unknown) => {}),
  setLocal: vi.fn(),
  fields: {} as Record<string, unknown>,
}));
vi.mock("react", () => ({
  useEffect: (effect: () => void | (() => void)) => {
    mocks.effects.push(effect);
  },
  useMemo: (read: () => unknown) => read(),
  useRef: (current: unknown) => ({ current }),
  useState: (current: unknown) => [current, vi.fn()],
}));
vi.mock("./ServerServicesContext", () => ({
  useServerApi: () => ({ serverAccess: mocks.access }),
  useServerServices: () => ({
    store: {
      multiServer: () => ({
        current: mocks.snapshot
          ? { status: "live", data: mocks.snapshot }
          : { status: "loading" },
      }),
    },
  }),
}));
vi.mock("../sync/uiPreferenceStore", () => ({
  readPreferenceFields: () => mocks.fields,
  capturePreferenceWrite: vi.fn(),
}));
vi.mock("./registry", () => ({
  readServers: () => [],
  writeServers: () => {},
  setLocalServerDetails: mocks.setLocal,
  serveOrigin: (origin: string) => origin,
}));
vi.mock("./trustedServers", () => ({
  adoptTrustedServers: (snapshot: unknown) => mocks.adopt(snapshot),
}));
vi.mock("./managementRequests", () => ({
  ServerManagementRequests: class {
    pending() {
      return [];
    }
  },
}));
import { useServerDiscovery } from "./useServerDiscovery.ts";
let cleanups: (() => void)[] = [];
afterEach(() => {
  cleanups.forEach((stop) => stop());
  cleanups = [];
  mocks.effects.length = 0;
  mocks.snapshot = undefined;
  mocks.fields = {};
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});
it("reads discovery from the shared facade and applies local registry details without HTTP", async () => {
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("location", { origin: "https://owner.test" });
  vi.stubGlobal("localStorage", {});
  const empty = useServerDiscovery(true);
  expect(empty.snapshot).toBeNull();
  mocks.snapshot = {
    protocol: 1,
    identity: {
      serverId: "owner",
      label: "Owner",
      origin: null,
      publicKey: "key",
      tailscaleUser: null,
    },
    clients: [],
    servers: [],
    invites: [],
    settings: { autoPair: true, aliases: { local: "OWN" } },
  };
  const view = useServerDiscovery(true);
  expect(view.snapshot?.localLabel).toBe("Owner");
  cleanups = mocks.effects.flatMap((effect) => {
    const stop = effect();
    return stop ? [stop] : [];
  });
  await Promise.resolve();
  expect(mocks.access).not.toHaveBeenCalled();
  expect(mocks.adopt).toHaveBeenCalledWith(
    expect.objectContaining({ localServerId: "owner" }),
  );
  expect(mocks.setLocal).toHaveBeenCalledWith({ label: "Owner", alias: "OWN" });
  mocks.fields = { '["server-alias:workspace"]': { value: "NEW" } };
  const updated = useServerDiscovery(true, true, { local: "workspace" });
  expect(updated.snapshot?.aliases?.local).toBe("NEW");
});
