import type { ServerAreas } from "../app/contracts.ts";
import { frameAccounts } from "../components/accounts/Accounts/Accounts.tsx";
import { compareStructural, reaction } from "mobx";
import {
  formatAgentStatus,
  type ReadState,
} from "../status/operationalStatus.ts";
import { errorText } from "../api.ts";
import { desktopAlerts, nextAlertChange } from "../desktop/desktopAlerts.ts";
import { serverViewId, serverParentOrigin } from "./environment.ts";
import {
  navigationSnapshot,
  readServerAccounts,
  type ServerAccount,
  type ServerCommand,
} from "./navigation.ts";
import { resourceData } from "../store/runtime.ts";
import { sidebarEntities } from "./sidebarSnapshot.ts";
import { createSidebarViewModel } from "../components/sidebar/support/viewModel.ts";
import { preferenceEvent } from "../sync/uiPreferenceMerge.ts";
import { serverStorageEventKey } from "./storage.ts";
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
  readonly accounts: ServerAccount[] | undefined;
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
  let lastNavigation: ReturnType<typeof navigation> | undefined;
  const publishNavigation = () => {
    const value = navigation();
    if (compareStructural(lastNavigation, value)) return;
    lastNavigation = value;
    publish({ kind: "studio-server-navigation", navigation: value });
  };
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
    reaction(navigation, publishNavigation, { fireImmediately: true }),
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
    window.addEventListener(preferenceEvent, preferencesChanged);
    window.addEventListener("storage", preferencesChanged);
  }
  return () => {
    disposed = true;
    clearTimeout(timer);
    stops.reverse().forEach((stop) => stop());
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
      return frameAccounts(areas.accounts);
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
  let releaseServerFrame = () => {};
  const stops = [
    reaction(
      () => scope.screen.channel,
      () => {
        releaseServerFrame();
        releaseServerFrame = startServerFrame(scope, source);
      },
      { fireImmediately: true },
    ),
    () => releaseServerFrame(),
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
