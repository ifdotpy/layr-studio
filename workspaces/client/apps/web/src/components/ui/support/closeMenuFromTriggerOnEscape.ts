import { makeAutoObservable } from "mobx";
import type { KeyboardEvent, MouseEvent } from "react";
import { useEffect, useState } from "react";

/** Escape feedback belongs to the mounted menu owner. */
export class MenuEscapeState {
  suppressNextEscape = false;
  openedKey: string | null = null;
  readonly triggers = new Map<string, HTMLButtonElement>();
  constructor() {
    makeAutoObservable(
      this,
      { triggers: false, triggerProps: false },
      { autoBind: true },
    );
  }
  setMenu(key: string, opened: boolean) {
    if (opened) this.clearSuppression();
    this.openedKey = opened
      ? key
      : this.openedKey === key
        ? null
        : this.openedKey;
  }
  changeMenu(key: string, opened: boolean) {
    this.setMenu(key, opened);
    if (!opened)
      restoreMenuTriggerFocusAfterClose(this.triggers.get(key) ?? null);
  }
  triggerProps(key: string) {
    const opened = this.openedKey === key;
    return {
      ref: (node: HTMLButtonElement | null) => {
        if (node) this.triggers.set(key, node);
        else this.triggers.delete(key);
      },
      onClick: (event: MouseEvent<HTMLElement>) =>
        event.currentTarget.focus({ preventScroll: true }),
      "data-mantine-stop-propagation":
        opened || this.suppressNextEscape ? "true" : undefined,
      onKeyDown: (event: KeyboardEvent<HTMLElement>) =>
        closeMenuFromTriggerOnEscape(
          event,
          opened,
          () => this.setMenu(key, false),
          this.suppressNextEscape,
          this.clearSuppression,
        ),
      onBlur: this.clearSuppression,
    };
  }
  clearSuppression() {
    this.suppressNextEscape = false;
  }
  rememberEscape(event: globalThis.KeyboardEvent) {
    if (event.key === "Escape") this.suppressNextEscape = true;
  }
}
export function useMenuEscapeCapture(opened: boolean, state: MenuEscapeState) {
  useEffect(() => {
    if (!opened) return;
    state.clearSuppression();
    window.addEventListener("keydown", state.rememberEscape, true);
    return () =>
      window.removeEventListener("keydown", state.rememberEscape, true);
  }, [opened, state]);
  return state;
}
// integration: ProjectLocationsSettings supplies its owned MenuEscapeState.
export function useMenuEscapeSuppression(opened: boolean) {
  const [state] = useState(() => new MenuEscapeState());
  return useMenuEscapeCapture(opened, state);
}

export function closeMenuFromTriggerOnEscape(
  event: KeyboardEvent<HTMLElement>,
  opened: boolean,
  close: () => void,
  suppressNextEscape: boolean,
  clearSuppression: () => void,
) {
  if (event.key !== "Escape") {
    clearSuppression();
    return;
  }
  if (!opened && suppressNextEscape) {
    clearSuppression();
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  if (!opened) return;
  event.preventDefault();
  event.stopPropagation();
  const trigger = event.currentTarget;
  close();
  requestAnimationFrame(() => trigger.focus({ preventScroll: true }));
}

export function restoreMenuTriggerFocusAfterClose(trigger: HTMLElement | null) {
  requestAnimationFrame(() => trigger?.focus({ preventScroll: true }));
}

export { MenuEscapeState as MenuState };
