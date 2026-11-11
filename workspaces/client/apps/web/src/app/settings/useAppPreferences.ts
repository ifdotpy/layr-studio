import { useEffect, type KeyboardEvent } from "react";
import { reaction, makeAutoObservable } from "mobx";
import { useMantineColorScheme } from "@mantine/core";
import type { ServerAreas } from "../contracts.ts";
import { writePreferenceEdit } from "../../sync/uiPreferenceStore.ts";
import {
  applyStudioPreferences,
  defaultStudioPreferences,
  normalizeStudioPreferences,
  parseStudioPreferences,
  parseSidebarShortcut,
  studioPreferencesStorageKey,
  MIN_HIDE_OLD_CHATS_THRESHOLD,
  MAX_HIDE_OLD_CHATS_THRESHOLD,
  type StudioPreferences,
} from "../../studioPreferences.ts";

export function loadStudioPreferences(storage: Storage) {
  try {
    const stored = storage.getItem(studioPreferencesStorageKey);
    return {
      value: stored ? parseStudioPreferences(stored) : defaultStudioPreferences,
      error: "",
    };
  } catch {
    return {
      value: defaultStudioPreferences,
      error:
        "Saved Studio preferences were invalid or unreadable; safe defaults are active.",
    };
  }
}
export class AppPreferences {
  readonly areas;
  preferenceError;
  thresholdEdit: string | null = null;
  shortcutFeedback = "";
  constructor(
    a: Pick<
      ServerAreas,
      "scope" | "preferences" | "frame" | "preferenceBridge"
    >,
  ) {
    this.areas = a;
    this.preferenceError = loadStudioPreferences(
      a.scope.services.storage,
    ).error;
    makeAutoObservable(this, { areas: false }, { autoBind: true });
  }
  get value() {
    return this.areas.preferences.value;
  }
  accept(value: StudioPreferences) {
    this.areas.preferences.value = value;
  }
  editThreshold(value: string | null) {
    this.thresholdEdit = value;
  }
  update(patch: Record<string, unknown>) {
    try {
      const next = normalizeStudioPreferences({ ...this.value, ...patch });
      const saved = writePreferenceEdit(
        studioPreferencesStorageKey,
        this.value,
        next,
        this.areas.scope.services.storage,
        normalizeStudioPreferences,
      );
      this.areas.preferences.value = saved;
      window.dispatchEvent(new Event("studio-preferences-change"));
      this.areas.frame.publish({
        kind: "studio-server-preferences",
        preferences: saved,
      });
      this.preferenceError = "";
    } catch {
      this.preferenceError =
        "Studio preferences could not be saved in this browser.";
    }
  }
  start() {
    const a = this.areas,
      { resources } = a.scope.services,
      bridge = a.preferenceBridge;
    let wasLive = false;
    const stopConnection = resources.watchResourceConnection((status) => {
      const reconnected = status === "live" && !wasLive;
      wasLive = status === "live";
      if (reconnected) bridge.retryPendingWrites();
    });
    const stopDocument = reaction(
      () => (a.scope.screen.active ? a.preferences.value : null),
      (next) => {
        if (next) {
          applyStudioPreferences(next, document.documentElement);
        }
      },
      { fireImmediately: true },
    );
    const receive = (event: Event) => {
      if (
        event instanceof StorageEvent &&
        event.key !== studioPreferencesStorageKey
      )
        return;
      try {
        this.accept(
          event instanceof StorageEvent
            ? parseStudioPreferences(event.newValue || "")
            : normalizeStudioPreferences((event as CustomEvent).detail),
        );
      } catch {
        /* Keep the last valid preference. */
      }
    };
    window.addEventListener("storage", receive);
    window.addEventListener("studio-shell-preferences", receive);
    return () => {
      stopConnection();
      stopDocument();
      window.removeEventListener("storage", receive);
      window.removeEventListener("studio-shell-preferences", receive);
    };
  }
  get hideOldChatsThresholdText() {
    return this.thresholdEdit ?? String(this.value.hideOldChatsThreshold);
  }
  commitThreshold() {
    const text = this.hideOldChatsThresholdText,
      count = Number(text);
    if (
      /^\d+$/.test(text) &&
      Number.isInteger(count) &&
      count >= MIN_HIDE_OLD_CHATS_THRESHOLD &&
      count <= MAX_HIDE_OLD_CHATS_THRESHOLD
    )
      this.update({ hideOldChatsThreshold: count });
    this.editThreshold(null);
  }
  resetShortcutFeedback() {
    this.shortcutFeedback = "";
  }
  captureShortcut(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Tab" || event.key === "Escape") {
      this.resetShortcutFeedback();
      return;
    }
    if (["Control", "Meta", "Alt", "Shift"].includes(event.key)) return;
    if (event.ctrlKey || event.metaKey) event.stopPropagation();
    event.preventDefault();
    const candidate =
      (event.metaKey ? "Meta" : event.ctrlKey ? "Control" : "") +
      (event.altKey ? "+Alt" : "") +
      (event.shiftKey ? "+Shift" : "") +
      "+" +
      event.key;
    if (!parseSidebarShortcut(candidate)) {
      this.shortcutFeedback =
        "Choose a letter or number with Ctrl or ⌘. Browser-reserved shortcuts cannot be used.";
      return;
    }
    this.resetShortcutFeedback();
    this.update({ sidebarShortcut: candidate });
  }
}

/** Mantine applies the theme to the DOM. */
export function useAppPreferences(
  a: Pick<ServerAreas, "preferenceControls">,
): AppPreferences {
  const owner = a.preferenceControls;
  const { setColorScheme } = useMantineColorScheme();
  const theme = owner.value.theme;
  useEffect(() => setColorScheme(theme), [setColorScheme, theme]);
  return owner;
}
