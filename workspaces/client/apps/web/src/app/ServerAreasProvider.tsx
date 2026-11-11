import { TeamOwner } from "../components/agents/support/TeamOwner.ts";
import { SettingsModels } from "./settings/SettingsModels.ts";
import { WorkspaceResources } from "../components/shell/Workspace/Workspace.tsx";
import { AgentsUi } from "../components/agents/support/AgentsUi.ts";
import { SignInPush } from "../components/accounts/SignInPushSettings/SignInPushSettings.tsx";
import { UnsentText } from "../unsent/useUnsentText.ts";
import { startPerformanceReporting } from "../perf/collector.ts";
import { ProjectDeletion } from "../useProjectDelete.ts";
import { AccountsUi } from "../components/accounts/support/AccountsUi.ts";
import { AppUi } from "../components/shell/App/App.tsx";
import { SettingsUi } from "./settings/useAppSettings.ts";
import { observable, observableRef, action, flow } from "mobx";
import { createArea } from "./area.ts";
import { resourceData } from "../store/runtime.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { focusSelectedComposer } from "../hooks/mobileViewport.ts";
import { receiveSidebarRead } from "../servers/receiveSidebarRead.ts";
import type { ServerCommand } from "../servers/navigation.ts";
import type { AppScope, Disposer } from "./contracts.ts";
import { StartupOwner } from "./startup/StartupOwner.ts";
import { publishFrame } from "../servers/frameBridge.ts";
import { NavigationOwner } from "./shell/NavigationOwner.ts";
import { LayoutOwner } from "./shell/LayoutOwner.ts";
import { SidebarOwner } from "./shell/SidebarOwner.ts";
import { CreationOwner } from "./projects/CreationOwner.ts";
import { createUiPreferenceBridge } from "../sync/uiPreferenceBridge.ts";
import {
  AppPreferences,
  loadStudioPreferences,
} from "./settings/useAppPreferences.ts";
import { DeliveryOwner } from "./conversation/DeliveryOwner.ts";
import { ReadStateOwner } from "./conversation/ReadStateOwner.ts";
import { NativeActionOwner } from "./conversation/NativeActionOwner.ts";
import { RenameOwner } from "./conversation/RenameOwner.ts";
import { SelectedConversationOwner } from "./conversation/SelectedConversationOwner.ts";

/** Stable area references are constructed once by the authenticated server lifetime. */
export class ServerAreas {
  readonly settingsModels: SettingsModels;
  readonly workspace: WorkspaceResources;
  readonly signInPush: SignInPush;
  readonly accountsUi: AccountsUi;
  readonly agentsUi: AgentsUi;
  readonly team: TeamOwner;
  readonly projectDeletion: ProjectDeletion;
  readonly unsentText: UnsentText;
  readonly scope: AppScope;
  readonly startup: StartupOwner;
  readonly frame;
  readonly layout: LayoutOwner;
  readonly ui: AppUi;
  readonly settings: SettingsUi;
  readonly sidebar: SidebarOwner;
  readonly navigation: NavigationOwner;
  readonly creation: CreationOwner;
  readonly preferences;
  get accounts() {
    return resourceData(this.scope.store.state().current)?.accountsSummary.get(
      "current",
    );
  }
  readonly preferenceBridge;
  readonly preferenceControls: AppPreferences;
  readonly read: ReadStateOwner;
  readonly rename: RenameOwner;
  readonly native: NativeActionOwner;
  readonly delivery: DeliveryOwner;
  readonly selected: SelectedConversationOwner;
  private toastTimer: ReturnType<typeof setTimeout> | undefined;
  readonly notices = observable({
    toast: "",
    notify: action((message: string) => {
      if (this.disposed) return;
      this.notices.toast = message;
      clearTimeout(this.toastTimer);
      this.toastTimer = setTimeout(
        action(() => {
          this.notices.toast = "";
        }),
        5000,
      );
    }),
  });
  stops: Disposer[] = [];
  disposed = false;
  constructor(scope: AppScope) {
    this.scope = scope;
    this.workspace = new WorkspaceResources(
      scope.services.api,
      scope.services.owner.id,
    );
    this.unsentText = new UnsentText(scope.services.owner.id);
    this.accountsUi = new AccountsUi(this);
    this.signInPush = new SignInPush(scope.services.api);
    this.preferenceBridge = createUiPreferenceBridge(
      scope.services.storage,
      scope.services.api.syncPost,
    );
    this.preferences = observable(
      { value: loadStudioPreferences(scope.services.storage).value },
      { value: observableRef },
    );
    this.preferenceControls = new AppPreferences(this);
    const { services, screen } = scope;
    this.startup = new StartupOwner(scope, {
      rendererBusy: () => {
        const conversation = this.selected.conversation,
          state = conversation?.context.dictation;
        return (
          !!conversation?.addingFiles ||
          !!conversation?.context.uploadRecovery.pending.length ||
          !!state?.active ||
          !!state?.busy ||
          !!state?.storageFailed ||
          this.delivery.hasLocalPersistenceFailure ||
          this.delivery.isSending
        );
      },
      canUpdateRenderer: () =>
        !this.delivery.hasLocalPersistenceFailure &&
        !this.delivery.isSending &&
        !this.creation.isCreating &&
        !this.ui.descriptor &&
        !this.creation.sharedCreate &&
        !this.settings.studioOpen,
      acceptUiPreferences: (value) =>
        this.preferenceBridge.receiveProjection(value),
    });
    this.layout = new LayoutOwner(this);
    this.ui = new AppUi(this);
    const { startup } = this;
    this.projectDeletion = new ProjectDeletion({
      get state() {
        return resourceData(scope.store.state().current);
      },
      get metadata() {
        return scope.services.sync.projectionMetadata;
      },
      get workspaceId() {
        return startup.workspaceId;
      },
      services: scope.services.sidebarServices,
      onSuspend: this.ui.setProjectScope,
      onResume: () => this.ui.setProjectScope(new Set()),
      onFinish: this.finishDeletion,
      notify: this.notices.notify,
    });
    this.settings = new SettingsUi(screen, services.shellSettings ?? null);
    this.sidebar = new SidebarOwner(this);
    this.navigation = new NavigationOwner(this);

    this.creation = new CreationOwner(this);
    this.read = new ReadStateOwner(this);
    this.rename = new RenameOwner(this);
    this.native = new NativeActionOwner(this);
    this.delivery = new DeliveryOwner(this);
    this.selected = new SelectedConversationOwner(this);
    this.settingsModels = new SettingsModels(this);
    this.agentsUi = new AgentsUi(
      scope,
      () => this.startup.workspaceId,
      () => this.navigation.selectedAgent?.id,
    );
    this.team = new TeamOwner(this);
    this.frame = {
      publish: (message: Parameters<typeof publishFrame>[1]) =>
        publishFrame(scope, message),
    };
  }
  get state() {
    return resourceData(this.scope.store.state().current);
  }
  open = (id: string) =>
    this.navigation.selectExternal(id, { requestId: randomUUID() });
  finishDeletion = (ids: string[]) => {
    this.creation.forget(ids);
    this.navigation.finishDeletion(ids.map((id) => ({ id })));
    this.scope.services.drafts.setDrafts((old) =>
      Object.fromEntries(
        Object.entries(old).filter(([id]) => !ids.includes(id)),
      ),
    );
  };
  run = flow(function* (this: ServerAreas, command: ServerCommand) {
    if (command.action === "open")
      this.navigation.selectExternal(command.id, {
        requestId: randomUUID(),
        ...(command.messageId ? { messageId: command.messageId } : {}),
      });
    else if (command.action === "new-chat")
      return yield this.creation.newChat(
        command.path,
        command.folder,
        undefined,
        command.projectId && command.projectServerId
          ? {
              projectId: command.projectId,
              homeServerId: command.projectServerId,
            }
          : undefined,
        command.settings,
      );
    else if (command.action === "mark-unread")
      yield receiveSidebarRead(command, this.read.markSidebarUnread);
    else if (command.action === "new-shared-chat") {
      this.sidebar.close();
      this.creation.openShared(command.path);
    } else if (command.action === "focus" && this.scope.screen.active) {
      focusSelectedComposer(document.getElementById("message"));
      window.dispatchEvent(new Event("focus"));
    } else if (command.action === "projects") this.sidebar.show();
    return undefined;
  }).bind(this);
  start() {
    this.disposed = false;
    this.stops = [
      this.layout.start(),
      this.preferenceBridge.start(),
      this.preferenceControls.start(),
      this.startup.start(),
      this.navigation.start(),
      this.creation.start(),
      this.read.start(),
      this.rename.start(),
      this.native.start(),
      this.delivery.start(),
      this.selected.start(),
      this.ui.start(),
      this.projectDeletion.start(),
      ...(typeof __STUDIO_PERF_REPORTING__ !== "undefined" &&
      __STUDIO_PERF_REPORTING__
        ? [startPerformanceReporting(this.scope.services.api)]
        : []),
    ];
    return this.dispose;
  }
  dispose = action(() => {
    this.disposed = true;
    this.workspace.dispose();
    this.accountsUi.dispose();
    this.settingsModels.dispose();
    this.team.dispose();
    this.agentsUi.dispose();
    this.ui.generation++;
    clearTimeout(this.toastTimer);
    this.notices.toast = "";
    this.stops
      .splice(0)
      .reverse()
      .forEach((stop) => stop());
  });
}

const [Provider, useServerAreas] = createArea<ServerAreas>("ServerAreas");
export { Provider };

export { useServerAreas, Provider as ServerAreasContextProvider };
