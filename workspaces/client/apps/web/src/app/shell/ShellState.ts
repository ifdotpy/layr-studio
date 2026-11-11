import {
  compareStructural,
  computed,
  flow,
  flowResult,
  makeAutoObservable,
  reaction,
  when,
} from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { ProjectServerSelection } from "../projects/CreationOwner.ts";
import type { NewChatSettings } from "../projects/newChatSettings.ts";
import { ScreenSelection } from "../startup/ScreenOwner.ts";
import { ShellSidebarSource } from "../startup/ShellSidebarSource.ts";
import {
  AccessClientStatus,
  Reachability,
  TerminalState,
} from "../../generated/api.ts";
import { startDesktopNotifications } from "../../hooks/desktopNotifications.ts";
import { onDesktopNavigate } from "../../native/notifications.ts";
import { CombinedSidebarOwner } from "../../servers/CombinedServerSidebar.tsx";
import type {
  ServerCommand,
  ServerNavigation,
} from "../../servers/navigation.ts";
import type { GetResult } from "../../servers/net/api.ts";
import { fetchServerSummary } from "../../servers/net/summary.ts";
import { orderPreferenceServers } from "../../servers/preferenceServerOrder.ts";
import { projectChatCommand } from "../../servers/projectChatCommand.ts";
import type { ProjectLocation } from "../../servers/projectLocations.ts";
import {
  projectServerChoices,
  type LocationProject,
} from "../../servers/projectLocations.ts";
import {
  localServer,
  readServers,
  SERVER_REGISTRY_EVENT,
  SERVER_REGISTRY_KEY,
  shareServers,
  writeServers,
  type StudioServer,
} from "../../servers/registry.ts";
import { recoverMatchingServers } from "../../servers/schemaRecovery.ts";
import {
  createScreenChannel,
  type ScreenChannel,
  type ScreenMessage,
  type ScreenSearchResults,
} from "../../servers/screenChannel.ts";
import { defaultServerAlias } from "../../servers/serverAliases.ts";
import { serverRows } from "../../servers/serverRows.ts";
import {
  createServerServices,
  type ServerServices,
} from "../../servers/serverServices.ts";
import { preferenceMemory } from "../../servers/storage.ts";
import { ServerDiscovery } from "../../servers/useServerDiscovery.ts";
import { startUiPreferenceSync } from "../../servers/useUiPreferenceSync.ts";
import { resourceData } from "../../store/runtime.ts";
import type { ConnectionHealth } from "../../sync/connectionState.ts";
import { writePreferenceEdit } from "../../sync/uiPreferenceStore.ts";
import { frameAccounts } from "../../components/accounts/Accounts/Accounts.tsx";
import { type StudioSettingsTab } from "../../components/settings/StudioSettingsLayout/StudioSettingsLayout.tsx";
import { isStudioSettingsTab } from "../../components/settings/StudioSettingsTabs/StudioSettingsTabs.tsx";
import { restoreSidebarFocus } from "../../components/sidebar/SidebarFrame/SidebarFrame.tsx";
import { accountRows } from "../../servers/AccountsPanelState.ts";

import { accountServers } from "../../servers/accountServers.ts";
import { type ServerAccount } from "../../servers/navigation.ts";
import { serverAccountRows } from "../../servers/serverAccountRows.ts";
import {
  parseStudioPreferences,
  studioPreferencesStorageKey,
} from "../../studioPreferences.ts";

export const uiOnly =
  new URLSearchParams(location.search).get("studio-ui-only") === "1";
export class ShellState {
  failure = "";
  projectSelection: ProjectServerSelection | null = null;
  newChatDialog: {
    project: LocationProject;
    server: string;
    folder?: string | undefined;
    binding?: { projectId: string; projectServerId: string } | undefined;
  } | null = null;
  paired = this.load();
  registryRevision = 0;
  lastSeen: Record<string, number> = {};
  searchOpen = false;
  searchQuery = "";
  searchResults: Record<string, ScreenSearchResults> = {};
  shellSidebar = false;
  settingsOpen = uiOnly && !this.paired.length;
  settingsTab: StudioSettingsTab = uiOnly ? "servers" : "accounts";
  restoreManagerAfterAccountDialog = false;
  restoreAccountActionFocus: string | null = null;
  searchRequest = "";
  readonly notificationStops = new Map<ServerServices, () => void>();
  screens = new Map<string, ScreenChannel>();
  pending = new Map<string, ServerCommand>();
  serviceInstances = new Map<string, ServerServices>();
  revived: Record<string, number> = {};
  revivedSeen: Record<string, number> = {};
  readonly selection = new ScreenSelection(
    this.servers.map((server) => server.id),
  );
  readonly sidebar = new ShellSidebarSource(this.selection, (id) =>
    this.serviceInstances.get(id),
  );
  readonly localServices;
  readonly discovery;
  readonly combinedSidebar;
  constructor(localServices = createServerServices(localServer())) {
    this.localServices = localServices;
    this.serviceInstances = new Map(
      this.servers.map((server) => [
        server.id,
        server.id === "local" && localServices
          ? localServices
          : createServerServices(server),
      ]),
    );
    makeAutoObservable(
      this,
      {
        selection: false,
        sidebar: false,
        screens: false,
        executeCommand: false,
        notificationStops: false,
        servers: computed({ equals: compareStructural }),
        aliases: computed({ equals: compareStructural }),
      },
      { autoBind: true, deep: false },
    );
    this.discovery = new ServerDiscovery(
      localServices,
      !uiOnly &&
        ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname),
      !uiOnly,
      () =>
        Object.fromEntries(
          this.servers.map((server) => [server.id, server.workspaceId]),
        ),
    );
    localServices.shellSettings = this;
    const creation = () => this.creation;
    this.combinedSidebar = new CombinedSidebarOwner(this.sidebar, this, {
      areas: localServices.areas,
      get creation() {
        return creation();
      },
    });
  }
  get directScreen() {
    return (
      !uiOnly &&
      !this.paired.length &&
      this.accountsServers.length === this.servers.length
    );
  }

  load(): StudioServer[] {
    try {
      return readServers();
    } catch (error) {
      this.failure = String((error as Error).message);
      return [];
    }
  }
  get servers(): StudioServer[] {
    void this.registryRevision;
    return orderPreferenceServers(
      (uiOnly ? this.paired : [localServer(), ...this.paired]).map((server) => {
        const workspaceId = this.sidebar?.navigation[server.id]?.workspaceId;
        return workspaceId ? { ...server, workspaceId } : server;
      }),
    );
  }
  get aliases() {
    return Object.fromEntries(
      this.servers.map((server) => [
        server.id,
        this.discovery?.snapshot?.aliases?.[server.id] ||
          server.alias ||
          defaultServerAlias(server),
      ]),
    );
  }
  get current() {
    return this.selection.current;
  }
  get accountsServers() {
    return accountServers(
      this.servers,
      this.discovery?.snapshot?.servers || [],
    );
  }
  get sidebarVisible() {
    return !this.sidebar.hidden;
  }

  set(fields: Partial<ShellState>) {
    if ("newChatDialog" in fields) {
      this.projectSelection?.dispose();
      const dialog = fields.newChatDialog;
      this.projectSelection = dialog
        ? new ProjectServerSelection(
            dialog.project,
            () => this.projectServerChoices,
            dialog.server,
            (id) => this.serviceInstances.get(id),
          )
        : null;
    }
    Object.assign(this, fields);
  }
  setSettingsOpen(settingsOpen: boolean) {
    this.settingsOpen = settingsOpen;
  }
  setSettingsTab(settingsTab: StudioSettingsTab) {
    this.settingsTab = settingsTab;
  }
  openManager() {
    this.settingsTab = "servers";
    this.settingsOpen = true;
  }

  reread() {
    const paired = this.load();
    this.selection.updateServers(
      orderPreferenceServers(uiOnly ? paired : [localServer(), ...paired]).map(
        (server) => server.id,
      ),
    );
    this.paired = shareServers(this.paired, paired);
    this.registryRevision++;
  }

  publishPreferences(value: unknown) {
    window.dispatchEvent(
      new CustomEvent("studio-shell-preferences", { detail: value }),
    );
  }
  select(id: string) {
    this.selection.select(id);
    this.sidebar.setMobileOpen(false);
    requestAnimationFrame(() => {
      if (this.current !== id) return;
      const screen = this.screens.get(id);
      if (screen?.ready)
        void screen.execute({ action: "focus" }).catch(() => {});
    });
  }
  send(id: string, command: ServerCommand) {
    this.select(id);
    const screen = this.screens.get(id);
    if (!screen?.ready || !this.sidebar.navigation[id]?.ready)
      this.pending.set(id, command);
    else
      void screen
        .execute(command)
        .catch((error) => this.set({ failure: String(error) }));
  }
  applyNavigation(id: string, value: ServerNavigation) {
    this.sidebar.setNavigation((old) => ({ ...old, [id]: value }));
    const screen = this.screens.get(id);
    if (this.pending.has(id) && value.ready && screen?.ready) {
      const command = this.pending.get(id)!;
      this.pending.delete(id);
      void screen
        .execute(command)
        .catch((error) => this.set({ failure: String(error) }));
    }
  }
  executeCommand = flow(function* (
    this: ShellState,
    owner: string,
    command: ServerCommand,
  ) {
    const screen = this.screens.get(owner);
    if (!screen) throw new Error("The server is no longer paired.");
    if (!this.sidebar.navigation[owner]?.ready || !screen.ready) {
      try {
        yield when(
          () => !!this.sidebar.navigation[owner]?.ready && screen.ready,
          { timeout: 15000 },
        );
      } catch {
        throw new Error("The server screen is unavailable.");
      }
    }
    if (this.screens.get(owner) !== screen)
      throw new Error(
        "The server connection changed. The action was not sent.",
      );
    yield screen.execute(command);
  }).bind(this);
  screenEvents(id: string, data: ScreenMessage, channel: ScreenChannel) {
    if (this.screens.get(id) !== channel) return;
    if (data.kind === "studio-open-new-chat") {
      this.sidebar.setMobileOpen(false);
      const view = this.sidebar.navigation[id];
      const project = view?.projects.find(
        (row) => row.id === data.projectId || row.path === data.path,
      );
      const directory =
        project?.path ||
        view?.chats.find((row) => row.id === view.opened)?.path ||
        (data.path as string) ||
        "";
      this.set({
        newChatDialog: {
          server: id,
          project: {
            ...project,
            id: project?.id || directory,
            path: directory,
            name: project?.name || directory || "Current folder",
            locations: project?.locations?.length
              ? project.locations
              : [
                  {
                    serverId: id,
                    path: directory,
                    projectId: project?.id || directory,
                  },
                ],
          },
          folder:
            typeof data.projectFolder === "string"
              ? data.projectFolder
              : undefined,
          // Keep the screen owner's project binding. A folder belongs to that project.
          binding:
            typeof data.projectId === "string" &&
            typeof data.projectServerId === "string"
              ? {
                  projectId: data.projectId,
                  projectServerId: data.projectServerId,
                }
              : undefined,
        },
      });
    } else if (data.kind === "studio-project-new-chat") {
      const target = this.servers.find((server) => server.id === data.target);
      const command =
        target && projectChatCommand(this.sidebar.navigation[id], data);
      if (target && command) this.send(target.id, command);
    } else if (data.kind === "studio-server-account-dialog") {
      if (typeof data.opened !== "boolean") return;
      if (data.opened) {
        if (this.settingsOpen) {
          this.restoreManagerAfterAccountDialog = true;
          this.set({ settingsOpen: false });
        }
      } else if (this.restoreManagerAfterAccountDialog) {
        this.restoreManagerAfterAccountDialog = false;
        this.set({ settingsOpen: true });
      }
    } else if (data.kind === "studio-server-open-settings") {
      this.set({
        settingsTab: data.tab === "accounts" ? "accounts" : "servers",
      });
      this.set({ settingsOpen: true });
    } else if (data.kind === "studio-server-preferences") {
      try {
        const value = parseStudioPreferences(JSON.stringify(data.preferences));
        const next = writePreferenceEdit(
          studioPreferencesStorageKey,
          { ...this.sidebar.value },
          { ...value },
        );
        this.sidebar.setPreferences(next);
        this.publishPreferences(next);
      } catch {}
    } else if (data.kind === "studio-server-toggle-sidebar") {
      if (typeof data.opened === "boolean")
        this.sidebar.setMobileOpen(data.opened);
      else this.toggleSidebar();
    } else if (data.kind === "studio-server-search")
      this.set({ searchOpen: true });
    else if (data.kind === "studio-server-navigation") {
      const value = data.navigation as ServerNavigation;
      this.publishPreferences(this.sidebar.value);
      if (
        !value ||
        !Array.isArray(value.projects) ||
        !Array.isArray(value.chats)
      )
        return;
      this.applyNavigation(id, value);
    } else if (
      data.kind === "studio-server-status" &&
      ["connecting", "live", "offline", "degraded", "schema-mismatch"].includes(
        data.status as string,
      )
    ) {
      this.sidebar.setStatus((old) => ({
        ...old,
        [id]: data.status as ConnectionHealth,
      }));
      if (data.status === "live")
        this.set({ lastSeen: { ...this.lastSeen, [id]: Date.now() / 1000 } });
    }
  }
  applyPreferences() {
    try {
      const next = parseStudioPreferences(
        preferenceMemory.getItem(studioPreferencesStorageKey) || "null",
      );
      this.sidebar.setPreferences(next);
      this.publishPreferences(next);
    } catch {}
  }
  start() {
    const stopDiscovery = this.discovery.start();
    const stopSidebarSource = this.sidebar.start();
    const stopServices = reaction(
      () => [this.servers, this.revived, this.directScreen],
      () => this.connectServices(this.localServices, this.directScreen),
      { fireImmediately: true },
    );
    let stopPreferences = () => {};
    const stopRegistry = reaction(
      () =>
        JSON.stringify(
          this.servers.map(({ id, origin, credentialId, workspaceId }) => [
            id,
            origin,
            credentialId,
            workspaceId,
          ]),
        ),
      () => {
        stopPreferences();
        stopPreferences = startUiPreferenceSync(this.servers);
      },
      { fireImmediately: true },
    );
    let stopCombined = () => {};
    const stopSidebar = reaction(
      () => this.directScreen,
      (direct) => {
        stopCombined();
        stopCombined = direct ? () => {} : this.combinedSidebar.start();
      },
      { fireImmediately: true },
    );
    const stored = (event: Event) => {
      const key = (event as StorageEvent).key;
      if (key === SERVER_REGISTRY_KEY || key === null) this.reread();
      this.applyPreferences();
    };
    const events: [string, EventListener][] = [
      ["storage", stored],
      [SERVER_REGISTRY_EVENT, this.reread],
      ["focus", this.reviveMatching],
      ["studio-preferences-change", this.applyPreferences],
    ];
    for (const [name, listener] of events)
      window.addEventListener(name, listener);
    const stopRevive = reaction(
      () => [this.settingsOpen, this.settingsTab] as const,
      ([opened, tab]) => {
        if (opened && (tab === "servers" || tab === "accounts"))
          this.reviveMatching();
      },
      { fireImmediately: true },
    );
    const stopNavigate = onDesktopNavigate((target) => {
      if (target.serverId)
        this.send(target.serverId, { action: "notifications", ...target });
    });
    return () => {
      for (const [name, listener] of events)
        window.removeEventListener(name, listener);
      stopRevive();
      stopDiscovery();
      stopSidebarSource();
      stopServices();
      stopRegistry();
      stopPreferences();
      stopSidebar();
      stopCombined();
      stopNavigate();
      this.close();
    };
  }
  connectServices(
    localServices: ServerServices | undefined,
    directScreen: boolean,
  ) {
    const services = new Map<string, ServerServices>(),
      channels = new Map<string, ScreenChannel>();
    for (const server of this.servers) {
      const previous = this.serviceInstances.get(server.id);
      const service =
        previous &&
        (this.revived[server.id] || 0) === (this.revivedSeen[server.id] || 0) &&
        previous.owner.origin === server.origin &&
        previous.owner.credentialId === server.credentialId
          ? previous
          : server.id === "local" && localServices
            ? localServices
            : createServerServices(server);
      service.shellSettings = this;
      this.revivedSeen[server.id] = this.revived[server.id] || 0;
      let channel =
        previous === service ? this.screens.get(server.id) : undefined;
      if (!channel) {
        const next = createScreenChannel(server.id, (message) =>
          this.screenEvents(server.id, message, next),
        );
        channel = next;
      }
      services.set(server.id, service);
      channels.set(server.id, channel);
      service.screen.connect(
        directScreen ? null : channel,
        () => this.selection.current === server.id,
      );
      if (!service.areas.stops.length) service.areas.start();
      const enabled = () => !this.directScreen;
      const opened = () =>
        this.current === server.id ? service.areas.navigation.selectedId : null;
      const row = () =>
        this.servers.find((candidate) => candidate.id === server.id) || server;
      if (!this.notificationStops.has(service))
        this.notificationStops.set(
          service,
          startDesktopNotifications({
            store: service.store,
            get metadata() {
              return service.sync.projectionMetadata;
            },
            get enabled() {
              return enabled();
            },
            get opened() {
              return opened();
            },
            get server() {
              return row();
            },
            navigate: (target) =>
              this.send(server.id, { action: "notifications", ...target }),
          }),
        );
    }
    for (const [id, previous] of this.serviceInstances) {
      const next = services.get(id);
      if (next !== previous)
        this.closeService(
          previous,
          !next ||
            next.owner.origin !== previous.owner.origin ||
            next.owner.credentialId !== previous.owner.credentialId,
        );
    }
    for (const [id, channel] of this.screens)
      if (channels.get(id) !== channel) channel.close();
    this.screens = channels;
    this.serviceInstances = services;
  }
  closeService(
    service: ServerServices,
    ...args: Parameters<ServerServices["close"]>
  ) {
    this.notificationStops.get(service)?.();
    this.notificationStops.delete(service);
    service.close(...args);
  }
  close() {
    this.projectSelection?.dispose();
    this.projectSelection = null;
    for (const channel of this.screens.values()) channel.close();
    for (const service of this.serviceInstances.values())
      this.closeService(service);
    this.screens = new Map();
    this.serviceInstances = new Map();
  }
  *reviveMatching() {
    const ids: string[] = yield recoverMatchingServers(
      this.servers.filter((server) => server.id !== "local"),
      (id) => !!this.serviceInstances.get(id)?.api.isApiSchemaMismatch(),
      (server) => fetchServerSummary(server, AbortSignal.timeout(10000)),
    );
    if (ids.length)
      this.revived = {
        ...this.revived,
        ...Object.fromEntries(
          ids.map((id) => [id, (this.revived[id] || 0) + 1]),
        ),
      };
  }
  add(server: StudioServer, focus = true) {
    if (server.id === "local")
      throw new Error("The remote server returned an invalid identity.");
    const old = readServers();
    const same = old.find((row) => row.id === server.id);
    if (same && same.origin !== server.origin)
      throw new Error("This server identity belongs to another address.");
    writeServers([...old.filter((row) => row.id !== server.id), server]);
    if (focus) this.select(server.id);
  }
  remove(server: StudioServer) {
    writeServers(readServers().filter((row) => row.id !== server.id));
    this.pending.delete(server.id);
  }
  rememberAccountFocus(
    serverId: string,
    command: Extract<
      ServerCommand,
      { action: "account-sign-in" | "account-action" }
    >,
  ) {
    const exact = command.accountKey
      ? serverAccountRows(
          this.accountsServers,
          this.managementData.accountsByServer,
        ).find(
          (row) => row.byServer[serverId]?.accountKey === command.accountKey,
        )
      : undefined;
    this.restoreAccountActionFocus =
      exact?.key ||
      `${command.provider}:${(command.email || command.label).toLocaleLowerCase()}`;
  }
  get terminal() {
    const service = this.serviceInstances.get(this.current);
    if (!this.sidebar.value.showTerminals || !service) return;
    const { ui, navigation } = service.areas;
    const terminal = ui.terminal;
    if (!ui.terminalVisible || terminal.measuredHeight === null) return;
    return {
      opened: terminal.opened,
      height: terminal.measuredHeight,
      count: (resourceData(service.store.terminals().current) || []).filter(
        (item) => item.status !== TerminalState.closed,
      ).length,
      canCreate: !!(navigation.selectedAgent || navigation.selectedLead),
    };
  }
  get accountRows() {
    return accountRows(
      {
        servers: this.accountsServers,
        accountsByServer: this.managementData.accountsByServer,
        statuses: this.accountStatuses,
        servicesFor: (id) => this.serviceInstances.get(id),
        startAccount: this.startAccount,
        accountAction: this.accountAction,
      },
      this.localServices.store,
    );
  }
  get serverRows() {
    return serverRows(
      this.servers,
      this.discovery.snapshot?.servers || [],
      this.sidebar.status,
      {
        snapshot: this.discovery.snapshot,
        checkedAt: this.discovery.checkedAt,
        localEnabled: this.discovery.enabled,
        lastSeen: this.lastSeen,
        ...this.managementData,
      },
    );
  }
  get managementData() {
    const navigation = { ...this.sidebar.navigation };
    const accountsByServer: Record<string, ServerAccount[]> = {};
    for (const [id, service] of this.serviceInstances) {
      const state = resourceData(service.store.state().current);
      if (!state) continue;
      const agentsRunning = [...state.agents.values()].filter(
        (agent) => agent.inFlight,
      ).length;
      const accounts = state.accountsSummary.get("current");
      const previous = navigation[id];
      if (previous)
        navigation[id] = {
          ...previous,
          agentsRunning,
          busy: agentsRunning > 0,
        };
      if (accounts) accountsByServer[id] = frameAccounts(accounts);
    }

    return { navigation, accountsByServer };
  }
  accountAction(
    id: string,
    command: Extract<
      ServerCommand,
      { action: "account-sign-in" | "account-action" }
    >,
  ) {
    this.restoreManagerAfterAccountDialog = this.settingsOpen;
    this.rememberAccountFocus(id, command);
    this.set({ settingsOpen: false });
    this.send(id, command);
  }

  get projectServerChoices() {
    return projectServerChoices(
      this.servers,
      this.sidebar.status,
      Object.fromEntries(
        (this.discovery.snapshot?.servers || []).map((server) => [
          server.id,
          server.reachability,
        ]),
      ),
    ).map((row) => ({
      ...row,
      system: this.sidebar.navigation[row.id]?.system,
    }));
  }
  addProjectServer(server: string) {
    const dialog = this.newChatDialog;
    if (!dialog) return;
    this.send(dialog.server, {
      action: "project-folders",
      projectId: dialog.project.id,
      server,
    });
    this.set({ newChatDialog: null });
  }
  chooseProjectServer(location: ProjectLocation, settings: NewChatSettings) {
    const dialog = this.newChatDialog;
    if (!dialog) return;
    const origin = location.serverId === dialog.server;
    this.send(location.serverId, {
      action: "new-chat",
      path: location.path,
      settings,
      ...(origin && dialog.folder ? { folder: dialog.folder } : {}),
      ...(origin && dialog.binding
        ? dialog.binding
        : dialog.project.homeServerId
          ? {
              projectId: dialog.project.id,
              projectServerId: dialog.project.homeServerId,
            }
          : {}),
    });
    this.set({ newChatDialog: null });
  }
  submitSearch() {
    const query = this.searchQuery.trim();
    if (!query) return;
    this.searchRequest = randomUUID();
    this.searchResults = {};
    for (const [id, service] of this.serviceInstances)
      void flowResult(
        this.searchServer(id, service, query, this.searchRequest),
      );
  }
  *searchServer(
    id: string,
    service: ServerServices,
    query: string,
    requestId: string,
  ) {
    let results: ScreenSearchResults;
    try {
      const value: GetResult<"/api/search"> = yield service.api.get(
        "/api/search",
        { query: { q: query } },
      );
      results = { results: value.results };
    } catch (error) {
      results = { results: [], error: String(error) };
    }
    if (
      this.searchRequest === requestId &&
      this.serviceInstances.get(id) === service
    )
      this.searchResults = { ...this.searchResults, [id]: results };
  }
  selectSettingsTab(tab: string | null) {
    if (!isStudioSettingsTab(tab)) return;
    if (tab !== "accounts") this.restoreAccountActionFocus = null;
    this.settingsTab = tab;
    if (tab === "accounts" || tab === "servers") return;
    this.settingsOpen = false;
    this.send(this.current, { action: "settings", tab });
  }
  get accountStatuses() {
    return {
      ...Object.fromEntries(
        (this.discovery.snapshot?.servers || [])
          .filter(
            (peer) =>
              peer.status === AccessClientStatus.unreachable ||
              peer.reachability === Reachability.unreachable,
          )
          .map((peer) => [peer.id, "offline"]),
      ),
      ...this.sidebar.status,
    };
  }
  startAccount(
    id: string,
    command: Extract<ServerCommand, { action: "account-sign-in" }>,
  ) {
    if (this.servers.some((server) => server.id === id))
      this.accountAction(id, command);
  }
  get backends() {
    return this.sidebar.backends;
  }
  online = this.sidebar.online;
  settings = () => this.openManager();
  search = () => this.set({ searchOpen: true });
  notify = (failure: string) => this.set({ failure });
  get creation(): ServerServices["areas"]["creation"] {
    return (
      (this.sidebar.navigation[this.current] &&
        this.serviceInstances.get(this.current)?.areas.creation) ||
      this.localServices.areas.creation
    );
  }
  toggleSidebar() {
    restoreSidebarFocus();
    if (this.sidebar.compact) this.sidebar.setMobileOpen((old) => !old);
    else this.sidebar.setHidden((old) => !old);
  }
}

export function createShellState() {
  const shell = new ShellState();
  const stop = shell.start();
  window.addEventListener("pagehide", (event) => !event.persisted && stop());
  return shell;
}
