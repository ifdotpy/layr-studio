import { makeAutoObservable } from "mobx";
import type { KeyboardEvent, MouseEvent } from "react";
import { useEffect } from "react";

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
        this.closeFromTrigger(event, opened, () => this.setMenu(key, false)),
      onBlur: this.clearSuppression,
    };
  }
  closeFromTrigger(
    event: KeyboardEvent<HTMLElement>,
    opened: boolean,
    close: () => void,
  ) {
    if (event.key !== "Escape") {
      this.clearSuppression();
      return;
    }
    if (!opened && this.suppressNextEscape) {
      this.clearSuppression();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (!opened) return;
    event.preventDefault();
    event.stopPropagation();
    const trigger = event.currentTarget;
    close();
    restoreMenuTriggerFocusAfterClose(trigger);
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
export function restoreMenuTriggerFocusAfterClose(trigger: HTMLElement | null) {
  requestAnimationFrame(() => trigger?.focus({ preventScroll: true }));
}

export { MenuEscapeState as MenuState };
