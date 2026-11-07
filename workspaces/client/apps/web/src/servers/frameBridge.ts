import type { ScreenSearchResultsMessage } from "./screenChannel";
import { useScreenChannel } from "./ScreenContext";
import { useServerApi, useServerResources, useServerStore } from "./ServerServicesContext";
import {
  normalizeStudioPreferences,
  studioPreferencesStorageKey,
} from "../studioPreferences";
import { errorText } from "../api";
import { executeSidebarRequest, serializeSidebarError } from "./sidebarRpc";
import { desktopAlerts } from "../desktop/desktopAlerts";
import { useAlertClock } from "../hooks/useAlertClock";
import { useEffect, useRef } from "react";
import { reaction, toJS } from "mobx";
import { serverViewId, serverParentOrigin } from "./environment";
import { publishServerAliases } from "./serverAliases";
import {
  isServerCommand,
  navigationSnapshot,
  readServerAccounts,
  type ServerAccount,
  type ServerCommand,
} from "./navigation";
import type { Snapshot } from "../types";
import "../sync/subscriptions/events";
import { preferenceEvent } from "../sync/uiPreferenceMerge";
import { preferenceMemory, serverStorageEventKey } from "./storage";
export function useServerFrame(
  data: Snapshot | null,
  opened: string | null,
  error: string,
  run: (command: ServerCommand) => void | Promise<void>,
  _unread: Set<string>,
  accounts: ServerAccount[],
  creating = false,
) {
  const screen = useScreenChannel();
  const store = useServerStore();
  const sidebarRows = () => {
    const current = store.sidebar().current;
    return current.status === "live" || current.status === "stale"
      ? toJS(current.data.rows)
      : [];
  };
  const { get, saved } = useServerApi();
  const { watchResourceConnection } = useServerResources();

  // Sign-in alerts become due without a record change (desktopAlerts.ts).
  const now = useAlertClock(data);
  const handler = useRef(run);
  handler.current = run;
  const latest = useRef({ data, opened, error, creating });
  latest.current = { data, opened, error, creating };
  useEffect(() => {
    if (!screen) return;
    const unbind = screen.bind(async (command) => {
      if (command.action === "search") {
        try {
          const value = await get("/api/search", {
            query: { q: command.query },
          });
          screen.publish({
            kind: "studio-server-search-results",
            requestId: command.requestId,
            results: value.results,
          } satisfies ScreenSearchResultsMessage);
        } catch (failure) {
          screen.publish({
            kind: "studio-server-search-results",
            requestId: command.requestId,
            results: [],
            error: errorText(failure),
          } satisfies ScreenSearchResultsMessage);
        }
      } else await handler.current(command);
    });
    const stop = watchResourceConnection((status) =>
      screen.publish({ kind: "studio-server-status", status }),
    );
    return () => {
      unbind();
      stop();
    };
  }, [screen, get, watchResourceConnection]);
  useEffect(() => {
    if (!screen) return;
    return reaction(() => ({
      kind: "studio-server-navigation" as const,
      navigation: {
        ...navigationSnapshot(
          data,
          opened,
          error,
          sidebarRows(),
          data ? desktopAlerts(data, now) : [],
          saved,
        ),
        creating,
      },
    }), (message) => screen.publish(message), { fireImmediately: true });
  }, [
    screen,
    store,
    data,
    opened,
    error,
    creating,
    saved,
    now,
  ]);
  useEffect(() => {
    const safeAccounts = readServerAccounts(accounts);
    if (!safeAccounts) return;
    screen?.publish({
      kind: "studio-server-accounts",
      accounts: safeAccounts,
    });
  }, [screen, accounts]);
  useEffect(() => {
    if (screen || !serverViewId || window.parent === window) return;
    const receive = (event: MessageEvent) => {
      if (
        !event.data ||
        typeof event.data !== "object" ||
        event.source !== window.parent ||
        event.origin !== serverParentOrigin ||
        event.data.serverId !== serverViewId
      )
        return;
      if (event.data.kind === "studio-sidebar-request") {
        if (typeof event.data.correlation !== "string") return;
        const reply = (result?: unknown, failure?: unknown) =>
          window.parent.postMessage(
            {
              kind: "studio-sidebar-result",
              serverId: serverViewId,
              correlation: event.data.correlation,
              result,
              ...(failure
                ? {
                    error: {
                      ...serializeSidebarError(failure),
                    },
                  }
                : {}),
            },
            serverParentOrigin,
          );
        void executeSidebarRequest(
          event.data.value,
          async () => {
            await handler.current({ action: "refresh" });
            // Hidden frames suspend animation callbacks. Wait for React's next task.
            await new Promise<void>((resolve) => setTimeout(resolve, 0));
            const value = latest.current;
            return {
              ...navigationSnapshot(
                value.data,
                value.opened,
                value.error,
                sidebarRows(),
                value.data ? desktopAlerts(value.data) : [],
              ),
              creating: value.creating,
            };
          },
          (command) => handler.current(command),
          latest.current.data?.token,
        ).then(
          (result) => reply(result),
          (failure) => reply(undefined, failure),
        );
        return;
      }
      if (event.data.kind === "studio-server-shell-layout") {
        // The shell's sidebar covers the left of this view while its terminal is open.
        const width = event.data.sidebarWidth;
        if (typeof width !== "number" || !(width >= 0) || width > 2000) return;
        const root = document.documentElement;
        root.style.setProperty(
          "--shell-sidebar-width",
          `${Math.round(width)}px`,
        );
        if (width > 0) root.dataset.shellSidebar = "true";
        else delete root.dataset.shellSidebar;
        return;
      }
      if (event.data.kind === "studio-server-preferences") {
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
        return;
      }
      if (
        event.data.kind !== "studio-server-command" ||
        !isServerCommand(event.data.command)
      )
        return;
      const command = event.data.command as ServerCommand;
      if (command.action === "terminal") {
        window.dispatchEvent(
          new CustomEvent("studio-terminal-command", {
            detail: command.operation,
          }),
        );
        return;
      }
      if (command.action === "search") {
        void get("/api/search", { query: { q: command.query } })
          .then((value) => {
            window.parent.postMessage(
              {
                kind: "studio-server-search-results",
                serverId: serverViewId,
                requestId: command.requestId,
                results: value.results,
              } satisfies ScreenSearchResultsMessage,
              serverParentOrigin,
            );
          })
          .catch((failure: unknown) => {
            window.parent.postMessage(
              {
                kind: "studio-server-search-results",
                serverId: serverViewId,
                requestId: command.requestId,
                error: errorText(failure),
                results: [],
              } satisfies ScreenSearchResultsMessage,
              serverParentOrigin,
            );
          });
      } else handler.current(command);
    };
    window.addEventListener("message", receive);
    const stop = watchResourceConnection((status) => {
      window.parent.postMessage(
        { kind: "studio-server-status", serverId: serverViewId, status },
        serverParentOrigin,
      );
    });
    return () => {
      window.removeEventListener("message", receive);
      stop();
    };
  }, []);
  useEffect(() => {
    if (screen || !serverViewId || window.parent === window) return;
    const navigation = () => ({
      ...navigationSnapshot(data, opened, error, sidebarRows(), data ? desktopAlerts(data, now) : []),
      creating,
    });
    const publish = () => window.parent.postMessage({
      kind: "studio-server-navigation", serverId: serverViewId, navigation: navigation(),
    }, serverParentOrigin);
    const stopRows = reaction(navigation, (value) => window.parent.postMessage({
      kind: "studio-server-navigation", serverId: serverViewId, navigation: value,
    }, serverParentOrigin), { fireImmediately: true });
    const keys = new Set([
      `codex-project-compact:${data?.stateDir}`,
      `codex-project-tree:${data?.stateDir}`,
      `codex-sidebar-order:${data?.stateDir}`,
    ]);
    const preferencesChanged = (event: Event) => {
      if (!data) return;
      if (event instanceof CustomEvent && keys.has(event.detail)) publish();
      if (
        event instanceof StorageEvent &&
        (event.key === null ||
          keys.has(event.key) ||
          keys.has(serverStorageEventKey(event) || ""))
      )
        publish();
    };
    window.addEventListener(preferenceEvent, preferencesChanged);
    window.addEventListener("storage", preferencesChanged);
    return () => {
      stopRows();
      window.removeEventListener(preferenceEvent, preferencesChanged);
      window.removeEventListener("storage", preferencesChanged);
    };
  }, [data, opened, error, creating, now, store]);
  useEffect(() => {
    if (screen || !serverViewId || window.parent === window) return;
    const safeAccounts = readServerAccounts(accounts);
    if (!safeAccounts) return;
    window.parent.postMessage(
      {
        kind: "studio-server-accounts",
        serverId: serverViewId,
        accounts: safeAccounts,
      },
      serverParentOrigin,
    );
  }, [accounts]);
}
