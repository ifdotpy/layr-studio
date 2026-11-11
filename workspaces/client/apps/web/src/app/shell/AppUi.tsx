import { ProjectAccountSettings } from "../../components/projects/ProjectAccountSettings/ProjectAccountSettings.tsx";
import { accountRows } from "../../servers/AccountsPanelState.ts";
import { frameAccounts } from "../../components/accounts/Accounts/Accounts.tsx";
import { createElement, type ComponentType } from "react";
import { flowValue } from "../../flowValue.ts";
import { StudioErrors } from "./StudioErrors.ts";
import { ProjectSave } from "../projects/ProjectSave.ts";
import { TerminalState } from "../../generated/api.ts";
import { TerminalDockStore } from "../../components/shell/TerminalDock/TerminalDock.tsx";
import { running } from "../../components/shell/TerminalDock/TerminalDock.tsx";
import {
  isClassicServerView,
  serverViewId,
} from "../../servers/environment.ts";
import { ProjectServerSelection } from "../projects/CreationOwner.ts";
import {
  createTeamChatsStore,
  type TeamChatsOwner,
} from "../../components/shell/messages/TeamChats/TeamChats.tsx";
import {
  complaintId,
  type ComplaintDetailResponse,
  type StateCollections,
} from "../../generated/observableResources.ts";
import { createAgentCatalog } from "../../components/agents/support/state.ts";
import { SidebarListOwner } from "./SidebarOwner.ts";
import { startAppFrame } from "../../servers/frameBridge.ts";
import type { ServerCommand } from "../../servers/navigation.ts";
import {
  makeAutoObservable,
  observableRef,
  reaction,
  flowResult,
  flow,
} from "mobx";
import {
  createBackgroundTaskArea,
  type BackgroundTaskActions,
} from "../../components/shell/tasks/BackgroundTasks/BackgroundTasks.tsx";
import { type ReactNode } from "react";
import { Button } from "@mantine/core";
import type { ServerAreas, AgentModel } from "../contracts.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { errorText } from "../../api.ts";
import { StopOutcome } from "../../generated/api.ts";
import {
  deleteConversation,
  definiteDeleteRejection,
} from "../../chatDeleteScope.ts";
import ProjectDirectoryPicker, {
  browseDirectory,
} from "../../components/projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";
import ProjectFoldersDialog, {
  isLocatedProject,
} from "../../components/projects/ProjectFoldersDialog/ProjectFoldersDialog.tsx";
import ProjectServerCards from "../../components/projects/ProjectServerCards/ProjectServerCards.tsx";
import {
  readProjectServers,
  registerProject,
} from "../../servers/projectLocations.ts";
import type { ProjectBinding } from "../projects/CreationOwner.ts";
import {
  openFilePreview,
  type PreviewTarget,
  type FilePreviewSelection,
} from "../../components/conversation/FilePreview/FilePreview.tsx";
import { needsAnswer } from "../../approvals.ts";
import { resourceData } from "../../store/runtime.ts";

export type ComplaintSummary =
  StateCollections["complaints"] extends ReadonlyMap<string, infer T>
    ? T
    : never;

export class AppUi {
  readonly complaintErrors = new Map<string, unknown>();
  complaintView(
    message: ComplaintSummary,
    scope: string,
    store: ServerAreas["scope"]["store"],
  ) {
    const key = JSON.stringify([scope, message.id]);
    const current = store.complaint(complaintId(message.id)).current;
    const source = resourceData(current) || message;
    const confirmed = this.messageCache.get(key);
    const detail =
      confirmed && confirmed.version > ("text" in source ? source.version : -1)
        ? confirmed
        : source;
    return {
      key,
      detailView: "text" in detail ? detail : null,
      detailError:
        this.complaintErrors.get(key) ||
        ("error" in current ? current.error : null),
      remember: (detail: ComplaintDetailResponse) =>
        this.rememberComplaint(key, detail),
      authorName: (id: string) =>
        id === "user"
          ? "You"
          : resourceData(store.state().current)?.agents.get(id)?.name ||
            message.authorName ||
            id,
    };
  }
  *retryComplaint(
    api: Pick<ServerAreas["scope"]["services"]["api"], "get">,
    id: string,
    key: string,
  ) {
    try {
      this.rememberComplaint(
        key,
        yield* flowValue(api.get("/api/complaint", { query: { id } })),
      );
      this.complaintErrors.delete(key);
    } catch (error) {
      this.complaintErrors.set(key, error);
    }
  }

  sidebarList: SidebarListOwner | undefined;
  activity: BackgroundTaskActions | undefined;
  readonly messageCache = new Map<string, ComplaintDetailResponse>();
  rememberComplaint(key: string, result: ComplaintDetailResponse) {
    const cached = this.messageCache.get(key);
    if (!cached || cached.version <= result.version)
      this.messageCache.set(key, result);
  }
  setTeamOpen(value: boolean) {
    this.teamOpen = value;
  }
  readonly errors;
  readonly catalog;
  readonly chats: TeamChatsOwner;
  readonly areas: ServerAreas;
  readonly terminal: TerminalDockStore;
  visited = false;
  section = "";
  focus: { id: string; requestId: string; roomId?: string } | undefined;
  teamOpen = false;
  tasksOpen = false;
  searchOpen = false;
  taskFocus: { leadId: string; id: string; requestId: string } | undefined;
  filePreview: FilePreviewSelection | null = null;
  projectSelection: ProjectServerSelection | null = null;
  descriptor: { title: string; body: ReactNode } | null = null;
  generation = 0;
  restore = 0;
  disposed = false;
  trigger: HTMLElement | null = null;
  get accountRows() {
    const a = this.areas,
      service = a.scope.services,
      server = service.owner;
    const send = (
      _id: string,
      command: Extract<
        ServerCommand,
        { action: "account-action" | "account-sign-in" }
      >,
    ) => a.settings.setCommand(command);
    return accountRows(
      {
        servers: [server],
        accountsByServer: { [server.id]: frameAccounts(a.accounts) || [] },
        statuses: { [server.id]: "live" },
        servicesFor: (id) => (id === server.id ? service : undefined),
        startAccount: send,
        accountAction: send,
      },
      service.store,
    );
  }
  get pending() {
    return this.areas.selected.pendingDeletion;
  }
  projectScope = new Set<string>();
  constructor(areas: ServerAreas) {
    this.areas = areas;
    this.errors = new StudioErrors(areas);
    this.terminal = new TerminalDockStore({
      ...areas.scope.services.api,
      notify: (message) => areas.notices.notify(message),
    });
    this.catalog = createAgentCatalog(areas.scope.store);
    this.chats = createTeamChatsStore(
      areas.scope.store,
      areas.scope.services.api,
      "",
      undefined,
      () => ({
        stateDir: areas.state?.workspaces.values().next().value?.stateDir || "",
        leadId: areas.navigation.selectedLead?.id,
        focus: this.focus,
      }),
    );
    this.teamPreferred = areas.scope.services.api.saved(
      "codex-team-open",
      false,
    );
    makeAutoObservable(
      this,
      {
        areas: false,
        descriptor: observableRef,
        filePreview: observableRef,
      },
      { autoBind: true },
    );
  }
  get services() {
    return this.areas.scope.services;
  }
  signIn(key: string) {
    const account = [
      ...(this.areas.accounts?.accounts || []),
      ...(this.areas.accounts?.archivedAccounts || []),
    ].find((row) => row.id === key);
    if (account) {
      this.areas.settings.closeChatSettings();
      this.areas.settings.setCommand({
        action: "account-sign-in",
        accountKey: key,
        provider: account.provider,
        email: account.email || null,
        label: account.label,
        serverLabel: this.services.owner.label,
      });
    }
  }
  run = flow(function* (this: AppUi, command: ServerCommand) {
    if (command.action === "remove-chat") {
      const model = command.room
        ? this.areas.state?.rooms.get(command.id)
        : this.areas.creation.lookup(command.id);
      if (model) this.requestDelete(model, command.room);
    } else if (command.action === "change-project") {
      const model = this.areas.creation.lookup(command.id);
      if (model) this.changeProjectFolder(model);
    } else if (command.action === "project-account")
      this.showProjectAccount(command.path);
    else if (command.action === "project-folders")
      yield* flowValue(this.showFolders(command.projectId, command.server));
    else if (command.action === "add-project") this.openAddProject();
    else if (command.action === "delete-project")
      this.areas.projectDeletion.open(command.projectId);
    else if (this.areas.settings.run(command)) return;
    else if (command.action === "search") {
      this.openSearch();
    } else if (command.action === "notifications") this.navigate(command);
    else {
      const result = yield* flowValue(this.areas.run(command));
      if (result && typeof result === "object")
        yield* flowValue(
          this.chooseProject(result.cwd, result.folder, result.binding),
        );
    }
  });
  navigate(target: { agentId: string; itemId?: string; requestId?: string }) {
    this.areas.navigation.applyNavigation({
      ...target,
      requestId: target.requestId || randomUUID(),
    });
    if (target.itemId)
      this.showMessages(target.itemId, undefined, target.requestId);
    else this.closeWorkspace();
  }
  escape() {
    this.closeDialog();
    this.areas.sidebar.close();
    this.closeTeam();
  }

  setTeamPreferred(open: boolean) {
    this.teamPreferred = open;
    this.services.api.save("codex-team-open", open);
  }

  workspaceOpen = false;
  get opened() {
    return this.workspaceOpen && this.areas.startup.isReady;
  }
  teamPreferred = false;
  get attentionCount(): number {
    const lead = this.areas.navigation.selectedLead?.id;
    return (
      this.chats.requests.allRequests.filter(
        (r) =>
          needsAnswer(r) &&
          !!lead &&
          (r.agent === lead ||
            this.areas.state?.agents.get(r.agent)?.rootId === lead),
      ).length +
      [...(this.areas.state?.complaints.values() || [])].filter(
        (c) => c.leadId === lead && c.needsUserResponse,
      ).length
    );
  }
  showSection(section: string) {
    this.section = section;
    this.visited = true;
    this.workspaceOpen = true;
  }
  showMessages(id?: string, roomId?: string, requestId = randomUUID()) {
    this.focus = id
      ? { id, requestId, ...(roomId ? { roomId } : {}) }
      : undefined;
    this.showSection("messages");
  }
  closeWorkspace() {
    this.workspaceOpen = false;
  }

  get isTeamVisible(): boolean {
    return (
      this.areas.scope.screen.active &&
      (this.areas.layout.isNarrowTeam ? this.teamOpen : this.teamPreferred)
    );
  }
  toggleTeam() {
    if (this.areas.layout.isNarrowTeam) this.teamOpen = !this.teamOpen;
    else this.setTeamPreferred(!this.teamPreferred);
  }
  closeTeam() {
    this.teamOpen = false;
    this.setTeamPreferred(false);
  }
  closeTeamDrawer() {
    this.teamOpen = false;
  }
  openActivity(id?: string) {
    this.trigger =
      document.activeElement instanceof HTMLElement &&
      this.areas.layout.host?.contains(document.activeElement)
        ? document.activeElement
        : null;
    cancelAnimationFrame(this.restore);
    const taskFocus =
      id && this.areas.navigation.selectedLead
        ? {
            leadId: this.areas.navigation.selectedLead.id,
            id,
            requestId: randomUUID(),
          }
        : undefined;
    this.taskFocus = taskFocus;
    this.tasksOpen = true;
  }
  closeTasks() {
    this.tasksOpen = false;
  }
  afterTaskClose() {
    cancelAnimationFrame(this.restore);
    this.restore = requestAnimationFrame(() => {
      if (this.areas.scope.screen.active && !this.tasksOpen) {
        const selector = this.taskFocus
          ? `[data-activity-id="${CSS.escape(this.taskFocus.id)}"]`
          : "#chat-actions";
        const source =
          this.trigger?.isConnected &&
          this.areas.layout.host?.contains(this.trigger)
            ? this.trigger
            : this.areas.layout.host?.querySelector<HTMLElement>(selector);
        source?.focus();
      }
    });
  }
  openSearch() {
    this.areas.sidebar.close();
    this.searchOpen = true;
  }
  closeSearch() {
    this.searchOpen = false;
  }
  showPreview(target: PreviewTarget) {
    this.filePreview = openFilePreview(this.services.net, target);
  }
  closePreview() {
    this.filePreview = null;
  }

  presentDialog<P extends object>(
    title: string,
    component: ComponentType<P>,
    props: P,
  ) {
    this.descriptor = { title, body: createElement(component, props) };
  }
  closeDialog() {
    this.projectSelection?.dispose();
    this.projectSelection = null;
    this.generation++;
    this.descriptor = null;
  }
  dialogCurrent() {
    const generation = this.generation,
      scope = this.areas.startup.creationScope;
    return () =>
      !this.disposed &&
      generation === this.generation &&
      scope === this.areas.startup.creationScope;
  }
  beginDialog() {
    this.closeDialog();
    this.areas.sidebar.close();
    return this.dialogCurrent();
  }
  *saveDialog(operation: Promise<unknown>, current: () => boolean) {
    yield* flowValue(operation);
    if (current()) this.closeDialog();
  }
  openAddProject() {
    const current = this.beginDialog();
    this.presentDialog("Add project", ProjectDirectoryPicker, {
      initialBrowse: browseDirectory(this.services.net),
      onSelect: (path, server) =>
        flowResult(
          this.saveDialog(
            registerProject(path, server, randomUUID(), {
              ...this.services.api,
              storage: this.services.storage,
            }),
            current,
          ),
        ),
    });
  }
  chooseProject = flow(function* (
    this: AppUi,
    path?: string,
    folder?: string,
    binding?: ProjectBinding,
  ) {
    const current = this.beginDialog();
    const found = [...(this.areas.state?.projects.values() || [])].find(
      (p) => p.path === path || p.id === path,
    ) ?? {
      id: path || "default",
      path: path || "",
      name: path?.split("/").pop() || "Current folder",
    };
    const project = {
      ...found,
      path: found.path || "",
      name: found.name || "Current folder",
    };
    try {
      const servers = yield* flowValue(
        readProjectServers(this.services.api.get),
      );
      if (!current()) return;
      {
        this.projectSelection = new ProjectServerSelection(
          project,
          () => servers,
          this.services.api.saved(`project-last-server:${project.id}`, "local"),
          (id) => (id === "local" ? this.services : undefined),
        );
        this.presentDialog("New chat", ProjectServerCards, {
          project: project,
          servers: servers,
          owner: this.projectSelection,
          onCancel: this.closeDialog,
          onAdd: this.openAddProject,
          onChoose: (location, settings) => {
            this.services.api.save(
              `project-last-server:${project.id}`,
              location.serverId,
            );
            this.closeDialog();
            if (location.serverId === "local")
              void this.areas.creation.newChat(
                location.path,
                folder,
                undefined,
                binding,
                settings,
              );
            else
              this.areas.frame.publish({
                kind: "studio-create-on-server",
                server: location.serverId,
                path: location.path,
                projectId: project.id,
                settings,
              });
          },
        });
      }
    } catch (error) {
      if (current()) this.areas.notices.notify(errorText(error));
    }
  });
  showFolders = flow(function* (this: AppUi, id: string, server?: string) {
    const request = ++this.generation;
    const current = this.dialogCurrent();
    try {
      const response = yield* flowValue(this.services.api.get("/api/projects"));
      if (!current()) return;
      const found = response.items.find((p) => p.id === id || p.path === id);
      const project =
        found &&
        (this.areas.state?.projects.get(found.id) ??
          [...(this.areas.state?.projects.values() || [])].find(
            (p) => p.path === found.path,
          ));
      if (isLocatedProject(project)) {
        const heads = (project.locations || []).map((location) => {
          const slot = this.areas.scope.store.reads.projectLocations(
            this.services.api,
            {
              project: project.id,
              server: location.serverId,
              path: location.path,
              action: "info",
            },
          );
          void slot.read().catch(() => {});
          return {
            project: project.id,
            server: location.serverId,
            path: location.path,
            slot,
          };
        });
        this.areas.sidebar.close();
        this.presentDialog("Project settings", ProjectFoldersDialog, {
          key: request,
          project: project,
          peers: this.areas.settings.openProjectPeers(
            project.id,
            this.services.api,
            this.areas.notices.notify,
          ),
          initialHeads: heads,
          initialServer: server,
          initialBrowse: server
            ? browseDirectory(this.services.net, undefined, server, project.id)
            : undefined,
        });
      }
    } catch (error) {
      this.areas.notices.notify(errorText(error));
    }
  });
  changeProjectFolder(agent: AgentModel) {
    const current = this.beginDialog();
    this.presentDialog("Choose project folder", ProjectDirectoryPicker, {
      initialPath: agent.projectPath || undefined,
      initialBrowse: browseDirectory(
        this.services.net,
        agent.projectPath || undefined,
      ),
      serverChoices: [{ id: "local", label: this.services.owner.label }],
      showServerSelector: false,
      onSelect: (cwd) =>
        flowResult(
          this.saveDialog(
            this.services.api.post("/api/conversation", {
              id: agent.id,
              cwd,
            }),
            current,
          ),
        ),
    });
  }
  showProjectAccount(path: string) {
    const current = this.beginDialog();
    const saved = () => {
      if (current()) this.closeDialog();
      return Promise.resolve();
    };
    const backend = this.services.sidebarServices.cache;
    this.presentDialog("Project settings", ProjectAccountSettings, {
      path: path,
      saves: {
        account: new ProjectSave(backend, "/api/projects", saved),
        worker: new ProjectSave(backend, "/api/projects", saved),
        order: new ProjectSave(backend, "/api/projects", saved),
        defaultOrder: new ProjectSave(
          backend,
          "/api/accounts/default-groups",
          saved,
        ),
      },
    });
  }
  setProjectScope(ids: ReadonlySet<string>) {
    this.areas.selected.resumeReaders(
      new Set(
        [...this.projectScope].filter(
          (id) => !ids.has(id) && !this.pending.has(id),
        ),
      ),
    );
    this.projectScope = new Set(ids);
    this.areas.selected.suspendReaders(ids);
  }
  requestDelete(model: { id: string }, room = false, worker = false) {
    const id = model.id;
    this.closeDialog();
    this.descriptor = {
      title: worker ? "Delete?" : "Delete chat?",
      body: (
        <>
          <p>
            {room
              ? "Hide this chat. It returns on a new message."
              : "Stop this agent and its workers, and remove their chats. Files and history stay on disk."}
          </p>
          <Button
            color="red"
            variant="filled"
            data-delete-chat={id}
            onClick={() => void this.deleteChat(id, room)}
          >
            {worker ? "Delete" : "Delete chat"}
          </Button>
        </>
      ),
    };
  }
  *deleteChat(id: string, room: boolean) {
    this.closeDialog();
    const { api } = this.services;
    if (room) {
      const scope = this.areas.startup.creationScope;
      try {
        const result = yield* flowValue(api.post("/api/room/delete", { id }));
        if (!this.disposed && scope === this.areas.startup.creationScope)
          this.areas.finishDeletion(
            typeof result.deleted === "string"
              ? [result.deleted]
              : (result.deleted || []).filter(
                  (id): id is string => typeof id === "string",
                ),
          );
      } catch (error) {
        this.areas.notices.notify(errorText(error));
      }
      return;
    }
    if (this.pending.has(id)) return;
    const controller = new AbortController();
    this.pending.set(id, controller);
    this.areas.selected.suspendReaders(new Set([id]));
    try {
      const result = yield* flowValue(
        deleteConversation(
          (chat, signal) => api.command.delete(chat, { signal }),
          id,
          this.areas.finishDeletion,
          controller.signal,
        ),
      );
      if (
        !controller.signal.aborted &&
        result.kind === "delete" &&
        result.result.stopped === StopOutcome.unconfirmed
      )
        this.areas.notices.notify(
          "The chat was deleted, but stopping its commands could not be confirmed.",
        );
    } catch (error) {
      if (!controller.signal.aborted)
        this.areas.notices.notify(
          definiteDeleteRejection(error)
            ? errorText(error)
            : "Delete may not have completed. Check the chat list before trying again.",
        );
    } finally {
      this.finishDelete(id, controller);
    }
  }
  finishDelete(id: string, controller: AbortController) {
    if (this.pending.get(id) !== controller) return;
    this.pending.delete(id);
    if (!this.projectScope.has(id))
      this.areas.selected.resumeReaders(new Set([id]));
  }
  get terminalVisible() {
    const a = this.areas;
    return (
      a.startup.isReady &&
      !(a.scope.screen.channel && a.startup.isSchemaGateVisible) &&
      !!a.state &&
      a.scope.screen.active &&
      !a.layout.isMobile &&
      a.preferences.value.showTerminals
    );
  }
  createTerminal() {
    const agent =
      this.areas.navigation.selectedAgent || this.areas.navigation.selectedLead;
    if (agent)
      void flowResult(this.terminal.create(agent.id, this.terminal.scroll));
  }
  start() {
    const a = this.areas;
    const stopTerminalOwner = reaction(
      () => [this.terminalVisible, a.scope.screen.channel] as const,
      ([visible, screen]) => {
        this.terminal.dispose();
        if (visible) this.terminal.start(screen, this.createTerminal);
      },
      { fireImmediately: true },
    );
    const stopTerminalFrame = reaction(
      () => {
        const screen = a.scope.screen.channel;
        if (
          !this.terminalVisible ||
          this.terminal.measuredHeight === null ||
          (!screen &&
            (!serverViewId || isClassicServerView || window.parent === window))
        )
          return null;
        const entries = (
          resourceData(a.scope.store.terminals().current) || []
        ).filter((shell) => shell.status !== TerminalState.closed);
        return {
          kind: "studio-server-terminal",
          serverId: screen?.owner ?? serverViewId,
          opened: this.terminal.opened,
          height: this.terminal.measuredHeight,
          count: entries.length,
          active: entries.filter((shell) => running(shell.status)).length,
          canCreate: !!(
            a.navigation.selectedAgent || a.navigation.selectedLead
          ),
        };
      },
      (message) => {
        if (message) a.frame.publish(message);
      },
      { fireImmediately: true },
    );
    this.disposed = false;
    this.sidebarList = new SidebarListOwner(this.areas, undefined, this.run);
    const stopSidebar = this.sidebarList.start();
    const stopFrame = startAppFrame(this.areas);
    const stopMessageCache = reaction(
      () =>
        [...this.messageCache].flatMap(([key, confirmed]) => {
          const latest = resourceData(
            a.scope.store.complaint(complaintId(confirmed.id)).current,
          );
          return latest && latest.version >= confirmed.version ? [key] : [];
        }),
      (keys) => {
        for (const key of keys) this.messageCache.delete(key);
      },
    );
    let stopActivity: (() => void) | undefined;
    const stopActivityOwner = reaction(
      () =>
        [
          a.scope.screen.active && this.tasksOpen,
          a.navigation.selectedLead?.id,
        ] as const,
      ([opened, leadId]) => {
        stopActivity?.();
        if (!leadId) {
          this.activity = undefined;
          return;
        }
        if (!opened) return;
        this.activity = createBackgroundTaskArea(
          a.scope.services.api,
          a.scope.store,
          a.scope.services.owner.id,
          {
            opened: true,
            leadId,
            close: this.closeTasks,
            openAgent: a.open,
            notify: a.notices.notify,
          },
        );
        stopActivity = this.activity?.start();
        if (this.taskFocus && this.taskFocus.leadId === leadId)
          this.activity?.focus(this.taskFocus.id);
      },
      { fireImmediately: true },
    );
    const stopTaskFocus = reaction(
      () => this.taskFocus,
      (focus) => {
        if (focus && focus.leadId === this.activity?.leadId)
          this.activity.focus(focus.id);
      },
    );
    const stopAccountCommand = reaction(
      () =>
        [
          a.settings.command,
          a.accountsUi.summary,
          a.accountsUi.pending,
        ] as const,
      ([command, summary, pending]) => {
        a.accountsUi.receive(command);
        if (command && summary && !pending) a.settings.setCommand(null);
      },
      { fireImmediately: true },
    );
    const stopAccountDialog = reaction(
      () => a.accountsUi.dialogOpen,
      a.settings.reportAccountDialog,
      { fireImmediately: true },
    );
    const stop = reaction(
      () => [...this.pending.keys()].filter((id) => !a.creation.lookup(id)),
      (ids) => {
        for (const id of ids) {
          this.pending.get(id)?.abort();
          this.pending.delete(id);

          a.selected.resumeReaders(new Set([id]));
        }
      },
    );
    const stopNavigation = reaction(
      () => [a.navigation.navigationIntent, a.scope.screen.active] as const,
      () => {
        this.closeTeamDrawer();
        if (!a.scope.screen.active) this.closePreview();
        cancelAnimationFrame(this.restore);
      },
    );
    return () => {
      stop();
      stopNavigation();
      stopActivityOwner();
      stopTaskFocus();
      stopActivity?.();
      stopSidebar();
      stopFrame();
      stopMessageCache();
      stopAccountCommand();
      stopAccountDialog();
      a.settings.reportAccountDialog(false);
      stopTerminalOwner();
      stopTerminalFrame();
      this.terminal.dispose();
      this.dispose();
    };
  }
  dispose() {
    this.projectSelection?.dispose();
    this.projectSelection = null;
    this.disposed = true;
    this.pending.forEach((c) => c.abort());
    this.pending.clear();
    this.setProjectScope(new Set());
    cancelAnimationFrame(this.restore);
  }
}
