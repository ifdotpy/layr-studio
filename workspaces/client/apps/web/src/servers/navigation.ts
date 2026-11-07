import {
  isStudioSettingsTab,
  type StudioSettingsTab,
} from "../components/StudioSettingsTabs";
import type { DesktopAlert } from "../desktop/desktopAlerts";
import type { Snapshot } from "../types";
import { saved } from "../api";
import {
  isNewChatSettings,
  type NewChatSettings,
} from "../components/newChatSettings";
import type { LocationProject } from "./projectLocations";
import type { SidebarChatRow } from "../generated/observableResources.ts";
import { sidebarSnapshot, type ServerSidebarSnapshot } from "./sidebarSnapshot";
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
export type ServerNavigation = {
  workspaceId?: string | undefined;
  sidebar?: ServerSidebarSnapshot;
  projects: (Omit<LocationProject, "id"> & { id?: string })[];
  chats: (Pick<SidebarChatRow, "id" | "name" | "archived" | "projectId" | "projectServerId" | "provider" | "indicator" | "compactPinned" | "activityAt"> & { path: string; serverId?: string | null })[];
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
  | { action: "refresh" }
  | { action: "terminal"; operation: "toggle" | "new" }
  | { action: "prepare-chat"; id: string }
  | { action: "mark-unread"; id: string; threadId: string; turnId: string }
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
  data: Snapshot | null,
  opened: string | null,
  error: string,
  rows: readonly SidebarChatRow[] = [],
  alerts: DesktopAlert[] = [],
  readSaved: typeof saved = saved,
): ServerNavigation {
  const projects = new Map<string, ServerNavigation["projects"][number]>();
  for (const project of data?.runtime?.projects || []) {
    if (project.path)
      projects.set(project.path, {
        id: project.id,
        compact: readSaved<Record<string, boolean>>(
          `codex-project-compact:${data?.stateDir}`,
          {},
        )[project.path],
        homeServerId: project.homeServerId,
        locations: project.locations,
        projectAliases: project.projectAliases,
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
          (project.id === chat.projectId &&
            project.homeServerId === chat.projectServerId) ||
          project.locations?.some(
            (location) =>
              location.path === path && location.serverId === chat.projectServerId,
          ),
      )
    )
      projects.set(path, { path, name: path || "Other chats" });
    chats.push({
      id: chat.id,
      name: chat.name || "Chat",
      path,
      archived: !!chat.archived,
      projectId: chat.projectId,
      projectServerId: chat.projectServerId,
      provider: chat.provider,
      activityAt: chat.activityAt,
      compactPinned: chat.compactPinned,
      indicator: chat.indicator,
    });
  }
  return {
    ...(data?.entityWorkspaceId ? { workspaceId: data.entityWorkspaceId } : {}),
    ...(data ? { sidebar: sidebarSnapshot(data, rows) } : {}),
    ready: !!data,
    busy: rows.some((row) => row.inFlight),
    alerts,
    alertsReady: !!data && data.entitiesComplete !== false,
    projects: [...projects.values()],
    chats,
    opened,
    error,
  };
}
export function isServerCommand(value: unknown): value is ServerCommand {
  if (!value || typeof value !== "object") return false;
  const command = value as Record<string, unknown>;
  if (command.action === "delete-project")
    return typeof command.projectId === "string" && !!command.projectId;
  if (command.action === "refresh") return true;
  if (command.action === "terminal")
    return command.operation === "toggle" || command.operation === "new";
  if (command.action === "prepare-chat" || command.action === "change-project")
    return typeof command.id === "string";
  if (command.action === "mark-unread")
    return (
      typeof command.id === "string" &&
      typeof command.threadId === "string" &&
      typeof command.turnId === "string"
    );
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
