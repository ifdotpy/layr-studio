import { AccessClientStatus, Reachability } from "../generated/api.ts";
import type { ConnectionHealth } from "../sync/connectionState.ts";
import { get, type GetResult } from "./net/api.ts";
import { z } from "zod";
import { ApiError } from "../api.ts";
import { requestOperation } from "./RequestRegistry.ts";
import type { ServerServices } from "./serverServices.ts";

import { readServers, type StudioServer } from "./registry.ts";

import type { DeepReadonly } from "../deepReadonly.ts";
import type { components } from "../generated/api.ts";
export type ProjectLocation = components["schemas"]["ProjectLocation"];
export type LocationProject = {
  id: string;
  path: string;
  name: string;
  workspacePolicy?: string | null | undefined;
  layrStoreRoot?: components["schemas"]["Project"]["layrStoreRoot"];
  layrStoreRootRevision?: components["schemas"]["Project"]["layrStoreRootRevision"];
  homeServerId?: string | null | undefined;
  compact?: boolean | undefined;
  locations?: ProjectLocation[] | null | undefined;
  projectAliases?:
    | { serverId: string; projectId: string; name: string }[]
    | null
    | undefined;
};
export type LocationProjectView = DeepReadonly<LocationProject>;
export type ProjectServerChoice = {
  id: string;
  label: string;
  disabled?: boolean;
  serverId?: string;
  system?: string | undefined;
  nativeLayr?: GetResult<"/api/ui-summary">["nativeLayr"] | undefined;
  nativeLayrMessage?: string | undefined;
  defaultWorkspaceMode?:
    | GetResult<"/api/ui-summary">["defaultWorkspaceMode"]
    | undefined;
  connection?: StudioServer | undefined;
};
export function projectServerChoices(
  servers: StudioServer[],
  statuses: Record<string, ConnectionHealth>,
  reachability: Record<string, string | null | undefined> = {},
): ProjectServerChoice[] {
  return servers.map((server) => ({
    id: server.id,
    connection: server,
    label: server.label,
    disabled:
      reachability[server.id] === "unreachable" ||
      ["offline", "degraded", "schema-mismatch"].includes(
        statuses[server.id] || "",
      ),
  }));
}
/** Explicit action compatibility; lifecycle consumers read the shared facade. */
export async function readProjectServers(
  read: typeof get = get,
): Promise<ProjectServerChoice[]> {
  return discoveryProjectServers(await read("/api/multi-server"));
}
export function discoveryProjectServers(
  snapshot: DeepReadonly<GetResult<"/api/multi-server">>,
): ProjectServerChoice[] {
  const connections = readServers();
  return [
    {
      id: "local",
      serverId: snapshot.identity.serverId,
      label: snapshot.identity.label,
    },
    ...snapshot.servers
      .filter(
        (server) =>
          server.status !== AccessClientStatus.discovered &&
          server.status !== AccessClientStatus.revoked,
      )
      .map((server) => ({
        id: server.id,
        connection: connections.find((row) => row.id === server.id),
        label: server.label,
        disabled:
          server.status === AccessClientStatus.unreachable ||
          server.reachability === Reachability.unreachable,
      })),
  ];
}
/** Registration and location changes have exact durable HTTP receipts. */
export const projectLocationOperation = requestOperation({
  path: "/api/projects",
  body: z.union([
    z.object({ path: z.string(), server: z.string().optional() }).strict(),
    z
      .object({
        action: z.enum(["add_location", "remove_location"]),
        project: z.string(),
        server: z.string(),
        path: z.string().optional(),
      })
      .strict(),
  ]),
  label: (body) =>
    "project" in body
      ? `Change project folders ${body.project}`
      : `Register project ${body.path}`,
  async send({ api }, body, entry) {
    const result = await api.post(
      "/api/projects",
      { ...body, request_id: entry.requestId },
      {
        requestId: entry.requestId,
      },
    );
    if (!("requestId" in result) || result.requestId !== entry.requestId)
      throw new TypeError("The project reply did not confirm this request.");
    if (result.outcome === "not_applied")
      throw new ApiError(
        result.error || "The project request was refused.",
        409,
      );
    if (result.outcome !== "applied")
      throw new TypeError(
        "The project result is unknown. Retry the saved request.",
      );
    return "applied";
  },
});

type LocationBody = Parameters<typeof projectLocationOperation.intent>[1];
type ProjectServices = Pick<ServerServices, "api" | "storage"> & {
  requests?: ServerServices["requests"] | undefined;
};
/** Resume an unresolved identity; a distinct action starts only after it settles. */
export function projectLocationRequest(
  services: ProjectServices,
  body: LocationBody,
) {
  const key = [
    "", // Locations belong to the server, independently of the selected workspace.
    "project-location",
    "project" in body ? body.project : body.path,
  ] as const;
  const intent = projectLocationOperation.intent(key, body);
  const { requests, storage } = services;
  if (!requests) throw new Error("The project request scope is unavailable.");
  let entry = requests.latest(key);
  const legacyKey = "studio-project-request:" + JSON.stringify(body);
  const legacy = storage.getItem(legacyKey);
  if (!entry && legacy) {
    entry = requests.restore(intent, legacy);
    storage.removeItem(legacyKey);
  }
  if (entry?.view.unresolved) {
    if (JSON.stringify(entry.body) !== JSON.stringify(intent.body))
      throw new Error(
        "Retry the pending project request before another change.",
      );
  } else entry = requests.start(intent);
  return entry;
}
export async function saveProjectLocation(
  services: ProjectServices,
  body: LocationBody,
) {
  const entry = projectLocationRequest(services, body);
  if (entry.view.canRetry) await services.requests!.retry(entry.requestId);
  else await entry.completion;
  if (entry.view.errorText) throw new Error(entry.view.errorText);
}

export function projectLocationOn(
  project: LocationProjectView,
  server: ProjectServerChoice,
) {
  return (
    project.locations?.find(
      (location) => location.serverId === (server.serverId || server.id),
    ) ||
    (server.id === "local"
      ? project.locations?.find(
          (location) => location.serverId === project.homeServerId,
        )
      : undefined) ||
    (!project.locations?.length && server.id === "local"
      ? {
          serverId: "local",
          path: project.path,
          projectId: project.id,
        }
      : undefined)
  );
}
