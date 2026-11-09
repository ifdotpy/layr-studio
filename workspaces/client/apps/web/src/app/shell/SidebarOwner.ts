import { COMPACT_RECENT_WINDOW_SECONDS } from "../../generated/sharedValues.ts";
import { remoteProjectGroups } from "../../components/sidebar/RemoteProjects/RemoteProjects.tsx";
import { SidebarFrameOwner } from "../../components/sidebar/SidebarFrame/SidebarFrame.tsx";
import {
  createSidebarActions,
  type SidebarActions,
} from "../../components/sidebar/support/sidebarActions.ts";
import {
  compareShallow,
  computed,
  makeAutoObservable,
  observable,
  reaction,
  runInAction,
} from "mobx";
import { errorText } from "../../api.ts";
import { outboxRequests } from "../../browserPersistence.ts";
import type { paths } from "../../generated/api.ts";
import { createStateMirror } from "../../generated/observableResources.ts";
import type {
  Project,
  ProjectFolder,
} from "../projects/ProjectOrganization.tsx";
import { PeerTeamMoveOwner } from "../projects/peerTeamMutation.ts";

import type { SidebarCommands } from "../../components/sidebar/support/commands.ts";
import type {
  SidebarServices,
  SidebarTarget,
} from "../../components/sidebar/support/services.tsx";
import {
  createSidebarList,
  localSidebarIdentity,
  createSidebarViewModel,
  SidebarProjectView,
  type SidebarChatView,
} from "../../components/sidebar/support/viewModel.ts";
import { SidebarOrderOwner } from "../../components/sidebar/support/useSidebarOrder.ts";

import { readServerAliases } from "../../servers/registry.ts";
import { observeServerAliases } from "../../servers/serverAliases.ts";
import { itemGroup } from "../../components/sidebar/support/catalog.ts";

import type { MergedSidebar } from "../../servers/mergedSidebar.ts";
type OrganizationRequest =
  paths["/api/organization"]["post"]["requestBody"]["content"]["application/json"];
type WithoutOrganizationId<Request> = Request extends { id: string }
  ? Omit<Request, "id">
  : never;
type OrganizationData = WithoutOrganizationId<OrganizationRequest>;
export type NavigableProject = SidebarProjectView;
const emptyCollections = createStateMirror().current;

import type { ServerServices } from "../../servers/serverServices.ts";
import type { ServerAreas } from "../ServerAreasProvider.tsx";
import {
  serverViewId,
  isClassicServerView,
} from "../../servers/environment.ts";

import { restoreSidebarFocus } from "../../components/sidebar/SidebarFrame/SidebarFrame.tsx";
/** Visibility choices only. SidebarFrame remains the single sidebar width authority. */
export class SidebarOwner {
  mobileOpen = false;
  desktopCollapsed: boolean;
  measuredCompact: boolean | undefined;
  readonly areas;
  constructor(areas: Pick<ServerAreas, "scope" | "layout" | "frame">) {
    this.areas = areas;
    this.desktopCollapsed = areas.scope.services.api.saved(
      "codex-sidebar-collapsed",
      false,
    );
    makeAutoObservable(this, { areas: false }, { autoBind: true });
  }
  get delegated() {
    return (
      !!this.areas.scope.screen.channel ||
      (!!serverViewId && !isClassicServerView)
    );
  }
  get isCompact() {
    return this.measuredCompact ?? this.areas.layout.isMobile;
  }
  get isExpanded() {
    return this.isCompact ? this.mobileOpen : !this.desktopCollapsed;
  }
  get isVisible() {
    return !this.delegated && this.isExpanded;
  }
  get hasDesktopSidebar() {
    return (
      !this.delegated && !this.areas.layout.isMobile && !this.desktopCollapsed
    );
  }
  get toggleLabel() {
    return this.isCompact ? "Back to chats" : "Toggle conversations";
  }
  recordCompact(compact: boolean) {
    this.measuredCompact = compact;
  }
  toggle() {
    if (!this.areas.scope.screen.active) return;
    if (this.delegated) {
      this.areas.frame.publish({
        kind: "studio-server-toggle-sidebar",
        ...(this.isCompact ? { opened: true } : {}),
      });
      return;
    }
    restoreSidebarFocus();
    if (this.isCompact) this.mobileOpen = !this.mobileOpen;
    else {
      this.desktopCollapsed = !this.desktopCollapsed;
      this.areas.scope.services.api.save(
        "codex-sidebar-collapsed",
        this.desktopCollapsed,
      );
    }
  }
  close() {
    this.mobileOpen = false;
  }
  show() {
    this.mobileOpen = true;
  }
  showAndFocusSearch() {
    this.mobileOpen = true;
    requestAnimationFrame(() => {
      if (this.areas.scope.screen.active)
        document.querySelector<HTMLElement>("#chat-search")?.focus();
    });
  }
}

export interface CombinedSidebarBoundary {
  readonly model: MergedSidebar;
  readonly commands: Pick<SidebarCommands, "rename" | "notify">;
  readonly run:
    | SidebarAreas["run"]
    | ((command: Parameters<SidebarAreas["run"]>[0]) => void);
  readonly services: SidebarServices;
  readonly visibility: Pick<
    SidebarOwner,
    "isCompact" | "mobileOpen" | "desktopCollapsed" | "recordCompact" | "close"
  >;
  readonly selection: Pick<ServerAreas["navigation"], "selectedId"> &
    Partial<Pick<ServerAreas["navigation"], "navigationIntent">>;
  readonly preferences: {
    readonly value: Pick<
      ServerAreas["preferences"]["value"],
      "hideOldChatsThreshold"
    >;
  };
  readonly read: Pick<ServerAreas["read"], "marking"> &
    Partial<Pick<ServerAreas["read"], "pendingReadTargets">>;
  readonly creation: Pick<
    ServerAreas["creation"],
    "isCreating" | "unreplicatedCreatedAgents" | "lookup"
  >;
}
export type SidebarAreas = Pick<
  ServerAreas,
  | "scope"
  | "sidebar"
  | "navigation"
  | "preferences"
  | "read"
  | "creation"
  | "rename"
  | "notices"
  | "run"
>;
type Conversion = {
  source: SidebarChatView;
  target: SidebarChatView;
  project: NavigableProject;
};
export class SidebarListOwner {
  readonly visibility;
  readonly selection;
  readonly preferences;
  readonly read;
  readonly creation;
  readonly run;
  readonly notify;
  readonly renameChat;
  readonly actions: SidebarActions;
  readonly frame: SidebarFrameOwner;
  readonly state: ReturnType<ServerServices["store"]["state"]>;
  readonly areas: SidebarAreas;
  readonly combined: CombinedSidebarBoundary | undefined;
  constructor(
    areas: SidebarAreas,
    combined?: CombinedSidebarBoundary,
    run: (
      command: import("../../servers/navigation.ts").ServerCommand,
    ) => void | Promise<unknown> = areas.run,
  ) {
    this.areas = areas;
    this.combined = combined;
    this.visibility = combined?.visibility ?? areas.sidebar;
    this.selection = combined?.selection ?? areas.navigation;
    this.preferences = combined?.preferences ?? areas.preferences;
    this.read = combined?.read ?? areas.read;
    this.creation = combined?.creation ?? areas.creation;
    this.run = combined?.run ?? run;
    this.notify = combined?.commands.notify ?? areas.notices.notify;
    this.renameChat = combined?.commands.rename ?? areas.rename.rename;
    this.state = areas.scope.store.state();
    this.lastOpened = { current: this.selection.selectedId };
    this.sorting = new SidebarOrderOwner(this, this.notify);
    this.teamMove = new PeerTeamMoveOwner(
      this.services.cache,
      this.notify,
      (path, member, team) =>
        this.services.owner(
          team
            ? this.projectTarget(path, undefined, team)
            : { kind: "chat", id: member },
        ),
    );
    this.model = this.combined
      ? createSidebarList(
          () => this.combined!.model.rows,
          () => this.listInput,
        )
      : createSidebarViewModel(
          () => this.collections,
          () => this.listInput,
          localSidebarIdentity,
          () => this.creation.unreplicatedCreatedAgents,
        );
    this.actions = createSidebarActions(this);
    this.frame = new SidebarFrameOwner(this);
    makeAutoObservable(
      this,
      {
        areas: false,
        combined: false,
        projectPaths: computed({ equals: compareShallow }),
      },
      { autoBind: true, deep: false },
    );
  }
  get services() {
    return this.combined?.services ?? this.areas.scope.services.sidebarServices;
  }
  get listInput() {
    return {
      query: this.query,
      archived: this.archive,
      opened: this.selection.selectedId,
      rank: this.sorting.rank,
    };
  }
  get collections() {
    return this.state.current.status === "live" ||
      this.state.current.status === "stale"
      ? this.state.current.data
      : emptyCollections;
  }
  lastOpened: { current: string | null };
  canEdit = (target: SidebarTarget) =>
    this.services.available?.(target) ?? true;
  unavailableReason = (target: SidebarTarget) =>
    this.canEdit(target)
      ? null
      : this.services.unavailable?.(target) || "The server is unavailable.";
  projectTarget = (
    path: string,
    folder?: string,
    team?: string,
  ): SidebarTarget => ({ kind: "project", path, folder, team });
  ownedProject = (project: NavigableProject, target: SidebarTarget): Project =>
    this.services.project?.(project.path, target) || project.project;
  ownsFolder = (
    project: NavigableProject,
    chat: SidebarChatView,
    folder: ProjectFolder | null,
  ) => {
    if (!this.services.project || !folder) return true;
    try {
      return !!this.ownedProject(project, {
        kind: "chat",
        id: chat.id,
      }).folders?.some((item) => item.id === folder.id);
    } catch {
      return false;
    }
  };
  get runtime() {
    return this.combined?.model.sidebar ?? this.workspace;
  }
  get rooms() {
    return (
      this.combined?.model.sidebar.rooms ?? [...this.collections.rooms.values()]
    );
  }
  aliases = observable.map(Object.entries(readServerAliases()));
  renaming: string | null = null;
  renameInPriority = false;
  name = "";
  organizing: string | null = null;
  overrides = observable.map<
    string,
    { pinned?: boolean; archived?: boolean }
  >();
  get offline() {
    return this.combined?.model.offlineServers ?? new Set<string>();
  }
  closeSidebar = () => {
    restoreSidebarFocus();
    this.visibility.close();
  };
  openChat = (id: string) => {
    if (this.visibility.isCompact) this.closeSidebar();
    void this.run({ action: "open", id });
  };
  get inline() {
    return !this.visibility.isCompact && !this.visibility.desktopCollapsed;
  }
  query = "";
  setQuery = (value: string) => {
    this.query = value;
  };
  archive = false;
  setArchive = (value: boolean) => {
    this.archive = value;
  };
  get workspace() {
    return this.collections.workspaces.values().next().value;
  }
  focusSelected() {
    if (
      this.lastOpened.current &&
      document.getElementById("sidebar")?.contains(document.activeElement)
    )
      document
        .querySelector<HTMLElement>(
          `#sidebar [data-chat="${CSS.escape(this.lastOpened.current)}"]`,
        )
        ?.focus();
  }
  get stateDir() {
    return (
      this.combined?.model.sidebar.stateDir ?? this.workspace?.stateDir ?? ""
    );
  }
  sorting: SidebarOrderOwner;
  model: ReturnType<typeof createSidebarViewModel>;
  conversion: Conversion | null = null;
  setConversion = (value: Conversion | null) => {
    this.conversion = value;
  };
  closeConversion = () => {
    try {
      if (
        this.conversion &&
        outboxRequests(
          this.services.owner({ kind: "chat", id: this.conversion.source.id })
            .storage,
        ).get(
          "projectSave",
          `${this.stateDir}:${this.conversion.source.id}:${this.conversion.target.id}`,
        )
      ) {
        this.notify(
          "Retry the saved request to confirm its result before closing.",
        );
        return;
      }
    } catch {
      this.notify(
        "Load the chat server to check the saved request before closing.",
      );
      return;
    }
    this.setConversion(null);
  };
  get compactKey() {
    return `codex-project-compact:${this.stateDir}`;
  }
  compactProjects: Record<string, boolean> = {};
  setCompactProjects = (value: Record<string, boolean>) => {
    this.compactProjects = value;
  };
  setCompactProject = (path: string, value: boolean) => {
    const next = { ...this.compactProjects, [path]: value };
    this.setCompactProjects(
      this.services.cache.editPreference(
        this.compactKey,
        this.compactProjects,
        next,
      ),
    );
  };
  teamDialog: {
    path: string;
    id?: string;
    action: "save" | "delete";
  } | null = null;
  setTeamDialog = (value: typeof this.teamDialog) => {
    this.teamDialog = value;
  };
  get peerTeams() {
    return this.combined
      ? this.runtime?.peerTeamsVersion === 1
        ? this.combined!.model.sidebar.peerTeams || []
        : []
      : [...this.collections.peerTeams.values()];
  }
  get projects() {
    return (
      this.combined
        ? (this.combined!.model.sidebar.projects ?? [])
        : [...this.collections.projects.values()]
    ).filter(
      (
        project,
      ): project is Project & {
        path: string;
      } => typeof project.path === "string" && !project.serverLabel,
    );
  }
  teamMove: PeerTeamMoveOwner;
  dialog: {
    title: string;
    path: string;
    folder?: string | undefined;
    parentId?: string | undefined;
    agentId?: string;
  } | null = null;
  setDialog = (value: typeof this.dialog) => {
    this.dialog = value;
  };
  get canOrganizeProjects() {
    return !!this.runtime?.projectOrganizationVersion;
  }
  requireProjectSupport = () => {
    if (this.canOrganizeProjects) return true;
    this.notify(
      "Restart Studio after active work finishes to edit project names and folders.",
    );
    return false;
  };
  savedProject = async () => {
    this.setDialog(null);
  };
  editProject = (
    project: NavigableProject,
    folder?: ProjectFolder | "new",
    parentId?: string,
  ) => {
    if (!this.requireProjectSupport()) return;
    if (
      !this.canEdit(
        this.projectTarget(
          project.path,
          folder === "new" ? parentId : folder?.id,
        ),
      )
    )
      return;
    this.setDialog({
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
  folderKey = (path: string, id: string) =>
    JSON.stringify([path, "folder", id]);
  projectTreeKey = (path: string) => JSON.stringify([path, "project"]);
  get projectKey() {
    return `codex-project-tree:${this.stateDir}`;
  }
  collapsed: Record<string, boolean> = {};
  setCollapsed = (
    value:
      | Record<string, boolean>
      | ((previous: Record<string, boolean>) => Record<string, boolean>),
  ) => {
    this.collapsed =
      typeof value === "function" ? value(this.collapsed) : value;
  };
  toggleTree = (key: string) => {
    const next = { ...this.collapsed, [key]: !this.collapsed[key] };
    this.setCollapsed(
      this.services.cache.editPreference(this.projectKey, this.collapsed, next),
    );
  };
  organize = async (id: string, data: OrganizationData) => {
    if (!this.canEdit({ kind: "chat", id })) {
      this.notify("The sidebar server is offline or unavailable.");
      return false;
    }
    if (this.organizing !== null) return false;
    runInAction(() => {
      this.organizing = id;
    });
    const optimistic = Object.fromEntries(
      Object.entries(data).filter(
        ([key, value]) =>
          (key === "pinned" || key === "archived") &&
          typeof value === "boolean",
      ),
    );
    const hasOptimistic = Object.keys(optimistic).length > 0;
    if (hasOptimistic)
      runInAction(() =>
        this.overrides.set(id, { ...this.overrides.get(id), ...optimistic }),
      );
    try {
      await this.services
        .owner({ kind: "chat", id })
        .post("/api/organization", { id, ...data });
      return true;
    } catch (error) {
      if (hasOptimistic) runInAction(() => this.overrides.delete(id));
      this.notify(errorText(error));
      return false;
    } finally {
      runInAction(() => {
        this.organizing = null;
      });
    }
  };
  findRow = (id: string) => this.model.rows.find((row) => row.id === id);
  get grouped() {
    return new Set(
      this.model.rows
        .filter((row) => row.team?.projectPath === row.agent.projectPath)
        .map((row) => row.id),
    );
  }
  teamFor = (id: string) =>
    this.peerTeams.find((team) => team.id === this.findRow(id)?.teamId);
  get sharedRooms() {
    return (this.rooms || []).filter((r) => r.radio?.direct && !r.userHidden);
  }
  get visibleSharedRooms() {
    return this.archive
      ? []
      : this.sharedRooms.filter((r) =>
          (
            r.name +
            " " +
            (r.projectPath || "") +
            " " +
            (this.projects.find((project) => project.path === r.projectPath)
              ?.name || "")
          )
            .toLowerCase()
            .includes(this.query.toLowerCase()),
        );
  }
  get selectedFolder() {
    return this.findRow(this.selection.selectedId || "")?.projectFolder;
  }

  teamKey = (path: string, id: string) => JSON.stringify([path, "team", id]);
  middleDrop = (event: React.DragEvent<HTMLElement>) => {
    const title = (event.target as Element).closest(
      ".peer-team-toggle, .project-tree-toggle",
    );
    const box = (title || event.currentTarget).getBoundingClientRect();
    return (
      event.clientY > box.top + box.height * 0.25 &&
      event.clientY < box.top + box.height * 0.75
    );
  };
  folderDrop = (
    project: NavigableProject,
    folder: ProjectFolder | null = null,
    reorder:
      | ReturnType<typeof this.sorting.bindings>
      | Record<string, never> = {},
  ) =>
    this.sorting.dropBindings(
      folder ? this.folderKey(project.path, folder.id) : project.path,
      (source, event) => {
        const chat = this.findRow(source.id);
        return !!(
          ((!(event.target as Element).closest(
            ".peer-team-toggle, .project-tree-toggle",
          ) &&
            !reorder.draggable) ||
            this.middleDrop(event)) &&
          (this.canOrganizeProjects || (!folder && this.teamFor(source.id))) &&
          !this.teamMove.blocked() &&
          this.organizing === null &&
          chat &&
          this.canEdit({ kind: "chat", id: chat.id }) &&
          this.ownsFolder(project, chat, folder) &&
          source.group === chat.groupKey &&
          chat.projectPath === project.path &&
          ((!folder && !!this.teamFor(chat.id)) ||
            (chat.projectFolder || null) !== (folder?.id || null))
        );
      },
      ({ id }) => {
        const chat = this.findRow(id)!;
        if (!folder && this.teamFor(id)) {
          this.teamMove.move(project.project, id, null);
          return;
        }
        void this.organize(id, {
          project_path: project.path,
          project_folder: folder?.id || null,
          expected_folder: chat.projectFolder || null,
          expected_revision: chat.agent.projectFolderRevision || 0,
        }).then((moved) => {
          if (!moved) return;
          this.setCollapsed((previous) => {
            const next = {
              ...previous,
              [this.projectTreeKey(project.path)]: false,
            };
            let parent = folder;
            const seen = new Set<string>();
            while (parent && !seen.has(parent.id)) {
              seen.add(parent.id);
              next[this.folderKey(project.path, parent.id)] = false;
              parent =
                project.folders?.find((f) => f.id === parent?.parentId) || null;
            }
            return this.services.cache.editPreference(
              this.projectKey,
              previous,
              next,
            );
          });
          this.setCompactProject(project.path, false);
          this.sorting.announce(
            `Moved ${chat.agent.name} to ${folder?.name || project.name}`,
          );
        });
      },
      reorder,
    );
  rename = async (e: React.FormEvent, id: string) => {
    e.preventDefault();
    try {
      await this.renameChat(id, this.name);
      runInAction(() => {
        this.renaming = null;
      });
    } catch {
      /* Keep the draft when the server rejects the name. */
    }
  };
  get sharedProjectPaths() {
    return new Set(this.sharedRooms.map((room) => room.projectPath));
  }
  nowSeconds = Date.now() / 1000;
  keptInCompact = (row: SidebarChatView) => row.compactVisible(this.nowSeconds);
  get viewChatCounts() {
    const counts = new Map<string, number>();
    for (const row of this.model.rows)
      if (!!row.agent.archived === this.archive)
        counts.set(row.projectPath, (counts.get(row.projectPath) || 0) + 1);
    return counts;
  }
  isCompactProject = (path: string) =>
    this.compactProjects[path] !== false &&
    (this.viewChatCounts.get(path) || 0) >=
      this.preferences.value.hideOldChatsThreshold;
  get groupMap() {
    const groups = new Map<string, SidebarProjectView>();
    for (const project of this.projects)
      groups.set(
        project.path,
        new SidebarProjectView(
          project,
          this.combined?.model.groups.has(project.path) ?? true,
          this.model.groups.has(project.path) ||
            this.sharedProjectPaths.has(project.path),
        ),
      );
    for (const path of new Set([
      ...this.model.rows.map((row) => row.projectPath || ""),
      ...this.sharedRooms.map((room) => room.projectPath || ""),
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
    for (const a of this.model.visibleRows) {
      if (
        this.query ||
        !this.isCompactProject(a.projectPath || "") ||
        a.id === this.selection.selectedId ||
        this.keptInCompact(a)
      )
        groups.get(a.projectPath || "")!.chats.push(a);
    }
    return groups;
  }
  itemIds = (group: NavigableProject, parent: string | null = null) =>
    [
      ...(!parent
        ? this.peerTeams
            .filter((t) => t.projectPath === group.path)
            .map((t) => this.teamKey(group.path, t.id))
        : []),
      ...(group.folders || [])
        .filter((f) => (f.parentId || null) === parent)
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((f) => this.folderKey(group.path, f.id)),
      ...(this.model.groups.get(group.path) || [])
        .filter(
          (a) =>
            !!a.agent.archived === this.archive &&
            !this.grouped.has(a.id) &&
            !a.agent.pinned &&
            (a.projectFolder || null) === parent,
        )
        .map((a) => a.id),
    ].sort(
      (a, b) =>
        this.sorting.rank(itemGroup(group.path, parent), a) -
        this.sorting.rank(itemGroup(group.path, parent), b),
    );
  get projectPaths() {
    return [...this.groupMap.values()]
      .sort(
        (a, b) =>
          this.sorting.rank("projects", a.path) -
            this.sorting.rank("projects", b.path) ||
          a.name.localeCompare(b.name),
      )
      .map((group) => group.path);
  }
  get remoteGroups() {
    return this.combined
      ? []
      : remoteProjectGroups(
          [...this.collections.projects.values()].filter(
            (project) => project.serverLabel,
          ),
          this.query,
        );
  }
  get projectGroups() {
    return this.projectPaths
      .map((path) => this.groupMap.get(path)!)
      .filter(
        (group) =>
          (group.registered ||
            this.viewChatCounts.has(group.path) ||
            this.visibleSharedRooms.some(
              (r) => r.projectPath === group.path,
            )) &&
          (!this.query ||
            group.chats.length ||
            this.visibleSharedRooms.some((r) => r.projectPath === group.path) ||
            `${group.path} ${group.name}`
              .toLowerCase()
              .includes(this.query.toLowerCase())),
      );
  }
  rowBindings = (row: SidebarChatView) => ({
    ...this.sorting.dropBindings(
      `convert:${row.id}`,
      (source, event) => {
        const chat = this.findRow(source.id);
        const box = event.currentTarget.getBoundingClientRect();
        return !!(
          chat &&
          this.canEdit({ kind: "chat", id: chat.id }) &&
          this.canEdit({ kind: "chat", id: row.id }) &&
          this.services.serverForChat(chat.id) ===
            this.services.serverForChat(row.id) &&
          chat.id !== row.id &&
          this.teamFor(chat.id) &&
          chat.projectPath === row.projectPath &&
          source.group === chat.groupKey &&
          !this.conversion &&
          this.organizing === null &&
          !this.teamMove.blocked() &&
          event.clientY > box.top + box.height * 0.25 &&
          event.clientY < box.top + box.height * 0.75
        );
      },
      ({ id }) =>
        this.setConversion({
          source: this.findRow(id)!,
          target: row,
          project: this.groupMap.get(row.projectPath || "")!,
        }),
      this.sorting.bindings(
        row.groupKey,
        row.id,
        this.teamFor(row.id) || row.agent.pinned
          ? this.model.orderedRows
              .filter((a) => a.groupKey === row.groupKey)
              .map((a) => a.id)
          : this.itemIds(
              this.groupMap.get(row.projectPath || "")!,
              row.projectFolder || null,
            ),
      ),
    ),
    draggable: this.canEdit({ kind: "chat", id: row.id }),
  });
  attachPresentation() {
    const root = document.documentElement;
    const stopInline = reaction(
      () => this.inline,
      (inline) => {
        if (inline) root.dataset.inlineSidebar = "true";
        else delete root.dataset.inlineSidebar;
      },
      { fireImmediately: true },
    );
    let restore = () => {};
    const stopBackground = reaction(
      () => this.visibility.isCompact && this.visibility.mobileOpen,
      (hidden) => {
        restore();
        restore = () => {};
        const background =
          document.querySelector<HTMLElement>(".server-views") ||
          document.querySelector<HTMLElement>(".workspace-frame");
        if (!hidden || !background) return;
        const inert = background.inert,
          previous = background.getAttribute("aria-hidden");
        background.inert = true;
        background.setAttribute("aria-hidden", "true");
        restore = () => {
          background.inert = inert;
          if (previous === null) background.removeAttribute("aria-hidden");
          else background.setAttribute("aria-hidden", previous);
        };
      },
      { fireImmediately: true },
    );
    return () => {
      stopInline();
      stopBackground();
      delete root.dataset.inlineSidebar;
      restore();
    };
  }
  start() {
    this.nowSeconds = Date.now() / 1000;
    let expiryTimer: ReturnType<typeof setTimeout> | undefined;
    let releasePreferences = () => {};
    const stopAliases = observeServerAliases(readServerAliases, (aliases) =>
      runInAction(() => this.aliases.replace(Object.entries(aliases))),
    );
    const stops = [
      reaction(
        () =>
          Math.min(
            ...this.model.rows
              .map((row) => row.activityAt + COMPACT_RECENT_WINDOW_SECONDS)
              .filter((expiry) => expiry > this.nowSeconds),
          ),
        (expiry) => {
          clearTimeout(expiryTimer);
          if (Number.isFinite(expiry))
            expiryTimer = setTimeout(
              () =>
                runInAction(() => {
                  this.nowSeconds = Date.now() / 1000;
                }),
              Math.max(1, Math.ceil(expiry * 1000 - Date.now())),
            );
        },
        { fireImmediately: true },
      ),
      this.sorting.start(),
      reaction(
        () => this.selection.selectedId,
        (id) => {
          if (id) this.lastOpened.current = id;
        },
        { fireImmediately: true },
      ),
      reaction(
        () => this.confirmedOverrides,
        (ids) => {
          for (const id of ids) this.overrides.delete(id);
        },
      ),
      reaction(
        () => this.selectionKey,
        (next, previous) => {
          if (next && previous) this.revealSelected();
        },
      ),
      reaction(
        () => [this.compactKey, this.projectKey] as const,
        (keys) => {
          releasePreferences();
          const subscribe = (
            key: string,
            set: (value: Record<string, boolean>) => void,
          ) => {
            const read = () => set(this.services.cache.saved(key, {}));
            read();
            return this.services.cache.subscribePreference(key, read);
          };
          const compact = subscribe(keys[0], this.setCompactProjects),
            tree = subscribe(keys[1], this.setCollapsed);
          releasePreferences = () => {
            compact();
            tree();
          };
        },
        { fireImmediately: true },
      ),
    ];
    return () => {
      clearTimeout(expiryTimer);
      stops.forEach((stop) => stop());
      releasePreferences();
      stopAliases();
    };
  }
  get selectionKey() {
    const { selectedId, navigationIntent } = this.selection;
    return selectedId
      ? JSON.stringify([selectedId, navigationIntent, this.selectedFolder])
      : null;
  }
  get confirmedOverrides() {
    return [...this.overrides]
      .filter(([id, values]) => {
        const row = this.findRow(id);
        return (
          !row ||
          ((values.pinned === undefined ||
            row.agent.pinned === values.pinned) &&
            (values.archived === undefined ||
              row.agent.archived === values.archived))
        );
      })
      .map(([id]) => id);
  }
  revealSelected() {
    const selected =
      this.findRow(this.selection.selectedId || "") ||
      this.sharedRooms.find((room) => room.id === this.selection.selectedId);
    if (!selected) return;
    this.query = "";
    const path = selected.projectPath;
    if (!path) return;
    const folders =
      this.projects.find((project) => project.path === path)?.folders || [];
    let folder = folders.find((item) => item.id === this.selectedFolder);
    const ancestors: Record<string, boolean> = {};
    while (folder && !(this.folderKey(path, folder.id) in ancestors)) {
      ancestors[this.folderKey(path, folder.id)] = false;
      folder = folders.find((item) => item.id === folder!.parentId);
    }
    if (this.collapsed[this.projectTreeKey(path)])
      ancestors[this.projectTreeKey(path)] = false;
    if (Object.keys(ancestors).length)
      this.collapsed = (
        this.services.cache.viewPreference || this.services.cache.editPreference
      )(this.projectKey, this.collapsed, { ...this.collapsed, ...ancestors });
  }
}
