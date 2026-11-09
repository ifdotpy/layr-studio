import type { components } from "../generated/api.ts";
import { isStudioSettingsTab } from "../components/settings/StudioSettingsTabs/StudioSettingsTabs.tsx";
import type { StudioSettingsTab } from "../components/settings/StudioSettingsLayout/StudioSettingsLayout.tsx";
import type { DesktopAlert } from "../desktop/desktopAlerts.ts";
import type {
  StateCollections,
  ProjectionMetadata,
} from "../generated/observableResources.ts";
import { saved } from "../api.ts";
import {
  isNewChatSettings,
  type NewChatSettings,
} from "../app/projects/newChatSettings.ts";
import type { LocationProject } from "./projectLocations.ts";
import type {
  SidebarAgent,
  SidebarChatView,
} from "../components/sidebar/support/viewModel.ts";
import { toJS } from "mobx";
import {
  sidebarSnapshot,
  type ServerSidebarSnapshot,
  type SidebarEntity,
} from "./sidebarSnapshot.ts";
import { isProviderKind, ProviderKind } from "../generated/providerKind.ts";
export type ServerAccount = {
  accountKey?: string;
  provider: ProviderKind;
  email: string | null;
  plan: string | null;
  status: string;
  label: string;
  isDefault: boolean;
};

/** Validate the public account snapshot before the server settings use it. */
export function readServerAccounts(value: unknown): ServerAccount[] | null {
  if (!Array.isArray(value) || value.length > 500) return null;
  const accounts: ServerAccount[] = [];
  for (const item of value) {
    if (!item || typeof item !== "object" || Array.isArray(item)) return null;
    const row = item as Record<string, unknown>;
    if (
      (row.provider !== "codex" && row.provider !== "claude") ||
      (row.email !== null && typeof row.email !== "string") ||
      (row.plan !== null && typeof row.plan !== "string") ||
      typeof row.status !== "string" ||
      typeof row.label !== "string" ||
      typeof row.isDefault !== "boolean" ||
      (row.accountKey !== undefined &&
        (typeof row.accountKey !== "string" || !row.accountKey))
    )
      return null;
    accounts.push({
      ...(row.accountKey === undefined ? {} : { accountKey: row.accountKey }),
      provider: row.provider,
      email: row.email,
      plan: row.plan,
      status: row.status,
      label: row.label,
      isDefault: row.isDefault,
    });
  }
  return accounts;
}
export type ServerChatSummary = {
  kind: "chat";
  id: string;
  path: string;
  agent: SidebarAgent;
  serverId?: string | null;
};
export type ServerRoomSummary = {
  kind: "room";
  id: string;
  name: string;
  path: string;
  archived: false;
  unread: boolean;
  serverId?: string | null;
};
export type ServerNavigation = {
  workspaceId?: string | undefined;
  sidebar?: ServerSidebarSnapshot;
  projects: (Omit<LocationProject, "id"> & { id?: string })[];
  chats: (ServerChatSummary | ServerRoomSummary)[];
  ready: boolean;
  busy?: boolean;
  creating?: boolean;
  system?: string;
  agentsRunning?: number;
  alerts: DesktopAlert[];
  /** The alerts are complete: the first sync pull of the server ended. */
  alertsReady?: boolean;
  opened: string | null;
  error: string;
};
type ReadStateDto = components["schemas"]["ReadStateDto"];
export type ServerCommand =
  | { action: "open"; id: string; messageId?: string }
  | {
      action: "new-chat";
      settings?: NewChatSettings;
      path?: string;
      folder?: string;
      projectId?: string | undefined;
      projectServerId?: string | undefined;
    }
  | { action: "add-project" }
  | { action: "terminal"; operation: "toggle" | "new" }
  | { action: "prepare-chat"; id: string }
  | { action: "mark-unread"; id: string; target: ReadStateDto }
  | { action: "remove-chat"; id: string; room: boolean }
  | { action: "change-project"; id: string }
  | { action: "project-account"; path: string }
  | { action: "new-shared-chat"; path?: string }
  | { action: "project-folders"; projectId: string; server?: string }
  | { action: "delete-project"; projectId: string }
  | { action: "projects" }
  | { action: "settings"; tab?: StudioSettingsTab }
  | { action: "search"; query: string; requestId: string }
  | { action: "notifications"; agentId: string; itemId?: string }
  | {
      action: "account-sign-in";
      accountKey?: string;
      provider: ProviderKind;
      email: string | null;
      label: string;
      serverLabel: string;
      forceAdd?: boolean;
    }
  | {
      action: "account-action";
      accountKey?: string;
      operation:
        | "rename"
        | "default"
        | "disconnect"
        | "remove"
        | "configure-profile";
      provider: ProviderKind;
      email: string | null;
      label: string;
      nextLabel?: string;
    }
  | { action: "focus" };
export function navigationSnapshot(
  data: StateCollections | undefined,
  opened: string | null,
  error: string,
  rows: readonly SidebarChatView[] = [],
  alerts: DesktopAlert[] = [],
  readSaved: typeof saved = saved,
  unread: ReadonlySet<string> = new Set(),
  entities: readonly SidebarEntity[] = [],
  metadata?: ProjectionMetadata,
): ServerNavigation {
  const stateDir = data?.workspaces.values().next().value?.stateDir ?? "";
  const projects = new Map<string, ServerNavigation["projects"][number]>();
  for (const project of data?.projects.values() ?? []) {
    if (project.path)
      projects.set(project.path, {
        id: project.id,
        compact: readSaved<Record<string, boolean>>(
          `codex-project-compact:${stateDir}`,
          {},
        )[project.path],
        homeServerId: project.homeServerId,
        locations: project.locations?.map((location) => ({ ...location })),
        projectAliases: project.projectAliases?.map((alias) => ({ ...alias })),
        path: project.path,
        name: project.name || project.path,
      });
  }
  const chats: ServerNavigation["chats"] = [];
  for (const chat of rows) {
    const path = chat.projectPath || "";
    if (
      !projects.has(path) &&
      ![...projects.values()].some(
        (project) =>
          (project.id === chat.agent.projectId &&
            project.homeServerId === chat.agent.projectServerId) ||
          project.locations?.some(
            (location) =>
              location.path === path &&
              location.serverId === chat.agent.projectServerId,
          ),
      )
    )
      projects.set(path, { path, name: path || "Other chats" });
    chats.push({ kind: "chat", id: chat.id, path, agent: toJS(chat.agent) });
  }
  for (const room of data?.rooms.values() ?? []) {
    if (!room.radio || room.userHidden) continue;
    const path = room.projectPath || "";
    if (!projects.has(path))
      projects.set(path, { path, name: path || "Other chats" });
    chats.push({
      kind: "room",
      id: room.id,
      name: room.name || "Shared chat",
      path,
      archived: false,
      unread: (room.members || []).some((id) => unread.has(id)),
    });
  }
  return {
    ...(metadata?.workspaceId ? { workspaceId: metadata.workspaceId } : {}),
    ...(data ? { sidebar: sidebarSnapshot(data, entities) } : {}),
    ready: !!data,
    busy: entities.some(
      (entity) => entity.collection === "agentSummary" && entity.value.inFlight,
    ),
    alerts,
    alertsReady: !!data && metadata?.complete === true,
    projects: [...projects.values()],
    chats,
    opened,
    error,
  };
}
const readStateFields = {
  threadId: (value: unknown) => typeof value === "string",
  turnId: (value: unknown) => typeof value === "string",
  read: (value: unknown) => typeof value === "boolean",
  revision: (value: unknown) =>
    typeof value === "number" && Number.isSafeInteger(value) && value >= 0,
} satisfies Record<keyof ReadStateDto, (value: unknown) => boolean>;
function isReadState(value: unknown): value is ReadStateDto {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const fields = value as Record<string, unknown>;
  return Object.entries(readStateFields).every(([name, valid]) =>
    valid(fields[name]),
  );
}
export function isServerCommand(value: unknown): value is ServerCommand {
  if (!value || typeof value !== "object") return false;
  const command = value as Record<string, unknown>;
  if (command.action === "delete-project")
    return typeof command.projectId === "string" && !!command.projectId;
  if (command.action === "terminal")
    return command.operation === "toggle" || command.operation === "new";
  if (command.action === "prepare-chat" || command.action === "change-project")
    return typeof command.id === "string";
  if (command.action === "mark-unread")
    return typeof command.id === "string" && isReadState(command.target);
  if (command.action === "remove-chat")
    return typeof command.id === "string" && typeof command.room === "boolean";
  if (command.action === "project-account")
    return typeof command.path === "string";
  if (command.action === "new-shared-chat")
    return command.path === undefined || typeof command.path === "string";
  if (command.action === "project-folders")
    return (
      typeof command.projectId === "string" &&
      (command.server === undefined || typeof command.server === "string")
    );
  if (command.action === "search")
    return (
      typeof command.query === "string" && typeof command.requestId === "string"
    );
  if (command.action === "notifications")
    return typeof command.agentId === "string";
  if (command.action === "account-sign-in")
    return (
      isProviderKind(command.provider) &&
      (command.email === null || typeof command.email === "string") &&
      typeof command.label === "string" &&
      typeof command.serverLabel === "string" &&
      (command.accountKey === undefined ||
        (typeof command.accountKey === "string" && !!command.accountKey)) &&
      !(command.accountKey !== undefined && command.forceAdd === true) &&
      (command.forceAdd === undefined || typeof command.forceAdd === "boolean")
    );
  if (command.action === "account-action")
    return (
      [
        "rename",
        "default",
        "disconnect",
        "remove",
        "configure-profile",
      ].includes(String(command.operation)) &&
      isProviderKind(command.provider) &&
      (command.operation !== "configure-profile" ||
        command.provider === ProviderKind.Claude) &&
      (command.email === null || typeof command.email === "string") &&
      typeof command.label === "string" &&
      (command.accountKey === undefined ||
        (typeof command.accountKey === "string" && !!command.accountKey)) &&
      (command.nextLabel === undefined || typeof command.nextLabel === "string")
    );
  return command.action === "open"
    ? typeof command.id === "string" && !!command.id
    : command.action === "new-chat"
      ? (command.settings === undefined ||
          isNewChatSettings(command.settings)) &&
        (command.path === undefined || typeof command.path === "string") &&
        (command.folder === undefined || typeof command.folder === "string") &&
        (command.projectId === undefined ||
          typeof command.projectId === "string") &&
        (command.projectServerId === undefined ||
          typeof command.projectServerId === "string")
      : (command.action === "settings" &&
          (command.tab === undefined || isStudioSettingsTab(command.tab))) ||
        command.action === "projects" ||
        command.action === "add-project" ||
        command.action === "focus";
}
