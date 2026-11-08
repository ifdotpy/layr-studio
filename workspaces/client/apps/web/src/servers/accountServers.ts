import { AccessClientStatus } from "../generated/api.ts";
import type { DiscoveredServer } from "./discoveryModel.ts";
import type { StudioServer } from "./registry.ts";

export function accountServers(
  registered: StudioServer[],
  peers: DiscoveredServer[],
): StudioServer[] {
  return registered.filter(
    (server) =>
      !peers.some(
        (peer) =>
          peer.id === server.id && peer.status === AccessClientStatus.revoked,
      ),
  );
}
