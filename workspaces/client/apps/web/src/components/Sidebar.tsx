import { outboxRequests } from "../browserPersistence";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../servers/ServerServicesContext.tsx";
import type { SidebarChatRow } from "../generated/observableResources.ts";
import { keepCompactSidebarChat } from "./sidebar/compact.ts";
import {
  localSidebarServices,
  SidebarBackendProvider,
  useSidebarVisualState,
  type SidebarServices,
  type SidebarTarget,
} from "./sidebar/services";
import {
  Fragment,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  ActionIcon,
  Menu,
  Modal,
  TextInput,
  Tooltip,
  UnstyledButton,
} from "@mantine/core";
import {
  layoutQueries,
  layoutValues,
} from "../layout/layout-contract.generated";
import { SidebarFrame, restoreSidebarFocus } from "./sidebar/SidebarFrame";
import {
  Users,
  FolderOpen,
  MoreHorizontal,
  SquarePen,
  Search,
  Folder,
  Plus,
  X,
  Settings,
} from "lucide-react";
import { errorText } from "../api";
import type { paths } from "../generated/api";
import "./sidebar-projects.css";
import { useSidebarOrder } from "./useSidebarOrder";
import {
  ProjectNameForm,
  MoveChatForm,
  ConvertChatForm,
  type Project,
  type ProjectFolder,
} from "./ProjectOrganization";
import { type Snapshot } from "../types";
import {
  PeerTeamForm,
  PeerTeamGroup,
  usePeerTeamMove,
} from "./shell/messages/PeerTeams";

import SidebarRow from "./SidebarRow";
import RemoteProjects, {
  remoteProjectGroups,
  type RemoteProjectList,
} from "./sidebar/RemoteProjects";
import { SidebarWarningsTarget } from "./SidebarWarnings";
import { readServerAliases } from "../servers/registry";
import {
  DEFAULT_LOCAL_SERVER_ALIAS,
  useServerAliases,
} from "../servers/serverAliases";
import { serverViewId } from "../servers/environment";
import { ChatServerLine } from "../servers/ProjectChatRows";
import { countProjectChats, itemGroup } from "./sidebar/catalog";
export { isVisibleSidebarAgent } from "./sidebar/catalog";
import { sidebarRows } from "./sidebar/rows.ts";
import {
  SidebarAttention,
  SidebarProjectStates,
} from "./sidebar/RowStatus.tsx";
import { reportPromptComposerRender } from "./prompt-composer/renderProbe";
import { DEFAULT_HIDE_OLD_CHATS_THRESHOLD } from "../studioPreferences";

type OrganizationRequest =
  paths["/api/organization"]["post"]["requestBody"]["content"]["application/json"];
type WithoutOrganizationId<Request> = Request extends { id: string }
  ? Omit<Request, "id">
  : never;
type OrganizationData = WithoutOrganizationId<OrganizationRequest>;
export type SidebarProps = {
  services?: SidebarServices;
  settings?: () => void;
  notice?: string;
  data: Snapshot;
  rows?: readonly SidebarChatRow[];
  /** Registered project paths when data also contains inferred view projects. */
  registeredProjectPaths?: ReadonlySet<string>;
  opened: string | null;
  selectionIdentity?: string;
  open: (id: string) => void;
  prepareChat?: (id: string) => void;
  newChat: (path?: string, folder?: string) => void;
  newSharedChat?: (path?: string) => void;
  addProject: () => void;
  /** Projects of paired servers, listed read-only below this server's projects. */
  remoteProjects?: RemoteProjectList;
  changeProject: (id: string) => void;
  projectAccount: (path: string) => void;
  projectFolders?: (path: string) => void;
  deleteProject?: (path: string) => void;
  creating: boolean;
  rename: (id: string, name: string) => Promise<void>;
  remove: (id: string, room: boolean) => void;
  mobile: boolean;
  collapsed?: boolean;
  onCompactChange?: (compact: boolean) => void;
  onSearch: () => void;
  close: () => void;
  refresh?: () => Promise<void>;
  notify?: (message: string) => void;
  offlineServers?: ReadonlySet<string>;
  markUnread: (
    id: string,
    target: NonNullable<SidebarChatRow["markRead"]>,
  ) => void;
  markingRead: Set<string>;
  pendingReadTargets?: ReadonlyMap<
    string,
    NonNullable<SidebarChatRow["markRead"]>
  >;
  hideOldChatsThreshold?: number;
};
type NavigableProject = Project & { path: string; name: string };
export function projectDisplayName(
  project: Pick<Project, "path" | "name">,
): string {
  return typeof project.name === "string" && project.name
    ? project.name
    : project.path || "Project";
}
export function projectSearchLabel(
  projectName: string | null | undefined,
  legacyProjectName: unknown,
): string {
  return (
    projectName ||
    (typeof legacyProjectName === "string" ? legacyProjectName : "")
  );
}
export default function Sidebar(p: SidebarProps) {
  const services = p.services || localSidebarServices;
  return (
    <SidebarBackendProvider value={services.cache}>
      <SidebarContents {...p} services={services} />
    </SidebarBackendProvider>
  );
}

const SidebarContents = observer(function SidebarContents(
  p: SidebarProps & { services: SidebarServices },
) {
  const services = p.services;
  const store = useServerStore();
  const resource = p.rows === undefined ? store.sidebar().current : undefined;
  const rows =
    p.rows ??
    (resource?.status === "live" || resource?.status === "stale"
      ? resource.data.rows
      : []);
  const lastOpened = useRef(p.opened);
  if (p.opened) lastOpened.current = p.opened;
  const canEdit = (target: SidebarTarget) =>
    services.available?.(target) ?? true;
  /** Why an action on this target is disabled, or null when it is available. */
  const unavailableReason = (target: SidebarTarget) =>
    canEdit(target)
      ? null
      : services.unavailable?.(target) || "The server is unavailable.";
  const projectTarget = (
    path: string,
    folder?: string,
    team?: string,
  ): SidebarTarget => ({ kind: "project", path, folder, team });
  const ownedProject = (
    project: NavigableProject,
    target: SidebarTarget,
  ): Project => services.project?.(project.path, target) || project;
  const ownsFolder = (
    project: NavigableProject,
    chat: SidebarChatRow,
    folder: ProjectFolder | null,
  ) => {
    if (!services.project || !folder) return true;
    try {
      return !!ownedProject(project, {
        kind: "chat",
        id: chat.id,
      }).folders?.some((item) => item.id === folder.id);
    } catch {
      return false;
    }
  };
  const { editPreference: writePreferenceEdit } = services.cache;
  reportPromptComposerRender("sidebar");
  const runtime = p.data.runtime;
  const aliases = useServerAliases(readServerAliases);
  const [compact, setCompact] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia(layoutQueries.navigationCompact).matches,
  );
  const reportCompact = useCallback(
    (value: boolean) => {
      setCompact(value);
      p.onCompactChange?.(value);
    },
    [p.onCompactChange],
  );
  const closeSidebar = () => {
    restoreSidebarFocus();
    p.close();
  };
  const openChat = (id: string) => {
    if (compact) closeSidebar();
    p.open(id);
  };
  const inline = !compact && !p.collapsed;
  // Page layout rules read this flag. It replaces #root:has(#sidebar), which
  // the browser had to test against the whole tree after each DOM change.
  useLayoutEffect(() => {
    if (!inline) return;
    const root = document.documentElement;
    root.dataset.inlineSidebar = "true";
    return () => {
      delete root.dataset.inlineSidebar;
    };
  }, [inline]);
  useEffect(() => {
    if (!compact || !p.mobile) return;
    const background =
      document.querySelector<HTMLElement>(".server-views") ||
      document.querySelector<HTMLElement>(".workspace-frame");
    if (!background) return;
    const wasInert = background.inert;
    const wasHidden = background.getAttribute("aria-hidden");
    background.inert = true;
    background.setAttribute("aria-hidden", "true");
    return () => {
      background.inert = wasInert;
      if (wasHidden == null) background.removeAttribute("aria-hidden");
      else background.setAttribute("aria-hidden", wasHidden);
    };
  }, [compact, p.mobile]);
  const [query, setQuery] = useState(""),
    [renaming, setRenaming] = useState<string | null>(null),
    [name, setName] = useState(""),
    [archive, setArchive] = useState(false);
  const [renameInPriority, setRenameInPriority] = useState(false);
  const hideOldChatsThreshold =
    p.hideOldChatsThreshold ?? DEFAULT_HIDE_OLD_CHATS_THRESHOLD;
  const sorting = useSidebarOrder(
    `codex-sidebar-order:${p.data.stateDir}`,
    runtime?.sidebarOrder ?? undefined,
    p.refresh,
    p.notify,
    services,
    p.data.token,
  );
  const [organizing, setOrganizing] = useState<string | null>(null);
  const organizationLock = useRef(false);
  type Conversion = {
    source: SidebarChatRow;
    target: SidebarChatRow;
    project: NavigableProject;
  };
  const [conversion, setConversion] = useState<Conversion | null>(null);
  const closeConversion = () => {
    try {
      if (
        conversion &&
        outboxRequests(
          services.owner({ kind: "chat", id: conversion.source.id }).storage,
        ).get(
          "projectSave",
          `${p.data.stateDir}:${conversion.source.id}:${conversion.target.id}`,
        )
      ) {
        p.notify?.(
          "Retry the saved request to confirm its result before closing.",
        );
        return;
      }
    } catch {
      p.notify?.(
        "Load the chat server to check the saved request before closing.",
      );
      return;
    }
    setConversion(null);
  };
  const compactKey = `codex-project-compact:${p.data.stateDir}`;
  const [compactProjects, setCompactProjects] = useSidebarVisualState<
    Record<string, boolean>
  >(compactKey, {});
  const setCompactProject = (path: string, value: boolean) => {
    const next = { ...compactProjects, [path]: value };
    setCompactProjects(writePreferenceEdit(compactKey, compactProjects, next));
  };
  const [teamDialog, setTeamDialog] = useState<{
    path: string;
    id?: string;
    action: "save" | "delete";
  } | null>(null);
  const peerTeams = useMemo(
    () => (runtime?.peerTeamsVersion === 1 ? runtime.peerTeams || [] : []),
    [runtime?.peerTeamsVersion, runtime?.peerTeams],
  );
  const projects = useMemo(
    () =>
      (runtime?.projects ?? [])
        .filter(
          (project): project is Project & { path: string } =>
            typeof project.path === "string",
        )
        .map((project): NavigableProject => ({
          ...project,
          name: projectDisplayName(project),
        })),
    [runtime?.projects],
  );
  const teamMove = usePeerTeamMove(p.refresh, p.notify, (path, member, team) =>
    services.owner(
      team
        ? projectTarget(path, undefined, team)
        : { kind: "chat", id: member },
    ),
  );
  const [dialog, setDialog] = useState<{
    title: string;
    path: string;
    folder?: string | undefined;
    parentId?: string | undefined;
    agentId?: string;
  } | null>(null);
  const canOrganizeProjects = !!runtime?.projectOrganizationVersion;
  const requireProjectSupport = () => {
    if (canOrganizeProjects) return true;
    p.notify?.(
      "Restart Studio after active work finishes to edit project names and folders.",
    );
    return false;
  };
  const savedProject = async () => {
    await p.refresh?.();
    setDialog(null);
  };
  const editProject = (
    project: NavigableProject,
    folder?: ProjectFolder | "new",
    parentId?: string,
  ) => {
    if (!requireProjectSupport()) return;
    if (
      !canEdit(
        projectTarget(project.path, folder === "new" ? parentId : folder?.id),
      )
    )
      return;
    setDialog({
      title:
        folder === "new"
          ? "New chat folder"
          : folder
            ? "Rename folder"
            : "Rename project",
      path: project.path,
      folder: folder === "new" ? "new" : folder?.id,
      parentId,
    });
  };
  const folderKey = (path: string, id: string) =>
    JSON.stringify([path, "folder", id]);
  // Not the bare path: saved values of the earlier project collapse stay unused.
  const projectTreeKey = (path: string) => JSON.stringify([path, "project"]);
  const projectKey = `codex-project-tree:${p.data.stateDir}`;
  const [collapsed, setCollapsed] = useSidebarVisualState<
    Record<string, boolean>
  >(projectKey, {});
  const toggleTree = (key: string) => {
    const next = { ...collapsed, [key]: !collapsed[key] };
    setCollapsed(writePreferenceEdit(projectKey, collapsed, next));
  };

  // Pin and archive show at once; the next snapshot confirms them.
  const [overrides, setOverrides] = useState<
    Record<string, { pinned?: boolean; archived?: boolean }>
  >({});
  const organize = async (id: string, data: OrganizationData) => {
    if (!canEdit({ kind: "chat", id })) {
      p.notify?.("The sidebar server is offline or unavailable.");
      return false;
    }
    if (organizationLock.current) return false;
    organizationLock.current = true;
    setOrganizing(id);
    const optimistic = Object.fromEntries(
      Object.entries(data).filter(
        ([key, value]) =>
          (key === "pinned" || key === "archived") &&
          typeof value === "boolean",
      ),
    );
    const hasOptimistic = Object.keys(optimistic).length > 0;
    if (hasOptimistic)
      setOverrides((old) => ({ ...old, [id]: { ...old[id], ...optimistic } }));
    try {
      await services
        .owner({ kind: "chat", id })
        .post("/api/organization", { id, ...data });
      if (hasOptimistic) void p.refresh?.();
      else await p.refresh?.();
      return true;
    } catch (error) {
      if (hasOptimistic)
        setOverrides((old) => {
          const next = { ...old };
          delete next[id];
          return next;
        });
      p.notify?.(errorText(error));
      return false;
    } finally {
      organizationLock.current = false;
      setOrganizing(null);
    }
  };
  useEffect(() => {
    // Drop an override once the snapshot shows the same value.
    setOverrides((old) => {
      const next = { ...old };
      let changed = false;
      for (const [id, values] of Object.entries(old)) {
        const thread = rows.find((a) => a.id === id);
        if (
          !thread ||
          ((values.pinned === undefined || thread.pinned === values.pinned) &&
            (values.archived === undefined ||
              thread.archived === values.archived))
        ) {
          delete next[id];
          changed = true;
        }
      }
      return changed ? next : old;
    });
  }, [rows]);
  const catalog = sidebarRows(rows, peerTeams);
  const { agents, byProject, grouped } = catalog;
  const teamFor = (id: string) => catalog.byTeam.get(id);
  const sharedRooms = useMemo(
    () =>
      (runtime?.rooms || []).filter((r) => r.radio?.direct && !r.userHidden),
    [runtime?.rooms],
  );
  const visibleSharedRooms = useMemo(
    () =>
      archive
        ? []
        : sharedRooms.filter((r) =>
            `${r.name} ${r.projectPath || ""} ${projects.find((project) => project.path === r.projectPath)?.name || ""}`
              .toLowerCase()
              .includes(query.toLowerCase()),
          ),
    [archive, sharedRooms, projects, query],
  );
  const selectedFolder = catalog.byId.get(p.opened || "")?.projectFolder;
  const initialSelection = useRef<{
    opened: string;
    folder: typeof selectedFolder;
    identity?: string | undefined;
  } | null>(null);
  useEffect(() => {
    if (!initialSelection.current) {
      if (p.opened)
        initialSelection.current = {
          opened: p.opened,
          folder: selectedFolder,
          identity: p.selectionIdentity,
        };
      return;
    }
    if (
      p.selectionIdentity !== undefined
        ? initialSelection.current.identity === p.selectionIdentity
        : initialSelection.current.opened === p.opened &&
          initialSelection.current.folder === selectedFolder
    )
      return;
    initialSelection.current = p.opened
      ? {
          opened: p.opened,
          folder: selectedFolder,
          identity: p.selectionIdentity,
        }
      : null;
    if (
      agents.some((a) => a.id === p.opened) ||
      sharedRooms.some((r) => r.id === p.opened)
    ) {
      setQuery("");
      const path =
        agents.find((a) => a.id === p.opened)?.projectPath ||
        sharedRooms.find((r) => r.id === p.opened)?.projectPath;
      const folders =
        projects.find((project) => project.path === path)?.folders || [];
      let folder = folders.find((item) => item.id === selectedFolder);
      const ancestors: Record<string, boolean> = {};
      while (folder && !(folderKey(path || "", folder.id) in ancestors)) {
        ancestors[folderKey(path || "", folder.id)] = false;
        folder = folders.find((item) => item.id === folder!.parentId);
      }
      if (path && collapsed[projectTreeKey(path)])
        ancestors[projectTreeKey(path)] = false;
      if (path && Object.keys(ancestors).length) {
        const next = { ...collapsed, ...ancestors };
        setCollapsed(
          (services.cache.viewPreference || writePreferenceEdit)(
            projectKey,
            collapsed,
            next,
          ),
        );
      }
    }
  }, [p.opened, selectedFolder, p.selectionIdentity]);
  const teamKey = (path: string, id: string) =>
    JSON.stringify([path, "team", id]);
  const chatGroup = (agent: SidebarChatRow) => agent.groupKey;
  const middleDrop = (event: React.DragEvent<HTMLElement>) => {
    const title = (event.target as Element).closest(
      ".peer-team-toggle, .project-tree-toggle",
    );
    const box = (title || event.currentTarget).getBoundingClientRect();
    return (
      event.clientY > box.top + box.height * 0.25 &&
      event.clientY < box.top + box.height * 0.75
    );
  };
  const folderDrop = (
    project: NavigableProject,
    folder: ProjectFolder | null = null,
    reorder: ReturnType<typeof sorting.bindings> | Record<string, never> = {},
  ) =>
    sorting.dropBindings(
      folder ? folderKey(project.path, folder.id) : project.path,
      (source, event) => {
        const chat = catalog.byId.get(source.id);
        return !!(
          ((!(event.target as Element).closest(
            ".peer-team-toggle, .project-tree-toggle",
          ) &&
            !reorder.draggable) ||
            middleDrop(event)) &&
          (canOrganizeProjects || (!folder && teamFor(source.id))) &&
          !teamMove.blocked() &&
          !organizationLock.current &&
          chat &&
          canEdit({ kind: "chat", id: chat.id }) &&
          ownsFolder(project, chat, folder) &&
          source.group === chatGroup(chat) &&
          chat.projectPath === project.path &&
          ((!folder && !!teamFor(chat.id)) ||
            (chat.projectFolder || null) !== (folder?.id || null))
        );
      },
      ({ id }) => {
        const chat = catalog.byId.get(id)!;
        if (!folder && teamFor(id)) {
          teamMove.move(project, id, null);
          return;
        }
        void organize(id, {
          project_path: project.path,
          project_folder: folder?.id || null,
          expected_folder: chat.projectFolder || null,
          expected_revision: chat.projectFolderRevision || 0,
        }).then((moved) => {
          if (!moved) return;
          setCollapsed((previous) => {
            const next = {
              ...previous,
              [projectTreeKey(project.path)]: false,
            };
            let parent = folder;
            const seen = new Set<string>();
            while (parent && !seen.has(parent.id)) {
              seen.add(parent.id);
              next[folderKey(project.path, parent.id)] = false;
              parent =
                project.folders?.find((f) => f.id === parent?.parentId) || null;
            }
            return writePreferenceEdit(projectKey, previous, next);
          });
          setCompactProject(project.path, false);
          sorting.announce(
            `Moved ${chat.name} to ${folder?.name || project.name}`,
          );
        });
      },
      reorder,
    );
  const orderedAgents = catalog.ordered;
  const filtered = (() => {
    const archived = orderedAgents.filter(
      (row) => (overrides[row.id]?.archived ?? row.archived) === archive,
    );
    const needle = query.toLowerCase();
    const matching = needle
      ? archived.filter((row) =>
          `${row.searchText} ${services.chatPath?.(row.id) || ""}`
            .toLowerCase()
            .includes(needle),
        )
      : archived;
    // Keep the selected team member visible under a search, as before.
    const selected = catalog.byId.get(p.opened || "");
    if (
      query &&
      selected &&
      !!selected.archived === archive &&
      grouped.has(selected.id) &&
      !matching.includes(selected)
    ) {
      const matches = new Set(matching);
      return archived.filter((row) => row === selected || matches.has(row));
    }
    return matching;
  })();
  const rename = async (e: React.FormEvent, id: string) => {
    e.preventDefault();
    try {
      await p.rename(id, name);
      setRenaming(null);
    } catch {
      /* Keep the draft when the server rejects the name. */
    }
  };
  const sharedProjectPaths = useMemo(
    () => new Set(sharedRooms.map((room) => room.projectPath)),
    [sharedRooms],
  );
  const keptInCompact = keepCompactSidebarChat;
  const viewChatCounts = useMemo(
    () => countProjectChats(agents, archive),
    [agents, archive],
  );
  const isCompactProject = (path: string) =>
    compactProjects[path] !== false &&
    (viewChatCounts.get(path) || 0) >= hideOldChatsThreshold;
  const groupMap = useMemo(() => {
    const groups = new Map<
      string,
      NavigableProject & {
        chats: SidebarChatRow[];
        registered: boolean;
        hasChats: boolean;
      }
    >();
    for (const project of projects)
      groups.set(project.path, {
        ...project,
        chats: [],
        registered: p.registeredProjectPaths?.has(project.path) ?? true,
        hasChats:
          byProject.has(project.path) || sharedProjectPaths.has(project.path),
      });
    for (const a of agents) {
      const path = a.projectPath || "";
      if (!groups.has(path))
        groups.set(path, {
          id: path,
          created: 0,
          path,
          name: path.split("/").filter(Boolean).at(-1) || "Other chats",
          chats: [],
          registered: false,
          hasChats: true,
        });
    }
    for (const room of sharedRooms) {
      const path = room.projectPath || "";
      if (!groups.has(path))
        groups.set(path, {
          id: path,
          created: 0,
          path,
          name: path.split("/").filter(Boolean).at(-1) || "Other chats",
          chats: [],
          registered: false,
          hasChats: true,
        });
    }
    for (const a of filtered) {
      if (
        query ||
        !isCompactProject(a.projectPath || "") ||
        a.id === p.opened ||
        keptInCompact(a)
      )
        groups.get(a.projectPath || "")!.chats.push(a);
    }
    return groups;
  }, [
    projects,
    p.registeredProjectPaths,
    agents,
    sharedRooms,
    sharedProjectPaths,
    filtered,
    query,
    compactProjects,
    viewChatCounts,
    hideOldChatsThreshold,
    p.opened,
  ]);
  const itemIds = (group: NavigableProject, parent: string | null = null) =>
    [
      ...(!parent
        ? peerTeams
            .filter((t) => t.projectPath === group.path)
            .map((t) => teamKey(group.path, t.id))
        : []),
      ...(group.folders || [])
        .filter((f) => (f.parentId || null) === parent)
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((f) => folderKey(group.path, f.id)),
      ...(byProject.get(group.path) || [])
        .filter(
          (a) =>
            !!a.archived === archive &&
            !grouped.has(a.id) &&
            !a.pinned &&
            (a.projectFolder || null) === parent,
        )
        .map((a) => a.id),
    ].sort(
      (a, b) =>
        sorting.rank(itemGroup(group.path, parent), a) -
        sorting.rank(itemGroup(group.path, parent), b),
    );
  const projectOrderKey = JSON.stringify(
    [...groupMap.values()].map((group) => [group.path, group.name]),
  );
  const projectPaths = useMemo(
    () =>
      [...groupMap.values()]
        .sort(
          (a, b) =>
            sorting.rank("projects", a.path) -
              sorting.rank("projects", b.path) || a.name.localeCompare(b.name),
        )
        .map((group) => group.path),
    [projectOrderKey, sorting.order],
  );
  const allProjectGroups = projectPaths.map((path) => groupMap.get(path)!);
  const remoteGroups = useMemo(
    () => remoteProjectGroups(p.remoteProjects, query),
    [p.remoteProjects, query],
  );
  const projectGroups = allProjectGroups.filter(
    (group) =>
      (group.registered ||
        viewChatCounts.has(group.path) ||
        visibleSharedRooms.some((r) => r.projectPath === group.path)) &&
      (!query ||
        group.chats.length ||
        visibleSharedRooms.some((r) => r.projectPath === group.path) ||
        `${group.path} ${group.name}`
          .toLowerCase()
          .includes(query.toLowerCase())),
  );
  const renderRow = (row: SidebarChatRow, priority = false) => (
    <SidebarRow
      key={row.id}
      row={row}
      selected={p.opened === row.id}
      showServer={services.serverForChat(row.id) !== services.cache.ownerId}
      serverAlias={
        services.alias?.({ kind: "chat", id: row.id }) ||
        aliases[services.serverForChat(row.id)] ||
        DEFAULT_LOCAL_SERVER_ALIAS
      }
      serverOffline={
        p.offlineServers?.has(services.serverForChat(row.id)) ?? false
      }
      renaming={renaming === row.id && renameInPriority === priority}
      organizing={organizing === row.id}
      name={renaming === row.id ? name : ""}
      disabled={!canEdit({ kind: "chat", id: row.id })}
      compact={compact}
      markingRead={p.markingRead.has(row.id)}
      pendingRead={p.pendingReadTargets?.get(row.id)}
      pinned={overrides[row.id]?.pinned}
      bindings={{
        ...sorting.dropBindings(
          `convert:${row.id}`,
          (source, event) => {
            const chat = catalog.byId.get(source.id);
            const box = event.currentTarget.getBoundingClientRect();
            return !!(
              chat &&
              canEdit({ kind: "chat", id: chat.id }) &&
              canEdit({ kind: "chat", id: row.id }) &&
              services.serverForChat(chat.id) ===
                services.serverForChat(row.id) &&
              chat.id !== row.id &&
              teamFor(chat.id) &&
              chat.projectPath === row.projectPath &&
              source.group === chatGroup(chat) &&
              !conversion &&
              !organizationLock.current &&
              !teamMove.blocked() &&
              event.clientY > box.top + box.height * 0.25 &&
              event.clientY < box.top + box.height * 0.75
            );
          },
          ({ id }) =>
            setConversion({
              source: catalog.byId.get(id)!,
              target: row,
              project: groupMap.get(row.projectPath || "")!,
            }),
          sorting.bindings(
            chatGroup(row),
            row.id,
            teamFor(row.id) || row.pinned
              ? orderedAgents
                  .filter((a) => chatGroup(a) === chatGroup(row))
                  .map((a) => a.id)
              : itemIds(
                  groupMap.get(row.projectPath || "")!,
                  row.projectFolder || null,
                ),
          ),
        ),
        draggable: canEdit({ kind: "chat", id: row.id }),
      }}
      actions={{
        prepare: () => p.prepareChat?.(row.id),
        open: () => openChat(row.id),
        pin: () => {
          void organize(row.id, { pinned: !row.pinned });
        },
        rename: (event) => {
          void rename(event, row.id);
        },
        name: setName,
        cancelRename: () => setRenaming(null),
        beginRename: () => {
          setName(row.name ?? "");
          setRenameInPriority(priority);
          setRenaming(row.id);
        },
        unread: () => {
          if (row.markRead) p.markUnread(row.id, { ...row.markRead });
        },
        move: () => {
          if (!requireProjectSupport()) return;
          const project = groupMap.get(row.projectPath || "")!;
          setDialog({
            title: "Move chat",
            path: project.path,
            agentId: row.id,
          });
        },
        changeProject: () => p.changeProject(row.id),
        archive: () => {
          void organize(row.id, { archived: !row.archived });
        },
        remove: () => p.remove(row.id, false),
      }}
    />
  );
  const renderPeerTeam = (
    group: NavigableProject & { chats: SidebarChatRow[] },
    team: (typeof peerTeams)[number],
  ): ReactNode => {
    const teamMembers = team.members ?? [];
    const members = group.chats.filter((a) => teamMembers.includes(a.id));
    const selected = agents.find(
      (a) => a.id === p.opened && teamMembers.includes(a.id),
    );
    if (selected && !members.includes(selected)) members.push(selected);
    if (!members.length && (query || archive)) return null;
    const key = JSON.stringify([group.path, "team", team.id]);
    const closed = !!collapsed[key] && !query;
    const teamDrop = (
      reorder: ReturnType<typeof sorting.bindings> | Record<string, never> = {},
    ) =>
      sorting.dropBindings(
        `peer-team:${team.id}`,
        (source, event) => {
          const chat = catalog.byId.get(source.id);
          return !!(
            ((!(event.target as Element).closest(
              ".peer-team-toggle, .project-tree-toggle",
            ) &&
              !reorder.draggable) ||
              middleDrop(event)) &&
            chat &&
            canEdit(projectTarget(group.path, undefined, team.id)) &&
            canEdit({ kind: "chat", id: chat.id }) &&
            services.serverForChat(chat.id) ===
              services.owner(projectTarget(group.path, undefined, team.id))
                .ownerId &&
            source.group === chatGroup(chat) &&
            chat.projectPath === group.path &&
            !teamMembers.includes(chat.id) &&
            !teamMove.blocked() &&
            !organizationLock.current
          );
        },
        ({ id }) => teamMove.move(group, id, team.id),
        reorder,
      );
    return (
      <SidebarBackendProvider
        key={team.id}
        value={services.owner(projectTarget(group.path, undefined, team.id))}
      >
        <PeerTeamGroup
          itemId={teamKey(group.path, team.id)}
          team={team}
          disabled={!canEdit(projectTarget(group.path, undefined, team.id))}
          serverAlias={
            services.alias &&
            services.owner(projectTarget(group.path, undefined, team.id))
              .ownerId !== services.owner(projectTarget(group.path)).ownerId
              ? services.alias(projectTarget(group.path, undefined, team.id))
              : undefined
          }
          serverOffline={
            p.offlineServers?.has(
              services.owner(projectTarget(group.path, undefined, team.id))
                .ownerId,
            ) ?? false
          }
          roomId={runtime?.rooms?.find((r) => r.radio?.teamId === team.id)?.id}
          scope={p.data.stateDir}
          openRoom={openChat}
          refresh={p.refresh}
          reorder={teamDrop(
            sorting.bindings(
              itemGroup(group.path),
              teamKey(group.path, team.id),
              itemIds(group),
            ),
          )}
          drop={teamDrop()}
          closed={closed}
          toggle={() => toggleTree(key)}
          edit={() =>
            setTeamDialog({
              path: group.path,
              id: team.id,
              action: "save",
            })
          }
          dissolve={() =>
            setTeamDialog({
              path: group.path,
              id: team.id,
              action: "delete",
            })
          }
        >
          <div className="peer-team-chats">
            {(runtime?.rooms || [])
              .filter(
                (r) =>
                  r.radio?.teamId === team.id && (!closed || r.id === p.opened),
              )
              .map((r) => (
                <UnstyledButton
                  key={r.id}
                  className="peer-team-shared-chat"
                  aria-current={r.id === p.opened ? "page" : undefined}
                  onClick={() => openChat(r.id)}
                >
                  Shared chat
                </UnstyledButton>
              ))}
            {(closed ? members.filter((a) => a.id === p.opened) : members).map(
              (row) => renderRow(row),
            )}
          </div>
        </PeerTeamGroup>
      </SidebarBackendProvider>
    );
  };
  const renderProjectChats = (
    group: NavigableProject & { chats: SidebarChatRow[] },
    parent: ProjectFolder | null = null,
  ): ReactNode => {
    const folders = group.folders || [];
    const assigned = group.chats
      .filter((chat) => !grouped.has(chat.id))
      .filter((chat) =>
        parent
          ? chat.projectFolder === parent.id
          : !folders.some((folder) => folder.id === chat.projectFolder),
      );
    const chats = [...assigned];
    const children = folders
      .filter((folder) => (folder.parentId || null) === (parent?.id || null))
      .filter((folder) => {
        if (query || !isCompactProject(group.path)) return true;
        const contains = (id: string, seen = new Set<string>()): boolean => {
          if (seen.has(id)) return false;
          seen.add(id);
          return (
            (!(byProject.get(group.path) || []).some(
              (a) => a.projectFolder === id,
            ) &&
              !folders.some((f) => f.parentId === id)) ||
            group.chats.some(
              (a) => a.projectFolder === id && !grouped.has(a.id),
            ) ||
            folders.some((f) => f.parentId === id && contains(f.id, seen))
          );
        };
        return contains(folder.id);
      })
      .sort((a, b) => a.name.localeCompare(b.name));
    const siblingIds = itemIds(group, parent?.id || null);
    return (
      <>
        {[
          ...(!parent
            ? peerTeams
                .filter((t) => t.projectPath === group.path)
                .map((t) => ({
                  id: teamKey(group.path, t.id),
                  pinned: false,
                  node: renderPeerTeam(group, t),
                }))
            : []),
          ...children.map((folder) => {
            const id = folderKey(group.path, folder.id);
            const closed = collapsed[id] && !query;
            const occupied =
              folders.some((item) => item.parentId === folder.id) ||
              (byProject.get(group.path) || []).some(
                (chat) => chat.projectFolder === folder.id,
              );
            return {
              id,
              pinned: false,
              node: (
                <section
                  className="project-folder"
                  data-folder-id={folder.id}
                  data-sidebar-item={id}
                  key={folder.id}
                >
                  <div
                    className="project-tree-heading"
                    {...folderDrop(group, folder)}
                  >
                    <UnstyledButton
                      {...folderDrop(
                        group,
                        folder,
                        sorting.bindings(
                          itemGroup(group.path, parent?.id || null),
                          id,
                          siblingIds,
                        ),
                      )}
                      aria-description="Drag to reorder. Alt + Up or Down also works."
                      className="project-tree-toggle"
                      aria-expanded={!closed}
                      onClick={() => toggleTree(id)}
                    >
                      {closed ? <Folder size={16} /> : <FolderOpen size={16} />}
                      <span>
                        {folder.name}
                        {services.alias &&
                          services.owner(projectTarget(group.path, folder.id))
                            .ownerId !==
                            services.owner(projectTarget(group.path))
                              .ownerId && (
                            <ChatServerLine
                              alias={
                                services.alias(
                                  projectTarget(group.path, folder.id),
                                ) || ""
                              }
                              offline={
                                p.offlineServers?.has(
                                  services.owner(
                                    projectTarget(group.path, folder.id),
                                  ).ownerId,
                                ) ?? false
                              }
                            />
                          )}
                      </span>
                    </UnstyledButton>
                    {!archive && (
                      <ActionIcon
                        className="project-tree-action"
                        aria-label={`New chat in folder ${folder.name}`}
                        disabled={
                          p.creating ||
                          !canEdit(projectTarget(group.path, folder.id))
                        }
                        onClick={() => {
                          if (requireProjectSupport())
                            p.newChat(group.path, folder.id);
                        }}
                      >
                        <Plus size={15} />
                      </ActionIcon>
                    )}
                    <Menu withinPortal position="bottom-end">
                      <Menu.Target>
                        <ActionIcon
                          className="project-tree-action"
                          aria-label={`Options for folder ${folder.name}`}
                        >
                          <MoreHorizontal size={15} />
                        </ActionIcon>
                      </Menu.Target>
                      <Menu.Dropdown className="fleet-menu">
                        <Menu.Item
                          disabled={
                            !canEdit(projectTarget(group.path, folder.id))
                          }
                          onClick={() => editProject(group, "new", folder.id)}
                        >
                          New subfolder
                        </Menu.Item>
                        <Menu.Item
                          disabled={
                            !canEdit(projectTarget(group.path, folder.id))
                          }
                          onClick={() => editProject(group, folder)}
                        >
                          Rename folder
                        </Menu.Item>
                        <Menu.Item
                          disabled={
                            occupied ||
                            !canEdit(projectTarget(group.path, folder.id))
                          }
                          onClick={async () => {
                            if (!requireProjectSupport()) return;
                            try {
                              await services
                                .owner(projectTarget(group.path, folder.id))
                                .post("/api/projects", {
                                  action: "remove_folder",
                                  path: group.path,
                                  folder_id: folder.id,
                                  expected_revision:
                                    group.organizationRevision || 0,
                                });
                              await p.refresh?.();
                            } catch (error) {
                              p.notify?.(errorText(error));
                            }
                          }}
                        >
                          Remove empty folder
                        </Menu.Item>
                      </Menu.Dropdown>
                    </Menu>
                  </div>
                  {!closed && (
                    <div className="project-folder-chats">
                      {renderProjectChats(group, folder)}
                    </div>
                  )}
                </section>
              ),
            };
          }),
          ...chats.map((chat) => ({
            id: chat.id,
            pinned: !!chat.pinned,
            node: renderRow(chat),
          })),
        ]
          .sort(
            (a, b) =>
              Number(b.pinned) - Number(a.pinned) ||
              sorting.rank(
                a.pinned
                  ? chatGroup(chats.find((c) => c.id === a.id)!)
                  : itemGroup(group.path, parent?.id || null),
                a.id,
              ) -
                sorting.rank(
                  b.pinned
                    ? chatGroup(chats.find((c) => c.id === b.id)!)
                    : itemGroup(group.path, parent?.id || null),
                  b.id,
                ),
          )
          .map((item) => (
            <Fragment key={item.id}>{item.node}</Fragment>
          ))}
        {!chats.length &&
          !children.length &&
          (parent ||
            !visibleSharedRooms.some((r) => r.projectPath === group.path)) &&
          (parent ||
            !peerTeams.some((team) => team.projectPath === group.path)) && (
            <p className="project-empty">No chats</p>
          )}
      </>
    );
  };
  const content = (
    <>
      <span className="sr-only" role="status">
        {sorting.announcement}
      </span>
      <div className="sidebar-header">
        <a
          className="brand"
          href={serverViewId ? location.pathname + location.search : "/"}
        >
          <span className="brand-name">
            <strong>LAYR</strong> <span>Studio</span>
          </span>
        </a>
        <SidebarWarningsTarget />
        <ActionIcon
          className="sidebar-search"
          aria-label="Search chats"
          title="Search chats (Ctrl+K / ⌘K)"
          aria-keyshortcuts="Control+k Meta+k"
          onClick={p.onSearch}
        >
          <Search size={18} />
        </ActionIcon>
        {p.settings && (
          <ActionIcon
            variant="subtle"
            aria-label="Studio settings"
            title="Settings"
            onClick={p.settings}
          >
            <Settings size={18} />
          </ActionIcon>
        )}
        {compact && (
          <ActionIcon aria-label="Close conversations" onClick={closeSidebar}>
            <X size={20} />
          </ActionIcon>
        )}
      </div>
      {p.notice && (
        <p role="alert" className="notice">
          {p.notice}
        </p>
      )}
      <TextInput
        id="chat-search"
        type="search"
        placeholder="Filter sidebar"
        aria-label="Filter projects and chats"
        leftSection={<Search size={15} />}
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
        }}
      />
      {teamMove.feedback}
      <nav id="chat-list" className="chat-scroll" aria-label="Chats">
        <SidebarAttention rows={filtered} renderRow={renderRow} />
        <div className="projects-heading">
          <span className="projects-heading-label">
            {archive ? "Archived projects" : "Projects"}
          </span>
          <Menu position="bottom-end" withinPortal>
            <Menu.Target>
              <ActionIcon aria-label="Project list options">
                <MoreHorizontal size={16} />
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown className="fleet-menu">
              <Menu.Item onClick={() => setArchive(!archive)}>
                {archive ? "Show active chats" : "Show archived chats"}
              </Menu.Item>
              {/* A narrow sidebar hides these two buttons; the menu keeps them. */}
              {p.newSharedChat && (
                <Menu.Item onClick={() => p.newSharedChat?.()}>
                  New shared chat
                </Menu.Item>
              )}
              <Menu.Item onClick={p.addProject}>Add project</Menu.Item>
            </Menu.Dropdown>
          </Menu>
          <ActionIcon
            className="sidebar-new-chat"
            aria-label="New chat"
            title="New chat"
            disabled={p.creating}
            onClick={() => p.newChat()}
          >
            <SquarePen size={16} />
          </ActionIcon>
          {p.newSharedChat && (
            <ActionIcon
              className="sidebar-shared-chat"
              aria-label="New shared chat"
              title="New shared chat"
              onClick={() => p.newSharedChat?.()}
            >
              <Users size={16} />
            </ActionIcon>
          )}
          <ActionIcon
            className="sidebar-add-project"
            aria-label="Add project"
            onClick={p.addProject}
          >
            <Plus size={18} />
          </ActionIcon>
        </div>
        {projectGroups.map((group) => {
          const oldChatCount = Math.max(
            0,
            filtered.filter((agent) => agent.projectPath === group.path)
              .length - group.chats.length,
          );
          const closed = !!collapsed[projectTreeKey(group.path)] && !query;
          return (
            <section
              className="sidebar-project"
              data-project-path={group.path}
              key={group.path}
            >
              <div className="project-tree-heading" {...folderDrop(group)}>
                <UnstyledButton
                  type="button"
                  {...sorting.bindings(
                    "projects",
                    group.path,
                    allProjectGroups.map((item) => item.path),
                  )}
                  aria-description="Drag to reorder. Alt + Up or Down also works."
                  className="project-tree-toggle"
                  aria-label={group.name}
                  title={services.displayPath?.(group.path) || group.path}
                  aria-expanded={!closed}
                  onClick={() => toggleTree(projectTreeKey(group.path))}
                >
                  {closed ? <Folder size={18} /> : <FolderOpen size={18} />}
                  <span className="project-tree-name" title={group.name}>
                    {group.name}
                  </span>
                  <SidebarProjectStates
                    rows={byProject.get(group.path) || []}
                    archived={archive}
                  />
                </UnstyledButton>
                {!archive &&
                  (!compact || !!group.path) &&
                  (() => {
                    const blocked = unavailableReason(
                      projectTarget(group.path),
                    );
                    // A disabled button gets no pointer events, so an unavailable
                    // owner keeps events to show its reason and ignores the click.
                    return (
                      <Tooltip label={blocked} disabled={!blocked} withinPortal>
                        <ActionIcon
                          className="project-tree-action"
                          aria-label={`New chat in ${group.name}`}
                          {...(blocked ? { "aria-description": blocked } : {})}
                          {...(blocked ? { "aria-disabled": true } : {})}
                          {...(blocked ? { "data-disabled": true } : {})}
                          onClick={() => {
                            if (!blocked) p.newChat(group.path || undefined);
                          }}
                          disabled={p.creating}
                        >
                          <Plus size={15} />
                        </ActionIcon>
                      </Tooltip>
                    );
                  })()}
                {!!group.path && (
                  <Menu withinPortal position="bottom-end">
                    <Menu.Target>
                      <ActionIcon
                        className="project-tree-action"
                        aria-label={`Options for project ${group.name}`}
                      >
                        <MoreHorizontal size={15} />
                      </ActionIcon>
                    </Menu.Target>
                    <Menu.Dropdown className="fleet-menu">
                      <Menu.Label>Create</Menu.Label>
                      <Menu.Item
                        disabled={!canEdit(projectTarget(group.path))}
                        onClick={() => editProject(group, "new")}
                      >
                        New chat folder
                      </Menu.Item>
                      {p.newSharedChat && (
                        <Menu.Item
                          disabled={!canEdit(projectTarget(group.path))}
                          onClick={() => p.newSharedChat?.(group.path)}
                        >
                          New shared chat
                        </Menu.Item>
                      )}
                      {runtime?.peerTeamsVersion === 1 && (
                        <Menu.Item
                          disabled={!canEdit(projectTarget(group.path))}
                          onClick={() =>
                            setTeamDialog({
                              path: group.path,
                              action: "save",
                            })
                          }
                        >
                          New team
                        </Menu.Item>
                      )}
                      <Menu.Divider />
                      <Menu.Label>View</Menu.Label>
                      {(viewChatCounts.get(group.path) || 0) >=
                        hideOldChatsThreshold && (
                        <Menu.Item
                          onClick={() =>
                            setCompactProject(
                              group.path,
                              !isCompactProject(group.path),
                            )
                          }
                        >
                          {isCompactProject(group.path)
                            ? `Show old (${oldChatCount})`
                            : "Hide old"}
                        </Menu.Item>
                      )}
                      <Menu.Divider />
                      <Menu.Label>Project settings</Menu.Label>
                      <Menu.Item
                        disabled={!canEdit(projectTarget(group.path))}
                        onClick={() => editProject(group)}
                      >
                        Rename project
                      </Menu.Item>
                      {p.projectFolders && (
                        <Menu.Item
                          disabled={!canEdit(projectTarget(group.path))}
                          onClick={() => p.projectFolders?.(group.path)}
                        >
                          Folders
                        </Menu.Item>
                      )}
                      <Menu.Item
                        disabled={!canEdit(projectTarget(group.path))}
                        onClick={() => p.projectAccount(group.path)}
                      >
                        Project account
                      </Menu.Item>
                      {group.registered && p.deleteProject && (
                        <Menu.Item
                          color="red"
                          disabled={!canEdit(projectTarget(group.path))}
                          onClick={() => p.deleteProject?.(group.path)}
                        >
                          Delete project…
                        </Menu.Item>
                      )}
                    </Menu.Dropdown>
                  </Menu>
                )}
              </div>
              {teamDialog?.path === group.path &&
                (teamDialog.id &&
                !peerTeams.some((t) => t.id === teamDialog.id) ? (
                  <p role="alert">
                    This team is no longer available.{" "}
                    <UnstyledButton onClick={() => setTeamDialog(null)}>
                      Close
                    </UnstyledButton>
                  </p>
                ) : (
                  <SidebarBackendProvider
                    value={services.owner(
                      projectTarget(group.path, undefined, teamDialog.id),
                    )}
                  >
                    <PeerTeamForm
                      key={`${teamDialog.path}:${teamDialog.id || "new"}:${teamDialog.action}`}
                      project={ownedProject(
                        group,
                        projectTarget(group.path, undefined, teamDialog.id),
                      )}
                      team={peerTeams.find((t) => t.id === teamDialog.id)}
                      teams={peerTeams}
                      agents={agents.filter(
                        (agent) =>
                          services.serverForChat(agent.id) ===
                          services.owner(
                            projectTarget(group.path, undefined, teamDialog.id),
                          ).ownerId,
                      )}
                      action={teamDialog.action}
                      refresh={p.refresh}
                      close={() => setTeamDialog(null)}
                    />
                  </SidebarBackendProvider>
                ))}
              {!closed && (
                <div className="project-chats">
                  {visibleSharedRooms
                    .filter((r) => r.projectPath === group.path)
                    .map((room) => {
                      const alias =
                        services.alias?.({ kind: "chat", id: room.id }) ||
                        aliases[services.serverForChat(room.id)] ||
                        DEFAULT_LOCAL_SERVER_ALIAS;
                      const serverOffline =
                        p.offlineServers?.has(
                          services.serverForChat(room.id),
                        ) ?? false;
                      const chatRow = (
                        <UnstyledButton
                          className="chat-row"
                          aria-description={
                            serverOffline
                              ? `${alias} is unreachable. Showing the last known state.`
                              : undefined
                          }
                          aria-current={
                            p.opened === room.id ? "page" : undefined
                          }
                          onClick={() => openChat(room.id)}
                        >
                          <span className="row-copy">
                            <strong>{room.name}</strong>
                            {services.serverForChat(room.id) !==
                              services.cache.ownerId && (
                              <ChatServerLine
                                provider={
                                  p.data.threads.find((agent) =>
                                    room.members?.includes(agent.id),
                                  )?.provider || undefined
                                }
                                alias={alias}
                                offline={serverOffline}
                              />
                            )}
                            {serverOffline &&
                              services.serverForChat(room.id) ===
                                services.cache.ownerId && (
                                <span
                                  className="fleet-server-badge"
                                  data-offline="true"
                                >
                                  Offline
                                </span>
                              )}
                          </span>
                        </UnstyledButton>
                      );
                      return (
                        <div
                          key={room.id}
                          className={`sidebar-row lead-row ${p.opened === room.id ? "selected" : ""}`}
                        >
                          {chatRow}
                        </div>
                      );
                    })}
                  {renderProjectChats(group)}
                  {!query &&
                    (viewChatCounts.get(group.path) || 0) >=
                      hideOldChatsThreshold &&
                    (isCompactProject(group.path)
                      ? filtered.filter((a) => a.projectPath === group.path)
                          .length > group.chats.length
                      : filtered.some((a) => a.projectPath === group.path)) && (
                      <UnstyledButton
                        className="project-show-all"
                        onClick={() =>
                          setCompactProject(
                            group.path,
                            !isCompactProject(group.path),
                          )
                        }
                      >
                        {isCompactProject(group.path)
                          ? `Show old (${oldChatCount})`
                          : "Hide old"}
                      </UnstyledButton>
                    )}
                </div>
              )}
            </section>
          );
        })}
        <RemoteProjects groups={remoteGroups} />
        {!filtered.length && !projectGroups.length && !remoteGroups.length && (
          <p className="notice">
            {query ? "No matching chats." : "No chats yet."}
          </p>
        )}
      </nav>
    </>
  );
  const dialogProject = dialog ? groupMap.get(dialog.path) : undefined;
  const dialogAgent = dialog?.agentId
    ? agents.find((item) => item.id === dialog.agentId)
    : undefined;
  const dialogFolder = dialogProject?.folders?.find(
    (item) => item.id === dialog?.folder,
  );
  const dialogAvailable =
    dialogProject &&
    (!dialog?.agentId || dialogAgent) &&
    (!dialog?.folder || dialog.folder === "new" || dialogFolder);
  return (
    <>
      <SidebarFrame
        scope={`${p.services.cache.ownerId}:${p.data.stateDir}`}
        opened={p.mobile}
        {...(p.collapsed !== undefined ? { collapsed: p.collapsed } : {})}
        close={closeSidebar}
        onCompactChange={reportCompact}
        {...(p.settings
          ? { drawerZIndex: layoutValues.layers.navigation }
          : {})}
        onEntered={() => {
          if (
            lastOpened.current &&
            document.getElementById("sidebar")?.contains(document.activeElement)
          )
            document
              .querySelector<HTMLElement>(
                `#sidebar [data-chat="${CSS.escape(lastOpened.current)}"]`,
              )
              ?.focus();
        }}
      >
        {content}
      </SidebarFrame>
      <Modal
        opened={!!conversion}
        onClose={closeConversion}
        title="Make chat a subagent"
        closeOnClickOutside={false}
      >
        {conversion &&
          (rows.some((row) => row.id === conversion.source.id) &&
          rows.some((row) => row.id === conversion.target.id) ? (
            <SidebarBackendProvider
              value={services.owner({ kind: "chat", id: conversion.source.id })}
            >
              <ConvertChatForm
                key={`${conversion.source.id}:${conversion.target.id}`}
                project={conversion.project}
                source={conversion.source}
                target={conversion.target}
                scope={p.data.stateDir}
                saved={async () => {
                  await p.refresh?.();
                  openChat(conversion.target.id);
                  setConversion(null);
                }}
              />
            </SidebarBackendProvider>
          ) : (
            <p>Load the chat server to retry the saved request.</p>
          ))}
      </Modal>
      <Modal
        opened={!!dialog}
        onClose={() => setDialog(null)}
        title={dialog?.title}
      >
        {dialog &&
          (dialogAvailable && dialogProject ? (
            dialogAgent ? (
              <SidebarBackendProvider
                value={services.owner({ kind: "chat", id: dialogAgent.id })}
              >
                <MoveChatForm
                  project={ownedProject(dialogProject, {
                    kind: "chat",
                    id: dialogAgent.id,
                  })}
                  agent={dialogAgent}
                  saved={savedProject}
                />
              </SidebarBackendProvider>
            ) : (
              <SidebarBackendProvider
                value={services.owner(
                  projectTarget(
                    dialogProject.path,
                    dialog.folder === "new" ? dialog.parentId : dialog.folder,
                  ),
                )}
              >
                <ProjectNameForm
                  project={ownedProject(
                    dialogProject,
                    projectTarget(
                      dialogProject.path,
                      dialog.folder === "new" ? dialog.parentId : dialog.folder,
                    ),
                  )}
                  displayPath={services.displayPath?.(
                    dialogProject.path,
                    projectTarget(
                      dialogProject.path,
                      dialog.folder === "new" ? dialog.parentId : dialog.folder,
                    ),
                  )}
                  folder={dialog.folder === "new" ? "new" : dialogFolder}
                  parentId={dialog.parentId}
                  saved={savedProject}
                />
              </SidebarBackendProvider>
            )
          ) : (
            <p role="alert">
              This project, chat, or folder is no longer available.
            </p>
          ))}
      </Modal>
    </>
  );
});
