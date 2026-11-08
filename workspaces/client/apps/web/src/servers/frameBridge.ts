import type { ServerAreas } from "../app/contracts.ts";
import { frameAccounts } from "../components/accounts/Accounts/Accounts.tsx";
import { reaction } from "mobx";
import {
  formatAgentStatus,
  type ReadState,
} from "../status/operationalStatus.ts";
import {
  normalizeStudioPreferences,
  studioPreferencesStorageKey,
} from "../studioPreferences.ts";
import { errorText } from "../api.ts";
import { executeSidebarRequest, serializeSidebarError } from "./sidebarRpc.ts";
import { desktopAlerts, nextAlertChange } from "../desktop/desktopAlerts.ts";
import { serverViewId, serverParentOrigin } from "./environment.ts";
import { publishServerAliases } from "./serverAliases.ts";
import {
  isServerCommand,
  navigationSnapshot,
  readServerAccounts,
  type ServerAccount,
  type ServerCommand,
} from "./navigation.ts";
import { resourceData } from "../store/runtime.ts";
import { sidebarEntities } from "./sidebarSnapshot.ts";
import { createSidebarViewModel } from "../components/sidebar/support/viewModel.ts";
import { preferenceEvent } from "../sync/uiPreferenceMerge.ts";
import { preferenceMemory, serverStorageEventKey } from "./storage.ts";
import type { AppScope, Disposer } from "../app/contracts.ts";
import type {
  ScreenMessage,
  ScreenSearchResultsMessage,
} from "./screenChannel.ts";

export interface FrameScope {
  readonly store: Pick<AppScope["store"], "state" | "requests" | "agentDetail">;
  readonly screen: AppScope["screen"];
  readonly services: {
    readonly api: Pick<
      AppScope["services"]["api"],
      "get" | "saved" | "getToken"
    >;
    readonly sync: Pick<AppScope["services"]["sync"], "projectionMetadata">;
    readonly resources: Pick<
      AppScope["services"]["resources"],
      "watchResourceConnection"
    >;
  };
}
export interface FrameInputs {
  readonly ready: boolean;
  readonly opened: string | null;
  readonly error: string;
  readonly pendingReads: ReadonlyMap<string, ReadState>;
  readonly accounts: ServerAccount[];
  readonly creating: boolean;
  run(command: ServerCommand): void | Promise<void>;
}

/** Protocol serialization is an external boundary, independent of React renders. */
export function startServerFrame(
  scope: FrameScope,
  source: FrameInputs,
): Disposer {
  const { store, services, screen: activation } = scope;
  const screen = activation.channel;
  const legacy = !screen && !!serverViewId && window.parent !== window;
  if (!screen && !legacy) return () => {};
  let disposed = false;
  const publish = (message: ScreenMessage) => {
    if (disposed) return;
    if (screen) screen.publish(message);
    else
      window.parent.postMessage(
        { ...message, serverId: serverViewId },
        serverParentOrigin,
      );
  };
  const view = createSidebarViewModel(
    () => {
      const current = store.state().current;
      if (current.status !== "live" && current.status !== "stale")
        throw new Error("The server state is unavailable.");
      return current.data;
    },
    () => ({ query: "", archived: false, opened: null, rank: () => 0 }),
  );
  const navigation = () => {
    const current = store.state().current;
    const available = current.status === "live" || current.status === "stale";
    return {
      ...navigationSnapshot(
        available && source.ready ? current.data : undefined,
        source.opened,
        source.error,
        available ? view.rows : [],
        source.ready ? desktopAlerts(store) : [],
        services.api.saved,
        new Set(
          available
            ? [...current.data.agents.values()]
                .filter(
                  (agent) =>
                    formatAgentStatus(agent, source.pendingReads.get(agent.id))
                      .kind === "unread",
                )
                .map((agent) => agent.id)
            : [],
        ),
        available && legacy ? sidebarEntities(current.data) : [],
        services.sync.projectionMetadata,
      ),
      creating: source.creating,
    };
  };
  const execute = async (command: ServerCommand) => {
    if (command.action === "search") {
      try {
        const value = await services.api.get("/api/search", {
          query: { q: command.query },
        });
        publish({
          kind: "studio-server-search-results",
          requestId: command.requestId,
          results: value.results,
        } satisfies ScreenSearchResultsMessage);
      } catch (failure) {
        publish({
          kind: "studio-server-search-results",
          requestId: command.requestId,
          results: [],
          error: errorText(failure),
        } satisfies ScreenSearchResultsMessage);
      }
    } else if (legacy && command.action === "terminal") {
      window.dispatchEvent(
        new CustomEvent("studio-terminal-command", {
          detail: command.operation,
        }),
      );
    } else await source.run(command);
  };
  const receive = (event: MessageEvent) => {
    if (
      !legacy ||
      !event.data ||
      typeof event.data !== "object" ||
      event.source !== window.parent ||
      event.origin !== serverParentOrigin ||
      event.data.serverId !== serverViewId
    )
      return;
    if (event.data.kind === "studio-sidebar-request") {
      if (typeof event.data.correlation !== "string") return;
      const correlation = event.data.correlation;
      void executeSidebarRequest(
        event.data.value,
        async () => {
          await source.run({ action: "refresh" });
          return navigation();
        },
        execute,
        services.api.getToken(),
      ).then(
        (result) =>
          publish({ kind: "studio-sidebar-result", correlation, result }),
        (failure) =>
          publish({
            kind: "studio-sidebar-result",
            correlation,
            error: serializeSidebarError(failure),
          }),
      );
    } else if (event.data.kind === "studio-server-shell-layout") {
      const width = event.data.sidebarWidth;
      if (typeof width !== "number" || !(width >= 0) || width > 2000) return;
      const root = document.documentElement;
      root.style.setProperty("--shell-sidebar-width", `${Math.round(width)}px`);
      if (width > 0) root.dataset.shellSidebar = "true";
      else delete root.dataset.shellSidebar;
    } else if (event.data.kind === "studio-server-preferences") {
      const aliases = event.data.aliases;
      if (
        aliases &&
        typeof aliases === "object" &&
        Object.values(aliases).every(
          (value) => typeof value === "string" && /^[A-Z]{1,3}$/.test(value),
        )
      )
        publishServerAliases(aliases);
      try {
        const value = JSON.stringify(
          normalizeStudioPreferences(event.data.preferences),
        );
        if (preferenceMemory.getItem(studioPreferencesStorageKey) !== value) {
          preferenceMemory.setItem(studioPreferencesStorageKey, value);
          window.dispatchEvent(
            new StorageEvent("storage", {
              key: studioPreferencesStorageKey,
              newValue: value,
            }),
          );
        }
      } catch {}
    } else if (
      event.data.kind === "studio-server-command" &&
      isServerCommand(event.data.command)
    ) {
      void execute(event.data.command);
    }
  };
  const publishNavigation = () =>
    publish({ kind: "studio-server-navigation", navigation: navigation() });
  const preferencesChanged = (event: Event) => {
    if (!source.ready) return;
    const stateDir =
      resourceData(store.state().current)?.workspaces.values().next().value
        ?.stateDir ?? "";
    const keys = new Set([
      `codex-project-compact:${stateDir}`,
      `codex-project-tree:${stateDir}`,
      `codex-sidebar-order:${stateDir}`,
    ]);
    if (
      (event instanceof CustomEvent && keys.has(event.detail)) ||
      (event instanceof StorageEvent &&
        (event.key === null ||
          keys.has(event.key) ||
          keys.has(serverStorageEventKey(event) || "")))
    )
      publishNavigation();
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const scheduleAlert = () => {
    clearTimeout(timer);
    const next = source.ready
      ? nextAlertChange(store, Date.now() / 1000)
      : null;
    if (next !== null && next !== undefined)
      timer = setTimeout(
        () => {
          publishNavigation();
          scheduleAlert();
        },
        Math.max(0, next * 1000 - Date.now()) + 50,
      );
  };
  const stops = [
    ...(screen ? [screen.bind(execute)] : []),
    services.resources.watchResourceConnection((status) =>
      publish({ kind: "studio-server-status", status }),
    ),
    reaction(
      navigation,
      (value) =>
        publish({ kind: "studio-server-navigation", navigation: value }),
      { fireImmediately: true },
    ),
    reaction(
      () => readServerAccounts(source.accounts),
      (accounts) => {
        if (accounts) publish({ kind: "studio-server-accounts", accounts });
      },
      { fireImmediately: true },
    ),
    reaction(
      () => (source.ready ? nextAlertChange(store, Date.now() / 1000) : null),
      scheduleAlert,
      { fireImmediately: true },
    ),
  ];
  if (legacy) {
    window.addEventListener("message", receive);
    window.addEventListener(preferenceEvent, preferencesChanged);
    window.addEventListener("storage", preferencesChanged);
  }
  return () => {
    disposed = true;
    clearTimeout(timer);
    stops.reverse().forEach((stop) => stop());
    window.removeEventListener("message", receive);
    window.removeEventListener(preferenceEvent, preferencesChanged);
    window.removeEventListener("storage", preferencesChanged);
  };
}

import { startDesktopNotifications } from "../hooks/desktopNotifications.ts";
import { onDesktopNavigate } from "../native/notifications.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { parseSidebarShortcut } from "../studioPreferences.ts";

export function publishFrame(
  scope: AppScope,
  message: Parameters<NonNullable<AppScope["screen"]["channel"]>["publish"]>[0],
) {
  if (scope.screen.channel) scope.screen.channel.publish(message);
  else if (window.parent !== window)
    window.parent.postMessage(message, serverParentOrigin);
}
export function startAppFrame(areas: ServerAreas): Disposer {
  const { scope, ui } = areas;
  const source: FrameInputs = {
    get ready() {
      return areas.startup.isReady;
    },
    get opened() {
      return areas.navigation.selectedId;
    },
    get error() {
      return areas.startup.error;
    },
    get pendingReads() {
      return areas.read.pendingReadTargets;
    },
    get accounts() {
      return frameAccounts(areas.accounts.data);
    },
    get creating() {
      return areas.creation.isCreating;
    },
    run: ui.run,
  };
  const navigate = (target: { agentId: string; itemId?: string }) =>
    ui.navigate({ ...target, requestId: randomUUID() });
  const key = (event: KeyboardEvent) => {
    if (!scope.screen.active) return;
    if ((event.metaKey || event.ctrlKey) && event.key === "k") {
      event.preventDefault();
      if (scope.screen.channel || serverViewId)
        publishFrame(scope, { kind: "studio-server-search" });
      else ui.openSearch();
    }
    if (event.key === "Escape") ui.escape();
    if (scope.screen.channel) return;
    const target = event.target;
    if (
      target instanceof HTMLElement &&
      (target.isContentEditable ||
        target.closest("input, textarea, select, [role='textbox']"))
    )
      return;
    const shortcut = parseSidebarShortcut(
      areas.preferences.value.sidebarShortcut,
    );
    if (
      shortcut &&
      event.key.toLowerCase() === shortcut.key &&
      event.metaKey === shortcut.meta &&
      event.ctrlKey === shortcut.ctrl &&
      event.altKey === shortcut.alt &&
      event.shiftKey === shortcut.shift
    ) {
      event.preventDefault();
      areas.sidebar.toggle();
    }
  };
  const onError = (event: Event) => {
    if (
      scope.screen.active &&
      event instanceof CustomEvent &&
      typeof event.detail === "string"
    )
      areas.notices.notify(event.detail);
  };
  const onNavigate = (event: Event) => {
    if (
      event instanceof CustomEvent &&
      event.detail &&
      typeof event.detail.agentId === "string"
    )
      navigate(event.detail);
  };
  window.addEventListener("desktop-error", onError);
  window.addEventListener("keydown", key);
  const stops = [
    startServerFrame(scope, source),
    startDesktopNotifications({
      store: scope.store,
      get metadata() {
        return scope.services.sync.projectionMetadata;
      },
      get opened() {
        return source.opened;
      },
      get enabled() {
        return !scope.screen.channel && !serverViewId;
      },
    }),
    () => {
      window.removeEventListener("desktop-error", onError);
      window.removeEventListener("keydown", key);
    },
  ];
  if (!scope.screen.channel) {
    const stop = onDesktopNavigate(navigate);
    window.addEventListener("studio-navigate", onNavigate);
    stops.push(() => {
      stop?.();
      window.removeEventListener("studio-navigate", onNavigate);
    });
  }
  return () => stops.reverse().forEach((stop) => stop());
}
