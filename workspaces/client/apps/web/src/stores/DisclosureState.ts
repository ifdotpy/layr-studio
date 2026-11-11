import { makeAutoObservable } from "mobx";

/** Open/closed state of one disclosure (menu, popover, expandable row, dialog). */
export class DisclosureState {
  opened: boolean;

  constructor(opened = false) {
    this.opened = opened;
    makeAutoObservable(this, {}, { autoBind: true });
  }

  set(opened: boolean): void {
    this.opened = opened;
  }

  open(): void {
    this.opened = true;
  }

  close(): void {
    this.opened = false;
  }

  toggle(): void {
    this.opened = !this.opened;
  }
}
