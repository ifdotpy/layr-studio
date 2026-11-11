import { ProjectSave } from "../../../app/projects/ProjectSave.ts";
import { TerminalState } from "../../../generated/api.ts";
import { TerminalDockStore } from "../TerminalDock/TerminalDock.tsx";
import { running } from "../TerminalDock/TerminalDock.tsx";
import {
  isClassicServerView,
  serverViewId,
} from "../../../servers/environment.ts";
import type { ComponentProps } from "react";
import { ProjectServerSelection } from "../../../app/projects/CreationOwner.ts";
import type { ShellFlow } from "../support/flow.ts";
import {
  createTeamChatsStore,
  type TeamChatsOwner,
} from "../messages/TeamChats/TeamChats.tsx";
import {
  complaintId,
  type ComplaintDetailResponse,
} from "../../../generated/observableResources.ts";
import { AppTeam } from "../AppTeam/AppTeam.tsx";
import { AppPanels } from "../AppPanels/AppPanels.tsx";
import { AppDialogs } from "../AppDialogs/AppDialogs.tsx";
import { lazy, Suspense } from "react";
import { Tabs } from "@mantine/core";
import Accounts, { frameAccounts } from "../../accounts/Accounts/Accounts.tsx";
import AccountSignInNotice from "../../accounts/AccountSignInNotice/AccountSignInNotice.tsx";
import ServerAccountsPanel from "../../servers/ServerAccountsPanel/ServerAccountsPanel.tsx";
import ServerAccessSettings from "../../servers/ServerAccessSettings/ServerAccessSettings.tsx";
import { useServerManagementPanel } from "../../../servers/ServerSettingsContext.tsx";
import { ChatSettingsDialog } from "../../agents/ChatSettingsDialog/ChatSettingsDialog.tsx";
import { StudioSettingsLayout } from "../../settings/StudioSettingsLayout/StudioSettingsLayout.tsx";
import { createAgentCatalog } from "../../agents/support/state.ts";
import { modalSizes } from "../../../theme.ts";
import {
  AppearanceSettings,
  HotkeySettings,
} from "../../settings/HotkeySettings/HotkeySettings.tsx";
import { useAppPreferences } from "../../../app/settings/useAppPreferences.ts";
import { SidebarListOwner } from "../../../app/shell/SidebarOwner.ts";
import { startAppFrame } from "../../../servers/frameBridge.ts";
import type { ServerCommand } from "../../../servers/navigation.ts";
import { makeAutoObservable, observableRef, reaction, flowResult } from "mobx";
import {
  createBackgroundTaskArea,
  type BackgroundTaskActions,
} from "../tasks/BackgroundTasks/BackgroundTasks.tsx";
import { type ReactNode } from "react";
import { Button, Modal } from "@mantine/core";
import type { ServerAreas, AgentModel } from "../../../app/contracts.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { errorText, type GetResult } from "../../../api.ts";
import { StopOutcome, AgentStatus } from "../../../generated/api.ts";
import {
  deleteConversation,
  definiteDeleteRejection,
} from "../../../chatDeleteScope.ts";
import ProjectDirectoryPicker, {
  browseDirectory,
} from "../../projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";
import ProjectFoldersDialog, {
  isLocatedProject,
} from "../../projects/ProjectFoldersDialog/ProjectFoldersDialog.tsx";
import ProjectFolderDialog from "../../projects/ProjectFolderDialog/ProjectFolderDialog.tsx";
import ProjectAccount from "../../projects/ProjectAccount/ProjectAccount.tsx";
import ProjectServerCards from "../../projects/ProjectServerCards/ProjectServerCards.tsx";
import {
  readProjectServers,
  registerProject,
} from "../../../servers/projectLocations.ts";
import type { ProjectBinding } from "../../../app/projects/CreationOwner.ts";
import {
  openFilePreview,
  type PreviewTarget,
  type FilePreviewSelection,
} from "../../conversation/FilePreview/FilePreview.tsx";
import { needsAnswer } from "../../../approvals.ts";
import { resourceData } from "../../../store/runtime.ts";
import { isRemoteServerView } from "../../../servers/environment.ts";
import Sidebar from "../../sidebar/Sidebar/Sidebar.tsx";
import { ChatActionsMenu } from "../../conversation/ChatActionsMenu/ChatActionsMenu.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import {
  StartupStatus,
  SchemaUpdateDialog,
  ServerUpdateStatus,
} from "../ServerUpdateStatus/ServerUpdateStatus.tsx";
import { LayoutHost } from "../LayoutHost/LayoutHost.tsx";
import { WorkspaceSlot } from "../WorkspaceSlot/WorkspaceSlot.tsx";
import { RailRoot } from "../RailRoot/RailRoot.tsx";
import { ConversationHeader } from "../../conversation/ConversationHeader/ConversationHeader.tsx";
import { ManagedConversationRoot } from "../../conversation/ManagedConversationRoot/ManagedConversationRoot.tsx";
import SearchOverlay from "../search/SearchOverlay/SearchOverlay.tsx";
import SidebarWarnings from "../../sidebar/SidebarWarnings/SidebarWarnings.tsx";
import SupervisorRecoveryNotice from "../SupervisorRecoveryNotice/SupervisorRecoveryNotice.tsx";

export default observer(function App() {
  const a = useServerAreas(),
    { scope, startup, layout } = a;
  const c = a.settings;
  const p = useAppPreferences(a);
  const signIn = (key: string) => {
    const account = [
      ...(a.accounts?.accounts || []),
      ...(a.accounts?.archivedAccounts || []),
    ].find((row) => row.id === key);
    if (account) {
      c.closeChatSettings();
      c.setCommand({
        action: "account-sign-in",
        accountKey: key,
        provider: account.provider,
        email: account.email || null,
        label: account.label,
        serverLabel: a.scope.services.owner.label,
      });
    }
  };
  reportPromptComposerRender("app");
  if (!scope.screen.active) return null;
  if (scope.screen.channel && startup.isSchemaGateVisible)
    return <ServerUpdateStatus />;
  if (!startup.isReady || !a.state)
    return startup.isSchemaGateVisible ? (
      <SchemaUpdateDialog />
    ) : (
      <StartupStatus />
    );
  return (
    <AppTeam>
      <div ref={layout.attachHost} style={{ display: "contents" }}>
        <AppSidebar />
        <SidebarWarnings />
        <RailRoot menu={<ChatActionsMenu />} />
        <LayoutHost>
          <AppWorkspace signIn={signIn} />
          <WorkspaceSlot />
          <AppPanels />
        </LayoutHost>
        <AppSearch />
        <AppSettings p={p} signIn={signIn} />
        <AppDialogs />
        <SchemaUpdateDialog />
        <ToastNotice />
      </div>
    </AppTeam>
  );
});

const AppWorkspace = observer(function AppWorkspace({
  signIn,
}: {
  signIn: (key: string) => void;
}) {
  const { layout, preferences } = useServerAreas();
  return (
    <main
      ref={layout.attachWorkspace}
      className="workspace"
      data-edge-rail={layout.railGeometry ? "true" : undefined}
      data-show-message-avatars={preferences.value.showMessageAvatars}
    >
      <SupervisorRecoveryNotice />
      {!layout.railGeometry && <ConversationHeader />}
      <AppNotices />
      <AppAccountNotices signIn={signIn} />
      <ManagedConversationRoot />
    </main>
  );
});

const AppSidebar = observer(function AppSidebar() {
  const a = useServerAreas();
  return a.sidebar.delegated || !a.ui.sidebarList ? null : (
    <Sidebar owner={a.ui.sidebarList} />
  );
});

export class AppUi {
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
  readonly catalog;
  readonly chats: TeamChatsOwner;
  readonly areas: ServerAreas;
  readonly terminal: TerminalDockStore;
  readonly requests;
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
  get pending() {
    return this.areas.selected.pendingDeletion;
  }
  projectScope = new Set<string>();
  constructor(areas: ServerAreas) {
    this.areas = areas;
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
    this.requests = areas.scope.store.requests();
    this.teamPreferred = areas.scope.services.api.saved(
      "codex-team-open",
      false,
    );
    makeAutoObservable(
      this,
      {
        areas: false,
        requests: false,
        descriptor: observableRef,
        filePreview: observableRef,
      },
      { autoBind: true },
    );
  }
  run(command: ServerCommand) {
    return flowResult(this.dispatch(command));
  }
  *dispatch(
    command: ServerCommand,
  ): ShellFlow<void, Awaited<ReturnType<ServerAreas["run"]>>> {
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
      yield this.showFolders(command.projectId, command.server);
    else if (command.action === "add-project") this.openAddProject();
    else if (command.action === "delete-project")
      this.areas.projectDeletion.open(command.projectId);
    else if (this.areas.settings.run(command)) return;
    else if (command.action === "search") {
      this.openSearch();
    } else if (command.action === "notifications") this.navigate(command);
    else {
      const result = yield this.areas.run(command);
      if (result && typeof result === "object")
        yield this.chooseProject(result.cwd, result.folder, result.binding);
    }
  }
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
    this.areas.scope.services.api.save("codex-team-open", open);
  }

  workspaceOpen = false;
  get opened() {
    return this.workspaceOpen && this.areas.startup.isReady;
  }
  teamPreferred = false;
  get attentionCount(): number {
    const lead = this.areas.navigation.selectedLead?.id;
    return (
      [
        ...(resourceData(this.requests.current)?.requests.values() || []),
      ].filter(
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
    this.filePreview = openFilePreview(this.areas.scope.services.net, target);
  }
  closePreview() {
    this.filePreview = null;
  }

  closeDialog() {
    this.projectSelection?.dispose();
    this.projectSelection = null;
    this.generation++;
    this.descriptor = null;
  }
  openAddProject() {
    this.closeDialog();
    this.areas.sidebar.close();
    const generation = this.generation,
      scope = this.areas.startup.creationScope;
    this.descriptor = {
      title: "Add project",
      body: (
        <ProjectDirectoryPicker
          initialBrowse={browseDirectory(this.areas.scope.services.net)}
          onSelect={async (path, server) => {
            await registerProject(path, server, randomUUID(), {
              ...this.areas.scope.services.api,
              storage: this.areas.scope.services.storage,
            });
            if (
              !this.disposed &&
              generation === this.generation &&
              scope === this.areas.startup.creationScope
            )
              this.closeDialog();
          }}
        />
      ),
    };
  }
  chooseProject(path?: string, folder?: string, binding?: ProjectBinding) {
    return flowResult(this.chooseProjectFlow(path, folder, binding));
  }
  *chooseProjectFlow(
    path?: string,
    folder?: string,
    binding?: ProjectBinding,
  ): ShellFlow<void, Awaited<ReturnType<typeof readProjectServers>>> {
    this.closeDialog();
    this.areas.sidebar.close();
    const request = this.generation,
      scope = this.areas.startup.creationScope;
    const project = [...(this.areas.state?.projects.values() || [])].find(
      (p) => p.path === path || p.id === path,
    ) ?? {
      id: path || "default",
      path: path || "",
      name: path?.split("/").pop() || "Current folder",
    };
    try {
      const servers = yield readProjectServers(
        this.areas.scope.services.api.get,
      );
      if (
        request !== this.generation ||
        scope !== this.areas.startup.creationScope
      )
        return;
      {
        this.projectSelection = new ProjectServerSelection(
          {
            ...project,
            path: project.path || "",
            name: project.name || "Current folder",
          },
          () => servers,
          this.areas.scope.services.api.saved(
            `project-last-server:${project.id}`,
            "local",
          ),
          (id) => (id === "local" ? this.areas.scope.services : undefined),
        );
        this.descriptor = {
          title: "New chat",
          body: (
            <ProjectServerCards
              project={{
                ...project,
                path: project.path || "",
                name: project.name || "Current folder",
              }}
              servers={servers}
              owner={this.projectSelection}
              onCancel={this.closeDialog}
              onAdd={() => this.openAddProject()}
              onChoose={(location, settings) => {
                this.areas.scope.services.api.save(
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
              }}
            />
          ),
        };
      }
    } catch (error) {
      if (
        request === this.generation &&
        scope === this.areas.startup.creationScope
      )
        this.areas.notices.notify(errorText(error));
    }
  }
  showFolders(id: string, server?: string) {
    return flowResult(this.showFoldersFlow(id, server));
  }
  *showFoldersFlow(
    id: string,
    server?: string,
  ): ShellFlow<void, GetResult<"/api/projects">> {
    const scope = this.areas.startup.creationScope,
      request = ++this.generation;
    try {
      const response = yield this.areas.scope.services.api.get("/api/projects");
      if (
        this.disposed ||
        scope !== this.areas.startup.creationScope ||
        request !== this.generation
      )
        return;
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
            this.areas.scope.services.api,
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
        this.descriptor = {
          title: "Project settings",
          body: (
            <ProjectFoldersDialog
              key={request}
              project={project}
              peers={this.areas.settings.openProjectPeers(
                project.id,
                this.areas.scope.services.api,
                this.areas.notices.notify,
              )}
              initialHeads={heads}
              initialServer={server}
              initialBrowse={
                server
                  ? browseDirectory(
                      this.areas.scope.services.net,
                      undefined,
                      server,
                      project.id,
                    )
                  : undefined
              }
            />
          ),
        };
      }
    } catch (error) {
      this.areas.notices.notify(errorText(error));
    }
  }
  changeProjectFolder(agent: AgentModel) {
    this.closeDialog();
    this.areas.sidebar.close();
    const generation = this.generation,
      scope = this.areas.startup.creationScope;
    this.descriptor = {
      title: "Choose project folder",
      body: (
        <ProjectDirectoryPicker
          initialPath={agent.projectPath || undefined}
          initialBrowse={browseDirectory(
            this.areas.scope.services.net,
            agent.projectPath || undefined,
          )}
          serverChoices={[
            { id: "local", label: this.areas.scope.services.owner.label },
          ]}
          showServerSelector={false}
          onSelect={async (cwd) => {
            await this.areas.scope.services.api.post("/api/conversation", {
              id: agent.id,
              cwd,
            });
            if (
              !this.disposed &&
              generation === this.generation &&
              scope === this.areas.startup.creationScope
            )
              this.closeDialog();
          }}
        />
      ),
    };
  }
  showProjectAccount(path: string) {
    this.closeDialog();
    this.areas.sidebar.close();
    const generation = this.generation,
      scope = this.areas.startup.creationScope;
    const saved = async () => {
      if (
        !this.disposed &&
        generation === this.generation &&
        scope === this.areas.startup.creationScope
      )
        this.closeDialog();
    };
    const backend = this.areas.scope.services.sidebarServices.cache;
    this.descriptor = {
      title: "Project settings",
      body: (
        <ProjectAccountSettings
          path={path}
          saves={{
            account: new ProjectSave(backend, "/api/projects", saved),
            worker: new ProjectSave(backend, "/api/projects", saved),
            order: new ProjectSave(backend, "/api/projects", saved),
            defaultOrder: new ProjectSave(
              backend,
              "/api/accounts/default-groups",
              saved,
            ),
          }}
        />
      ),
    };
  }
  showProjectFolder() {
    this.closeDialog();
    const agent = this.areas.navigation.selectedAgent;
    if (!agent) return;
    if (agent.isLead && !agent.threadId) this.changeProjectFolder(agent);
    else if (agent.projectPath)
      this.descriptor = {
        title: "Project folder",
        body: (
          <ProjectFolderDialog
            agent={agent}
            canReveal={
              this.areas.scope.services.fileDialogs.canRevealPath() &&
              !isRemoteServerView
            }
            onReveal={(path) =>
              void this.areas.scope.services.fileDialogs
                .revealPath(path)
                .catch((e) => this.areas.notices.notify(errorText(e)))
            }
          />
        ),
      };
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
            onClick={() => {
              this.closeDialog();
              if (room) {
                const scope = this.areas.startup.creationScope;
                void this.areas.scope.services.api
                  .post("/api/room/delete", { id })
                  .then((r) => {
                    if (
                      !this.disposed &&
                      scope === this.areas.startup.creationScope
                    )
                      this.areas.finishDeletion(
                        typeof r.deleted === "string"
                          ? [r.deleted]
                          : (r.deleted || []).filter(
                              (id): id is string => typeof id === "string",
                            ),
                      );
                  })
                  .catch((e) => this.areas.notices.notify(errorText(e)));
                return;
              }
              if (this.pending.has(id)) return;
              const controller = new AbortController();
              this.pending.set(id, controller);
              this.areas.selected.suspendReaders(new Set([id]));
              void deleteConversation(
                (chat, signal) =>
                  this.areas.scope.services.api.command.delete(chat, {
                    signal,
                  }),
                id,
                this.areas.finishDeletion,
                controller.signal,
              )
                .then((r) => {
                  if (
                    !controller.signal.aborted &&
                    r.kind === "delete" &&
                    r.result.stopped === StopOutcome.unconfirmed
                  )
                    this.areas.notices.notify(
                      "The chat was deleted, but stopping its commands could not be confirmed.",
                    );
                })
                .catch((e) => {
                  if (!controller.signal.aborted)
                    this.areas.notices.notify(
                      definiteDeleteRejection(e)
                        ? errorText(e)
                        : "Delete may not have completed. Check the chat list before trying again.",
                    );
                })
                .finally(() => this.finishDelete(id, controller));
            }}
          >
            {worker ? "Delete" : "Delete chat"}
          </Button>
        </>
      ),
    };
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

const MaintenanceSettings = observer(function MaintenanceSettings() {
  const { delivery } = useServerAreas();
  return (
    <section className="settings-group" aria-label="Maintenance">
      <h2>Maintenance</h2>
      <Button
        loading={delivery.removingAllSending}
        onClick={delivery.removeAllSending}
      >
        Remove all sending messages
      </Button>
    </section>
  );
});

const ToastNotice = observer(function ToastNotice() {
  const { notices, delivery } = useServerAreas();
  return notices.toast && !delivery.ownsOfflineError(notices.toast) ? (
    <div id="toast" role="status">
      {notices.toast}
    </div>
  ) : null;
});

const AppNotices = observer(function AppNotices() {
  const { startup, delivery, native, selected, navigation, creation } =
    useServerAreas();
  const agent = navigation.selectedAgent;
  const pending = agent && native.pending(agent.id);
  const error = !navigation.selectedId && startup.error;
  return (
    <>
      {agent && pending && (
        <div
          role="status"
          className="native-action-pending"
          style={{ padding: "8px 16px" }}
        >
          <span>The {pending.action} request has no confirmed reply. </span>
          <Button
            size="compact-sm"
            loading={native.active}
            onClick={() => selected.nativeAction(agent, pending.action)}
          >
            Check action request
          </Button>
        </div>
      )}
      {!creation.isCreating &&
        creation.pendingCreationRequests.map((request) => (
          <div className="sync-status" key={request.id}>
            <p>
              Studio lost confirmation for this chat request in{" "}
              {request.cwd || "default project"}
              {request.project_folder ? ` (${request.project_folder})` : ""}.
              Retrying uses the saved request ID to check whether the chat was
              created. Dismissing stops checking; a chat this request created
              still appears in the chat list.
            </p>
            <Button onClick={() => void creation.retry(request)}>
              Retry chat request
            </Button>
            <Button onClick={() => creation.dismiss(request)}>Dismiss</Button>
          </div>
        ))}
      {error && !delivery.ownsOfflineError(error) && (
        <div id="error" role="alert">
          {error}
        </div>
      )}
      <div className="sync-notices">
        {delivery.draftSyncError && (
          <p className="sync-status" role="status" data-draft-sync-status>
            {delivery.draftSyncError}
          </p>
        )}
        {delivery.outboxSyncError && (
          <p className="sync-status" role="alert">
            {delivery.outboxSyncError}
          </p>
        )}
      </div>
    </>
  );
});

const AppSearch = observer(function AppSearch() {
  const a = useServerAreas(),
    panels = a.ui,
    workspace = a.ui;
  return (
    <SearchOverlay
      source={{
        opened: panels.searchOpen,
        close: panels.closeSearch,
        select: (id, messageId) =>
          a.navigation.selectExternal(id, {
            requestId: randomUUID(),
            ...(messageId ? { messageId } : {}),
          }),
        navigate: (section, agent) => {
          if (agent) a.open(agent);
          workspace.showSection(section);
        },
      }}
    />
  );
});

const Models = lazy(() =>
  import("../../settings/models/ModelWhitelistSettings/ModelWhitelistSettings.tsx").then(
    (m) => ({
      default: m.ModelWhitelistSettings,
    }),
  ),
);

const Orchestration = lazy(() =>
  import("../../settings/OrchestrationSettings/OrchestrationSettings.tsx").then(
    (m) => ({
      default: m.OrchestrationSettings,
    }),
  ),
);

const Worktrees = lazy(() =>
  import("../../settings/LayrWorktreesSettings/LayrWorktreesSettings.tsx").then(
    (m) => ({
      default: m.LayrWorktreesSettings,
    }),
  ),
);

const Alerts = lazy(() =>
  import("../../accounts/SignInPushSettings/SignInPushSettings.tsx").then(
    (m) => ({
      default: m.SignInPushSettings,
    }),
  ),
);

const Federation = lazy(() =>
  import("../../settings/FederationSettings/FederationSettings.tsx").then(
    (m) => ({
      default: m.FederationSettings,
    }),
  ),
);

const AppSettings = observer(function AppSettings({
  p,
  signIn,
}: {
  p: ReturnType<typeof useAppPreferences>;
  signIn: (key: string) => void;
}) {
  reportPromptComposerRender("settings");
  const a = useServerAreas(),
    management = useServerManagementPanel(),
    c = a.settings,
    ui = a.ui,
    run = ui.run;
  const catalog = a.ui.catalog;
  const lead = a.navigation.selectedLead || a.navigation.selectedAgent;
  const server = a.scope.services.owner;
  return (
    <>
      <Accounts />
      <Modal
        opened={c.studioOpen && !a.accountsUi.dialogOpen}
        transitionProps={{ duration: 0 }}
        closeOnEscape={!a.accountsUi.dialogOpen}
        closeOnClickOutside={!a.accountsUi.dialogOpen}
        onClose={() => c.setStudioOpen(false)}
        size={modalSizes.settings}
        title="Studio settings"
        classNames={{ body: "studio-settings-body" }}
      >
        <div className="studio-settings-panel" data-testid="studio-settings">
          <StudioSettingsLayout value={c.tab} onChange={c.activateTab}>
            <Tabs.Panel value="accounts" pt="md">
              <section className="settings-group" aria-label="Studio accounts">
                {a.accounts ? (
                  <ServerAccountsPanel
                    servers={[server]}
                    accountsByServer={{
                      [server.id]: frameAccounts(a.accounts),
                    }}
                    statuses={{ [server.id]: "live" }}
                    servicesFor={(id) =>
                      id === server.id ? a.scope.services : undefined
                    }
                    startAccount={(_id, command) => c.setCommand(command)}
                    accountAction={(_id, command) => c.setCommand(command)}
                    onDiscover={() => {
                      void a.scope.services.api
                        .post("/api/accounts/discover", {})
                        .catch((error) => a.notices.notify(errorText(error)));
                    }}
                  />
                ) : (
                  <p role="status">Loading accounts…</p>
                )}
              </section>
            </Tabs.Panel>
            <Tabs.Panel value="servers" pt="md" keepMounted={false}>
              {management || <ServerAccessSettings active={c.studioOpen} />}
            </Tabs.Panel>
            <Tabs.Panel
              value="models"
              className="model-whitelist-panel"
              pt="md"
              keepMounted={false}
            >
              <Suspense fallback={null}>
                <Models />
              </Suspense>
            </Tabs.Panel>
            <Tabs.Panel value="appearance" pt="md">
              <AppearanceSettings p={p}>
                <MaintenanceSettings />
              </AppearanceSettings>
            </Tabs.Panel>
            <Suspense fallback={null}>
              <Tabs.Panel value="federation" pt="md">
                {lead?.isLead && (
                  <Federation
                    active={c.studioOpen}
                    leadId={lead.id}
                    agents={catalog.agents}
                    notify={a.notices.notify}
                  />
                )}
              </Tabs.Panel>
              <Tabs.Panel value="agents" pt="md">
                <Orchestration active={c.studioOpen && c.tab === "agents"} />
              </Tabs.Panel>
              <Tabs.Panel value="layr-worktrees" pt="md">
                <Worktrees active={c.studioOpen} />
              </Tabs.Panel>
              <Tabs.Panel value="phone-alerts" pt="md" keepMounted={false}>
                <Alerts active={c.studioOpen} />
              </Tabs.Panel>
            </Suspense>
            <Tabs.Panel value="hotkeys" pt="md">
              <HotkeySettings p={p} />
            </Tabs.Panel>
          </StudioSettingsLayout>
          {p.preferenceError && (
            <p className="studio-preferences-error" role="alert">
              {p.preferenceError}
            </p>
          )}
        </div>
      </Modal>
      <ChatSettingsDialog
        areas={a}
        settings={c}
        accounts={a.accounts}
        onSignIn={signIn}
        onNewChat={(path) => {
          void run({ action: "new-chat", ...(path ? { path } : {}) });
        }}
        onChangeProject={ui.changeProjectFolder}
      />
    </>
  );
});

const AppAccountNotices = observer(function AppAccountNotices({
  signIn,
}: {
  signIn: (key: string) => void;
}) {
  const a = useServerAreas(),
    catalog = a.ui.catalog;
  const agent = a.navigation.selectedAgent,
    lead = a.navigation.selectedLead || agent;
  const key =
    agent?.accountKey ||
    lead?.accountKey ||
    (agent ? "default" : a.accounts?.defaultAccountKey) ||
    "default";
  return (
    <>
      {[
        ...(a.accounts?.accounts || []),
        ...(a.accounts?.archivedAccounts || []),
      ]
        .filter(
          (account) =>
            account.id === key ||
            catalog.agents.some(
              (member) =>
                member.rootId === lead?.id &&
                (member.accountKey || "default") === account.id &&
                member.status === AgentStatus.failed,
            ),
        )
        .map((account) => (
          <AccountSignInNotice
            key={account.id}
            account={account}
            errors={catalog.errors(account.id, agent?.id, lead?.id)}
            onSignIn={signIn}
          />
        ))}
    </>
  );
});

const ProjectAccountSettings = observer(function ProjectAccountSettings({
  path,
  saves,
}: {
  path: string;
  saves: ComponentProps<typeof ProjectAccount>["saves"];
}) {
  const a = useServerAreas();
  const accounts = a.accounts;
  if (!accounts) return <p role="status">Loading accounts…</p>;
  const projects = [...(a.state?.projects.values() || [])];
  return (
    <ProjectAccount
      path={path}
      project={projects.find((p) => p.path === path)}
      accounts={accounts}
      defaultAccountKey={
        projects
          .filter(
            (p) =>
              p.path &&
              (path === p.path ||
                path.startsWith(p.path.replace(/\/$/, "") + "/")),
          )
          .sort((x, y) => (y.path?.length || 0) - (x.path?.length || 0))[0]
          ?.accountKey || accounts.defaultAccountKey
      }
      saves={saves}
    />
  );
});
