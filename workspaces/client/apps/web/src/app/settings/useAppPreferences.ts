import { useEffect, useEffectEvent, useState, type KeyboardEvent } from "react";
import { reaction, runInAction } from "mobx";
import { useMantineColorScheme } from "@mantine/core";
import type { ServerAreas } from "../contracts.ts";
import type { createUiPreferenceBridge } from "../../sync/uiPreferenceBridge.ts";
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
export function useAppPreferences(
  a: Pick<ServerAreas, "scope" | "preferences">,
  bridge: ReturnType<typeof createUiPreferenceBridge>,
  publish: (preferences: StudioPreferences) => void,
) {
  const { storage, resources } = a.scope.services;
  const { setColorScheme } = useMantineColorScheme();
  const applyTheme = useEffectEvent(setColorScheme);
  const value = a.preferences.value;
  const [preferenceError, setError] = useState(
    () => loadStudioPreferences(storage).error,
  );
  const [thresholdEdit, editThreshold] = useState<string | null>(null),
    [shortcutFeedback, setShortcutFeedback] = useState("");
  const accept = (next: StudioPreferences) =>
    runInAction(() => {
      a.preferences.value = next;
    });
  const update = (patch: Record<string, unknown>) => {
    try {
      const next = normalizeStudioPreferences({ ...value, ...patch });
      const saved = writePreferenceEdit(
        studioPreferencesStorageKey,
        value,
        next,
        storage,
        normalizeStudioPreferences,
      );
      accept(saved);
      window.dispatchEvent(new Event("studio-preferences-change"));
      publish(saved);
      setError("");
    } catch {
      setError("Studio preferences could not be saved in this browser.");
    }
  };
  useEffect(() => {
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
          applyTheme(next.theme);
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
        accept(
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
  }, [a, bridge, resources]);
  const hideOldChatsThresholdText =
    thresholdEdit ?? String(value.hideOldChatsThreshold);
  const commitThreshold = () => {
    const text = hideOldChatsThresholdText,
      count = Number(text);
    if (
      /^\d+$/.test(text) &&
      Number.isInteger(count) &&
      count >= MIN_HIDE_OLD_CHATS_THRESHOLD &&
      count <= MAX_HIDE_OLD_CHATS_THRESHOLD
    )
      update({ hideOldChatsThreshold: count });
    editThreshold(null);
  };
  const resetShortcutFeedback = () => setShortcutFeedback("");
  const captureShortcut = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Tab" || event.key === "Escape") {
      resetShortcutFeedback();
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
      setShortcutFeedback(
        "Choose a letter or number with Ctrl or ⌘. Browser-reserved shortcuts cannot be used.",
      );
      return;
    }
    resetShortcutFeedback();
    update({ sidebarShortcut: candidate });
  };
  return {
    value,
    preferenceError,
    shortcutFeedback,
    update,
    hideOldChatsThresholdText,
    editThreshold,
    commitThreshold,
    captureShortcut,
    resetShortcutFeedback,
  };
}
