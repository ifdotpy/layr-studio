import {
  makeAutoObservable,
  computed,
  observableStruct,
  compareStructural,
  flowResult,
  reaction,
} from "mobx";
import type { DiscoveredServer } from "../../../servers/discoveryModel.ts";
import { ShellSidebarSource } from "../../../app/startup/ShellSidebarSource.ts";
import { ServerAreasProvider } from "../../shell/ServerAreasProvider/ServerAreasProvider.tsx";
import { observer } from "mobx-react-lite";
import { ScreenSelection } from "../../../app/startup/ScreenOwner.ts";
import { AccessClientStatus, Reachability } from "../../../generated/api.ts";
import type { ConnectionHealth } from "../../../sync/connectionState.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { restoreSidebarFocus } from "../../sidebar/SidebarFrame/SidebarFrame.tsx";
import { readTerminalDockHeight } from "../../../layout/terminalGeometry.ts";
import { layoutQueries } from "../../../layout/layout-contract.generated.ts";
import { ScreenContext } from "../../../servers/ScreenContext.tsx";
import {
  createScreenChannel,
  screenSearchResults,
  type ScreenChannel,
  type ScreenMessage,
  type ScreenSearchResults,
} from "../../../servers/screenChannel.ts";
import { ServerServicesProvider } from "../ServerServicesProvider/ServerServicesProvider.tsx";
import {
  createServerServices,
  type ServerServices,
} from "../../../servers/serverServices.ts";
import {
  hasDesktopNotifications,
  notifyDesktop,
  onDesktopNavigate,
} from "../../../native/notifications.ts";
import { orderPreferenceServers } from "../../../servers/preferenceServerOrder.ts";
import { preferenceMemory } from "../../../servers/storage.ts";
import { writePreferenceEdit } from "../../../sync/uiPreferenceStore.ts";
import { useUiPreferenceSync } from "../../../servers/useUiPreferenceSync.ts";
import {
  StudioSettingsLayout,
  type StudioSettingsTab,
} from "../../settings/StudioSettingsLayout/StudioSettingsLayout.tsx";
import { isStudioSettingsTab } from "../../settings/StudioSettingsTabs/StudioSettingsTabs.tsx";
import {
  ServerSettingsContext,
  ServerManagementPanelContext,
} from "../../../servers/ServerSettingsContext.tsx";
import { useServerDiscovery } from "../../../servers/useServerDiscovery.ts";
import { modalSizes } from "../../../theme.ts";
import {
  Button,
  TextInput,
  Modal,
  useMantineColorScheme,
  Tabs,
  ActionIcon,
} from "@mantine/core";
import {
  ChevronUp,
  Plus,
  Settings,
  Terminal as TerminalIcon,
} from "lucide-react";
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import App from "../../shell/App/App.tsx";
import { DEFAULT_LOCAL_SERVER_ALIAS } from "../../../servers/serverAliases.ts";
import { CombinedSidebarOwner } from "../../../servers/CombinedServerSidebar.tsx";
import Sidebar from "../../sidebar/Sidebar/Sidebar.tsx";
import ProjectServerCards from "../../projects/ProjectServerCards/ProjectServerCards.tsx";
import {
  projectServerChoices,
  type LocationProject,
} from "../../../servers/projectLocations.ts";
import { projectChatCommand } from "../../../servers/projectChatCommand.ts";
import { fetchServerSummary } from "../../../servers/net/summary.ts";
import { frameAccounts } from "../../accounts/Accounts/Accounts.tsx";
import { resourceData } from "../../../store/runtime.ts";
import { recoverMatchingServers } from "../../../servers/schemaRecovery.ts";
import {
  localServer,
  readServers,
  shareServers,
  writeServers,
  SERVER_REGISTRY_EVENT,
  SERVER_REGISTRY_KEY,
  type StudioServer,
} from "../../../servers/registry.ts";
import type {
  ServerCommand,
  ServerNavigation,
} from "../../../servers/navigation.ts";

import ServerManager from "../ServerManager/ServerManager.tsx";
import ServerAccountsPanel from "../ServerAccountsPanel/ServerAccountsPanel.tsx";
import { accountServers } from "../../../servers/accountServers.ts";
import {
  readServerAccounts,
  type ServerAccount,
} from "../../../servers/navigation.ts";
import { serverAccountRows } from "../../../servers/serverAccountRows.ts";
import {
  studioPreferencesStorageKey,
  parseStudioPreferences,
  parseSidebarShortcut,
  fontFamilies,
} from "../../../studioPreferences.ts";
import { alertsSince } from "../../../desktop/desktopAlerts.ts";

const uiOnly =
  new URLSearchParams(location.search).get("studio-ui-only") === "1";
export class ShellState {
  failure = "";
  newChatDialog: {
    project: LocationProject;
    server: string;
    folder?: string | undefined;
    binding?: { projectId: string; projectServerId: string } | undefined;
  } | null = null;
  paired = this.load();
  registryRevision = 0;
  accountsByServer: Record<string, ServerAccount[]> = {};
  lastSeen: Record<string, number> = {};
  searchOpen = false;
  searchQuery = "";
  searchResults: Record<string, ScreenSearchResults> = {};
  terminals: Record<
    string,
    {
      opened: boolean;
      height: number;
      count: number;
      active: number;
      canCreate: boolean;
    }
  > = {};
  shellSidebar = false;
  settingsOpen = uiOnly && !this.paired.length;
  settingsTab: StudioSettingsTab = uiOnly ? "servers" : "accounts";
  restoreManagerAfterAccountDialog = false;
  restoreAccountActionFocus: string | null = null;
  searchRequest = "";
  notificationState = new Map<string, { seen: Set<string>; started: number }>();
  screens = new Map<string, ScreenChannel>();
  pending = new Map<string, ServerCommand>();
  readyScreens = new Set<string>();
  readyWaiters = new Map<string, Set<() => void>>();
  serviceInstances = new Map<string, ServerServices>();
  revived: Record<string, number> = {};
  revivedSeen: Record<string, number> = {};
  peers: DiscoveredServer[] = [];
  discoveredAliases: Record<string, string> = {};
  readonly server = localServer();
  readonly selection = new ScreenSelection(
    this.servers.map((server) => server.id),
  );
  readonly sidebar = new ShellSidebarSource(this.selection, (id) =>
    this.serviceInstances.get(id),
  );
  constructor(localServices?: ServerServices) {
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
        server: false,
        selection: false,
        sidebar: false,
        screens: false,
        // Discovery derives fresh objects per render; equal content must not re-render the shell.
        peers: observableStruct,
        discoveredAliases: observableStruct,
        servers: computed({ equals: compareStructural }),
        aliases: computed({ equals: compareStructural }),
      },
      { autoBind: true, deep: false },
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
  setDiscovery(peers: DiscoveredServer[], aliases: Record<string, string>) {
    this.peers = peers;
    this.discoveredAliases = aliases;
  }
  get aliases() {
    return Object.fromEntries(
      this.servers.map((server) => [
        server.id,
        this.discoveredAliases[server.id] ||
          server.alias ||
          (server.id === "local"
            ? DEFAULT_LOCAL_SERVER_ALIAS
            : server.label
                .replace(/[^a-z]/gi, "")
                .slice(0, 3)
                .toUpperCase()),
      ]),
    );
  }
  get current() {
    return this.selection.current;
  }
  get accountsServers() {
    return accountServers(this.servers, this.peers);
  }
  get sidebarVisible() {
    return !this.sidebar.hidden;
  }

  set(fields: Partial<ShellState>) {
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
    if (!screen?.ready || !this.readyScreens.has(id))
      this.pending.set(id, command);
    else
      void screen
        .execute(command)
        .catch((error) => this.set({ failure: String(error) }));
  }
  applyNavigation(id: string, value: ServerNavigation) {
    if (value.ready) this.readyWaiters.get(id)?.forEach((resolve) => resolve());
    this.sidebar.setNavigation((old) => ({ ...old, [id]: value }));
    if (value.ready && value.alertsReady !== false) {
      const previous = this.notificationState.get(id);
      const alerts = value.alerts || [];
      const known = previous || {
        seen: new Set<string>(),
        started: Date.now(),
      };
      const fresh = previous
        ? alerts
            .filter((alert) => !known.seen.has(alert.id))
            .filter(alertsSince(known.started))
        : [];
      for (const alert of alerts) known.seen.add(alert.id);
      this.notificationState.set(id, known);
      for (const alert of fresh) {
        const server =
          readServers().find((row) => row.id === id) || localServer();
        const title = `${server.label}: ${alert.title}`.slice(0, 160);
        if (hasDesktopNotifications())
          void notifyDesktop({
            title,
            body: alert.body,
            ...(alert.sound ? { sound: true } : {}),
            target: { ...alert.target, serverId: id },
          }).catch(() => {});
        else if (
          "Notification" in window &&
          Notification.permission === "granted"
        ) {
          const notification = new Notification(title, {
            body: alert.body,
            tag: `${id}:${alert.id}`,
            silent: !alert.sound,
          });
          notification.onclick = () => {
            notification.close();
            this.send(id, { action: "notifications", ...alert.target });
          };
        }
      }
    }
    const screen = this.screens.get(id);
    if (this.pending.has(id) && value.ready && screen?.ready) {
      const command = this.pending.get(id)!;
      this.pending.delete(id);
      void screen
        .execute(command)
        .catch((error) => this.set({ failure: String(error) }));
    }
  }
  *executeCommand(owner: string, command: ServerCommand) {
    const screen = this.screens.get(owner);
    if (!screen) throw new Error("The server is no longer paired.");
    if (!this.readyScreens.has(owner) || !screen.ready) {
      yield new Promise<void>((resolve, reject) => {
        const waiters = this.readyWaiters.get(owner) || new Set<() => void>();
        const done = () => {
          clearTimeout(timer);
          waiters.delete(done);
          resolve();
        };
        const timer = setTimeout(() => {
          waiters.delete(done);
          reject(new Error("The server screen is unavailable."));
        }, 15000);
        waiters.add(done);
        this.readyWaiters.set(owner, waiters);
      });
    }
    if (this.screens.get(owner) !== screen)
      throw new Error(
        "The server connection changed. The action was not sent.",
      );
    yield screen.execute(command);
  }
  screenEvents(id: string, data: ScreenMessage, channel: ScreenChannel) {
    if (this.screens.get(id) !== channel) return;
    if (
      data.kind === "studio-server-activity" &&
      typeof data.busy === "boolean"
    ) {
      // Activity stays with the service; screen controllers remain mounted.
    } else if (data.kind === "studio-open-new-chat") {
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
    } else if (data.kind === "studio-server-terminal") {
      const value = data;
      const height = readTerminalDockHeight(value.height);
      if (
        typeof value.opened !== "boolean" ||
        height === undefined ||
        !Number.isInteger(value.count) ||
        !Number.isInteger(value.active) ||
        typeof value.canCreate !== "boolean"
      )
        return;
      this.set({
        terminals: {
          ...this.terminals,
          [id]: {
            opened: value.opened as boolean,
            height: height,
            count: value.count as number,
            active: value.active as number,
            canCreate: value.canCreate as boolean,
          },
        },
      });
    } else if (data.kind === "studio-server-toggle-sidebar") {
      if (typeof data.opened === "boolean")
        this.sidebar.setMobileOpen(data.opened);
      else this.toggleSidebar();
    } else if (data.kind === "studio-server-search")
      this.set({ searchOpen: true });
    else if (
      data.kind === "studio-server-search-results" &&
      data.requestId === this.searchRequest
    ) {
      const results = screenSearchResults(data);
      this.set({ searchResults: { ...this.searchResults, [id]: results } });
    } else if (data.kind === "studio-server-navigation") {
      const value = data.navigation as ServerNavigation;
      this.publishPreferences(this.sidebar.value);
      if (
        !value ||
        !Array.isArray(value.projects) ||
        !Array.isArray(value.chats)
      )
        return;
      if (value.ready) this.readyScreens.add(id);
      this.applyNavigation(id, value);
    } else if (data.kind === "studio-server-accounts") {
      const accounts = readServerAccounts(data.accounts);
      if (!accounts) return;
      this.set({
        accountsByServer: { ...this.accountsByServer, [id]: accounts },
      });
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
        this.readyScreens.delete(server.id);
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
    }
    for (const [id, previous] of this.serviceInstances) {
      const next = services.get(id);
      if (next !== previous)
        previous.close(
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
  close() {
    for (const channel of this.screens.values()) channel.close();
    for (const service of this.serviceInstances.values()) service.close();
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
  *refreshSidebarOwner(owner: string) {
    const service = this.serviceInstances.get(owner);
    if (!service) throw new Error("The sidebar server is no longer paired.");
    const value: Awaited<ReturnType<typeof fetchServerSummary>> =
      yield fetchServerSummary(service.owner, new AbortController().signal);
    if (this.serviceInstances.get(owner) !== service)
      throw new Error("The sidebar server connection changed.");
    return {
      ...value,
      opened: this.sidebar.navigation[owner]?.opened || null,
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
    const next = { ...this.accountsByServer };
    delete next[server.id];

    this.set({ accountsByServer: next });
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
  get managementData() {
    const navigation = { ...this.sidebar.navigation };
    const accountsByServer = { ...this.accountsByServer };
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

  toggleSidebar() {
    restoreSidebarFocus();
    if (this.sidebar.compact) this.sidebar.setMobileOpen((old) => !old);
    else this.sidebar.setHidden((old) => !old);
  }
}

export default function MultiServerApp() {
  // Pairing changes the shell, but must keep the local database and transport owner.
  const [localServices] = useState(() => createServerServices(localServer()));
  const shell = <MultiServerShell localServices={localServices} />;
  return localServices ? (
    <ServerServicesProvider services={localServices}>
      {shell}
    </ServerServicesProvider>
  ) : (
    shell
  );
}

const MultiServerShell = observer(function MultiServerShell({
  localServices,
}: {
  localServices?: ServerServices | undefined;
}) {
  const [shell] = useState(() => new ShellState(localServices));
  const newChatDialog = shell.newChatDialog;
  if (localServices) localServices.shellSettings = shell;
  const shellRef = useRef<HTMLDivElement>(null);
  const { setColorScheme } = useMantineColorScheme();
  const discovery = useServerDiscovery(
    !uiOnly && ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname),
    !uiOnly,
    Object.fromEntries(
      shell.servers.map((server) => [server.id, server.workspaceId]),
    ),
  );
  useLayoutEffect(() => {
    shell.setDiscovery(
      discovery.snapshot?.servers || [],
      discovery.snapshot?.aliases || {},
    );
  }, [discovery.snapshot]);
  useUiPreferenceSync(shell.servers);
  const services = shell.serviceInstances;
  const directScreen =
    !uiOnly &&
    !shell.paired.length &&
    shell.accountsServers.length === shell.servers.length;
  useLayoutEffect(
    () => shell.connectServices(localServices, directScreen),
    [shell.servers, shell.revived, localServices, directScreen],
  );
  useEffect(() => shell.start(), [shell]);
  const mergedSidebar = shell.sidebar.model;
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        shell.set({ searchOpen: true });
      }
      const shortcut = parseSidebarShortcut(
        shell.sidebar.value.sidebarShortcut,
      );
      if (
        shortcut &&
        event.key.toLowerCase() === shortcut.key &&
        event.metaKey === shortcut.meta &&
        event.ctrlKey === shortcut.ctrl &&
        event.altKey === shortcut.alt &&
        event.shiftKey === shortcut.shift &&
        !(event.target as HTMLElement)?.closest?.(
          "input,textarea,select,[contenteditable]",
        )
      ) {
        event.preventDefault();
        shell.toggleSidebar();
      }
    };
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("keydown", key);
    };
  }, [shell.sidebar.value.sidebarShortcut, shell.toggleSidebar]);
  useEffect(() => {
    setColorScheme(shell.sidebar.value.theme);
    document.documentElement.style.setProperty(
      "--studio-font-family",
      fontFamilies[shell.sidebar.value.fontFamily].css,
    );
    document.documentElement.style.setProperty(
      "--studio-sidebar-font-size",
      shell.sidebar.value.sidebarFontSize + "px",
    );
  }, [shell.sidebar.value, setColorScheme]);
  const { navigation, accountsByServer } = shell.managementData;
  const management = (
    <ServerManager
      servers={shell.servers}
      add={shell.add}
      remove={shell.remove}
      discovery={discovery}
      localEnabled={
        !uiOnly &&
        ["localhost", "127.0.0.1", "[::1]"].includes(location.hostname)
      }
      statuses={shell.sidebar.status}
      lastSeen={shell.lastSeen}
      navigation={navigation}
      accountsByServer={accountsByServer}
    />
  );
  const settings = (
    <Modal
      opened={shell.settingsOpen}
      transitionProps={{ duration: 0 }}
      onClose={() => {
        shell.restoreAccountActionFocus = null;
        shell.set({ settingsOpen: false });
      }}
      size={modalSizes.settings}
      title="Studio settings"
      classNames={{ body: "studio-settings-body" }}
    >
      <div className="studio-settings-panel" data-testid="studio-settings">
        <StudioSettingsLayout
          value={shell.settingsTab}
          initialDetail={!!shell.restoreAccountActionFocus}
          onChange={(tab) => {
            if (!isStudioSettingsTab(tab)) return;
            if (tab !== "accounts") shell.restoreAccountActionFocus = null;
            shell.set({ settingsTab: tab });
            if (tab === "accounts" || tab === "servers") return;
            shell.set({ settingsOpen: false });
            shell.send(shell.current, { action: "settings", tab });
          }}
        >
          <Tabs.Panel value="accounts" pt="md">
            {discovery.error && <p role="alert">{discovery.error}</p>}
            <ServerAccountsPanel
              initialFocusIdentity={
                shell.restoreAccountActionFocus || undefined
              }
              servicesFor={(id) => services.get(id)}
              servers={shell.accountsServers}
              accountsByServer={accountsByServer}
              statuses={{
                ...Object.fromEntries(
                  (discovery.snapshot?.servers || [])
                    .filter(
                      (peer) =>
                        peer.status === AccessClientStatus.unreachable ||
                        peer.reachability === Reachability.unreachable,
                    )
                    .map((peer) => [peer.id, "offline"]),
                ),
                ...shell.sidebar.status,
              }}
              startAccount={(id, command) => {
                if (shell.servers.some((server) => server.id === id))
                  shell.accountAction(id, command);
              }}
              accountAction={shell.accountAction}
            />
          </Tabs.Panel>
          <Tabs.Panel value="servers" pt="md">
            {management}
          </Tabs.Panel>
        </StudioSettingsLayout>
      </div>
    </Modal>
  );
  const [combinedSidebar] = useState(
    () =>
      new CombinedSidebarOwner(
        shell.sidebar,
        {
          get backends() {
            return shell.sidebar.backends;
          },
          get aliases() {
            return shell.aliases;
          },
          online: shell.sidebar.online,
          refreshOwner: (owner) => flowResult(shell.refreshSidebarOwner(owner)),
          executeCommand: (owner, command) =>
            flowResult(shell.executeCommand(owner, command)),
          send: shell.send,
          applyNavigation: shell.applyNavigation,
          settings: shell.openManager,
          search: () => shell.set({ searchOpen: true }),
          notify: (failure: ShellState["failure"]) => shell.set({ failure }),
        },
        {
          areas: localServices!.areas,
          get creation() {
            const selected = shell.sidebar.navigation[shell.sidebar.current];
            return (
              (selected &&
                shell.serviceInstances.get(shell.sidebar.current)?.areas
                  .creation) ||
              localServices!.areas.creation
            );
          },
        },
      ),
  );
  useEffect(() => {
    const narrow = window.matchMedia(layoutQueries.navigationCompact);
    let observed: Element | null = null;
    // A resize reports the new width; reading it again would force layout.
    const measure = (reported?: number) => {
      const sidebar = document.querySelector(".multi-server-shell > #sidebar");
      if (sidebar !== observed) {
        if (observed) observer.unobserve(observed);
        if (sidebar) observer.observe(sidebar);
        observed = sidebar;
      }
      const width =
        !shell.sidebarVisible || narrow.matches || !sidebar
          ? 0
          : Math.round(reported ?? sidebar.getBoundingClientRect().width);
      shellRef.current?.style.setProperty(
        "--shell-sidebar-width",
        `${width}px`,
      );
      shell.set({ shellSidebar: width > 0 });
      const root = document.documentElement;
      if (width > 0 && root.dataset.shellSidebar !== "true")
        root.dataset.shellSidebar = "true";
      else if (width === 0 && "shellSidebar" in root.dataset)
        delete root.dataset.shellSidebar;
    };
    const observer = new ResizeObserver((entries) =>
      measure(entries.at(-1)?.borderBoxSize[0]?.inlineSize),
    );
    const remeasure = () => measure();
    measure();
    narrow.addEventListener("change", remeasure);
    return () => {
      observer.disconnect();
      narrow.removeEventListener("change", remeasure);
    };
  }, [
    !shell.sidebarVisible,
    shell.sidebar.compact,
    mergedSidebar?.sources.length,
    shell.servers.length,
    combinedSidebar.list.frame.aside,
  ]);
  useEffect(
    () => (directScreen ? undefined : combinedSidebar.start()),
    [directScreen, combinedSidebar],
  );
  if (directScreen)
    return (
      <ServerSettingsContext.Provider value={shell}>
        <ServerManagementPanelContext.Provider value={management}>
          <ServerAreasProvider areas={localServices!.areas}>
            <App />
          </ServerAreasProvider>
        </ServerManagementPanelContext.Provider>
      </ServerSettingsContext.Provider>
    );
  const terminal = shell.sidebar.value.showTerminals
    ? shell.terminals[shell.current]
    : undefined;
  const terminalLayout =
    terminal && shell.shellSidebar
      ? terminal.opened
        ? "open"
        : "closed"
      : undefined;
  return (
    <div
      ref={shellRef}
      className={`multi-server-shell ${!shell.sidebarVisible ? "server-sidebar-hidden" : ""}`}
      data-navigation="combined"
      data-navigation-compact={shell.sidebar.compact}
      data-shell-terminal={terminalLayout}
      style={
        {
          "--shell-terminal-height": `${terminal?.height || 0}px`,
        } as CSSProperties
      }
    >
      <>
        <Button
          className="server-mobile-toggle"
          onClick={() => shell.sidebar.setMobileOpen(!shell.sidebar.mobileOpen)}
          aria-expanded={shell.sidebar.mobileOpen}
        >
          Servers and chats
        </Button>
        {mergedSidebar?.sources.length ? (
          <Sidebar owner={combinedSidebar.list} notice={shell.failure} />
        ) : (
          <aside className="server-sidebar" aria-label="Servers and projects">
            <header>
              <strong>LAYR Studio</strong>
              <ActionIcon
                aria-label="Studio settings"
                title="Studio settings"
                onClick={shell.openManager}
              >
                <Settings size={18} />
              </ActionIcon>
            </header>
            {shell.servers.length ? (
              <>
                <p role="status" className="sr-only">
                  Loading chats...
                </p>
                {/* Placeholder rows hold the sidebar's shape while chats load. */}
                <div className="server-sidebar-skeleton" aria-hidden="true">
                  {[62, 48, 74, 40, 56].map((width) => (
                    <span key={width} style={{ width: `${width}%` }} />
                  ))}
                </div>
              </>
            ) : (
              <p role="status">Pair a server to open its projects and chats.</p>
            )}
            {shell.failure && <p role="alert">{shell.failure}</p>}
          </aside>
        )}
      </>
      {terminalLayout === "closed" && terminal && (
        <div className="shell-terminal-bar">
          <button
            type="button"
            className="shell-terminal-toggle"
            aria-expanded={false}
            aria-label="Show terminals"
            onClick={() =>
              shell.send(shell.current, {
                action: "terminal",
                operation: "toggle",
              })
            }
          >
            <TerminalIcon size={15} />
            <strong>Terminals</strong>
            {terminal.count > 0 && (
              <span className="terminal-dock-count">{terminal.count}</span>
            )}
            <ChevronUp size={14} />
          </button>
          <ActionIcon
            variant="filled"
            size={28}
            aria-label="New terminal"
            title="New terminal"
            disabled={!terminal.canCreate}
            onClick={() =>
              shell.send(shell.current, {
                action: "terminal",
                operation: "new",
              })
            }
          >
            <Plus size={13} />
          </ActionIcon>
        </div>
      )}
      <main className="server-views">
        {!shell.servers.length && (
          <div className="startup">
            <p>No server is paired.</p>
            <Button onClick={shell.openManager}>Open Settings</Button>
          </div>
        )}
        {shell.servers.map((server) => {
          const service = services.get(server.id)!;
          const channel = shell.screens.get(server.id);
          if (!service || !channel) return null;
          return (
            <ServerServicesProvider
              key={JSON.stringify([
                server.id,
                service.owner.origin,
                service.owner.credentialId,
                shell.revivedSeen[server.id] || 0,
              ])}
              services={service}
            >
              <ScreenContext.Provider value={channel}>
                <ServerAreasProvider areas={service.areas}>
                  <App />
                </ServerAreasProvider>
              </ScreenContext.Provider>
            </ServerServicesProvider>
          );
        })}
      </main>
      {settings}
      <Modal
        opened={!!newChatDialog}
        closeOnEscape={false}
        onClose={() => shell.set({ newChatDialog: null })}
        title="New chat"
        aria-label="New chat"
        size="lg"
      >
        {newChatDialog && (
          <ProjectServerCards
            servicesFor={(id) => services.get(id)}
            project={newChatDialog.project}
            servers={projectServerChoices(
              shell.servers,
              shell.sidebar.status,
              Object.fromEntries(
                (discovery.snapshot?.servers || []).map((server) => [
                  server.id,
                  server.reachability,
                ]),
              ),
            ).map((row) => ({
              ...row,
              system: shell.sidebar.navigation[row.id]?.system,
            }))}
            lastServer={newChatDialog.server}
            onCancel={() => shell.set({ newChatDialog: null })}
            onAdd={(server) => {
              shell.send(newChatDialog.server, {
                action: "project-folders",
                projectId: newChatDialog.project.id,
                server,
              });
              shell.set({ newChatDialog: null });
            }}
            onChoose={(location, settings) => {
              const origin = location.serverId === newChatDialog.server;
              shell.send(location.serverId, {
                action: "new-chat",
                path: location.path,
                settings,
                ...(origin && newChatDialog.folder
                  ? { folder: newChatDialog.folder }
                  : {}),
                ...(origin && newChatDialog.binding
                  ? newChatDialog.binding
                  : newChatDialog.project.homeServerId
                    ? {
                        projectId: newChatDialog.project.id,
                        projectServerId: newChatDialog.project.homeServerId,
                      }
                    : {}),
              });
              shell.set({ newChatDialog: null });
            }}
          />
        )}
      </Modal>
      <Modal
        opened={shell.searchOpen}
        onClose={() => shell.set({ searchOpen: false })}
        title="Search all servers"
        aria-label="Search all servers"
        size="lg"
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (!shell.searchQuery.trim()) return;
            const requestId = randomUUID();
            shell.searchRequest = requestId;
            shell.set({ searchResults: {} });
            for (const [id, service] of services) {
              void service.api
                .get("/api/search", { query: { q: shell.searchQuery.trim() } })
                .then(
                  (value) => {
                    if (
                      shell.searchRequest === requestId &&
                      shell.serviceInstances.get(id) === service
                    )
                      shell.set({
                        searchResults: {
                          ...shell.searchResults,
                          [id]: { results: value.results },
                        },
                      });
                  },
                  (error) => {
                    if (
                      shell.searchRequest === requestId &&
                      shell.serviceInstances.get(id) === service
                    )
                      shell.set({
                        searchResults: {
                          ...shell.searchResults,
                          [id]: { results: [], error: String(error) },
                        },
                      });
                  },
                );
            }
          }}
        >
          <TextInput
            autoFocus
            label="Search all messages"
            value={shell.searchQuery}
            onChange={(event) =>
              shell.set({ searchQuery: event.currentTarget.value })
            }
          />
          <Button type="submit" disabled={!shell.searchQuery.trim()}>
            Search
          </Button>
        </form>
        {shell.searchRequest &&
          shell.servers.map((server) => (
            <section key={server.id}>
              <h3>{server.label}</h3>
              {!shell.searchResults[server.id] && (
                <p role="status">Searching this server...</p>
              )}
              {shell.searchResults[server.id]?.error && (
                <p role="alert">{shell.searchResults[server.id]?.error}</p>
              )}
              {shell.searchResults[server.id]?.results.map((result, index) => (
                <button
                  className="server-search-result"
                  key={`${result.id}:${index}`}
                  onClick={() => {
                    shell.send(server.id, {
                      action: "open",
                      id: result.room || result.agent,
                      messageId: result.id,
                    });
                    shell.set({ searchOpen: false });
                  }}
                >
                  <small>{result.kind}</small>
                  <p>{result.text}</p>
                </button>
              ))}
              {shell.searchResults[server.id]?.results.length === 0 &&
                !shell.searchResults[server.id]?.error && <p>No results.</p>}
            </section>
          ))}
      </Modal>
    </div>
  );
});
