import { AccountsUi } from "../components/accounts/support/AccountsUi.ts";
import { AppUi } from "../components/shell/App/App.tsx";
import { SettingsUi } from "./settings/useAppSettings.ts";
import { observable, observableRef, runInAction } from "mobx";
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
import { loadStudioPreferences } from "./settings/useAppPreferences.ts";
import { DeliveryOwner } from "./conversation/DeliveryOwner.ts";
import { ReadStateOwner } from "./conversation/ReadStateOwner.ts";
import { NativeActionOwner } from "./conversation/NativeActionOwner.ts";
import { RenameOwner } from "./conversation/RenameOwner.ts";
import { SelectedConversationOwner } from "./conversation/SelectedConversationOwner.ts";

/** Stable area references are constructed once by the authenticated server lifetime. */
export class ServerAreas {
  readonly accountsUi: AccountsUi;
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
  readonly read: ReadStateOwner;
  readonly rename: RenameOwner;
  readonly native: NativeActionOwner;
  readonly delivery: DeliveryOwner;
  readonly selected: SelectedConversationOwner;
  private toastTimer: ReturnType<typeof setTimeout> | undefined;
  readonly notices = observable({
    toast: "",
    notify: (message: string) => {
      if (this.disposed) return;
      runInAction(() => {
        this.notices.toast = message;
      });
      clearTimeout(this.toastTimer);
      this.toastTimer = setTimeout(
        () =>
          runInAction(() => {
            this.notices.toast = "";
          }),
        5000,
      );
    },
  });
  stops: Disposer[] = [];
  disposed = false;
  constructor(scope: AppScope) {
    this.scope = scope;
    this.accountsUi = new AccountsUi(this);
    this.preferenceBridge = createUiPreferenceBridge(
      scope.services.storage,
      scope.services.api.syncPost,
    );
    this.preferences = observable(
      { value: loadStudioPreferences(scope.services.storage).value },
      { value: observableRef },
    );
    const { services, screen } = scope;
    this.startup = new StartupOwner(scope, {
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
    this.settings = new SettingsUi(screen, services.shellSettings ?? null);
    this.sidebar = new SidebarOwner(this);
    this.navigation = new NavigationOwner(this);

    this.creation = new CreationOwner(this);
    this.read = new ReadStateOwner(this);
    this.rename = new RenameOwner(this);
    this.native = new NativeActionOwner(this);
    this.delivery = new DeliveryOwner(this);
    this.selected = new SelectedConversationOwner(this);
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
  run = async (command: ServerCommand) => {
    if (command.action === "open")
      this.navigation.selectExternal(command.id, {
        requestId: randomUUID(),
        ...(command.messageId ? { messageId: command.messageId } : {}),
      });
    else if (command.action === "new-chat")
      return this.creation.newChat(
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
      await receiveSidebarRead(command, this.read.markSidebarUnread);
    else if (command.action === "new-shared-chat") {
      this.sidebar.close();
      this.creation.openShared(command.path);
    } else if (command.action === "focus" && this.scope.screen.active) {
      focusSelectedComposer(document.getElementById("message"));
      window.dispatchEvent(new Event("focus"));
    } else if (command.action === "projects") this.sidebar.show();
    return undefined;
  };
  start() {
    this.disposed = false;
    this.stops = [
      this.layout.start(),
      this.preferenceBridge.start(),
      this.startup.start(),
      this.navigation.start(),
      this.creation.start(),
      this.read.start(),
      this.rename.start(),
      this.native.start(),
      this.delivery.start(),
      this.selected.start(),
      this.ui.start(),
    ];
    return this.dispose;
  }
  dispose = () => {
    this.disposed = true;
    this.accountsUi.dispose();
    this.ui.generation++;
    clearTimeout(this.toastTimer);
    runInAction(() => {
      this.notices.toast = "";
    });
    runInAction(() =>
      this.stops
        .splice(0)
        .reverse()
        .forEach((stop) => stop()),
    );
  };
}

const [Provider, useServerAreas] = createArea<ServerAreas>("ServerAreas");
export { Provider };

export { useServerAreas, Provider as ServerAreasContextProvider };
