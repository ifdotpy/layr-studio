import { AccessClientStatus } from "../generated/api.ts";
import { ProviderKind } from "../generated/providerKind.ts";
import type { ConnectionHealth } from "../sync/connectionState.ts";
import type { DiscoveredServer, DiscoverySnapshot } from "./discoveryModel.ts";
import type { ServerAccount, ServerNavigation } from "./navigation.ts";
import type { StudioServer } from "./registry.ts";
import { localServer } from "./registry.ts";
import { defaultServerAlias } from "./serverAliases.ts";

export type ServerStatus =
  | "Active"
  | "Online"
  | "Paired"
  | "Discovered"
  | "Unreachable"
  | "Offline"
  | "Revoked"
  | "Update required";
type ServerRowDetails = {
  snapshot: DiscoverySnapshot | null;
  checkedAt: number | null;
  localEnabled: boolean;
  lastSeen?: Record<string, number>;
  navigation?: Record<string, ServerNavigation>;
  accountsByServer?: Record<string, ServerAccount[]>;
};
export function serverRows(
  servers: StudioServer[],
  peers: DiscoveredServer[],
  statuses: Record<string, ConnectionHealth>,
  details?: ServerRowDetails,
) {
  const rows = [
    ...servers,
    ...peers.filter((peer) => !servers.some((server) => server.id === peer.id)),
  ].map((server) => {
    const peer = peers.find((row) => row.id === server.id);
    const registered = servers.find((row) => row.id === server.id);
    const connection = statuses[server.id];
    const status: ServerStatus =
      peer?.status === AccessClientStatus.revoked
        ? "Revoked"
        : connection === "live"
          ? "Active"
          : connection === "offline" || connection === "degraded"
            ? "Offline"
            : connection === "schema-mismatch"
              ? "Update required"
              : server.id === "local"
                ? "Online"
                : peer?.status === AccessClientStatus.discovered
                  ? "Discovered"
                  : peer?.status === AccessClientStatus.unreachable
                    ? "Unreachable"
                    : "Paired";
    const local = server.id === "local";
    const state = ["Offline", "Unreachable", "Revoked"].includes(status)
      ? "error"
      : status === "Update required" || connection === "connecting"
        ? "paused"
        : ["Active", "Online"].includes(status)
          ? "working"
          : "none";
    const accounts = details?.accountsByServer?.[server.id] || [];
    const codexCount = accounts.filter(
      (account) => account.provider === ProviderKind.Codex,
    ).length;
    const summary = details?.navigation?.[server.id];
    const alias =
      details?.snapshot?.aliases?.[server.id] ||
      registered?.alias ||
      (local && details ? localServer().alias : undefined) ||
      defaultServerAlias(server);
    return {
      server,
      peer,
      registered,
      status,
      state,
      alias,
      name: local
        ? details?.snapshot?.localLabel || server.label
        : server.label,
      displayStatus:
        status === "Update required"
          ? "Other version"
          : local && state === "working"
            ? "This computer"
            : status,
      address: local
        ? details?.snapshot?.localOrigin || server.origin
        : server.origin,
      seen:
        peer?.lastSeen ??
        details?.lastSeen?.[server.id] ??
        (local ? (details?.checkedAt ?? null) : null),
      codexCount,
      claudeCount: accounts.length - codexCount,
      agentsRunning: summary?.agentsRunning ?? (summary?.busy ? 1 : 0),
      canRevoke: !!peer?.paired && !!details?.localEnabled,
      canAllow:
        peer?.status === AccessClientStatus.revoked &&
        !peer.replacedBy &&
        !!details?.localEnabled,
      editable: !!(registered || local || peer?.paired),
      canSaveName:
        !!(registered || (details?.localEnabled && peer?.paired)) &&
        status !== "Revoked",
      usedAliases: [
        ...servers
          .filter((row) => row.id !== server.id)
          .map((row) => row.alias || defaultServerAlias(row)),
        ...Object.entries(details?.snapshot?.aliases || {})
          .filter(
            ([id]) =>
              id !== server.id &&
              (!local || id !== details?.snapshot?.localServerId),
          )
          .map(([, alias]) => alias),
      ],
    };
  });
  const rank: Record<ServerStatus, number> = {
    Active: 1,
    Online: 1,
    Paired: 1,
    Discovered: 2,
    Unreachable: 3,
    Offline: 3,
    Revoked: 4,
    "Update required": 3,
  };
  return rows.sort(
    (a, b) =>
      (a.server.id === "local" ? 0 : rank[a.status]) -
        (b.server.id === "local" ? 0 : rank[b.status]) ||
      a.server.label.localeCompare(b.server.label),
  );
}
export function relativeSeen(seconds: number, now = Date.now() / 1000) {
  const elapsed = Math.max(0, now - seconds);
  if (elapsed < 60) return "just now";
  if (elapsed < 3600) return `${Math.floor(elapsed / 60)} min ago`;
  if (elapsed < 86400) return `${Math.floor(elapsed / 3600)} h ago`;
  return `${Math.floor(elapsed / 86400)} d ago`;
}
