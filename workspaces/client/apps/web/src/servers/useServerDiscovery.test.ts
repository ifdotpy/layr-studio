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
vi.mock("../sync/uiPreferenceStore.ts", () => ({
  readPreferenceFields: () => mocks.fields,
  capturePreferenceWrite: vi.fn(),
}));
vi.mock("./registry.ts", () => ({
  readServers: () => [],
  writeServers: () => {},
  setLocalServerDetails: mocks.setLocal,
  serveOrigin: (origin: string) => origin,
}));
vi.mock("./trustedServers.ts", () => ({
  adoptTrustedServers: (snapshot: unknown) => mocks.adopt(snapshot),
}));
vi.mock("./managementRequests.ts", () => ({
  ServerManagementRequests: class {
    pending() {
      return [];
    }
  },
}));
import { ServerDiscovery } from "./useServerDiscovery.ts";
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
  const services = {
    api: { serverAccess: mocks.access, refreshSession: vi.fn() },
    store: {
      multiServer: () => ({
        current: mocks.snapshot
          ? { status: "live" as const, data: mocks.snapshot }
          : { status: "loading" as const },
      }),
    },
  };
  const empty = new ServerDiscovery(services, true, true, () => ({}));
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
  const view = new ServerDiscovery(services, true, true, () => ({}));
  expect(view.snapshot?.localLabel).toBe("Owner");
  cleanups.push(view.start());
  await Promise.resolve();
  expect(mocks.access).not.toHaveBeenCalled();
  expect(mocks.adopt).toHaveBeenCalledWith(
    expect.objectContaining({ localServerId: "owner" }),
  );
  expect(mocks.setLocal).toHaveBeenCalledWith({ label: "Owner", alias: "OWN" });
  mocks.fields = { '["server-alias:workspace"]': { value: "NEW" } };
  const updated = new ServerDiscovery(services, true, true, () => ({
    local: "workspace",
  }));
  expect(updated.snapshot?.aliases?.local).toBe("NEW");
});
