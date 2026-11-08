import { AccessClientStatus } from "../generated/api.ts";
import type { DiscoveredServer, DiscoverySnapshot } from "./discoveryModel.ts";
import { trustedServer } from "./browserCredentials.ts";
import { excludedServers } from "./excludedServers.ts";
import { readServers, writeServers, type StudioServer } from "./registry.ts";

/** Peers that trust this device and that this UI does not list yet, minus removed ones. */
export function trustedCandidates(
  peers: DiscoveredServer[],
  saved: StudioServer[],
  excluded: Set<string>,
): DiscoveredServer[] {
  return peers.filter(
    (peer) =>
      peer.paired &&
      peer.status !== AccessClientStatus.revoked &&
      !excluded.has(peer.id) &&
      !saved.some((row) => row.id === peer.id),
  );
}

/** Lists each trusted server in this UI. Each one needs no pairing request. */
export async function adoptTrustedServers(
  snapshot: DiscoverySnapshot,
): Promise<void> {
  const candidates = trustedCandidates(
    snapshot.servers,
    readServers(),
    excludedServers(),
  );
  for (const peer of candidates) {
    const server = await trustedServer(peer);
    writeServers([
      ...readServers().filter((row) => row.id !== server.id),
      server,
    ]);
  }
}
