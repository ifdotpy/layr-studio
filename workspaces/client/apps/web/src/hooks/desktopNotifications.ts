import {
  hasDesktopNotifications,
  notifyDesktop,
} from "../native/notifications.ts";
import { isServerView } from "../servers/environment.ts";
import { autorun } from "mobx";
import { alertsSince, desktopAlerts } from "../desktop/desktopAlerts.ts";
import { nextAlertChange } from "../desktop/desktopAlerts.ts";
import type { AlertStore } from "../desktop/desktopAlerts.ts";
import type { ProjectionMetadata } from "../generated/observableResources.ts";
import { resourceData } from "../store/runtime.ts";

export function startDesktopNotifications(source: {
  readonly store: AlertStore;
  readonly metadata: ProjectionMetadata;
  readonly opened: string | null;
  readonly enabled: boolean;
}) {
  const state = {
    current: {
      scope: "",
      initialized: false,
      permissionReported: false,
      seen: new Set<string>(),
      started: Date.now(),
    },
  };
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const publish = () => {
    if (stopped) return;
    const { store, metadata, opened, enabled } = source;
    const data = resourceData(store.state().current);
    clearTimeout(timer);
    if (enabled && data) {
      const next = nextAlertChange(store, Date.now() / 1000);
      if (next !== null)
        timer = setTimeout(publish, Math.max(0, next * 1000 - Date.now()) + 50);
    }

    if (!enabled || !data || isServerView) return;
    const scope = data.workspaces.values().next().value?.stateDir ?? "";
    if (state.current.scope !== scope)
      state.current = {
        scope,
        initialized: false,
        permissionReported: false,
        seen: new Set(),
        started: Date.now(),
      };
    const current = state.current;
    // Before the first pull ends, missing records would make old alerts look new later.
    if (!current.initialized && !metadata.complete) return;
    const alerts = desktopAlerts(store);
    const added = alerts
      .filter((alert) => !current.seen.has(alert.id))
      .filter(alertsSince(current.started));
    for (const alert of alerts) current.seen.add(alert.id);
    // The initial snapshot establishes a baseline, not an old-alert backlog.
    if (!current.initialized) {
      current.initialized = true;
      return;
    }
    const openedShared = opened ? data.rooms.get(opened) : undefined;
    for (const alert of added) {
      if (
        document.visibilityState === "visible" &&
        document.hasFocus() &&
        (opened === alert.target.agentId ||
          openedShared?.members?.includes(alert.target.agentId))
      )
        continue;
      const { id, ...notification } = alert;
      if (hasDesktopNotifications()) {
        void notifyDesktop(notification).catch((error) => {
          if (stopped) return;
          const message =
            error instanceof Error ? error.message : String(error);
          const denied =
            /STUDIO_NOTIFICATIONS_DENIED|Notifications are not allowed for this application/i.test(
              message,
            );
          if (denied && current.permissionReported) return;
          if (denied) current.permissionReported = true;
          else current.seen.delete(id);
          window.dispatchEvent(
            new CustomEvent("desktop-error", {
              detail: denied
                ? "Allow notifications for LAYR Studio in this device's notification settings."
                : error instanceof Error
                  ? error.message
                  : "Could not show the desktop notification.",
            }),
          );
        });
      } else if (
        "Notification" in window &&
        Notification.permission === "granted"
      ) {
        const native = new Notification(alert.title, {
          body: alert.body,
          tag: id,
          silent: !alert.sound,
        });
        native.onclick = () => {
          native.close();
          if (stopped) return;
          window.focus();
          window.dispatchEvent(
            new CustomEvent("studio-navigate", { detail: alert.target }),
          );
        };
      }
    }
  };
  const stop = autorun(publish);
  return () => {
    stopped = true;
    stop();
    clearTimeout(timer);
  };
}
