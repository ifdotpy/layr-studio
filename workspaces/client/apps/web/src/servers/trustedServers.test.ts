import { AccessClientStatus } from "../generated/api";
import { beforeEach, expect, it, vi } from "vitest";
import type { DiscoveredServer, DiscoverySnapshot } from "./discoveryModel";
import type { StudioServer } from "./registry";

const mocks = vi.hoisted(() => ({
  saved: [] as StudioServer[],
  excluded: new Set<string>(),
  trusted: vi.fn(),
  written: [] as StudioServer[][],
}));
vi.mock("./browserCredentials", () => ({ trustedServer: mocks.trusted }));
vi.mock("./excludedServers", () => ({
  excludedServers: () => mocks.excluded,
}));
vi.mock("./registry", () => ({
  readServers: () => mocks.saved,
  writeServers: (servers: StudioServer[]) => mocks.written.push(servers),
}));

import { adoptTrustedServers, trustedCandidates } from "./trustedServers";

function peer(
  id: string,
  patch: Partial<DiscoveredServer> = {},
): DiscoveredServer {
  return {
    id,
    label: id,
    origin: `https://${id}.tail.ts.net`,
    status: AccessClientStatus.paired,
    lastSeen: null,
    paired: true,
    publicKey: `key-${id}`,
    generation: `gen-${id}`,
    ...patch,
  } as DiscoveredServer;
}

beforeEach(() => {
  mocks.saved = [];
  mocks.excluded = new Set();
  mocks.trusted.mockReset();
  mocks.written = [];
});

it("lists a paired, not revoked server that this UI does not have", () => {
  const picked = trustedCandidates([peer("remote")], [], new Set());
  expect(picked.map((row) => row.id)).toEqual(["remote"]);
});

it("does not list a server that is not paired, revoked, removed or already saved", () => {
  const peers = [
    peer("unpaired", { paired: false, status: "discovered" }),
    peer("revoked", { status: "revoked" }),
    peer("removed"),
    peer("saved"),
  ];
  const saved = [{ id: "saved" } as StudioServer];
  expect(
    trustedCandidates(peers, saved, new Set(["removed"])).map((row) => row.id),
  ).toEqual([]);
});

it("adds each trusted server with the device key and no pairing request", async () => {
  mocks.trusted.mockImplementation(async (value: DiscoveredServer) => ({
    id: value.id,
    label: value.label,
    origin: value.origin,
    credentialId: `${value.id}:device`,
  }));
  const snapshot = {
    localServerId: "owner",
    autoPair: true,
    autoUpdate: true,
    servers: [peer("remote"), peer("stranger", { paired: false })],
  } as DiscoverySnapshot;
  await adoptTrustedServers(snapshot);
  expect(mocks.trusted).toHaveBeenCalledOnce();
  expect(mocks.trusted.mock.calls[0]?.[0]).toMatchObject({ id: "remote" });
  expect(mocks.written.at(-1)?.map((row) => row.id)).toEqual(["remote"]);
});
