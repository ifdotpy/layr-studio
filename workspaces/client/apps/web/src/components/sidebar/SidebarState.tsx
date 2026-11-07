import { compareShallow, computed } from "mobx";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { errorText } from "../../api";
import { outboxRequests } from "../../browserPersistence";
import type { paths } from "../../generated/api";
import { createStateMirror } from "../../generated/observableResources.ts";
import { layoutQueries } from "../../layout/layout-contract.generated";
import { useServerStore } from "../../servers/ServerServicesContext.tsx";
import type { ChatRowActions } from "../chat-row/actions.ts";
import { type Project, type ProjectFolder } from "../ProjectOrganization";
import { usePeerTeamMove } from "../shell/messages/PeerTeams";

import { useSidebarCommands } from "../sidebar/commands.ts";
import {
  useSidebarVisualState,
  type SidebarServices,
  type SidebarTarget,
} from "../sidebar/services";
import { restoreSidebarFocus } from "../sidebar/SidebarFrame";
import {
  createSidebarList,
  createSidebarViewModel,
  SidebarProjectView,
  type SidebarChatView,
} from "../sidebar/viewModel.ts";
import { useSidebarOrder } from "../useSidebarOrder";

import { readServerAliases } from "../../servers/registry";
import { useServerAliases } from "../../servers/serverAliases";
import { itemGroup } from "../sidebar/catalog";
import { remoteProjectGroups } from "../sidebar/RemoteProjects";

import { DEFAULT_HIDE_OLD_CHATS_THRESHOLD } from "../../studioPreferences";
import { reportPromptComposerRender } from "../prompt-composer/renderProbe";

import { createArea } from "../area.ts";
import type { SidebarProps } from "../Sidebar.tsx";
type OrganizationRequest =
  paths["/api/organization"]["post"]["requestBody"]["content"]["application/json"];
type WithoutOrganizationId<Request> = Request extends { id: string }
  ? Omit<Request, "id">
  : never;
type OrganizationData = WithoutOrganizationId<OrganizationRequest>;
export type NavigableProject = SidebarProjectView;
const emptyCollections = createStateMirror().current;

export function useSidebarController(
  p: SidebarProps & { services: SidebarServices },
) {
  const services = p.services;
  const commands = useSidebarCommands();
  const store = useServerStore();
  const resource =
    p.rows === undefined && p.collections === undefined
      ? store.state().current
      : undefined;
  const collections =
    p.collections ??
    (resource?.status === "live" || resource?.status === "stale"
      ? resource.data
      : emptyCollections);
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
  ): Project => services.project?.(project.path, target) || project.project;
  const ownsFolder = (
    project: NavigableProject,
    chat: SidebarChatView,
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
    commands.open(id);
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
  const workspace = p.rows
    ? undefined
    : collections.workspaces.values().next().value;
  const stateDir = p.rows ? p.data.stateDir : workspace?.stateDir || "";
  const sorting = useSidebarOrder(
    `codex-sidebar-order:${stateDir}`,
    (p.rows ? runtime?.sidebarOrder : workspace?.sidebarOrder) ?? undefined,
    commands.refresh,
    commands.notify,
    services,
    p.data.token,
  );
  const model = useMemo(() => {
    const input = () => ({
      query,
      archived: archive,
      opened: p.opened,
      rank: sorting.rank,
    });
    return p.rows
      ? createSidebarList(() => p.rows!, input)
      : createSidebarViewModel(() => collections, input);
  }, [collections, p.rows, query, archive, p.opened, sorting.order]);
  const [organizing, setOrganizing] = useState<string | null>(null);
  const organizationLock = useRef(false);
  type Conversion = {
    source: SidebarChatView;
    target: SidebarChatView;
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
          `${stateDir}:${conversion.source.id}:${conversion.target.id}`,
        )
      ) {
        commands.notify?.(
          "Retry the saved request to confirm its result before closing.",
        );
        return;
      }
    } catch {
      commands.notify?.(
        "Load the chat server to check the saved request before closing.",
      );
      return;
    }
    setConversion(null);
  };
  const compactKey = `codex-project-compact:${stateDir}`;
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
  const peerTeams = computed(() =>
    p.rows
      ? runtime?.peerTeamsVersion === 1
        ? runtime.peerTeams || []
        : []
      : [...collections.peerTeams.values()],
  );
  const projects = computed(() =>
    (p.rows
      ? (runtime?.projects ?? [])
      : [...collections.projects.values()]
    ).filter(
      (project): project is Project & { path: string } =>
        typeof project.path === "string",
    ),
  );
  const teamMove = usePeerTeamMove(
    commands.refresh,
    commands.notify,
    (path, member, team) =>
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
    commands.notify?.(
      "Restart Studio after active work finishes to edit project names and folders.",
    );
    return false;
  };
  const savedProject = async () => {
    await commands.refresh?.();
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
  const projectKey = `codex-project-tree:${stateDir}`;
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
      commands.notify?.("The sidebar server is offline or unavailable.");
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
      if (hasOptimistic) void commands.refresh?.();
      else await commands.refresh?.();
      return true;
    } catch (error) {
      if (hasOptimistic)
        setOverrides((old) => {
          const next = { ...old };
          delete next[id];
          return next;
        });
      commands.notify?.(errorText(error));
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
        const thread = model.rows.find((a) => a.id === id);
        if (
          !thread ||
          ((values.pinned === undefined ||
            thread.agent.pinned === values.pinned) &&
            (values.archived === undefined ||
              thread.agent.archived === values.archived))
        ) {
          delete next[id];
          changed = true;
        }
      }
      return changed ? next : old;
    });
  }, [model.rows]);
  const findRow = (id: string) => model.rows.find((row) => row.id === id);
  const grouped = computed(
    () =>
      new Set(
        model.rows
          .filter((row) => row.team?.projectPath === row.agent.projectPath)
          .map((row) => row.id),
      ),
  );
  const teamFor = (id: string) =>
    peerTeams.get().find((team) => team.id === findRow(id)?.teamId);
  const sharedRooms = computed(() =>
    (runtime?.rooms || []).filter((r) => r.radio?.direct && !r.userHidden),
  );
  const visibleSharedRooms = computed(() =>
    archive
      ? []
      : sharedRooms
          .get()
          .filter((r) =>
            (
              r.name +
              " " +
              (r.projectPath || "") +
              " " +
              (projects.get().find((project) => project.path === r.projectPath)
                ?.name || "")
            )
              .toLowerCase()
              .includes(query.toLowerCase()),
          ),
  );
  const selectedFolder = findRow(p.opened || "")?.projectFolder;
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
      model.rows.some((a) => a.id === p.opened) ||
      sharedRooms.get().some((r) => r.id === p.opened)
    ) {
      setQuery("");
      const path =
        model.rows.find((a) => a.id === p.opened)?.projectPath ||
        sharedRooms.get().find((r) => r.id === p.opened)?.projectPath;
      const folders =
        projects.get().find((project) => project.path === path)?.folders || [];
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
  const chatGroup = (agent: SidebarChatView) => agent.groupKey;
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
        const chat = findRow(source.id);
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
        const chat = findRow(id)!;
        if (!folder && teamFor(id)) {
          teamMove.move(project.project, id, null);
          return;
        }
        void organize(id, {
          project_path: project.path,
          project_folder: folder?.id || null,
          expected_folder: chat.projectFolder || null,
          expected_revision: chat.agent.projectFolderRevision || 0,
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
            `Moved ${chat.agent.name} to ${folder?.name || project.name}`,
          );
        });
      },
      reorder,
    );
  const rename = async (e: React.FormEvent, id: string) => {
    e.preventDefault();
    try {
      await commands.rename(id, name);
      setRenaming(null);
    } catch {
      /* Keep the draft when the server rejects the name. */
    }
  };
  const sharedProjectPaths = computed(
    () => new Set(sharedRooms.get().map((room) => room.projectPath)),
  );
  const nowSeconds = Date.now() / 1000;
  const keptInCompact = (row: SidebarChatView) =>
    row.compactVisible(nowSeconds);
  const viewChatCounts = computed(() => {
    const counts = new Map<string, number>();
    for (const row of model.rows)
      if (!!row.agent.archived === archive)
        counts.set(row.projectPath, (counts.get(row.projectPath) || 0) + 1);
    return counts;
  });
  const isCompactProject = (path: string) =>
    compactProjects[path] !== false &&
    (viewChatCounts.get().get(path) || 0) >= hideOldChatsThreshold;
  const groupMap = computed(() => {
    const groups = new Map<string, SidebarProjectView>();
    for (const project of projects.get())
      groups.set(
        project.path,
        new SidebarProjectView(
          project,
          p.registeredProjectPaths?.has(project.path) ?? true,
          model.groups.has(project.path) ||
            sharedProjectPaths.get().has(project.path),
        ),
      );
    for (const path of new Set([
      ...model.rows.map((row) => row.projectPath || ""),
      ...sharedRooms.get().map((room) => room.projectPath || ""),
    ]))
      if (!groups.has(path))
        groups.set(
          path,
          new SidebarProjectView(
            {
              id: path,
              path,
              name: path.split("/").filter(Boolean).at(-1) || "Other chats",
            },
            false,
            true,
          ),
        );
    for (const a of model.visibleRows) {
      if (
        query ||
        !isCompactProject(a.projectPath || "") ||
        a.id === p.opened ||
        keptInCompact(a)
      )
        groups.get(a.projectPath || "")!.chats.push(a);
    }
    return groups;
  });
  const itemIds = (group: NavigableProject, parent: string | null = null) =>
    [
      ...(!parent
        ? peerTeams
            .get()
            .filter((t) => t.projectPath === group.path)
            .map((t) => teamKey(group.path, t.id))
        : []),
      ...(group.folders || [])
        .filter((f) => (f.parentId || null) === parent)
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((f) => folderKey(group.path, f.id)),
      ...(model.groups.get(group.path) || [])
        .filter(
          (a) =>
            !!a.agent.archived === archive &&
            !grouped.get().has(a.id) &&
            !a.agent.pinned &&
            (a.projectFolder || null) === parent,
        )
        .map((a) => a.id),
    ].sort(
      (a, b) =>
        sorting.rank(itemGroup(group.path, parent), a) -
        sorting.rank(itemGroup(group.path, parent), b),
    );
  const projectPaths = computed(
    () =>
      [...groupMap.get().values()]
        .sort(
          (a, b) =>
            sorting.rank("projects", a.path) -
              sorting.rank("projects", b.path) || a.name.localeCompare(b.name),
        )
        .map((group) => group.path),
    { equals: compareShallow },
  );
  const allProjectGroups = computed(() =>
    projectPaths.get().map((path) => groupMap.get().get(path)!),
  );
  const remoteGroups = useMemo(
    () => remoteProjectGroups(p.remoteProjects, query),
    [p.remoteProjects, query],
  );
  const projectGroups = computed(() =>
    allProjectGroups
      .get()
      .filter(
        (group) =>
          (group.registered ||
            viewChatCounts.get().has(group.path) ||
            visibleSharedRooms
              .get()
              .some((r) => r.projectPath === group.path)) &&
          (!query ||
            group.chats.length ||
            visibleSharedRooms
              .get()
              .some((r) => r.projectPath === group.path) ||
            `${group.path} ${group.name}`
              .toLowerCase()
              .includes(query.toLowerCase())),
      ),
  );
  const rowActions = (
    row: SidebarChatView,
    priority: boolean,
  ): ChatRowActions => ({
    prepare: () => commands.prepareChat?.(row.id),
    open: () => openChat(row.id),
    pin: () => {
      void organize(row.id, { pinned: !row.agent.pinned });
    },
    rename: (event) => {
      void rename(event, row.id);
    },
    name: setName,
    cancelRename: () => setRenaming(null),
    beginRename: () => {
      setName(row.agent.name ?? "");
      setRenameInPriority(priority);
      setRenaming(row.id);
    },
    unread: () => {
      if (row.agent.markRead)
        commands.markUnread(row.id, { ...row.agent.markRead });
    },
    move: () => {
      if (!requireProjectSupport()) return;
      const project = groupMap.get().get(row.projectPath || "")!;
      setDialog({
        title: "Move chat",
        path: project.path,
        agentId: row.id,
      });
    },
    changeProject: () => commands.changeProject(row.id),
    archive: () => {
      void organize(row.id, { archived: !row.agent.archived });
    },
    remove: () => commands.remove(row.id, false),
  });

  const rowBindings = (row: SidebarChatView) => ({
    ...sorting.dropBindings(
      `convert:${row.id}`,
      (source, event) => {
        const chat = findRow(source.id);
        const box = event.currentTarget.getBoundingClientRect();
        return !!(
          chat &&
          canEdit({ kind: "chat", id: chat.id }) &&
          canEdit({ kind: "chat", id: row.id }) &&
          services.serverForChat(chat.id) === services.serverForChat(row.id) &&
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
          source: findRow(id)!,
          target: row,
          project: groupMap.get().get(row.projectPath || "")!,
        }),
      sorting.bindings(
        chatGroup(row),
        row.id,
        teamFor(row.id) || row.agent.pinned
          ? model.orderedRows
              .filter((a) => chatGroup(a) === chatGroup(row))
              .map((a) => a.id)
          : itemIds(
              groupMap.get().get(row.projectPath || "")!,
              row.projectFolder || null,
            ),
      ),
    ),
    draggable: canEdit({ kind: "chat", id: row.id }),
  });

  return {
    p,
    services,
    commands,
    lastOpened,
    canEdit,
    unavailableReason,
    projectTarget,
    ownedProject,
    runtime,
    aliases,
    compact,
    reportCompact,
    closeSidebar,
    openChat,
    query,
    setQuery,
    renameInPriority,
    hideOldChatsThreshold,
    stateDir,
    sorting,
    organizing,
    organizationLock,
    conversion,
    setConversion,
    closeConversion,
    setCompactProject,
    teamDialog,
    setTeamDialog,
    get peerTeams() {
      return peerTeams.get();
    },
    teamMove,
    dialog,
    setDialog,
    requireProjectSupport,
    savedProject,
    editProject,
    folderKey,
    projectTreeKey,
    collapsed,
    toggleTree,
    overrides,
    findRow,
    get grouped() {
      return grouped.get();
    },
    get visibleSharedRooms() {
      return visibleSharedRooms.get();
    },
    teamKey,
    chatGroup,
    middleDrop,
    folderDrop,
    get viewChatCounts() {
      return viewChatCounts.get();
    },
    isCompactProject,
    get groupMap() {
      return groupMap.get();
    },
    itemIds,
    get allProjectGroups() {
      return allProjectGroups.get();
    },
    remoteGroups,
    get projectGroups() {
      return projectGroups.get();
    },
    model,
    rowActions,
    rowBindings,
    renaming,
    name,
    archive,
    setArchive,
  };
}
export const [SidebarStateProvider, useSidebarState] =
  createArea<ReturnType<typeof useSidebarController>>("Sidebar state");
