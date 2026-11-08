import { lazy, Suspense, useMemo } from "react";
import { Tabs } from "@mantine/core";
import Accounts, { frameAccounts } from "./components/Accounts.tsx";
import AccountSignInNotice from "./components/AccountSignInNotice.tsx";
import ServerAccountsPanel from "./servers/ServerAccountsPanel.tsx";
import ServerAccessSettings from "./servers/ServerAccessSettings.tsx";
import { useServerManagementPanel } from "./servers/ServerSettingsContext.tsx";
import { ChatSettingsDialog } from "./components/agents/ChatSettingsDialog.tsx";
import { StudioSettingsLayout } from "./components/StudioSettingsTabs.tsx";
import { createAgentCatalog } from "./components/agents/state.ts";
import { modalSizes } from "./theme.ts";
import {
  AppearanceSettings,
  HotkeySettings,
} from "./app/settings/PreferenceViews.tsx";
import { useAppPreferences } from "./app/settings/useAppPreferences.ts";
import { SidebarListOwner } from "./app/shell/SidebarOwner.ts";
import { startAppFrame } from "./servers/frameBridge.ts";
import type { ServerCommand } from "./servers/navigation.ts";
import { makeAutoObservable, observableRef, reaction, runInAction } from "mobx";
import { useEffect, useState, type ReactNode } from "react";
import { Button, Drawer, Modal } from "@mantine/core";
import type { ServerAreas, AgentModel } from "./app/contracts.ts";
import { randomUUID } from "./uuid.ts";
import { errorText } from "./api.ts";
import { StopOutcome, AgentStatus } from "./generated/api.ts";
import {
  deleteConversation,
  definiteDeleteRejection,
} from "./chatDeleteScope.ts";
import { useProjectDelete } from "./useProjectDelete.ts";
import { ProjectDeleteDialog } from "./components/ProjectDeleteDialog.tsx";
import ProjectDirectoryPicker, {
  browseDirectory,
} from "./components/ProjectDirectoryPicker.tsx";
import ProjectFoldersDialog, {
  isLocatedProject,
} from "./components/ProjectFoldersDialog.tsx";
import ProjectFolderDialog from "./components/ProjectFolderDialog.tsx";
import ProjectAccount from "./components/ProjectAccount.tsx";
import ProjectServerCards from "./components/ProjectServerCards.tsx";
import {
  readProjectServers,
  registerProject,
} from "./servers/projectLocations.ts";
import type { ProjectBinding } from "./app/projects/CreationOwner.ts";
import SharedChatCreate from "./components/SharedChatCreate.tsx";
import TeamPanel from "./components/agents/TeamPanel.tsx";
import {
  TeamProvider,
  TeamPresence,
  useTeamArea,
} from "./components/agents/TeamPanelContext.tsx";
import Workspace from "./components/shell/Workspace.tsx";
import BackgroundTasks from "./components/shell/BackgroundTasks.tsx";
import TerminalDock from "./components/TerminalDock.tsx";
import FilePreview, {
  openFilePreview,
  type PreviewTarget,
  type FilePreviewSelection,
} from "./components/FilePreview.tsx";
import { needsAnswer } from "./approvals.ts";
import { resourceData } from "./store/runtime.ts";
import { isRemoteServerView } from "./servers/environment.ts";
import Sidebar from "./components/Sidebar.tsx";
import { ChatActionsMenu } from "./app/conversation/ChatActionsMenu.tsx";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "./app/ServerAreasProvider.tsx";
import { reportPromptComposerRender } from "./components/prompt-composer/renderProbe.ts";
import {
  StartupStatus,
  SchemaUpdateDialog,
  ServerUpdateStatus,
} from "./app/startup/StartupViews.tsx";
import { LayoutHost, WorkspaceSlot } from "./app/shell/LayoutHost.tsx";
import { RailRoot } from "./app/shell/NavigationButtons.tsx";
import { ConversationHeader } from "./app/conversation/ConversationHeader.tsx";
import { ManagedConversationRoot } from "./app/conversation/ManagedConversationRoot.tsx";
import SearchOverlay from "./components/shell/SearchOverlay.tsx";
import SidebarWarnings from "./components/SidebarWarnings.tsx";
import SupervisorRecoveryNotice from "./components/SupervisorRecoveryNotice.tsx";

export default observer(function App() {
  const a = useServerAreas(),
    { scope, startup, layout } = a;
  const c = a.settings;
  const p = useAppPreferences(a, a.preferenceBridge, (preferences) =>
    a.frame.publish({ kind: "studio-server-preferences", preferences }),
  );
  const signIn = (key: string) => {
    const account = [
      ...a.accounts.data.accounts,
      ...a.accounts.data.archivedAccounts,
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
  useEffect(() => startAppFrame(a), [a]);
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
  const [owner] = useState(() => new SidebarListOwner(a, undefined, a.ui.run));
  useEffect(() => owner.start(), [owner]);
  return a.sidebar.delegated ? null : <Sidebar owner={owner} />;
});
export class AppUi {
  readonly areas: ServerAreas;
  readonly requests;
  visited = false;
  section = "";
  focus: { id: string; requestId: string; roomId?: string } | undefined;
  teamOpen = false;
  tasksOpen = false;
  searchOpen = false;
  taskFocus: { leadId: string; id: string; requestId: string } | undefined;
  filePreview: FilePreviewSelection | null = null;
  descriptor: { title: string; body: ReactNode } | null = null;
  generation = 0;
  restore = 0;
  disposed = false;
  trigger: HTMLElement | null = null;
  get pending() {
    return this.areas.selected.pendingDeletion;
  }
  projectScope = new Set<string>();
  openProject: (id: string) => void = () => {};
  constructor(areas: ServerAreas) {
    this.areas = areas;
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
  async run(command: ServerCommand) {
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
      await this.showFolders(command.projectId, command.server);
    else if (command.action === "add-project") this.openAddProject();
    else if (command.action === "delete-project")
      this.openProject(command.projectId);
    else if (this.areas.settings.run(command)) return;
    else if (command.action === "search") {
      this.openSearch();
    } else if (command.action === "notifications") this.navigate(command);
    else {
      const result = await this.areas.run(command);
      if (result && typeof result === "object")
        await this.chooseProject(result.cwd, result.folder, result.binding);
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
    this.filePreview = openFilePreview(
      this.areas.scope.store,
      this.areas.scope.services.api,
      target,
    );
  }
  closePreview() {
    this.filePreview = null;
  }

  closeDialog() {
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
          initialBrowse={browseDirectory(
            this.areas.scope.store,
            this.areas.scope.services.api,
          )}
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
  async chooseProject(
    path?: string,
    folder?: string,
    binding?: ProjectBinding,
  ) {
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
      const servers = await readProjectServers(
        this.areas.scope.services.api.get,
      );
      if (
        request !== this.generation ||
        scope !== this.areas.startup.creationScope
      )
        return;
      runInAction(() => {
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
              lastServer={this.areas.scope.services.api.saved(
                `project-last-server:${project.id}`,
                "local",
              )}
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
      });
    } catch (error) {
      if (
        request === this.generation &&
        scope === this.areas.startup.creationScope
      )
        this.areas.notices.notify(errorText(error));
    }
  }
  async showFolders(id: string, server?: string) {
    const scope = this.areas.startup.creationScope,
      request = ++this.generation;
    try {
      const response = await this.areas.scope.services.api.get("/api/projects");
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
      if (isLocatedProject(project))
        runInAction(() => {
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
                initialHeads={heads}
                initialServer={server}
                initialBrowse={
                  server
                    ? browseDirectory(
                        this.areas.scope.store,
                        this.areas.scope.services.api,
                        undefined,
                        server,
                        project.id,
                      )
                    : undefined
                }
                saved={this.areas.refresh}
              />
            ),
          };
        });
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
            this.areas.scope.store,
            this.areas.scope.services.api,
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
            await this.areas.refresh();
          }}
        />
      ),
    };
  }
  showProjectAccount(path: string) {
    this.closeDialog();
    const projects = [...(this.areas.state?.projects.values() || [])];
    this.areas.sidebar.close();
    const generation = this.generation,
      scope = this.areas.startup.creationScope;
    this.descriptor = {
      title: "Project settings",
      body: (
        <ProjectAccount
          path={path}
          project={projects.find((p) => p.path === path)}
          accounts={this.areas.accounts.data}
          defaultAccountKey={
            projects
              .filter(
                (p) =>
                  p.path &&
                  (path === p.path ||
                    path.startsWith(p.path.replace(/\/$/, "") + "/")),
              )
              .sort((x, y) => (y.path?.length || 0) - (x.path?.length || 0))[0]
              ?.accountKey || this.areas.accounts.data.defaultAccountKey
          }
          saved={async () => {
            if (
              !this.disposed &&
              generation === this.generation &&
              scope === this.areas.startup.creationScope
            )
              this.closeDialog();
            await this.areas.refresh();
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
              runInAction(() => {
                this.areas.selected.suspendReaders(new Set([id]));
              });
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
                .finally(() =>
                  runInAction(() => {
                    if (this.pending.get(id) === controller) {
                      this.pending.delete(id);

                      if (!this.projectScope.has(id))
                        this.areas.selected.resumeReaders(new Set([id]));
                    }
                  }),
                );
            }}
          >
            {worker ? "Delete" : "Delete chat"}
          </Button>
        </>
      ),
    };
  }
  start() {
    this.disposed = false;
    const stop = reaction(
      () =>
        [...this.pending.keys()].filter(
          (id) => !this.areas.creation.lookup(id),
        ),
      (ids) => {
        for (const id of ids) {
          this.pending.get(id)?.abort();
          this.pending.delete(id);

          this.areas.selected.resumeReaders(new Set([id]));
        }
      },
    );
    const stopNavigation = reaction(
      () =>
        [
          this.areas.navigation.navigationIntent,
          this.areas.scope.screen.active,
        ] as const,
      () => {
        this.closeTeamDrawer();
        if (!this.areas.scope.screen.active) this.closePreview();
        cancelAnimationFrame(this.restore);
      },
    );
    return () => {
      stop();
      stopNavigation();
      this.dispose();
    };
  }
  dispose() {
    this.disposed = true;
    this.pending.forEach((c) => c.abort());
    this.pending.clear();
    this.setProjectScope(new Set());
    cancelAnimationFrame(this.restore);
  }
}

export const AppDialogs = observer(function AppDialogs() {
  const a = useServerAreas(),
    dialogs = a.ui;
  const project = useProjectDelete({
    state: a.state,
    metadata: a.scope.services.sync.projectionMetadata,
    workspaceId: a.startup.workspaceId,
    services: a.scope.services.sidebarServices,
    onSuspend: dialogs.setProjectScope,
    onResume: () => dialogs.setProjectScope(new Set()),
    onFinish: a.finishDeletion,
    notify: a.notices.notify,
  });
  useEffect(() => {
    dialogs.openProject = project.open;
    return () => {
      dialogs.openProject = () => {};
    };
  }, [dialogs, project.open]);
  const shared = a.creation.sharedCreate;
  return (
    <>
      <Modal
        opened={!!dialogs.descriptor}
        onClose={dialogs.closeDialog}
        title={dialogs.descriptor?.title}
      >
        {dialogs.descriptor?.body}
      </Modal>
      <ProjectDeleteDialog {...project.dialog} />
      {shared && (
        <Modal
          opened
          onClose={a.creation.closeShared}
          title="New shared chat"
          size="lg"
        >
          <SharedChatCreate
            source={a}
            accounts={a.accounts.data}
            initialPath={a.creation.sharedCreationInitialPath}
            refresh={a.refresh}
            created={a.creation.sharedCreated}
          />
        </Modal>
      )}
    </>
  );
});
export const AppTeam = observer(function AppTeam({
  children,
}: {
  children: ReactNode;
}) {
  const a = useServerAreas(),
    ui = a.ui,
    lead = a.navigation.selectedLead;
  const [query, setQuery] = useState(""),
    [filter, setFilter] = useState("all"),
    [override, setOverride] = useState<{
      id: string | null;
      value: boolean;
    } | null>(null);
  const view = useTeamArea(
    a.scope.screen.active ? lead?.id : undefined,
    query,
    filter,
    a.read.pendingReadTargets,
    a.selected.hasRetainedTeam,
  );
  useEffect(
    () =>
      reaction(
        () => a.navigation.selectedId,
        () => setOverride(null),
      ),
    [a],
  );
  useEffect(() => {
    setQuery("");
    setFilter("all");
  }, [lead?.id]);
  const actions = useMemo(
    () => ({
      setCompletedOpen: (value: boolean) =>
        setOverride({ id: a.navigation.selectedId, value }),
      setTeamOpen: (open: boolean) =>
        runInAction(() => {
          ui.teamOpen = open;
        }),
      setWideTeamOpen: (open: boolean) => ui.setTeamPreferred(open),
      open: a.open,
      previewResult: ui.showPreview,
      remove: (id: string) => ui.requestDelete({ id }, false, true),
    }),
    [a, ui],
  );
  if (view.view.members.length && ui.isTeamVisible)
    a.scope.services.tokenRates.observeTokenRates();
  const value = useMemo(
    () => ({
      view: view.view,
      retainedWorktrees: view.retainedWorktrees,
      hasTeam: () => view.view.workers.length > 0 || a.selected.hasRetainedTeam,
      get opened() {
        return a.navigation.selectedId;
      },
      workerQuery: query,
      setWorkerQuery: setQuery,
      workerFilter: filter,
      setWorkerFilter: setFilter,
      get completedOpen() {
        const selected = a.navigation.selectedAgent;
        return override?.id === a.navigation.selectedId
          ? override.value
          : !!selected &&
              !selected.isLead &&
              selected.status === AgentStatus.completed;
      },
      ...actions,
    }),
    [a, view.view, query, filter, override, actions],
  );
  return <TeamProvider value={value}>{children}</TeamProvider>;
});
export const AppPanels = observer(function AppPanels() {
  const a = useServerAreas(),
    ui = a.ui,
    l = a.layout,
    active = a.scope.screen.active,
    lead = a.navigation.selectedLead,
    agent = a.navigation.selectedAgent;
  return (
    <>
      {a.layout.isNarrowTeam ? (
        <TeamPresence>
          <Drawer
            opened={ui.isTeamVisible}
            onClose={ui.closeTeamDrawer}
            position="right"
            size="var(--studio-team-size)"
            padding={0}
            withCloseButton={false}
            title="Team"
            classNames={{ header: "sr-only" }}
          >
            <TeamPanel />
          </Drawer>
        </TeamPresence>
      ) : ui.isTeamVisible ? (
        <TeamPanel />
      ) : null}
      {ui.visited && a.startup.isReady && (
        <Workspace
          key={`workspace:${lead?.id || "none"}`}
          opened={active && ui.opened}
          onClose={ui.closeWorkspace}
          initialSection={ui.section}
          initialFocus={ui.focus}
          inlineTarget={l.slot}
          panelLayout={l.panelLayout}
          onPanelWidthChange={l.setPreferredPanelWidth}
          resizeScope={
            a.startup.workspaceId || a.startup.creationScope || "pending"
          }
          agent={agent || lead}
          onSelect={a.open}
          refresh={a.refresh}
          notify={a.notices.notify}
        />
      )}
      <BackgroundTasks
        key={`tasks:${lead?.id || "none"}`}
        opened={active && ui.tasksOpen}
        initialFocus={ui.taskFocus}
        close={ui.closeTasks}
        afterClose={ui.afterTaskClose}
        leadId={active && ui.tasksOpen ? lead?.id : undefined}
        openAgent={a.open}
        refresh={a.refresh}
        notify={a.notices.notify}
      />
      {active && !l.isMobile && a.preferences.value.showTerminals && (
        <TerminalDock agent={agent || lead} notify={a.notices.notify} />
      )}
      <FilePreview
        target={active ? ui.filePreview : null}
        onClose={ui.closePreview}
      />
    </>
  );
});

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
  import("./components/models/ModelWhitelistSettings.tsx").then((m) => ({
    default: m.ModelWhitelistSettings,
  })),
);
const Orchestration = lazy(() =>
  import("./components/OrchestrationSettings.tsx").then((m) => ({
    default: m.OrchestrationSettings,
  })),
);
const Worktrees = lazy(() =>
  import("./components/LayrWorktreesSettings.tsx").then((m) => ({
    default: m.LayrWorktreesSettings,
  })),
);
const Alerts = lazy(() =>
  import("./components/SignInPushSettings.tsx").then((m) => ({
    default: m.SignInPushSettings,
  })),
);
const Federation = lazy(() =>
  import("./components/FederationSettings.tsx").then((m) => ({
    default: m.FederationSettings,
  })),
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
  const catalog = useMemo(() => createAgentCatalog(a.scope.store), [a]);
  const lead = a.navigation.selectedLead || a.navigation.selectedAgent;
  const server = a.scope.services.owner;
  return (
    <>
      <Accounts
        managerOnly
        dialogsOnly
        state={a.accounts}
        onError={a.notices.notify}
        onModalOpenChange={c.reportAccountDialog}
        externalCommand={c.command}
        onExternalCommandHandled={() => c.setCommand(null)}
      />
      <Modal
        opened={c.studioOpen && !c.accountModalOpen}
        transitionProps={{ duration: 0 }}
        closeOnEscape={!c.accountModalOpen}
        closeOnClickOutside={!c.accountModalOpen}
        onClose={() => c.setStudioOpen(false)}
        size={modalSizes.settings}
        title="Studio settings"
        classNames={{ body: "studio-settings-body" }}
      >
        <div className="studio-settings-panel" data-testid="studio-settings">
          <StudioSettingsLayout value={c.tab} onChange={c.activateTab}>
            <Tabs.Panel value="accounts" pt="md">
              <section className="settings-group" aria-label="Studio accounts">
                <ServerAccountsPanel
                  servers={[server]}
                  accountsByServer={{
                    [server.id]: frameAccounts(a.accounts.data),
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
                    refresh={a.refresh}
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
    catalog = useMemo(() => createAgentCatalog(a.scope.store), [a]);
  const agent = a.navigation.selectedAgent,
    lead = a.navigation.selectedLead || agent;
  const key =
    agent?.accountKey ||
    lead?.accountKey ||
    (agent ? "default" : a.accounts.data.defaultAccountKey) ||
    "default";
  return (
    <>
      {[...a.accounts.data.accounts, ...a.accounts.data.archivedAccounts]
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
